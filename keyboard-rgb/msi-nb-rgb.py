#!/usr/bin/env python3
"""Minimal per-key RGB writer for MSI notebook SteelSeries keyboards (e.g. GS66 1038:113a).

Faithful reimplementation of MSI Center's `Device_GE73` frame formats, driven entirely by
data extracted from MSI Center (`extract-msi-layouts.py` -> `msi-layouts.json`):

  * steady/group : cmd 0x0E, 12-byte entries, keys partitioned into Group1..Group6_Offset
                   (== MSI's `Set_Keyboard_Color`). Sending ALL six groups is what finally
                   addresses every key incl. Power (Group6) — which msi-perkeyrgb misses.
  * per-key free : cmd 0x0C, 4-byte [hid,R,G,B] entries, <=130 keys/report
                   (== MSI's `Set_Keyboard_SyncColor_Free`).

Writes HID feature reports straight to /dev/hidrawN (report id 0 -> 524 bytes on-wire).
Needs root (hidraw is root-only). No hidapi dependency.

Usage:
    sudo ./msi-nb-rgb.py off
    sudo ./msi-nb-rgb.py all FF0000
    sudo ./msi-nb-rgb.py keys CLK_Escape=FF0000 CLK_Power=00FF00 CLK_W=0000FF
"""
import argparse
import fcntl
import json
import os
import sys
import time

SEND_DELAY = 0.01   # the controller drops reports sent too fast (MSI does Thread.Sleep(10))

REPORT_LEN = 525          # Data[0] = report id 0  ->  524-byte feature report on the wire

_IOC_WRITE, _IOC_READ = 1, 2
def _IOC(d, t, nr, sz): return (d << 30) | (sz << 16) | (t << 8) | nr
def HIDIOCSFEATURE(sz): return _IOC(_IOC_WRITE | _IOC_READ, ord('H'), 0x06, sz)


def send_feature(fd, data: bytearray):
    assert len(data) == REPORT_LEN
    fcntl.ioctl(fd, HIDIOCSFEATURE(len(data)), data, True)


def refresh(fd):
    """Commit/show colors. MSI's Style_Keyboard_Show: output report Data2[1]=0x0D, Data2[3]=2
    (report id 0 -> prepend 0x00). (msi-perkeyrgb uses 0x09 instead; 0x0D is MSI's own 'show'.)"""
    os.write(fd, bytes([0x00, 0x0D, 0x00, 0x02] + [0x00] * 60))   # 64-byte output report


def set_all_groups(fd, groups, r, g, b):
    """MSI Set_Keyboard_Color: one 0x0E feature report per region group."""
    for offsets in groups:
        d = bytearray(REPORT_LEN)
        d[1] = 0x0E
        d[3] = len(offsets)
        for m, off in enumerate(offsets):
            base = 5 + 12 * m
            d[base + 0], d[base + 1], d[base + 2] = r, g, b   # RGB
            d[base + 6] = 0x2C                                 # Data[11+12m]
            d[base + 7] = 1                                    # Data[12+12m]
            d[base + 9] = 1                                    # Data[14+12m]
            d[base + 11] = off                                 # Data[16+12m] = HID usage
        send_feature(fd, d)
        time.sleep(SEND_DELAY)


def set_keys_free(fd, key_rgb):
    """MSI Set_Keyboard_SyncColor_Free: one 0x0C feature report, 4-byte [hid,R,G,B] entries."""
    d = bytearray(REPORT_LEN)
    d[1] = 0x0C
    for b, (hid, r, g, bl) in enumerate(key_rgb):
        base = 5 + 4 * b
        d[base + 0], d[base + 1], d[base + 2], d[base + 3] = hid, r, g, bl
    d[3] = len(key_rgb)
    send_feature(fd, d)


def set_all_openrgb(fd, hids, r, g, b, do_refresh=False, pkt3=0x66):
    """Faithful replica of OpenRGB MSILaptopController::SetLEDs (single 0x0C report).

    cmd 0x0C, buf[3]=0x66 (OpenRGB's hardcoded KLC packet id), all 130 slots padded with led-id
    0xFF (ignore), then one [hid,R,G,B] entry per key, sent as ONE feature report. No refresh in
    OpenRGB. NOTE: MSI's own code sets Data[3]=count; if the device reads buf[3] as the entry count,
    OpenRGB's fixed 0x66 (=102) truncates keyboards with >102 keys. pkt3=None -> use len(hids).
    """
    d = bytearray(REPORT_LEN)
    d[1] = 0x0C
    d[3] = len(hids) if pkt3 is None else pkt3
    for i in range((REPORT_LEN - 5) // 4):        # pad unused slots -> ignored
        d[5 + 4 * i] = 0xFF
    for i, hid in enumerate(hids):
        base = 5 + 4 * i
        d[base + 0], d[base + 1], d[base + 2], d[base + 3] = hid, r, g, b
    send_feature(fd, d)
    if do_refresh:
        refresh(fd)


def hexcolor(s):
    s = s.lstrip('#')
    return int(s[0:2], 16), int(s[2:4], 16), int(s[4:6], 16)


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--device", default="/dev/hidraw1")
    ap.add_argument("--layouts", default=os.path.join(here, "msi-layouts.json"))
    ap.add_argument("--keymap", default="GE73Keys", help="which keymap enum to use for key names")
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("off")
    p_all = sub.add_parser("all"); p_all.add_argument("color")
    p_orgb = sub.add_parser("orgb-all", help="OpenRGB-exact single 0x0C frame (de-risk test)")
    p_orgb.add_argument("color"); p_orgb.add_argument("--refresh", action="store_true")
    p_orgb.add_argument("--count", action="store_true", help="buf[3]=len(keys) instead of OpenRGB's 0x66")
    p_keys = sub.add_parser("keys"); p_keys.add_argument("pairs", nargs="+", help="NAME=RRGGBB ...")
    args = ap.parse_args()

    L = json.load(open(args.layouts))
    groups = [L["region_groups"][f"Group{i}_Offset"] for i in range(1, 7)]
    keymap = L["keymaps"][args.keymap]   # e.g. CLK_Escape -> 41

    fd = os.open(args.device, os.O_RDWR)
    try:
        if args.cmd == "off":
            set_all_groups(fd, groups, 0, 0, 0)
        elif args.cmd == "all":
            set_all_groups(fd, groups, *hexcolor(args.color))
        elif args.cmd == "orgb-all":
            # OpenRGB drives every key in the model's LED table; use the region-group union.
            set_all_openrgb(fd, L["region_group_union"], *hexcolor(args.color),
                            do_refresh=args.refresh, pkt3=(None if args.count else 0x66))
            return
        elif args.cmd == "keys":
            key_rgb = []
            for pair in args.pairs:
                name, col = pair.split("=")
                if name not in keymap:
                    sys.exit(f"unknown key {name!r} (see {args.keymap} in {args.layouts})")
                key_rgb.append((keymap[name], *hexcolor(col)))
            set_keys_free(fd, key_rgb)
        refresh(fd)
    finally:
        os.close(fd)


if __name__ == "__main__":
    main()

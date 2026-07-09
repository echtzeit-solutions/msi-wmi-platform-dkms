#!/usr/bin/env python3
"""Send MSI KLC EP0 vendor commands (usb_set_report sub-type 2) via hidraw OUTPUT reports,
and read replies from the INPUT report. Command bytes RE'd from the GE66 sibling firmware
(usb_set_report@0x08002628). Report id 0 -> payload[0] = command byte.

SAFE read-only GETs: 0x86 (mode+brightness), 0x10 / 0x80 (device info), 0x15 (fw CRC).
State-changing: 0x09 (save->flash), 0x06 (set mode), 0x40 (load default). 0xF1 (reset) intentionally
not exposed.

Usage:
  sudo ./klc-cmd.py query 86              # send cmd 0x86, print the input-report reply
  sudo ./klc-cmd.py send 09               # send cmd 0x09 (no reply expected)
  sudo ./klc-cmd.py send 06 02            # cmd 0x06 param 0x02
Needs root (hidraw is root-only).
"""
import argparse
import os
import select
import sys
import time

def send_output(fd, payload):
    # report id 0 prefix + payload; firmware sees payload[0] as the command byte
    os.write(fd, bytes([0x00] + payload))

def read_reply(fd, timeout=0.4):
    r, _, _ = select.select([fd], [], [], timeout)
    if not r:
        return None
    return os.read(fd, 64)

def hexb(b):
    return " ".join(f"{x:02x}" for x in b)

def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--device", default="/dev/hidraw1")
    sub = ap.add_subparsers(dest="mode", required=True)
    q = sub.add_parser("query"); q.add_argument("bytes", nargs="+", help="cmd + params, hex")
    s = sub.add_parser("send");  s.add_argument("bytes", nargs="+", help="cmd + params, hex")
    args = ap.parse_args()

    payload = [int(x, 16) for x in args.bytes]
    fd = os.open(args.device, os.O_RDWR | os.O_NONBLOCK)
    try:
        # drain any stale input first
        while read_reply(fd, 0.05):
            pass
        send_output(fd, payload)
        print(f"sent OUTPUT: 00 {hexb(payload)}")
        if args.mode == "query":
            rep = read_reply(fd, 0.5)
            if rep is None:
                print("no reply within 0.5s")
            else:
                print(f"reply INPUT ({len(rep)}B): {hexb(rep)}")
    finally:
        os.close(fd)

if __name__ == "__main__":
    main()

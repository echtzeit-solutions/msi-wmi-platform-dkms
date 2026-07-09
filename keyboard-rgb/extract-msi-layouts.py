#!/usr/bin/env python3
"""Extract MSI Center per-key keyboard layouts from a decompiled MysticLight_AllDevice assembly.

MSI Center's `MysticLight_AllDevice.dll` (a .NET assembly; decompile with
`ilspycmd MysticLight_AllDevice.dll -o out/`) is the authoritative source for
MSI's notebook per-key RGB layouts. This tool parses that decompiled C# into
portable JSON so tools like msi-perkeyrgb / OpenRGB can be driven data-first
instead of hand-maintaining per-model tables.

It pulls three things:
  * SupportList_* enums          -> which USB PIDs are per-key ("Keyboard") vs zone ("Aurora")
  * *Keys enums (e.g. GE73Keys)  -> key name -> HID usage code (the keymap)
  * Group[1-6]_Offset byte[]      -> the region partition (which HID usages ride in which
                                     feature-report group); MSI splits the NB keyboard into 6 groups

Usage:
    ./extract-msi-layouts.py MysticLight_AllDevice.decompiled.cs -o layouts.json
"""
import argparse
import json
import re
import sys

# `public enum Name : byte {` ... `}`  — capture name + body
ENUM_RE = re.compile(r'enum\s+(\w+)\s*(?::\s*\w+\s*)?\{(.*?)\}', re.DOTALL)
# `byte[] Name = new byte[N] { ... }`  (also matches `public static readonly byte[]`)
BYTES_RE = re.compile(r'byte\[\]\s+(\w+)\s*=\s*new\s+byte\[\d*\]\s*\{(.*?)\}', re.DOTALL)


def parse_enum_body(body: str) -> dict:
    """Parse a C# enum body honoring C#'s auto-increment (members without `= value`)."""
    out, nxt = {}, 0
    for raw in body.split(','):
        m = re.match(r'\s*(\w+)\s*(?:=\s*([0-9xXa-fA-F]+))?\s*$', raw)
        if not m:
            continue
        name, val = m.group(1), m.group(2)
        v = (int(val, 0) if val is not None else nxt)
        out[name] = v
        nxt = v + 1
    return out


def parse_byte_array(body: str) -> list:
    return [int(x, 0) for x in re.findall(r'0x[0-9a-fA-F]+|\d+', body)]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("source", help="MysticLight_AllDevice.decompiled.cs")
    ap.add_argument("-o", "--outfile", default="-")
    args = ap.parse_args()

    src = open(args.source, encoding="utf-8", errors="replace").read()

    enums = {name: parse_enum_body(body) for name, body in ENUM_RE.findall(src)}

    # Array names collide across device classes (e.g. two Group6_Offset). Keep every
    # definition with its source offset so we can disambiguate by proximity.
    array_defs = [(m.start(), m.group(1), parse_byte_array(m.group(2)))
                  for m in BYTES_RE.finditer(src)]

    support = {n: v for n, v in enums.items() if n.startswith("SupportList")}
    keymaps = {n: v for n, v in enums.items() if n.endswith("Keys") or n.endswith("Keys_US")}

    # The NB keyboard region partition is the ONE cluster where Group1..Group6_Offset are
    # all defined together (same class). Anchor on Group1_Offset (unique) and, for each
    # Group N, pick the definition nearest to that anchor — rejecting same-named arrays
    # belonging to other devices elsewhere in the assembly.
    group_defs = [(pos, n, v) for pos, n, v in array_defs if re.fullmatch(r"Group\d_Offset", n)]
    groups = {}
    anchor = next((pos for pos, n, _ in group_defs if n == "Group1_Offset"), None)
    if anchor is not None:
        for gname in sorted({n for _, n, _ in group_defs}):
            pos, _, vals = min((d for d in group_defs if d[1] == gname),
                               key=lambda d: abs(d[0] - anchor))
            groups[gname] = vals

    result = {
        "support_lists": support,          # e.g. SupportList_Keyboard: {PID_1122:4386, PID_113A:4410}
        "keymaps": keymaps,                # e.g. GE73Keys: {CLK_Escape:41, ...}
        "region_groups": groups,           # Group1_Offset..Group6_Offset -> [hid usages]
        "region_group_union": sorted({c for g in groups.values() for c in g}),
    }

    out = json.dumps(result, indent=2)
    if args.outfile == "-":
        print(out)
    else:
        open(args.outfile, "w").write(out)
        # brief census to stderr
        for n, v in support.items():
            print(f"{n}: {list(v)}", file=sys.stderr)
        print(f"keymaps: {list(keymaps)}", file=sys.stderr)
        print(f"region groups: { {n: len(v) for n, v in groups.items()} } "
              f"union={len(result['region_group_union'])} keys -> {args.outfile}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

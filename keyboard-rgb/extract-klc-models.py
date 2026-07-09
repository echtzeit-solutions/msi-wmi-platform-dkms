#!/usr/bin/env python3
"""Extract the per-model MSI KLC data table from decrypted SteelSeries .edevice specs (golisp).

The KLC protocol is shared (ss-klc-base); per-model files only override *data*:
  - USB id  `(define <name>-id 0x1038PPPP)` -> VID:PID
  - color scale `(define (red|green|blue-scale) (integer (* 255 <f>)))`
  - physical key coords `(define (get-prism-sync-nonvariant-hid-info) '((hid x y) ...))`
  - `(include "<parent>")` inheritance chain (child overrides parent, ... -> ss-klc-base)

Resolves each MSI KLC model's *effective* values by walking its include chain, and emits
`msi-klc-models.json` for the msi-klc tool.

Usage: ./extract-klc-models.py edevice-decrypted/ -o msi-klc-models.json
"""
import argparse, glob, json, os, re

def load_all(d):
    specs = {}
    for f in glob.glob(os.path.join(d, "*.lisp")):
        specs[os.path.basename(f)[:-5]] = open(f, encoding="latin1").read()
    return specs

def parent_of(src):
    m = re.search(r'\(include\s+"([^"]+)"\)', src)
    return m.group(1) if m else None

def find_id(src):
    m = re.search(r'\(define\s+[\w-]*-id\s+0x([0-9A-Fa-f]{8})\)', src)
    if m:
        v = int(m.group(1), 16)
        return v >> 16, v & 0xFFFF          # vid, pid
    return None

def find_scale(src, ch):
    m = re.search(r'\(define\s+\(%s-scale\)\s+\(integer\s+\(\*\s+255\s+([0-9.]+)\)\)\)' % ch, src)
    return float(m.group(1)) if m else None

def find_coords(src):
    m = re.search(r'\(get-prism-sync-nonvariant-hid-info\)(.*?)\n\s*\)\s*\)', src, re.DOTALL)
    if not m: return None
    coords = {}
    for hid, x, y in re.findall(r'\((\d+)\s+(\d+)\s+(\d+)\)', m.group(1)):
        coords[int(hid)] = [int(x), int(y)]
    return coords or None

def find_int(src, name):
    m = re.search(r'\(define\s+%s\s+(\d+)\)' % re.escape(name), src)
    return int(m.group(1)) if m else None

def resolve(model, specs, fn, *a):
    """Walk model -> parent -> ... returning the first non-None result of fn(src, *a)."""
    seen = set()
    cur = model
    while cur and cur in specs and cur not in seen:
        seen.add(cur)
        r = fn(specs[cur], *a)
        if r is not None:
            return r, cur
        cur = parent_of(specs[cur])
    return None, None

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dir")
    ap.add_argument("-o", "--out", default="msi-klc-models.json")
    args = ap.parse_args()
    specs = load_all(args.dir)
    klc = [n for n in specs if ("klc" in n or "per-key" in n) and "alc" not in n and n != "ss-klc-base"]

    out = {}
    for m in sorted(klc):
        vidpid = find_id(specs[m])
        scale = {c: resolve(m, specs, find_scale, c)[0] or 1.0 for c in ("red", "green", "blue")}
        coords, coords_from = resolve(m, specs, find_coords)
        kc, _ = resolve(m, specs, lambda s: find_int(s, "klc-key-count"))
        chain = []
        cur = m
        while cur and cur in specs and cur not in chain:
            chain.append(cur); cur = parent_of(specs[cur])
        out[m] = {
            "usb": (f"{vidpid[0]:04x}:{vidpid[1]:04x}" if vidpid else None),
            "color_scale": [scale["red"], scale["green"], scale["blue"]],
            "key_count": kc,
            "key_coords_from": coords_from,
            "num_key_coords": len(coords) if coords else 0,
            "key_coords": coords,
            "include_chain": chain,
        }
    json.dump(out, open(args.out, "w"), indent=1)
    print(f"{len(out)} MSI KLC models -> {args.out}\n")
    for m, d in out.items():
        print(f"  {m:32s} usb={d['usb'] or '?':10s} scale={d['color_scale']} "
              f"coords={d['num_key_coords']}(from {d['key_coords_from']}) chain={'->'.join(d['include_chain'][:3])}")

if __name__ == "__main__":
    main()

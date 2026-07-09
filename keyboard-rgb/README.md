# Keyboard per-key RGB (SteelSeries KLC)

The GS66 Stealth 12UHS / MS-16V5 keyboard is a **SteelSeries per-key RGB** controller on **USB-HID**
(`1038:113a`, "SteelSeries KLC") — **not** driven by the EC, so the `msi-wmi-platform` driver does
not touch it. Per-key **color** goes over HID; **brightness/on-off** is a separate layer (EC
`0xEC.1`/`0xD3`, or the Fn brightness keys) — colors are invisible until brightness is raised.

This directory holds a self-contained, **hardware-validated** toolchain plus the reverse-engineered
layout data, derived from MSI Center itself rather than hand-maintained per model.

## Files
| File | What |
|---|---|
| `extract-msi-layouts.py` | Parse a decompiled `MysticLight_AllDevice.dll` → `msi-layouts.json` (PID lists, keymaps, region groups). |
| `msi-layouts.json` | Extracted layouts: `SupportList_Keyboard {1122,113A}`, `GE73Keys` (key→HID-usage), `Group1..6_Offset` (region partition). |
| `gs66-keymap.json` | GS66-specific keymap (88 keys, numpad dropped), each key with its HID usage + region group. |
| `msi-nb-rgb.py` | Minimal per-key RGB writer (hidraw, no hidapi). Reproduces MSI's frame formats. |

## The protocol (reverse-engineered from MSI Center)
- Device interface: `/dev/hidraw1` (USB interface 0, vendor usage page `0xFFC0`). No report IDs.
  Feature report **524 B** (per-key color), output report **64 B** (refresh/commit).
- Keys are addressed by **USB HID usage code** (e.g. Esc `0x29`, A `0x04`, Power `0x66`).
- **Steady/group frame** (`Set_Keyboard_Color`): cmd `0x0E`, 12-byte entries, keys split across
  **6 region groups** (`Group1..6_Offset`). All six must be sent to cover the whole keyboard —
  Power/Ins/Del live in Group6, which is why 4-region tools (msi-perkeyrgb) never light Power.
- **Free per-key frame** (`Set_Keyboard_SyncColor_Free`): cmd `0x0C`, 4-byte `[hid,R,G,B]` entries,
  ≤130 keys/report. (This is the frame OpenRGB's `MSILaptopController` already implements.)
- Two gotchas: (1) send a **refresh** (`0x09` output report) to commit; (2) sleep **~10 ms between
  group reports** or the controller silently drops some (symptom: a fraction of keys keep the old
  color). MSI does `Thread.Sleep(10)` between groups.

## Usage
```sh
# needs root (hidraw is root-only)
sudo ./msi-nb-rgb.py all 00ff00                 # whole keyboard green (all 6 groups)
sudo ./msi-nb-rgb.py off                        # blank everything, incl. Power
sudo ./msi-nb-rgb.py keys CLK_Escape=ff0000 CLK_Power=00ff00 CLK_W=0000ff
```

## Regenerating the layout data (from your own MSI Center install)
The layouts come from **MSI Center**, not from any third-party tool. Point at your install
(the Windows partition is fine):
```sh
DLL="/mnt/win/Program Files (x86)/MSI/MSI Center/Mystic Light/MysticLight_AllDevice.dll"
ilspycmd "$DLL" -o /tmp/mlad_src              # dotnet tool install -g ilspycmd
./extract-msi-layouts.py /tmp/mlad_src/MysticLight_AllDevice.decompiled.cs -o msi-layouts.json
```
The per-key **laptop** layout is model-independent: one `GE73Keys` map + the 6-group partition
covers the whole MSI SteelSeries per-key notebook family (controller PIDs `1122` and `113A`).

## Upstreaming (in progress)
- **OpenRGB** (recommended): its `MSILaptopController` already implements the `0x0C` KLC frame
  (byte-identical: report id 0, cmd `0x0C`, 525 B, 4-byte entries) — currently gated to the MSI
  Raider A18 (`0x1122`). Adding the GS66 = a new `REGISTER_HID_DETECTOR(…, 0x1038, 0x113A)` + a
  model entry (`Stealth GS66 12UHS`, board `MS-16V5`) with the keymap from `gs66-keymap.json`.
- **msi-perkeyrgb**: exact protocol but dormant, and uses a **4-region** split (missing Group6 /
  Power); would need `REGION_KEYCODES` extended to the full 6-group partition.

## Tip: RGB off on suspend
The keyboard stays lit during s2idle. A systemd hook can blank it before sleep — see `../suspend/`.

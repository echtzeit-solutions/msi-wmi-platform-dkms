# Keyboard per-key RGB (SteelSeries)

The GS66 12UHS keyboard is a **SteelSeries per-key RGB** controller on **USB-HID**
(`1038:113a`, "SteelSeries KLC") — it is **not** driven by the EC, so the `msi-wmi-platform`
driver does not touch it. Control it from userspace.

## Option A — OpenRGB (recommended)
```bash
# distro package or AppImage from https://openrgb.org
sudo apt install openrgb        # or grab the AppImage
openrgb                          # GUI; or: openrgb --device 0 --mode static --color FF0000
```
- Needs the udev rules OpenRGB ships (so it can talk to the HID device without root).
- If the keyboard isn't detected, verify support/PID against the OpenRGB device list; the
  SteelSeries per-key protocol may need a recent OpenRGB build or a profile addition.

## Option B — msi-perkeyrgb
```bash
pipx install msi-perkeyrgb       # https://github.com/Askannz/msi-perkeyrgb
msi-perkeyrgb --model GS66 -s steady -c FFFFFF
```
Note: upstream `msi-perkeyrgb` is older and may not list GS66/`1038:113a` yet — you may need a
model/protocol patch.

## Status
Not yet turnkey on this exact controller — tracked as a TODO. Contributions (an OpenRGB
profile or a `msi-perkeyrgb` model entry for `1038:113a`) welcome.

## Tip: RGB off on suspend (save power)
The keyboard stays lit during s2idle. A simple systemd hook can blank it before sleep — see
`../suspend/`.

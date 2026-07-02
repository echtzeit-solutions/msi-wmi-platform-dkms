# Linux support for MSI Stealth GS66 12UHS (board MS-16V5)

Community hardware-enablement for the MSI **Stealth GS66 12UHS** and its **MS-16V5**
board siblings (EC firmware `16V5EMS1.*`, e.g. 12UGS/12UE) on modern Linux.

The centrepiece is a DKMS build of **`msi-wmi-platform`** with fan control, performance
profiles, battery-charge limiting and a suspend fix, plus setup notes for the rest of the
machine. Everything here was validated on a GS66 12UHS (EC `16V5EMS1.108`).

## What works

| Feature | How | Status |
|---|---|---|
| Fan RPM + **fan-curve control** (CPU/GPU) | `msi-wmi-platform` → hwmon `pwmN`/`pwmN_auto_point*` | ✅ |
| **Performance profiles** (low-power/balanced/…/performance) | `platform_profile` (EC shift-mode) | ✅ |
| **Battery charge limit** | `power_supply` `charge_control_end_threshold` | ✅ |
| **Profile kept across suspend** | driver `.resume` re-applies it | ✅ |
| Keyboard **per-key RGB** | SteelSeries USB-HID → OpenRGB / msi-perkeyrgb | see `keyboard-rgb/` |
| **Suspend / lid-wake** | s2idle + optional suspend-then-hibernate | see `suspend/` |
| Webcam, Wi-Fi/BT, trackpad | in-kernel | ✅ out of the box |
| Audio (speakers), Fingerprint | *planned* | ⏳ later |

## Quick install

```bash
git clone <this-repo> && cd linux-msi-ms16v5
sudo ./install.sh          # builds + installs the DKMS driver, prints next steps
```

Then see `keyboard-rgb/` and `suspend/` for the userspace/config bits.

## Layout
- `msi-wmi-platform/` — the DKMS kernel module (+ upstream patch drafts).
- `fan-curve/` — apply a custom fan curve (script + systemd unit + resume hook); ported from
  the old msi-ec/isw model to the new hwmon interface.
- `keyboard-rgb/` — per-key RGB setup.
- `suspend/` — s2idle + suspend-then-hibernate config.
- `msi-center-manifest/` — reverse-engineered MSI Center feature manifest: `!!MSI!!` decrypt tool +
  a queryable SQLite census (1,919 models × 21 features) showing how MSI gates features.
- `docs/` — reverse-engineering notes, EC register map, MSI Center architecture, fan-curve/RPM
  data, upstream status.

## Status / upstreaming
The driver is based on the in-review mainline `msi-wmi-platform`
fan-curve/profile/charge series; our MS-16V5 additions are drafted as patches in
`msi-wmi-platform/patches-upstream/` for contribution. See `docs/upstream-state.md`.

## Disclaimer
Community project, not affiliated with MSI. It pokes the embedded controller; use at your
own risk. GPL-2.0 (matches the kernel module).

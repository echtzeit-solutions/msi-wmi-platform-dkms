# Linux enablement for MSI notebooks (msi-wmi-platform)

Capability/feature-based Linux hardware support for MSI notebooks, built on the mainline
**`msi-wmi-platform`** WMI driver: fan control, performance profiles, battery-charge limiting and a
suspend/resume fix, plus keyboard-RGB and suspend setup notes.

The driver detects features at runtime (like MSI Center): presence via the `Get_Device` bitmap, and
control (profile/charge/fan curves) via the same heuristic MSI Center uses — an MSI notebook/
convertible with a modern (WMI v2, Tigerlake+) ABI — so it works **generically across modern MSI
notebooks**, no per-model table required. **Verified on** the MSI Stealth GS66 12UHS (board MS-16V5,
EC `16V5EMS1.*` — e.g. 12UHS/12UGS/12UE).

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

Requires a **kernel ≥ 6.15** (the driver uses the `power_supply` extension and
new `platform_profile` APIs; developed and tested on 7.0.x) plus `dkms` and the
headers for your running kernel.

```bash
git clone <this-repo> && cd msi-wmi-platform-dkms
sudo ./install.sh          # builds + installs the DKMS driver, prints next steps
```

Then see `fan-curve/`, `keyboard-rgb/` and `suspend/` for the userspace/config bits.

**Conflicts:** don't run this together with the out-of-tree
[msi-ec](https://github.com/BeardOverflow/msi-ec) driver — both write the same EC
registers (shift mode, fan mode) and will fight. Blacklist or remove one of them
(the in-tree msi-ec built into kernels ≥ 6.4 is charge-thresholds-only and also
overlaps: pick one owner for the charge limit).

## Trying this on another MSI notebook

The driver enables control features on any modern MSI notebook via the same
heuristic MSI Center uses — your model does **not** need to be listed. Verified
so far only on the MS-16V5; on other boards the sensors are safe, and a wrong
heuristic hit typically means a control that silently no-ops. If anything
misbehaves:

- fan control: `echo 2 | sudo tee /sys/class/hwmon/hwmonX/pwm1_enable` puts the
  EC back in auto mode and restores the factory curve; unloading the module
  (`sudo modprobe -r msi_wmi_platform`) does the same.
- worst case: uninstall (below) — the in-tree read-only driver takes over.

Please report success or failure either way (open an issue) with:
board name (`cat /sys/class/dmi/id/board_name`), the `dmesg | grep msi_wmi`
lines (they include the EC firmware ID, how the model was matched, and the
`Get_Device(0x01)` presence bitmap), and which sysfs features appeared/worked.
That is exactly the data needed to grow the support table.

## Uninstall

```bash
sudo modprobe -r msi_wmi_platform
sudo dkms remove -m msi-wmi-platform-dkms --all
sudo rm -rf /usr/src/msi-wmi-platform-dkms-*
sudo modprobe msi_wmi_platform   # loads the in-tree (read-only fans) module again
```

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
own risk. Licensed GPL-2.0 (full text in `LICENSE`); the kernel-module sources are
GPL-2.0-or-later per their SPDX headers, combined with the GPL-2.0-only
`firmware_attributes_class.h` the module is effectively GPL-2.0-only.

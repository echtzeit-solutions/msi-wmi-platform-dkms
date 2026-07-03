# msi-wmi-platform (MS-16V5 build)

DKMS build of the mainline `msi-wmi-platform` driver with the in-review
fan-curve / platform_profile / charge series applied, **plus MS-16V5 additions**:

- **MS-16V5 board quirk** (Stealth GS66 12UHS and siblings).
- **State restore on firmware resume** — deep S3 / hibernate reset the EC's shift-mode, fan mode
  and fan curve tables (s2idle keeps them). The driver snapshots fan state on suspend and, gated
  on `pm_resume_via_firmware()`, re-applies `platform_profile` + fan curve/mode on resume (no-op
  on s2idle). No userspace resume hook needed.
- **EC-firmware-ID matching** — one `16V5` entry covers all `16V5EMS1.*` SKUs (no per-DMI list).
- **Capability/feature-based architecture** — a runtime capability cache (`Get_Device(0x01)`
  presence bitmap) + a `msi_features[]` descriptor table (detect/setup/suspend/resume per feature)
  driven by a two-pass probe, mirroring how MSI Center decides features. Control features
  (profile/charge/fan-curves) are gated by `msi_control_supported()` — the same heuristic MSI
  Center's `IsSupport()` uses (MSI vendor + notebook/convertible chassis + WMI v2 + Tigerlake EC
  flag) — so control works generically on modern MSI notebooks with no per-model entry. The `model`
  table only carries fan count / TDP limits + `force_control`/`deny_control` edge overrides. See
  `../docs/msi-center-architecture.md`.

## Install (recommended: the repo installer)
```bash
sudo ../install.sh
```

## Manual DKMS
```bash
sudo cp -r . /usr/src/msi-wmi-platform-dkms-0.6
sudo dkms add    -m msi-wmi-platform-dkms -v 0.6
sudo dkms install -m msi-wmi-platform-dkms -v 0.6 --force
sudo modprobe -r msi_wmi_platform && sudo modprobe msi_wmi_platform
```
Installs to `updates/dkms/` (overrides the in-tree module), DKMS auto-signs with your MOK
(loads under Secure Boot) and auto-rebuilds on kernel updates. `firmware_attributes_class` is
pulled in as a dependency automatically.

## Usage
- **Fans:** `hwmon` device `msi_wmi_platform`. `pwmN_enable` = **1 manual** (user curve) / **2 auto**
  (EC control). Curve points: `pwmN_auto_pointM_temp` / `pwmN_auto_pointM_pwm` (pwm1=CPU, pwm2=GPU).
  See `../docs/fan-curve-rpm.md` for measured setpoint→RPM (note: 0=off, ~14–50%≈3000 rpm floor,
  ~57–100% is the real range).
- **Profiles:** `/sys/firmware/acpi/platform_profile` (low-power/balanced/balanced-performance/performance).
- **Battery:** `echo 80 | sudo tee /sys/class/power_supply/BAT1/charge_control_end_threshold`
  (10–100%; hardware resumes charging below end−10%).

## Single source of truth
`msi-wmi-platform.c` is **generated**, not hand-edited:

```
base.c  +  patches-upstream/00NN-*.patch   --( ./regen.sh )-->  msi-wmi-platform.c
```

`base.c` is Antheas Kapenekakis's in-review v1 series applied on mainline v7.0;
`patches-upstream/00NN-*.patch` is our follow-up series — the **same code** that
gets built here and taken to LKML, so "tested" == "submitted". `make verify`
fails if the committed `.c` has drifted from `base.c` + the patches. To change
the driver, edit a patch (or add one) and `./regen.sh` — never edit the `.c`.
Series contents, submission plan and the version-tracking/rebase workflow:
`patches-upstream/NOTES.md`. Attribution follows
`Documentation/process/coding-assistants.rst`.

Package files: `base.c` + `patches-upstream/` (source), `msi-wmi-platform.c`
(generated build artifact), `firmware_attributes_class.h` (vendored kernel
header), `regen.sh`, `Makefile`, `dkms.conf`.

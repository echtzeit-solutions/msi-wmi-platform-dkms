# msi-wmi-platform (MS-16V5 build)

DKMS build of the mainline `msi-wmi-platform` driver with the in-review
fan-curve / platform_profile / charge series applied, **plus MS-16V5 additions**:

- **MS-16V5 board quirk** (Stealth GS66 12UHS and siblings).
- **`platform_profile` re-apply on resume** — the EC drops shift-mode across suspend and the
  firmware doesn't restore it, so the selected profile would otherwise be lost.
- **EC-firmware-ID matching** — one `16V5` entry covers all `16V5EMS1.*` SKUs (no per-DMI list).

## Install (recommended: the repo installer)
```bash
sudo ../install.sh
```

## Manual DKMS
```bash
sudo cp -r . /usr/src/msi-wmi-platform-ms16v5-0.3
sudo dkms add    -m msi-wmi-platform-ms16v5 -v 0.3
sudo dkms install -m msi-wmi-platform-ms16v5 -v 0.3 --force
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

## Upstream
`patches-upstream/` holds the MS-16V5 patches as drafts (needs rebasing onto the current
series before submission). Attribution follows `Documentation/process/coding-assistants.rst`.

Source: `msi-wmi-platform.c` (base from linux-source-7.0.0 + series + our changes),
`firmware_attributes_class.h` (vendored kernel header), `Makefile`, `dkms.conf`.

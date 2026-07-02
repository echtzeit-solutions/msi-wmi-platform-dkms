#!/usr/bin/env bash
# Installer for the msi-wmi-platform DKMS module (MS-16V5 / GS66 12UHS).
# Builds, MOK-signs (via DKMS) and installs so it overrides the in-tree module and
# auto-rebuilds on kernel updates. Run with sudo.
set -euo pipefail

PKG=msi-wmi-platform-dkms
VER=0.6
SRC="$(cd "$(dirname "$0")/msi-wmi-platform" && pwd)"

if [[ $EUID -ne 0 ]]; then echo "Run as root (sudo ./install.sh)"; exit 1; fi

echo "==> Checking prerequisites"
command -v dkms >/dev/null || { echo "Install 'dkms' first (apt install dkms)"; exit 1; }
[[ -d /lib/modules/$(uname -r)/build ]] || { echo "Install kernel headers for $(uname -r)"; exit 1; }

echo "==> Installing sources to /usr/src/${PKG}-${VER}"
rm -rf "/usr/src/${PKG}-${VER}"
cp -r "$SRC" "/usr/src/${PKG}-${VER}"
# ensure dkms.conf package name/version match
sed -i "s/^PACKAGE_NAME=.*/PACKAGE_NAME=\"${PKG}\"/;s/^PACKAGE_VERSION=.*/PACKAGE_VERSION=\"${VER}\"/" \
    "/usr/src/${PKG}-${VER}/dkms.conf"

echo "==> dkms add/build/install"
dkms remove -m "$PKG" -v "$VER" --all 2>/dev/null || true
dkms add    -m "$PKG" -v "$VER"
dkms build  -m "$PKG" -v "$VER"
dkms install -m "$PKG" -v "$VER" --force   # --force overrides the in-tree module

echo "==> Loading"
modprobe -r msi_wmi_platform 2>/dev/null || true
modprobe msi_wmi_platform

echo
echo "Done. Verify:"
echo "  sensors | grep -A6 msi_wmi_platform      # fans + pwm"
echo "  cat /sys/firmware/acpi/platform_profile_choices"
echo "  ls /sys/class/power_supply/BAT*/charge_control_end_threshold"
echo
echo "Note: Secure Boot — DKMS signs with your enrolled MOK automatically."
echo "Next: see keyboard-rgb/ (RGB) and suspend/ (lid-wake + hibernate)."

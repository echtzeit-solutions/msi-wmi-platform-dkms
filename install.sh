#!/usr/bin/env bash
# Installer for the msi-wmi-platform DKMS module.
# Builds, installs and loads the module so it overrides the in-tree one and
# auto-rebuilds on kernel updates. Run with sudo.
set -euo pipefail

PKG=msi-wmi-platform-dkms
MIN_KVER=6.15   # power_supply extension + new platform_profile APIs
SRC="$(cd "$(dirname "$0")/msi-wmi-platform" && pwd)"
VER="$(sed -n 's/^PACKAGE_VERSION="\(.*\)"/\1/p' "$SRC/dkms.conf")"
[[ -n "$VER" ]] || { echo "cannot read PACKAGE_VERSION from $SRC/dkms.conf"; exit 1; }

if [[ $EUID -ne 0 ]]; then echo "Run as root (sudo ./install.sh)"; exit 1; fi

echo "==> Checking prerequisites"
command -v dkms >/dev/null || {
    echo "Install 'dkms' first (Debian/Ubuntu: apt install dkms; Fedora: dnf install dkms; Arch: pacman -S dkms)"
    exit 1
}
[[ -d /lib/modules/$(uname -r)/build ]] || {
    echo "Install kernel headers for $(uname -r) first"
    echo "(Debian/Ubuntu: linux-headers-$(uname -r); Fedora: kernel-devel; Arch: linux-headers)"
    exit 1
}
kver="$(uname -r)"
if ! printf '%s\n%s\n' "$MIN_KVER" "${kver%%-*}" | sort -V -C; then
    echo "Kernel $kver is too old: this driver needs >= $MIN_KVER (developed on 7.0.x)."
    echo "It will not compile against older kernels."
    exit 1
fi
if lsmod | grep -q '^msi_ec\b'; then
    echo "WARNING: the msi-ec driver is loaded. It writes the same EC registers"
    echo "(shift mode / fan mode) and will fight this driver. Blacklist one of them."
fi

# single-source-of-truth check: the shipped .c must match base.c + the patch series
if command -v patch >/dev/null; then
    make -s -C "$SRC" verify || exit 1
fi

echo "==> Installing sources to /usr/src/${PKG}-${VER}"
rm -rf "/usr/src/${PKG}-${VER}"
cp -r "$SRC" "/usr/src/${PKG}-${VER}"

echo "==> dkms add/build/install"
# remove any previously registered version of this package (also old ones)
while read -r oldver; do
    [[ -n "$oldver" ]] && dkms remove -m "$PKG" -v "$oldver" --all </dev/null
done < <(dkms status "$PKG" 2>/dev/null | sed -n "s|^${PKG}[/, ]*\([0-9][^,: ]*\).*|\1|p" | sort -u)
dkms add    -m "$PKG" -v "$VER"
dkms build  -m "$PKG" -v "$VER"
dkms install -m "$PKG" -v "$VER" --force   # --force overrides the in-tree module

echo "==> Loading"
if ! modprobe -r msi_wmi_platform 2>/dev/null && lsmod | grep -q '^msi_wmi_platform\b'; then
    echo "Could not unload the currently loaded msi_wmi_platform (in use)."
    echo "Reboot to pick up the DKMS module."
    exit 0
fi
if ! modprobe msi_wmi_platform; then
    echo
    echo "modprobe failed. If Secure Boot is enabled, the DKMS signing key is"
    echo "probably not enrolled ('Key was rejected by service' in dmesg). Enroll it:"
    echo "  sudo mokutil --import /var/lib/dkms/mok.pub    # then reboot and confirm"
    echo "(Key location varies by distro; Ubuntu also uses /var/lib/shim-signed/mok/MOK.der.)"
    command -v mokutil >/dev/null && mokutil --sb-state || true
    exit 1
fi
loaded="$(modinfo -F filename -k "$(uname -r)" msi_wmi_platform 2>/dev/null || true)"
if [[ "$loaded" != *"/updates/dkms/"* ]]; then
    echo "WARNING: the module resolved to '$loaded', not the DKMS copy under updates/dkms."
fi

echo
echo "Done. Verify:"
echo "  sensors | grep -A6 msi_wmi_platform      # fans + pwm"
echo "  cat /sys/firmware/acpi/platform_profile_choices"
echo "  ls /sys/class/power_supply/BAT*/charge_control_end_threshold"
echo
echo "Next: see fan-curve/ (custom fan curve), keyboard-rgb/ (RGB) and"
echo "suspend/ (lid-wake + hibernate). Please report success/failure on other"
echo "MSI models -- see 'Trying this on another MSI notebook' in the README."

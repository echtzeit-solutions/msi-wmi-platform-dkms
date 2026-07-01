# Suspend / lid-wake / hibernate (MS-16V5)

## TL;DR
Use **s2idle** for the light sleep (lid-open wakes the machine); optionally add
**suspend-then-hibernate** so it drops to hibernate (zero drain) after a delay.

## Why s2idle, not deep S3
This laptop is validated for **s2idle** (Modern Standby / S0ix). If you force **deep S3**
(`mem_sleep_default=deep`), the EC no longer asserts the **lid-open wake** — you must press the
power button. (The ACPI `_WAK` path and EC firmware only arm lid wake in s2idle; deep S3 wakes
on the power button only.) So for working lid-wake, keep `mem_sleep = s2idle`.

Check current mode:
```bash
cat /sys/power/mem_sleep      # the [bracketed] one is active
```
Make s2idle the default — ensure **no** `mem_sleep_default=deep` in
`/etc/default/grub` (`GRUB_CMDLINE_LINUX_DEFAULT`), then `sudo update-grub`.
Verified: reaches S0i3.0 / Package-C10 (deep idle) when suspended.

## suspend-then-hibernate (optional, best of both)
Light suspend first (instant resume, lid-wake), then hibernate after a timeout (zero drain,
LUKS re-unlock on resume). Requires working hibernate (swap ≥ RAM + resume configured).
```bash
sudo cp sleep.conf.example /etc/systemd/sleep.conf.d/10-ms16v5.conf   # adjust delay
# test a single hibernate/resume cycle FIRST before relying on it:
systemctl hibernate
# then use / wire to lid:
systemctl suspend-then-hibernate
```

## Other s2idle drain knobs (optional)
- **PCIe ASPM** is FADT-disabled on this board; only `pcie_aspm=force` re-enables OS control
  (small stability risk). NVMe L1 is already firmware-enabled.
- **RGB keyboard** stays lit in s2idle — blank it on suspend (see `keyboard-rgb/`).
- **`msi_wmi` "Unknown event" spam** — can cause wakeups; unbind the legacy hotkey driver if unused.
- `powertop --auto-tune` / TLP apply USB-autosuspend + runtime-PM (watch out for USB input devices).

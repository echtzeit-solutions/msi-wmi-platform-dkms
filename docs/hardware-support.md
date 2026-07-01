# Hardware support matrix — MSI Stealth GS66 12UHS (MS-16V5)

Verified on EC firmware `16V5EMS1.108`, kernel 7.0.x.

| Component | Device / IDs | Support | Notes |
|---|---|---|---|
| Fans (RPM read) | WMI `ABBC0F6E` → hwmon | ✅ | RPM = 480000/tach |
| Fan curves (CPU/GPU) | hwmon `pwm1`/`pwm2` | ✅ | `pwmN_enable=1` manual; see `fan-curve-rpm.md` |
| Performance profiles | `platform_profile` (EC 0xD2 shift-mode) | ✅ | low-power/balanced/balanced-performance/performance |
| Battery charge limit | `power_supply` (EC 0xD7) | ✅ | end 10–100%, start=end−10 |
| Profile across suspend | driver `.resume` | ✅ | EC resets shift-mode on resume; driver re-applies |
| Cooler boost | EC 0x98 bit7 | ➖ | not a driver sysfs; resets on resume |
| Keyboard per-key RGB | SteelSeries `1038:113a` (USB-HID) | ⚠️ | userspace (OpenRGB/msi-perkeyrgb); see `keyboard-rgb/` |
| Suspend / lid-wake | s2idle | ✅ | use s2idle, not deep S3 (see `suspend/`) |
| Hibernate | swap ≥ RAM | ✅* | if resume configured (LUKS re-unlock ok) |
| Webcam | Bison UVC `5986:2127` | ✅ | stock UVC |
| Wi-Fi / BT | Intel AX211 `8087:0033` | ✅ | iwlwifi |
| Trackpad | Synaptics I2C-HID | ✅ | |
| Audio | SOF `sof-hda-dsp` | ⏳ | loads; verify speakers — planned |
| Fingerprint | Synaptics `06cb:009b` | ⏳ | libfprint "synaptics" + fprintd — planned |
| Fn-key hotkeys | `msi_wmi` | ⚠️ | some keys unmapped; 157 spurious WMI events |
| dGPU | NVIDIA (RTD3/D3cold) | ➖ | proprietary/nouveau, out of scope |

Legend: ✅ works · ⚠️ partial/needs setup · ⏳ planned · ➖ n/a or out of scope

## Reverse-engineering references (this repo)
- `ec-register-map.md` — EC registers, cross-validated (DSDT + msi-ec + MSI Center + live).
- `feature-catalog.md` — WMI method map (WMAM / ABBC0F6E), generic EC access ABI.
- `capability-map.md` — DSDT/WMI capability mapping.
- `fan-curve-rpm.md` — measured fan setpoint → RPM.
- `upstream-state.md` — mainline status + our contribution plan.

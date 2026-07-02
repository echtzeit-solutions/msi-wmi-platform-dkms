# Capability map — MSI Stealth GS66 12UHS (MS-16V5, EC 16V5EMS1.10F)

Data sources: live DSDT (`re/DSDT.dsl`), sysfs/WMI enumeration, EC firmware RE (`re/E16V5_MAIN_EC.bin`).

## Decision gate → WMI-first (resolved)
The MSI WMI device (`_WDG` @ DSDT:84251) exposes its **own `EmbeddedControl` region**
(DSDT:84267) with byte fields `TD28`, `TD33`, `TD68`, `TD80`, `TDA0`, `TDB7`, … i.e. **generic
per-offset access to the entire EC RAM**. Method GUIDs are the classic MSI set
`ABBC0F6{A,B,D,E}-8EA1-11D1-00A0-C90629100000` (methods AK/AL/AJ/AM = WMI read / write / access),
which `msi-wmi-platform` binds via `05901221-D566-11D1-B2F0-00A0C9062910`.

**Conclusion:** WMI can read/write *any* EC register → the driver can implement fan control,
cooler boost, charge thresholds, profiles, etc. purely over WMI (clean, upstreamable). Raw
`ec_read`/`ec_write` (via ACPI EC) is an optional fallback only.

## ACPI-named EC fields (from the EC device Field, DSDT:83305) — cross-validates msi-ec
| Offset | ACPI name | Meaning | msi-ec CONF29 |
|---|---|---|---|
| 0x2C.1 | MICL | mic-mute LED | mute/micmute leds |
| 0x2D.1 | MUTL | mute LED | |
| 0x2E.1 | CAML | camera enable | webcam 0x2e ✓ |
| 0x30 | POWS/LIDS | power state / lid | |
| 0x31 | MBTS…MBFL | main battery status bits | |
| 0x38–0x7B | MDCL/MDVL/MTEL… | battery design/volt/temp regs | |
| 0x7C | CPUT | CPU temperature | (msi-ec rt temp 0x68 = private view) |
| 0x7E.3 | CHET | charge-end flag | charge_control 0xd7 (private) |
| 0x80 | SYST | system/GPU temperature | GPU temp 0x80 ✓ |
| 0xD2 (2b) | SYSM | shift/system mode | shift_mode 0xd2 ✓ |
| 0xE6 | DBG + bits | debug + RSUS/FBST/… (suspend-related) | |
| 0xEB (7b) | PSNM | power scheme / super-battery | super_battery 0xeb ✓ |
| 0xEC.0/.1 | MODS / KBBL | mode / keyboard-backlight enable bit | |

**Not ACPI-exposed (need WMI-raw / firmware RE):** fan RPM & duty, fan curve tables, cooler
boost bit (msi-ec 0x98.7), charge start/end threshold values (msi-ec 0xd7), fan mode.

## Runtime presence probing — `Get_Device(0x01)` bitmap (how MSI Center detects hardware)
MSI Center does **not** use a static table for hardware *presence*. It calls
`InvokeWmiMethod(Get_Device /*enum 16*/, selector 0x01, byte[6])` and reads a capability bitmap
from EC/BIOS (`Data[1..]`), re-checked on every property access. Decoded from
`BaseModule` (`API_NB_Base Module.dll`) property getters:

| Bit | Feature | Source getter |
|---|---|---|
| `Data[1]` bit1 | WebCam present | `SupportedWebCam` |
| `Data[1]` bit4 (0x10) | Panel OD (overdrive) | `SupportedPanelOD` |
| `Data[2]` bit3 | Keyboard backlight | `EnableBackLight` |
| `Data[2]` bit6 | HSR panel | `EnableHSRpanel` |

The driver mirrors this: cache the bitmap once in probe (`msi_wmi_platform_caps_probe`) and gate
read-only *presence* features on it. **Control** features (fan/profile/charge) have **no**
capability bit — MSI offers them generically and lets the EC firmware decide; the driver keeps a
per-family allow-list for those instead (see `msi-center-architecture.md`).

## EC firmware confirmation (Ghidra, 8051, 447 fns in base bank)
Strings confirm feature areas: thermal (`CPU_CrtT`,`CPU_ThtlT`,`SYS_CrtT`,`SYS_ThtlT`),
**suspend** (`S0i3 Wake up`,`Not enter deep sleep mode`,`Wakeup by WDT/IKB/CIR/TMR`),
battery (`Batt_In/Out`,`BATT_OFF`,`BattThrottleST`), lid/mute/camera, USB-PD/UCSI (`OEM
Processing UCSI Cmd …`), and a built-in EC debug monitor (`<01>IDATA <02>ECRAM <03>GPIO
<04>KBC <05>ESB`).

## Host tooling / constraints (verified)
- Kernel 7.0.0-22; headers present. `acpi_call` (MOK-signed) + `ec_sys write_support=1` loaded.
- Secure Boot **enabled** but shim validation disabled; DKMS signs modules with enrolled MOK
  (`/var/lib/shim-signed/mok/MOK.der`) → **we can sign our driver the same way**.
- `msi_wmi_platform` in-tree currently exposes only `fan1..4_input` (RPM) via hwmon.

## Open items for RE / next
- ✅ Map fan RPM/duty/curve + cooler-boost + charge-threshold EC offsets — done (see
  `ec-register-map.md`, live-validated).
- ✅ Reverse **MSI Center (Windows)** — done: `Set_Data(idx,val)`=raw EC write; presence via
  `Get_Device(0x01)`; manifest census decrypted; architecture in `msi-center-architecture.md`.
- Remaining (low-value): Ghidra pass on native `API_Kernel.dll` to confirm the command-frame →
  EC-register mapping (values already validated on hardware).

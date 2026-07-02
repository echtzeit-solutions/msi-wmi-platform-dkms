# EC register map — MSI Stealth GS66 12UHS (MS-16V5, EC 16V5EMS1.10F)

Consolidated from three independent sources:
- **DSDT** ACPI EC field names (`re/DSDT.dsl`)
- **msi-ec** CONF29 (`../msi-ec/msi-ec.c:2398`)
- **MSI Center** RE — .NET debug strings + logic in `msi-center-re/decompiled/` (authoritative:
  MSI's own code literally prints "into EC address 0xNN" and the bit math)

Access from OS: **WMI** (MSI_System / `MSIWMIACPI2`, GUID `ABBC0F6x-8EA1-11D1`, EC-RAM base
token `FC0008`) or **HID feature reports** (`HidD_Set/GetFeature`) — both reach EC RAM. Our
driver will use WMI EC read/write (see `capability-map.md`).

## Confirmed control registers
| EC addr | Feature | Encoding (from MSI Center logic) | Corroboration |
|---|---|---|---|
| **0x98** | **Cooler Boost** | bit7: `on ? v|0x80 : v&0x7F` | msi-ec 0x98.7 ✓ |
| **0xD2** | **Shift / perf mode** (`SYSM`) | `v &= 0xC3; \|0x80 ability; \|0x40 active; low2: eco=+0, mode+=4/+1` | msi-ec 0xd2, DSDT SYSM ✓ |
| **0xD4** | **Fan mode** (auto/silent/advanced) | set in `init()` (6 sites) | msi-ec fan_mode 0xd4 ✓ |
| **0xE8** | **Fn/Win key swap** | `Set_EC_Mem_Flag_Of_WinAndFnKeyChange` | msi-ec 0xe8 ✓ |
| **0xEB** | Super-battery / power scheme (`PSNM`, 7b) | WMI value & mask | msi-ec 0xeb, DSDT PSNM ✓ |
| **0xDB** | **USB LED / USB backlight** | `setUSBLED(status)` | isw USB backlight |
| **0x2C** | mic-mute LED (`MICL`) | bit write | DSDT MICL |
| **0x2E** | camera / webcam (`CAML`) | bit write | msi-ec webcam 0x2e, DSDT CAML |
| **0x2F** | resume-related toggle | `ResumeAutomatic()` | — |
| **0xBE / 0xD1** | status get/set (GetStatus/SetStatus) | mode/status byte | investigate |
| **0xEC** | mode(.0)/keyboard-backlight enable(.1) (`MODS`/`KBBL`) | bit | DSDT |

## Sensors (read)
| EC addr | Field | Notes |
|---|---|---|
| 0x68 | CPU realtime temp (msi-ec) | ACPI `CPUT`@0x7C is a separate view |
| 0x80 | GPU/system temp (`SYST`) | msi-ec GPU temp ✓ |
| fan tach | CPU/GPU fan RPM (16-bit) | **RPM = 60000000 / (((Hi<<8)+Lo) × 2 × 62.5)** — from MSI Center `CalculateFanSpeed`; read as Data[0..1]=CPU, Data[2..3]=GPU |

## Fan curves — CONFIRMED LIVE (ec_dump_baseline.bin)
7-point tables, values are °C / % (read straight from the running EC):
- **CPU temp points @ 0x6A** = `55,60,65,70,90,95,100`
- **CPU speed points @ 0x72** = `45,50,60,65,80,85,100`
- **GPU temp points @ 0x82** = `50,60,70,82,90,93,100`
- **GPU speed points @ 0x8B** = `45,60,70,80,85,100`
Matches msi-ec CONF29 layout. Fan duty/RPM read-back likely at 0xC9/0xCB/0xCD (live: a0/9b/c6).

## Battery charge threshold — location confirmed, encoding TBD
- **0xD7** live = `0x80`. msi-ec uses 0xd7. Need live diff (set 60/80/100% in MSI Center or via
  Set_MasterBattery) to decode: likely `0x80 | percent` (0x80=off/100%). DSDT flag `CHET`@0x7E.3.

## Live baseline snapshot (re/ec_dump_baseline.bin, 256 B ACPI EC space)
Confirmed: 0x68=0x34 (52°C CPU), 0xD2=0xC1 (shift comfort: ability+active+eco base),
0xD4=0x0D (fan auto), 0x98=0x02 (boost off), 0xD7=0x80 (batt). EC version string @0xA0 =
`16V5EMS1.108` (bundled in BIOS .10F; covered by msi-ec CONF29).

## LIVE-CONFIRMED via write→diff→restore (ec_tool.py / ec_probe_batch.py, all restored OK)
Controls (each writable, single-purpose, cleanly restored):
- **0x98 bit7 = Cooler Boost** (on=0x82); when set, fan duty regs 0xC9/0xCB/0xCD jump.
- **0xDB = USB backlight LED** (0x00↔0xFF)
- **0xE8 = Fn/Win swap** (bit4 / 0x10)
- **0xEC = keyboard-backlight enable** (bit1 / 0x02; `KBBL`)
- **0x2C = mic-mute LED**, **0x2D = mute LED** (base 0x04)

Dynamic sensor/fan registers (auto-drift; READ-only telemetry, do not treat as controls):
**0x68** (CPU temp), **0x4A**, **0x9E**, **0xC9/0xCB/0xCD** (CPU/GPU fan duty or tach), **0xDD**,
**0xED** (~0xC5/0xC6). RPM from tach via `60e6/(((Hi<<8)+Lo)*2*62.5)`.

Profile/power controls — LIVE-CONFIRMED writable & cleanly restored:
- **0xD2 shift**: comfort=0xC1 (base), eco=0xC2, turbo=0xC4. **Coupling:** setting eco (0xC2)
  also sets super-battery **0xEB=0x0F** (perf mode drives the power scheme).
- **0xD4 fan mode**: auto=0x0D (base), silent=0x1D, advanced=0x8D.
- **0xEB super-battery**: 0x00 off / nonzero on (0x0F seen via eco).
- **0xD7 charge threshold**: base 0x80; accepts 0xBC/0xE4 (encoding `0x80|percent` likely) —
  needs AC-cycle to confirm charge behavior.
Full-EC restore verified: after all probes, every control reg == baseline (only sensors drift).

Remaining: WMI command domains (Get_Fan/Get_Thermal/… via acpi_call) for structured + hidden
(Debug/AP) selectors; confirm 0xD7 charge encoding under AC cycling.

## Model abstraction (how MSI Center supports many models) — key for sibling strategy
MSI Center is **model-generic**: it does NOT keep per-model register tables. Instead it uses a
**generic address-parameterized EC-RAM ABI** that every model's firmware implements:
- `InvokeWmiMethod(Methods.25, <ec_addr>, {value})` = **write EC[addr]=value** (confirmed args:
  152=0x98 boost, 210=0xD2 shift, 212=0xD4 fan mode, 232=0xE8 Fn/Win, 219=0xDB USB LED, 44/46/47).
- `Methods.16` / `Methods.22` = **read** EC block.
- Legacy fallback: `MemRwService.Read/Write("98",…)` via named pipe → `root\WMI:Q|S:MSI_System`.
Model differences are handled by: (a) `WmiMajorVersion` (firmware ABI version, not model),
(b) runtime capability flags (`IsHavingAbilityToSupportShiftMode`, `IsSupport()`, `Get_Device`
bitmap), (c) DMI `SystemProductName` + cloud manifest (`DefineBaseV1.dat` + the AES-encrypted
`PackageDataV2.dat`, keyed by marketing name) for *feature enablement only*. Register addresses
are a **line-wide convention** — confirmed **uniform, no per-model/family branch** in MSI's own
setters (only the WMI-version branch). The manifest was decrypted and censused (1,919 models ×
21 NB components); the CDN serves **feature-generic** packages only — **no device-specific DLL**.
See `../msi-center-manifest/` (decrypt tool + SQLite census) and `msi-center-architecture.md`.

**Implication:** this is the same generic WMI method interface the in-tree `msi-wmi-platform`
binds (`05901221` GUID). Our driver should mirror it — generic WMI read/write-byte + a small
shared address map + capability probing — which scales to MS-16Vx siblings far better than
msi-ec's per-firmware CONF tables.

## MSI Center command protocol (for reference / live tracing)
8-byte command frames, e.g. `CMD_FanCoolerBoostON = {2,0,0,9,1,7,0,1}` / `OFF …0,2`,
`CMD_FanData {2,0,0,9,1,7,0,3}`, `CMD_SetFanAdvanced {0,21}`, `CMD_SetScenarioMode {0,18}`.
These are built in the **managed** `BaseModule` and dispatched via
`DataCenter.Transfer_ToAPI("Kernel", frame)` to the **native** engine (`API_Kernel.dll` /
`*_Engine.dll`), which is where the frame→WMI/EC-register translation happens — it is **not** in
the decompilable managed layer, so pinning MSI's exact charge/fan-curve selectors would need a
Ghidra pass on `API_Kernel.dll` (low value: our msi-ec/EC-diff values above are hardware-validated).
See `msi-center-architecture.md` for the full 3-layer model.

## Next confirmations
1. EC firmware RE (labeling workflow + targeted): find the command handler that maps
   WMI/HID packets → these EC offsets; pin charge-threshold + fan-curve addresses.
2. Live: `acpi_call`/`ec_sys` diff EC dump while toggling each MSI Center control (full
   experimentation approved) to verify every offset/encoding before driver use.

## Charge threshold encoding — DERIVED (decomp, task #10)
EC 0xD7 stores `percent | 0x80` (== percent + 0x80). threshold% = raw & 0x7F (== raw - 0x80).
Valid raw 0x8A..0xE4 => 10..100%. Current 0x80 => unset/unlimited (driver reports end=0).
msi-ec CONF29: address=0xd7, offset_start=0x8a, offset_end=0x80, range 0x8a..0xe4, start=end-10 (HW).
Driver set `val|BIT(7)` and get `val&~BIT(7)` are CORRECT. Driver exposes end only (start=end-10 auto).
Physical validation (AC cycle, does charging stop at limit) = task #13.

## Charge threshold — PHYSICALLY VALIDATED (task #13)
Set end=80% via driver sysfs -> EC 0xD7=0xD0. Drained to 78%, replugged AC at 75%:
status="Not charging" (held), NOT charging to 100%. Confirms enforcement + start=end-10
hysteresis (start=70%): only resumes charging below 70%, tops to 80%. Full chain validated
(driver -> WMI -> EC 0xD7 -> HW charge stop). Encoding percent|0x80 correct.

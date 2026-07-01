# Authoritative feature catalog — MSI MS-16V5 (WORK IN PROGRESS)

Goal: the definitive list of features the firmware exposes to query/control, mapped to (a) the
WMI/EC access primitive, and (b) the Linux kernel interface we'll wire it into. Built by
cross-referencing three authoritative sources and RE'ing the firmware dispatch.

## Host ↔ EC interfaces (how we query anything)
1. **Standard ACPI EC** (ports 0x62/0x66) — named fields in DSDT (`capability-map.md`).
2. **Generic index/data mailbox = I/O ports 0x360 (index) / 0x361 (data)** — DSDT `WMIO`
   region; `WMRD(addr)->val`, `WMWT(addr,val)`. Exposes the *entire* EC RAM byte-wise.
3. **WMI method interface** — real GUID is **`ABBC0F6E-8EA1-11D1-00A0-C90629100000`** (`MSI_ACPI`,
   bound by `msi-wmi-platform`); `05901221-…` is just the generic Binary-MOF blob (wmi-bmof), NOT
   the method interface (see docs/upstream-state.md). The ACPI-level `WMAM` (EC.SCM0) takes a
   **32-byte command buffer** `{IPAR selector, BS00..BS31 payload}`; MSI's `MSIWMIACPI2` wraps it
   as `InvokeWmiMethod(Methods, selector, data[])`.

## Authoritative WMI method domains (from MSIWMIACPI2 `Methods` enum — MSI's own names)
Get/Set pairs for each domain (index in enum):
`WMI(0/…)`, `EC`, `BIOS`, `SMBUS`, **MasterBattery**, **SlaveBattery**, **Temperature**,
**Thermal**, **Fan**, **Device**, **Power**, **Debug**, **AP**, **Data**, **Package**.
Each domain takes a **selector byte** (2nd arg) + payload — the per-feature sub-commands are
decoded by the EC firmware (see RE plan). `Debug` and `AP` are not surfaced in the MSI Center UI
→ hidden-feature candidates.

## Known features so far → EC register → target kernel interface
| Feature | Access (confirmed) | Kernel interface |
|---|---|---|
| Fan RPM (CPU/GPU) | read; RPM=60e6/(((Hi<<8)+Lo)*2*62.5) | hwmon `fanN_input` |
| CPU/GPU temp | EC 0x68 / 0x80 (`SYST`) | hwmon `tempN_input` |
| Fan mode (auto/silent/adv) | EC 0xD4 (Set_Fan) | hwmon `pwmN_enable` + platform_profile |
| Cooler Boost | EC 0x98 bit7 | platform_profile "performance" / attr |
| Shift/perf mode | EC 0xD2 (`&0xC3;\|0x80\|0x40;+mode`) | **platform_profile** |
| Super-battery | EC 0xEB (`PSNM`) | platform_profile "low-power" |
| Battery charge threshold | EC 0xD7 (msi-ec) / Set_MasterBattery | power_supply `charge_control_*` |
| Fn/Win swap | EC 0xE8 | sysfs attr / keyboard |
| USB backlight | EC 0xDB | leds |
| Mic-mute / mute / camera | EC 0x2C / 0x2E | leds / rfkill |
| Keyboard backlight enable | EC 0xEC.1 (`KBBL`) | leds (per-key RGB is USB-HID, deferred) |
| Fan curve tables (7-pt) | EC 0x6a/0x72 (CPU), 0x82/0x8a (GPU) — verify | hwmon curve attrs |

## Hidden / not-user-surfaced — hunt plan
Sources to enumerate features the UI never shows:
- **`Get_Debug`/`Set_Debug`, `Get_AP`/`Set_AP`** WMI domains — RE their EC dispatch.
- **EC serial debug monitor** in firmware strings: `<01>IDATA <02>ECRAM <03>GPIO <04>KBC
  <05>ESB` — a built-in diagnostic console (read/write IDATA/ECRAM/GPIO, KBC, ESB).
- **EC-RAM fields present in DSDT/firmware but never touched by MSI Center** (diff the used-set
  vs full map).
- **Full EC-RAM dump via port 0x360/0x361** and diff across power/thermal states to surface
  undocumented live values.
- Undocumented **selector values** in each domain's firmware switch (e.g. extra fan modes,
  thermal policies, shift-mode levels beyond eco/comfort/turbo).

## AUTHORITATIVE WMI method map — WMAM (extracted from DSDT + live acpi_call verified)
Path: `\_SB.PC00.LPCB.EC.SCM0.WMAM(Arg0=inst, Arg1=cmd, Arg2=32B buf{IPAR,BS00..BS30})` →
returns 32B `BFL0{BL00=status, BL01..=data}`. Callable via acpi_call (verified: cmd 3 returns
EC version "16V5EMS1.108"). It is a **generic EC-RAM access ABI**, not secret per-feature logic:

| Arg1 | Function |
|---|---|
| 0x01/0x02 | buffer echo / init (return BFL0) |
| 0x03 | firmware info: EC 0x37 + 0xA0–0xBB (version string) |
| 0x04 | ping (return 1) |
| 0x05 / 0x06 | **read / write** EC page 0xE0–0xFF (IPAR = sub-block) |
| 0x07 / 0x08 | read / write EC page 0x00–0x26 |
| 0x09–0x1A | read/write further pages + semantic groups (temps 0x68/0x80; Fn/superbatt/kbd 0xE8/0xEB/0xEC; fan 0xD4–0xDF; etc.) |
| **0x1B** | **read ANY EC addr** (BL01 = WMRD(IPAR)) |
| **0x1C** | **write ANY EC addr** (WMWT(IPAR, BS00)) |
| 0x1D | interface version (BL01=MAJR, BL02=MINR) |

Full extraction: `re/wmam_dispatch.txt`. Generic mailbox also at ports 0x360/0x361 (WMRD/WMWT).

**Consequence:** host-accessible feature surface == the 256-byte EC RAM (IPAR is 8-bit). The
driver only needs generic read/write-byte (Arg1 0x1B/0x1C) + our register map. No hidden WMI-only
domains; "hidden" = unused EC bytes (dump all 256) + EC-internal XRAM 0xf000+ (RE-only, mapped).

## RE plan to complete this catalog (authoritative)
1. In Ghidra, find the EC firmware handler for the **0x360/0x361 mailbox** and the **WMAM
   command buffer** (dispatch on `IPAR`). That switch enumerates every domain+selector.
2. Enumerate each `Set_X`/`Get_X` selector table → names + EC effects (incl. hidden).
3. Live-confirm via port 0x360/0x361 (or acpi_call WMI) dumps + diffs (full experimentation ok).
4. Freeze the catalog; drive the driver's capability list + kernel-interface wiring from it.

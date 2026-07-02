# Upstream state & strategy (MS-16V5 / GS66 12UHS) — checked 2026

## Headline: don't build fresh — base on an in-flight upstream series
There is a pending patch series that adds **exactly our feature set** to `msi-wmi-platform`:
**"[PATCH] platform/x86: msi-wmi-platform: Add fan curves/platform profile/tdp/battery limiting"**
- Author **Antheas Kapenekakis** (lkml@antheas.dev), first posted 2025-05-11 (targets MSI Claw).
- Carried forward by **Derek J. Clark** (derekjohn.clark@gmail.com), repost 2026-05-08, still in review.
- Adds: **hwmon PWM + fan-curve control**, **platform_profile** (MSI shift mode), **TDP PL1/PL2**
  via firmware-attributes, **battery charge thresholds**, a **quirk system**, unlocked query
  funcs, input-buffer handling, and **OEM fan-curve restore on disable/unload**.
- Status: NOT merged (mainline msi-wmi-platform is still read-only fans only). In review, Armin engaged.
- Refs: https://yhbt.net/lore/all/20250511204427.327558-1-lkml@antheas.dev/ ;
  https://marc.info/?l=linux-kernel&m=177826569805402&w=2 ; https://www.phoronix.com/news/MSI-Linux-Driver-Windows-Parity

**Plan pivot (task #6):** rebase that series, add **MS-16V5 as a quirk-gated board**, contribute
back (it currently only covers the Claw). This is upstreamable and avoids duplicate work.

## Maintainer (Armin Wolf) acceptance criteria — design directives
1. Fan/PWM control **OFF by default, quirk-gated per verified model**.
2. Detect via **4-char board name / EC-ID (`16V5`)**, not full DMI string (covers SKU variants).
3. Keep sensors + debugfs available on **unrecognized** boards (so users can report EC-IDs).
4. **Restore OEM fan curves on disable/unload** (disabling manual curve leaves fan half-governed).
5. Handle **suspend/resume**.
Meet these → upstreamable via the existing thread.

## Correction: the WMI GUIDs
- `05901221-D566-11D1-B2F0-00A0C9062910` = generic **WMI Binary-MOF descriptor** (wmi-bmof.c),
  NOT the MSI method interface. (Decode the blob with `bmfdec`/`bmf2mof` to recover MSI's method
  signatures.)
- **Real MSI method interface = `ABBC0F6E-8EA1-11D1-00A0-C90629100000`** (class `MSI_ACPI`;
  package GUIDs `ABBC0F60/63`). This is what `msi-wmi-platform` binds. (Our `WMAM` under
  `EC.SCM0` is a separate ACPI-level EC mailbox method — also valid, but not the driver path.)
- Method/feature IDs on the ABBC0F6E interface (verify in source before coding):
  GET_PACKAGE 0x01, GET_EC 0x03, GET_BIOS 0x05, GET_SMBUS 0x07, GET_MASTER_BATTERY 0x09,
  GET_SLAVE_BATTERY 0x0b, GET_TEMPERATURE 0x0d, GET_THERMAL 0x0f, **GET_FAN 0x11**, GET_DEVICE
  0x13, GET_POWER 0x15, GET_DEBUG 0x17, GET_AP 0x19 (Advanced Performance), GET_DATA 0x1b,
  GET_WMI 0x1d; SET = GET+1. 32-byte in/out buffers (byte0 in=selector, out=status; 0=fail).

## Current mainline msi-wmi-platform (baseline)
- hwmon `msi_wmi_platform`, **4 read-only fan tach channels**, **RPM = 480000 / reading**
  (== our 60e6/(x*2*62.5), consistent). No pwm/temp/profile/battery yet. Has debugfs for the ~29
  methods. Merged v6.10; only refactors since. Maintainer Armin Wolf (Wer-Wolf).
- Doc: https://docs.kernel.org/wmi/devices/msi-wmi-platform.html

## msi-ec — already supports our exact EC (cross-ref, opposite architecture)
- Out-of-tree; **CONF_G2_2 lists `16V5EMS1.107` and `16V5EMS1.108`** → **our .108 unit is covered**
  (fan modes, cooler boost, shift mode, charge thresholds). Battery: **bit7=enable, start=end−10**.
- Mainline msi-ec: **only battery charge thresholds** upstream (6.4); fan/shift/etc. out-of-tree.
- Architecture: custom sysfs + DMI whitelist, power_supply + leds, **no hwmon/platform_profile**.
  → Use as **RE cross-reference & register validation**, not as the driver base.
- Refs: https://github.com/BeardOverflow/msi-ec (issue #385 = 16V5EMS1.108), Linux 6.4 news.

## Community references
- **MControlCenter**: lists GS66 12-UGS (MS-16V5): battery ✔, cooler-boost ✔, **fan CPU ✔ / GPU ✘**,
  kbd backlight ✘. Confirms **GPU-fan / full-profile is the hard part** on this board.
- alexzk1/MsiFanControl (active, adaptive curves — logic reference). ISW/YAMDCC = offset refs.
- **RGB unsolved**: GS66 SteelSeries per-key; msi-perkeyrgb stale, no OpenRGB GS66 profile → own RE (deferred).

## Immediate next actions
1. Fetch the Antheas/Derek-Clark series (mbox from lore) → rebase onto our kernel; build as .ko.
2. Add `16V5` quirk (fan default-off, EC-ID detect, OEM-curve restore, suspend/resume).
3. Cross-validate curve/battery semantics vs msi-ec CONF_G2_2 + our live register map.
4. `bmfdec` the BMOF blob to confirm this firmware's ABBC0F6E method set.
5. Sanity: does msi-ec load on our .108? (quick check — it should).

## Our contribution plan (to the Antheas/Derek-Clark series)
1. **MS-16V5 notebook quirk** — first non-Claw device; board `MS-16V5`, EC `16V5EMS1.108`,
   with tested values: shift 0xD2 (eco C2/comfort C1/turbo C4), fan curves 0x6A/0x72/0x82/0x8B,
   charge 0xD7 (bit7=enable, start=end-10), cooler-boost 0x98.7. Include EC dump.
2. **EC-ID / 4-char board detection** (Armin requested) — detect via EC ver string @0xA0 prefix
   ("16V5") instead of full DMI board name, to cover SKU variants (12UHS/UGS/UE...).
3. **Fix GPU-fan control** — MControlCenter shows CPU fan OK / GPU fan broken on MS-16V5; RE the
   dual fan tables/handlers to solve. Unsolved in community.
4. **Suspend/resume PM ops** — Armin flagged this as needing work. Evidence the EC resets state
   across S3/s2idle: MSI Center Shift class reapplies shift mode on resume (isS3/m_bIsResuming);
   EC has S0i3 sequencer (dbf5 / f331). Plan: dev_pm_ops .resume re-applies platform_profile +
   fan curves + charge threshold. Pin exact reset set via before/after suspend EC diff.
5. Tested-by on the series for a notebook; confirm RPM formula (480000/reading).

## Suspend/resume findings (rtcwake S3 experiment + DSDT corroboration)
DSDT `_PTS/_WAK` (RPTS/RWAK): only OSVR OS-handshake + RSUS/E706 flags; **no re-apply** of
shift/fan/charge/boost. MSI Center reapplies shift mode on resume (isS3/m_bIsResuming).
Empirical (set performance+boost, S3 40s, wake):
- **0xD2 shift 0xC4->0xC1 (RESET to default)** — platform_profile silently lost on resume.
- **0x98 cooler boost 0x82->0x02 (RESET off)**.
- Survived: 0xD4 fan mode (0x0D), **0xD7 charge threshold (0x80)**, 0xEB super-battery.
mem_sleep = "s2idle [deep]" (deep/S3 default).
=> Driver needs `.resume` to re-apply platform_profile (+ cooler boost, + manual fan curves).
   Charge threshold & fan mode need no re-apply. This fills the exact gap Armin flagged.

## Suspend fix — IMPLEMENTED & VALIDATED
Added dev_pm_ops .resume (pm_sleep_ptr) that re-applies the cached platform_profile
(msi_wmi_platform_profile_apply(data, data->cur_profile)); cur_profile cached in profile_set,
seeded from EC at probe. Built, DKMS 0.2 installed+signed.
Verified via rtcwake S3: set performance -> suspend 40s -> wake => profile stays performance
(0xD2=0xC4; previously reset to 0xC1). Fills Armin's flagged suspend/resume gap.
(Note: cooler boost 0x98 still resets on resume — not a driver-owned sysfs feature.)

## GPU-fan "not working" — ROOT CAUSE (decomp + live, task #11)
Driver GPU path is correct (pwm2 -> SET_FAN subfeature 0x2, same as CPU). The real cause:
the dGPU (01:00.0) sits in runtime **D3cold** (fine-grained RTD3). While the dGPU is powered off,
the GPU fan is off and its hwmon channel isn't even present (fan count 4->2, only CPU fan1/fan2).
=> GPU fan control only takes effect when the dGPU is active (under load / RTD3 disabled).
Not a driver bug. Also note: an "enable fan tables" bit exists (AP fan-mode BIT(7)); manual
curves apply when pwm_enable=manual. hwmon GPU-fan channels are dynamic with dGPU power state.
Physical validation under GPU load = task #14 (or `echo on > .../power/control` to hold dGPU up).

## EC-ID detection — IMPLEMENTED & VALIDATED (task #12)
Added msi_wmi_platform_match_ec_id(): reads EC 0xA0..0xA3 via GET_DATA, matches 4-char prefix
against msi_ec_quirks[] ({"16V5", &quirk_16v5}). Runs after WMI init, overrides DMI (fallback).
dmesg confirms: "quirks matched via EC firmware ID". Covers all 16V5EMS1.* SKUs (12UHS/UGS/UE)
without per-DMI-string entries — matches Armin's requested design. DKMS 0.3 installed+signed.

## GPU-fan — CORRECTED FINDING (physical test, task #14)
Earlier task#11 guess (D3cold gating) was WRONG. Facts:
- fan count 4->2 is patch 8 dual_fans (drop excess tach), NOT dGPU power state.
- GPU fan control WORKS: with pwm2_enable=1 (manual, ENABLE_FAN_TABLES set), forcing the GPU
  curve to max drove fan2 3116->3609 RPM; auto restored it. fan2 = GPU fan, pwm2 = GPU curve.
- The "GPU fan broken" (MControlCenter) = user curves are IGNORED in auto mode (pwm_enable=2,
  correct hwmon semantics). Engage manual (pwm_enable=1) and GPU fan responds.
- EC enforces a temp-based MIN RPM floor even in manual (curve=0 did not stop the fan) — safety.
pwm_enable convention here: 1=manual (tables on), 2=auto (EC control; restore_curves reloads factory).

## Lid-wake / suspend — VALIDATED (task #15)
Runtime-switched mem_sleep deep->s2idle. Set profile=performance (0xD2=0xC4), closed lid ->
s2idle suspend (dmesg: "suspend entry (s2idle)", EC interrupt block/unblock, "suspend exit"),
opened lid -> WOKE on lid-open (no power button). Confirms: under deep S3 the EC doesn't assert
lid-wake, under s2idle it does. Post-wake profile=performance preserved (s2idle keeps EC powered;
.resume handler covers deep S3 where 0xD2 resets). To keep lid-wake permanently: drop
mem_sleep_default=deep from GRUB (else reverts to deep on reboot). Tradeoff: s2idle = lid wake +
higher sleep drain; deep = lower drain + button-only wake.

## Generic / feature-based refactor — PLANNED (RFC candidate)
RE of MSI Center 2.0.71.0 confirms the register layout is a **line-wide convention with no
per-model/family branch** (only a WMI-version branch); MSI probes *presence* at runtime
(`Get_Device(0x01)` bitmap) and offers *control* features generically, letting the EC firmware
decide. See `msi-center-architecture.md` + `../msi-center-manifest/`.

Planned driver restructure (matches Armin's capability-based direction; possible upstream RFC):
- Replace quirk booleans with a **feature-descriptor table** (`detect()`/`setup()`/`suspend()`/
  `resume()` per feature) + a **capability cache** (WMI/EC version + `Get_Device(0x01)` bitmap).
- Per-family table (keyed by EC-ID) shrinks to a **control allow-list + register conventions**;
  default-off on unrecognized boards (control leaks nowhere), presence features auto-probed.
- Pre-req fix (upstreamable standalone): `msi_wmi_platform_profile_setup()` uses an
  **uninitialized `err`** — should `return PTR_ERR_OR_ZERO(data->ppdev)`.

# Review guide — recent work

Orientation for reviewing the recent `msi-wmi-platform` work in this repo. It summarises what
changed, why, how it was validated, and where to look.

## Scope
Two bodies of work, all on `master`:
1. **Driver: capability/feature-based refactor** of `msi-wmi-platform/msi-wmi-platform.c` (the main
   reviewable artifact; ~+465/−61 vs the upstream base+series).
2. **RE + tooling**: MSI Center manifest census tooling (`msi-center-manifest/`) and docs
   (`docs/`) that justify the driver design.

The driver sits on top of the in-review mainline series (Antheas Kapenekakis,
`[PATCH v1 00/10] … fan curves/platform profile/tdp/battery limiting`). **The DKMS driver and the
LKML series are now one source of truth** — `msi-wmi-platform/msi-wmi-platform.c` is *generated*
from `base.c` (that series applied on mainline v7.0) + `patches-upstream/00NN-*.patch` (our 8-patch
follow-up series) via `./regen.sh`; `make verify` fails on drift. So the code reviewed here, built
by DKMS, and proposed upstream are the same bytes ("tested" == "submitted"). Series breakdown and
submission/rebase plan: `msi-wmi-platform/patches-upstream/NOTES.md`.

## What changed in the driver (review these)
The 8-patch series (on `base.c`): `add MS-16V5 quirk` → `restore state on firmware resume` →
`fix uninitialized err` → `capability cache + Get_Device probe` → `feature-descriptor architecture
+ two-pass probe` → `rename quirk→model` → `heuristic control gate` → `fix issues found in review`
(the last folds into the two refactor patches before list submission). The heart is the
**heuristic control gate** (patch 0007).

| Area | Change | Why |
|---|---|---|
| Capability cache | `struct msi_wmi_platform_caps` (WMI/EC version, `is_tigerlake`, EC-ID, `Get_Device(0x01)` presence bitmap), filled in probe | Runtime capability layer, like MSI Center |
| **Control gate** | `msi_control_supported()` = MSI vendor + chassis `0x0A/0x1F` + WMI v2 + Tigerlake EC flag (+ `force_control`/`deny_control` model overrides) | Mirrors MSI Center's `Features.IsSupport()`; makes control generic across modern MSI notebooks instead of a per-model allow-list. See `docs/capability-map.md`. |
| Feature table | `msi_features[]` (HWMON/PROFILE/CHARGE/FAN_CURVES) with `detect/setup/remove/suspend/resume`; two-pass probe (detect-all → setup-all); generic `remove/suspend/resume` iterators | Replaces scattered `if (quirk->x)` gating |
| hwmon | `is_visible` gates `pwm_enable` on `MSI_FEAT_FAN_CURVES`; curve attr-groups passed only when enabled | Unknown boards get read-only sensors, no EC-driving control |
| model table | `struct msi_wmi_platform_model`: fan count + TDP limits + force/deny overrides (no control booleans) | The per-family table now holds only un-probeable hardware facts |

## Design decisions (rationale)
- **Control is gated by heuristic, not per-model, and not by EC probe.** RE proved MSI Center has
  no EC capability bit for control (the shift `0x80` "ability" bit is *written*, not read); it gates
  on `IsSupport` (vendor + chassis + CPU-gen window + model lists). We replicate the safe core. See
  `docs/msi-center-architecture.md`, `docs/capability-map.md`.
- **Presence** features come from the runtime `Get_Device(0x01)` bitmap (generic).
- Kept `struct msi_wmi_platform_model` (not a cosmetic-only churn beyond the rename) as the home for
  fan count / TDP, which genuinely can't be probed.

## Validation
- **Builds clean** (kernel 7.0.0-22); `checkpatch.pl --strict --file`: **0 errors, 0 warnings**
  (only soft CHECKs, mostly pre-existing base-driver style).
- **Hardware (MS-16V5 / GS66 12UHS)**: control lit up purely by the heuristic (fans=2, pwm + 12
  curve points, `platform_profile`, charge threshold); identical sysfs to the prior per-model build;
  clean probe, no feature-setup failures.
- **Deep-S3 `rtcwake`**: per-feature resume restored `0xD2` (profile) + `0xD4` (fan mode) that deep
  S3 wipes; s2idle unaffected.
- Persisted as **DKMS 0.6** (MOK-signed).

## Known limitations / open items
- `deny_control` list is **empty** — MSI excludes a few Modern/Creator thin-and-lights; without them
  the heuristic can false-*positive* (expose a `platform_profile` that no-ops — soft failure).
- `is_tigerlake` as a "modern EC (gen ≥ 10/11)" proxy is validated on one board; confirm on others.
- Fan count for unrecognized boards defaults to 4 (shows all channels; 2 may be bogus).
- EC firmware (Ghidra, 8051) accesses EC RAM via `MOVX @DPTR` (runtime addresses) → no clean
  per-offset map; the host EC-RAM map is sourced from DSDT + RE (`docs/ec-ram-map.md`).

## How to verify
```sh
# single source: the committed .c == base.c + the patch series
cd msi-wmi-platform && make verify && cd ..
# build + checkpatch
make -C /lib/modules/$(uname -r)/build M=$PWD/msi-wmi-platform modules
scripts/checkpatch.pl --strict --file msi-wmi-platform/msi-wmi-platform.c
# on an MSI notebook: load and confirm the heuristic + sysfs
sudo insmod msi-wmi-platform/…/msi-wmi-platform.ko
sensors | grep -A6 msi_wmi_platform; cat /sys/firmware/acpi/platform_profile_choices
ls /sys/class/power_supply/BAT*/charge_control_end_threshold
```

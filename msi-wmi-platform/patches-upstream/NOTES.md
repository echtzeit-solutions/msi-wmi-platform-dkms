# Upstream contribution plan

All of this sits on top of Antheas Kapenekakis's in-review series
`[PATCH v1 00/10] platform/x86: msi-wmi-platform: Add fan curves/platform
profile/tdp/battery limiting`. **Nothing here is a preemptive repost of that
series** — the plan is to *propose* the heuristic on that thread (with the
MSI Center `IsSupport` evidence) and to send our add-ons once it's welcome.

## The star patch
- **`0001-...gate-control-by-a-runtime-heuristic.patch`** — replaces the series'
  per-model control gating with `msi_control_supported()` (MSI vendor + chassis
  0x0A/0x1F; WMI v2 + Tigerlake EC already enforced at bind). Mirrors MSI
  Center's own `Features.IsSupport()`, so control works on modern MSI notebooks
  with **no per-model entry** (verified on MS-16V5, which has no quirk). The
  quirk table keeps only fan count + TDP limits + force/deny overrides.
  checkpatch `--strict` clean (bar the `Assisted-by:` trailer). Built + loaded
  on hardware.

## Independent fixes (`series-fixes/`)
- `0001` uninitialized-`err` bugfix (drop if the author folds their own),
  `0002` optional checkpatch formatting cleanup. Both valid regardless of the
  heuristic.

## To rework (`pre-heuristic/`)
The old MS-16V5 add-ons were written against the *per-model* control model and
are now superseded by the heuristic. They need reworking onto it before
sending:
- **MS-16V5 quirk** → shrinks to just the fan count (control is now generic);
  matched by EC-ID.
- **restore state on firmware resume** → gate on `msi_control_supported()`
  instead of `shift_mode`.
- **match quirks by EC firmware ID** → unchanged in spirit (selects the
  fan-count/TDP quirk for a board family).

## Intended series order (v-next, once welcome)
1. gate control by a runtime heuristic  *(the general win)*
2. restore state on firmware resume
3. match quirks by EC firmware ID
4. add MS-16V5 fan-count quirk
5. (optional) the two `series-fixes/`

Branch: `upstream` in `../../../msi-16v5/driver/ktree` = pristine series +
heuristic. Rebase onto `linux-next` when there's interest.

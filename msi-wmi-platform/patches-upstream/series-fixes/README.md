# Fixes for the in-review fan-curve/profile/charge series

Two follow-up patches against Antheas Kapenekakis's in-review series
`[PATCH v1 00/10] platform/x86: msi-wmi-platform: Add fan curves/platform
profile/tdp/battery limiting` (they apply on top of it, base `bb42ceb`):

- **0001 — fix uninitialized `err` in `profile_setup`.** Real bug in patch 05
  of the series. **Drop this if the series author folds their own fix** — Kurt
  Borja already flagged it in review (*"`err` is not initialized. Is it a
  leftover?"*), so v2 will likely fix it.
- **0002 — clean up wrapped-argument formatting.** Cosmetic only; takes
  `checkpatch.pl --strict --file` from 18 checks to 1 (the lone remaining
  `devm_hwmon_device_register_with_info()` call can't be reflowed under 100
  columns). Purely optional; send only if welcome.

Both carry a real-name DCO `Signed-off-by` + the `Assisted-by:` trailer
(`Documentation/process/coding-assistants.rst`; checkpatch doesn't recognise the
tag yet, hence its one "unrecognized email" error — expected).

Rebase onto the series' v2 before sending. Separate from `../` (our MS-16V5
feature patches).

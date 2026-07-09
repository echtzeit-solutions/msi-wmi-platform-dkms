# Upstream contribution — single source of truth

The driver shipped by this DKMS package and the patch series we take to LKML are
**the same code**. `msi-wmi-platform.c` is *generated*:

```
base.c  +  patches-upstream/00NN-*.patch   --( ./regen.sh )-->  msi-wmi-platform.c
```

so "tested on hardware" and "submitted upstream" are literally the same bytes.
`make verify` re-applies the series to `base.c` and diffs the result against the
committed `.c`, failing on any drift. **Edit the patches, never the generated
`.c`.**

## What `base.c` is
`base.c` is Antheas Kapenekakis's in-review series
`[PATCH v1 00/10] platform/x86: msi-wmi-platform: Add fan curves/platform
profile/tdp/battery limiting` applied on **mainline v7.0**. It is reproducible:

```
git clone --filter=blob:none https://github.com/torvalds/linux
git checkout 62b1dcf2e7af3dc2879d1a39bf6823c99486a8c2   # the series' base-commit
git am <the 10 v1 patches>                               # -> branch `antheas-v1`
# + the two already-merged mainline fixes v7.0 carries but 62b1dcf2 predates
#   (WMI GUID case, MSI-only autoload whitelist) == series patch 0000 below.
```

We keep this as a real git stack in the mainline clone at
`~/src-laptop/linux` (`antheas-v1` → `ours`), so tracking a new version is
just: re-`am` the new series, `git rebase --onto` ours on top, re-export
`base.c` + the patches, `./regen.sh`, **re-test**. See "Update workflow".

## The series (`patches-upstream/00NN-*.patch`)
`git format-patch` of our stack on top of `base.c`. checkpatch `--strict` clean
**when run from a current tree**: the `Assisted-by: AGENT:MODEL` trailer format
follows Documentation/process/coding-assistants.rst, which checkpatch knows —
but checkpatch from trees older than that doc flags it as an unrecognized email
address. Always run pre-submission checkpatch from the up-to-date rebase clone.

| # | patch | kind |
|---|---|---|
| 0001 | add MS-16V5 (Stealth GS66 12Ux) quirk | board |
| 0002 | restore state on firmware resume | board (deep-S3) |
| 0003 | fix uninitialized `err` in profile_setup | **bugfix in the series' patch 05** |
| 0004 | runtime capability cache + Get_Device probe | refactor |
| 0005 | feature-descriptor architecture + two-pass probe + `is_visible` gating | refactor |
| 0006 | rename quirk table to model | refactor (cosmetic) |
| 0007 | gate control by a runtime heuristic (`msi_control_supported`) | **the general win** |
| 0008 | fix issues found in review (2 passes) | fold into 0002/0004/0005/0007 before submission |
| 0009 | add a `disable_control` module parameter | user veto (the deny list ships empty) |
| 0010 | fix debugfs writes executing stale data | **bugfix in the base series** (offer on-thread like 0003) |
| 0011 | add status/USB/keyboard LEDs | feature (EC bit/byte via bounded Get_Data/Set_Data) |
| 0012 | add webcam power control (`camera_power`) | feature (gated by SupportedWebCam) |
| 0013 | lock down debugfs EC access | security (refuse WMI writes under kernel lockdown) |

The heart is **0007**: control (platform_profile, charge threshold, fan curves)
has no runtime capability bit, so instead of a per-model allow-list we enable it
the way MSI Center's own `Features.IsSupport()` does — MSI vendor + chassis
`0x0A/0x1F` + modern ABI (WMI v2, Tigerlake+ EC). This lights up control on
modern MSI notebooks with **no per-model entry** (verified on MS-16V5, which has
no control quirk). Evidence: `docs/msi-center-architecture.md`,
`docs/capability-map.md`.

## Before sending to the list
- **Fold 0008** into the commits it fixes (0002: suspend snapshot validity +
  hibernate restore; 0004: caps trim; 0005: profile_read, hwmon required;
  0007: vendor-case match). A reviewer never saw a v1 of ours; it's kept
  discrete here only to preserve the exact tested-on-hardware history.
- **Message polish during the fold**: capitalize the subject verbs (this
  driver's upstream history uses "Add …"/"Rename …"), make 0005's subject
  imperative (it's currently a noun phrase), replace the `+` in the 0004/0005
  subjects with "and", and re-wrap all bodies to ≤ 75 columns.
- The **0000 v7.0-sync** boundary (GUID case + autoload whitelist) is *not*
  submitted — both are already in mainline; they vanish once we rebase onto
  current mainline. It lives inside `base.c`, not as a patch.
- We *propose* 0007 on Antheas's thread first (with the `IsSupport` evidence)
  and offer the rest as rebasable follow-ups — this is not a preemptive repost
  of his series. Expect pushback on reversing Armin's earlier "control off by
  default, per verified model" directive; the fallback position is enabling
  profile+charge generically while keeping fan-curve control conservative,
  plus 0009's `disable_control` escape hatch.
- **Raise on the thread** (base-series code we did not patch): the
  `pr_format` typo (should be `pr_fmt`; inherited from mainline, trivial
  standalone cleanup), the `oxp_psy_ext_props` copy-paste name from the
  OneXPlayer driver, and the fw_attr `u32 min/max` fields whose `>= 0`
  checks and `-1` sentinels are incoherent for an unsigned type.

## Update workflow (tracking Antheas's next version)
```
# in the mainline clone that holds antheas-v1 + ours
b4 am -o /tmp <msgid-of-vN>            # or save the mbox by hand
git checkout <new base-commit>; git checkout -b antheas-vN && git am /tmp/*.mbx
git rebase --onto antheas-vN antheas-v1 ours   # replay our stack; resolve
# re-export the single source (13 = current patch count; adjust on fold):
git show ours~13:drivers/platform/x86/msi-wmi-platform.c > .../msi-wmi-platform/base.c
git format-patch --zero-commit -o .../patches-upstream 'ours~13'..ours
cd .../msi-wmi-platform && ./regen.sh && make verify
# REBUILD DKMS from the regenerated .c and RE-TEST on the MS-16V5, then
# re-assert Tested-by. (`pip install --user b4` for `b4 am`.)
```

## `series-fixes/`
Independent of our refactor: `0001` is the same uninitialized-`err` bug as our
0003 but expressed as a standalone fix against Antheas's v1 (offer it if he
hasn't folded his own — Kurt Borja already flagged it); `0002` is optional
formatting cleanup. Kept for the courtesy offer; our series already carries the
fix as 0003. Our 0010 (debugfs stale-buffer execute) is the same kind of
base-series bug — before offering it on-thread, diff the debugfs hunk against
the actual v1 mbox to confirm the bug is in the posted series and not an
artifact of the base.c reconstruction.

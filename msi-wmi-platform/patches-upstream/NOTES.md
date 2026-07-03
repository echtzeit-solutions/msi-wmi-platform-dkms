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

We keep this as a real git stack (a mainline clone with `antheas-v1` → `ours`),
so tracking a new version is just: re-`am` the new series, `git rebase --onto`
ours on top, re-export `base.c` + the patches, `./regen.sh`, **re-test**. See
"Update workflow".

## The series (`patches-upstream/00NN-*.patch`)
`git format-patch` of our stack on top of `base.c`. checkpatch `--strict` clean
(the `Assisted-by:` trailer carries no email, so it doesn't trip the tag check).

| # | patch | kind |
|---|---|---|
| 0001 | add MS-16V5 (Stealth GS66 12Ux) quirk | board |
| 0002 | restore state on firmware resume | board (deep-S3) |
| 0003 | fix uninitialized `err` in profile_setup | **bugfix in the series' patch 05** |
| 0004 | runtime capability cache + Get_Device probe | refactor |
| 0005 | feature-descriptor architecture + two-pass probe + `is_visible` gating | refactor |
| 0006 | rename quirk table to model | refactor (cosmetic) |
| 0007 | gate control by a runtime heuristic (`msi_control_supported`) | **the general win** |
| 0008 | fix issues found in review | fold into 0005/0007 before submission |

The heart is **0007**: control (platform_profile, charge threshold, fan curves)
has no runtime capability bit, so instead of a per-model allow-list we enable it
the way MSI Center's own `Features.IsSupport()` does — MSI vendor + chassis
`0x0A/0x1F` + modern ABI (WMI v2, Tigerlake+ EC). This lights up control on
modern MSI notebooks with **no per-model entry** (verified on MS-16V5, which has
no control quirk). Evidence: `docs/msi-center-architecture.md`,
`docs/capability-map.md`.

## Before sending to the list
- **Fold 0008** into 0005/0007 (a reviewer never saw a v1 of ours; it's kept
  discrete here only to preserve the exact tested-on-hardware history).
- The **0000 v7.0-sync** boundary (GUID case + autoload whitelist) is *not*
  submitted — both are already in mainline; they vanish once we rebase onto
  current mainline. It lives inside `base.c`, not as a patch.
- We *propose* 0007 on Antheas's thread first (with the `IsSupport` evidence)
  and offer the rest as rebasable follow-ups — this is not a preemptive repost
  of his series.

## Update workflow (tracking Antheas's next version)
```
# in the mainline clone that holds antheas-v1 + ours
b4 am -o /tmp <msgid-of-vN>            # or save the mbox by hand
git checkout <new base-commit>; git checkout -b antheas-vN && git am /tmp/*.mbx
git rebase --onto antheas-vN antheas-v1 ours   # replay our stack; resolve
# re-export the single source:
git show ours~8:drivers/platform/x86/msi-wmi-platform.c > .../msi-wmi-platform/base.c
git format-patch --zero-commit -o .../patches-upstream 'ours~8'..ours
cd .../msi-wmi-platform && ./regen.sh && make verify
# REBUILD DKMS from the regenerated .c and RE-TEST on the MS-16V5, then
# re-assert Tested-by. (`pip install --user b4` for `b4 am`.)
```

## `series-fixes/`
Independent of our refactor: `0001` is the same uninitialized-`err` bug as our
0003 but expressed as a standalone fix against Antheas's v1 (offer it if he
hasn't folded his own — Kurt Borja already flagged it); `0002` is optional
formatting cleanup. Kept for the courtesy offer; our series already carries the
fix as 0003.

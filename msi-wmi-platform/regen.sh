#!/bin/sh
# Regenerate msi-wmi-platform.c from base.c + the LKML patch series.
#
# Single source of truth: the DKMS build artifact (msi-wmi-platform.c) is
# GENERATED, never hand-edited. To change the driver, edit a patch in
# patches-upstream/ (or add a new one) and re-run this script.
#
#   base.c                      Antheas Kapenekakis's in-review v1 series applied
#                               on mainline v7.0 (== the `antheas-v1` mirror plus
#                               the v7.0 GUID/whitelist sync; see patches-upstream/NOTES.md).
#   patches-upstream/00NN-*.patch  our follow-up series -- the LKML deliverable
#                               AND the source for this file. Kernel-tree paths
#                               (drivers/platform/x86/...), hence `patch -p4`.
#
# `make verify` re-runs this into a scratch copy and diffs, failing on drift.
set -eu
cd "$(dirname "$0")"

out=msi-wmi-platform.c
tmp="$(mktemp)"
cp base.c "$tmp"

n=0
for p in patches-upstream/0[0-9][0-9][0-9]-*.patch; do
	[ -e "$p" ] || continue
	patch -s -p4 --no-backup-if-mismatch "$tmp" < "$p"
	n=$((n + 1))
done

mv "$tmp" "$out"
echo "regenerated $out from base.c + $n patch(es)"

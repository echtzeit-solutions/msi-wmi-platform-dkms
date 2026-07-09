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
# REGEN_OUT overrides the output path (used by `make verify` to regenerate into
# a scratch file and diff). `make verify` fails on any drift.
set -eu
cd "$(dirname "$0")"

out="${REGEN_OUT:-msi-wmi-platform.c}"
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT
cp base.c "$tmp"

n=0
for p in patches-upstream/0[0-9][0-9][0-9]-*.patch; do
	[ -e "$p" ] || continue
	# the cover letter is part of the LKML submission, not a code patch
	case "$p" in */0000-*) continue ;; esac
	# patch(1) is pointed at the single target file, ignoring per-file paths:
	# refuse multi-file patches, which it would silently misapply
	if [ "$(grep -c '^diff --git ' "$p")" -ne 1 ]; then
		echo "regen: $p touches more than one file; cannot apply to $out" >&2
		exit 1
	fi
	patch -s -p4 --no-backup-if-mismatch "$tmp" < "$p" || {
		echo "regen: $p failed to apply" >&2
		exit 1
	}
	n=$((n + 1))
done

cp "$tmp" "$out"
chmod 644 "$out"
echo "regenerated $out from base.c + $n patch(es)"

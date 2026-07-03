obj-m += msi-wmi-platform.o

# msi-wmi-platform.c is GENERATED from base.c + patches-upstream/*.patch.
# Edit the patches, not the .c. See regen.sh / patches-upstream/NOTES.md.
#
# DKMS builds the module via `make -C <kdir> M=<dir> modules`, which consumes
# only the obj-m line above; the dev-helper targets below are never on the
# module build path. A bare `make` here prints help (never regenerates).

.PHONY: all regen verify
all:
	@echo "msi-wmi-platform (single source: base.c + patches-upstream/*.patch)"
	@echo "  make regen   - regenerate msi-wmi-platform.c from base.c + patches"
	@echo "  make verify  - fail if msi-wmi-platform.c has drifted from them"
	@echo "  module build - make -C /lib/modules/\$$(uname -r)/build M=\$$(PWD) modules"

regen:
	./regen.sh

# Fail if the committed .c has drifted from base.c + the patch series.
verify:
	@tmp=$$(mktemp); cp base.c $$tmp; \
	for p in patches-upstream/0[0-9][0-9][0-9]-*.patch; do \
		[ -e "$$p" ] || continue; \
		patch -s -p4 --no-backup-if-mismatch $$tmp < "$$p"; \
	done; \
	if diff -q $$tmp msi-wmi-platform.c >/dev/null; then \
		echo "verify: msi-wmi-platform.c matches base.c + patches-upstream/ OK"; \
		rm -f $$tmp; \
	else \
		echo "verify: DRIFT -- msi-wmi-platform.c != base.c + patches-upstream/"; \
		echo "        run ./regen.sh (edit the patches, not the generated .c)"; \
		diff -u msi-wmi-platform.c $$tmp | head -40; \
		rm -f $$tmp; exit 1; \
	fi

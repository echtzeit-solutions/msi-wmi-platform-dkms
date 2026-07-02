# Exhaustive host-visible EC RAM map (MS-16V5)

The 256-byte host EC window (ACPI `EmbeddedControl`, offsets 0x00-0xFF). Two provenances:
- **DSDT** — named ACPI fields from `re/DSDT.dsl` (`EmbeddedControl` region): authoritative, 106 fields.
- **RE** — undocumented *control* registers (fan curves, charge, boost, LEDs) reached via WMI
  `Set_Data(idx)` / msi-ec / live diff; NOT exposed as ACPI fields. See `ec-register-map.md`,
  `register-consistency.md`.

> Note: the EC firmware (Ghidra, 8051) accesses this window via `MOVX @DPTR` with runtime-computed
> addresses, so Ghidra yields no clean per-offset xref map; this table is sourced from the DSDT + RE,
> which is authoritative for the host-visible window.

| Offset | Src | Field(s) / meaning |
|---|---|---|
| 0x00 | DSDT | SMPR(8b) |
| 0x01 | DSDT | SMST(8b) |
| 0x02 | DSDT | SMAD(8b) |
| 0x03 | DSDT | SMCM(8b) |
| 0x04 | DSDT | SMD0(264b) |
| 0x25 | DSDT | SMAA(8b) |
| 0x2C | DSDT | MICL.1(1b) |
| 0x2D | DSDT | MUTL.1(1b) |
| 0x2E | DSDT | CAML.1(1b); PXCT.6(1b) |
| 0x30 | DSDT | POWS.0(1b); LIDS.1(1b) |
| 0x31 | DSDT | MBTS.0(1b); MBCS.1(1b); MBDS.2(1b); MBFS.3(1b); MBWS.4(1b); MBLS.5(1b); MBCL.6(1b); MBFL.7(1b) |
| 0x32 | DSDT | SBTS.0(1b); SBCS.1(1b); SBDS.2(1b); SBFS.3(1b); SBWS.4(1b); SBLS.5(1b); SBCL.6(1b); SBFL.7(1b) |
| 0x36 | DSDT | OSVR.4(4b) |
| 0x38 | DSDT | MDCL(8b) |
| 0x39 | DSDT | MDCH(8b) |
| 0x3A | DSDT | MDVL(8b) |
| 0x3B | DSDT | MDVH(8b) |
| 0x3C | DSDT | MCAL(8b) |
| 0x3D | DSDT | MCAH(8b) |
| 0x3E | DSDT | MSTL(8b) |
| 0x3F | DSDT | MSTH(8b) |
| 0x40 | DSDT | MCCL(8b) |
| 0x41 | DSDT | MCCH(8b) |
| 0x42 | DSDT | MPOL(8b) |
| 0x43 | DSDT | MPOH(8b) |
| 0x44 | DSDT | MFCL(8b) |
| 0x45 | DSDT | MFCH(8b) |
| 0x46 | DSDT | MCUL(8b) |
| 0x47 | DSDT | MCUH(8b) |
| 0x48 | DSDT | MRCL(8b) |
| 0x49 | DSDT | MRCH(8b) |
| 0x4A | DSDT | MVOL(8b) |
| 0x4B | DSDT | MVOH(8b) |
| 0x4C | DSDT | MTEL(8b) |
| 0x4D | DSDT | MTEH(8b) |
| 0x4E | DSDT | MCVL(8b) |
| 0x4F | DSDT | MCVH(8b) |
| 0x50 | DSDT+RE | SDCL(8b)  ‖ PL1 TDP (LE32 @0x50) |
| 0x51 | DSDT+RE | SDCH(8b)  ‖ PL2 TDP |
| 0x52 | DSDT | SDVL(8b) |
| 0x53 | DSDT | SDVH(8b) |
| 0x54 | DSDT | SCAL(8b) |
| 0x55 | DSDT | SCAH(8b) |
| 0x56 | DSDT | SSTL(8b) |
| 0x57 | DSDT | SSTH(8b) |
| 0x58 | DSDT | SCCL(8b) |
| 0x59 | DSDT | SCCH(8b) |
| 0x5A | DSDT | SPOL(8b) |
| 0x5B | DSDT | SPOH(8b) |
| 0x5C | DSDT | SFCL(8b) |
| 0x5D | DSDT | SFCH(8b) |
| 0x5E | DSDT | SCUL(8b) |
| 0x5F | DSDT | SCUH(8b) |
| 0x60 | DSDT | SRCL(8b) |
| 0x61 | DSDT | SRCH(8b) |
| 0x62 | DSDT | SVOL(8b) |
| 0x63 | DSDT | SVOH(8b) |
| 0x64 | DSDT | STEL(8b) |
| 0x65 | DSDT | STEH(8b) |
| 0x66 | DSDT | SCVL(8b) |
| 0x67 | DSDT | SCVH(8b) |
| 0x68 | DSDT | CPUT(8b) |
| 0x6A | RE | CPU fan-curve TEMP table x7 (0x6A..0x70) |
| 0x72 | RE | CPU fan-curve SPEED table x7 (0x72..0x78) |
| 0x7E | DSDT | RES1.0(3b); CHET.3(1b) |
| 0x80 | DSDT | SYST(8b) |
| 0x82 | RE | GPU fan-curve TEMP table x7 (0x82..0x88) |
| 0x8A | RE | GPU fan-curve SPEED table x7 (0x8A..0x90; [0]=idle base %) |
| 0x98 | RE | Cooler Boost (bit7) |
| 0xA0 | RE | EC firmware ID string (0xA0..0xBB, e.g. 16V5EMS1.10F) |
| 0xC9 | RE | fan tach/duty |
| 0xCB | RE | fan tach/duty |
| 0xCD | RE | fan tach/duty |
| 0xD2 | DSDT+RE | SYSM.0(2b)  ‖ Shift/perf mode (SYSM; C1/C2/C4) |
| 0xD4 | RE | Fan mode (auto 0x0D / silent 0x1D / advanced 0x8D) |
| 0xD7 | RE | Charge threshold (percent|0x80) |
| 0xDB | RE | USB LED / keyboard USB backlight |
| 0xE3 | DSDT | OSC1(8b) |
| 0xE4 | DSDT | OSC2(8b) |
| 0xE6 | DSDT | DBG(8b) |
| 0xE7 | DSDT | DTOK.0(1b); DTNG.1(1b); FBST.2(1b); ESGI.3(1b); ESGO.4(1b); ESGN.5(1b); E706.6(1b); DTDR.7(1b) |
| 0xE8 | DSDT | RSUS.0(1b) |
| 0xEB | DSDT | PSNM.0(7b) |
| 0xEC | DSDT | MODS.0(1b); KBBL.1(1b) |
| 0xED | DSDT | SCIC(8b) |
| 0xEE | DSDT | ISHS.0(2b) |
| 0xF4 | DSDT | TSIT(8b) |
| 0xF5 | DSDT | TSTU(8b) |
| 0xF6 | DSDT | TSTL(8b) |
| 0xF7 | DSDT | TST2(8b) |
| 0xF8 | DSDT | TSU2(8b) |
| 0xF9 | DSDT | TSL2(8b) |
| 0xFD | DSDT | CFID(8b) |

**93 occupied offsets** (81 DSDT-named, 15 with RE'd control regs).

# Cross-project EC register consistency (MS-16V5)

**Board:** MSI Stealth GS66 12UHS · MS-16V5 · EC `16V5EMS1.107/.108/.10F`

A sanity check: extract each project's EC register/encoding map and cross-compare, to surface
either **our gaps** or **bugs in their mappings**. Authoritative column = **Ours**, which is
live write→diff→restore validated (`ec-register-map.md`).

Sources: **Ours** `ec-register-map.md` · **msi-ec** `../../msi-ec/msi-ec.c` CONF29 (`:2404`–`:2493`,
FW list `:2397`) · **MCC** MControlCenter raw map `../../MControlCenter/src/operate.cpp:27`–`:90`
(its `helper/msi-ec.cpp` just delegates to the msi-ec driver; the raw map is its own fallback) ·
**isw** `../../isw/etc/isw.conf` `[MSI_ADDRESS_DEFAULT]` · **MSI-Center** RE'd `Set_Data(idx)` selectors.

## Per-register matrix
| Reg | Feature | Ours | msi-ec | MCC | isw | MSI-Center | Agree |
|---|---|---|---|---|---|---|---|
| 0x98 | Cooler Boost | bit7 | 0x98 bit7 | 0x98 bit7 | 0x98 | 0x98 (152) | ✓ |
| 0xD2 | Shift/perf | C1/C2/**C4** | c1/c2/**c4** | C1/C2/**C0** | — | 0xD2 (210) | ⚠ |
| 0xD4 | Fan mode | 0D/1D/8D | 0d/1d/8d | probes 0xD4 else 0xF4 | **0xF4** 0C/4C/8C | 0xD4 (212) | ⚠ |
| 0x6A | CPU temp table | ×7 | ×7 | ×6 | 0x6A–6F ×6 | native | ✓addr |
| 0x72 | CPU speed table | ×7 | ×7 | ×7 | ×7 | native | ✓ |
| 0x82 | GPU temp table | ×7 | ×7 | ×6 | ×6 | native | ✓addr |
| 0x8A | GPU speed table | **0x8A ×7** | 0x8a ×7 | 0x8A ×7 | 0x8A ×7 | native | ✓ (doc fixed) |
| 0xD7 | Charge threshold | **0xD7** \|0x80 | 0xd7 | probes **0xEF** then 0xD7 | **0xEF** | native | ⚠ |
| 0xEB | Super-battery | nonzero | 0x0f mask | ±15 | — | 0xEB | ✓ |
| 0x2E/0x2F | Webcam(+block) | bit1/blk | 0x2e/0x2f | 0x2E bit1 | — | 46/47 | ✓ |
| 0xE8 | Fn/Win | bit4 | bit4 | bit4 | — | 232 | ✓ |
| 0x68 / 0x80 | CPU / GPU temp | 0x68/0x80 | 0x68/0x80 | 0x68/0x80 | 0x68/0x80 | native | ✓ |
| 0xDB | USB LED | **0xDB** | — | — | **0xF7** | 0xDB (219) | ⚠ |
| 0xBF | USB power-share | — | `//`0xbf.5 | 0xBF (0x08/0x28) | — | — | ? our gap |
| 0xEC/0xD3 | Kbd backlight | **0xEC**.1 enable | UNSUPP | 0xD3/0xF3 levels | — | 0xEC/0xD3 | ⚠ |
| 0x2C | mic-mute LED / kbd-BL timeout | bit2 mic | UNSUPP | bit3 timeout | — | 44 | ? bitshare |
| 0x42 / 0x31 | Batt capacity / status | — | (ACPI) | 0x42 / 0x31 | — | — | ? our gap |
| 0xC9–0xCD | Fan tach/duty | 0xC9/CB/CD | gpu 0xcb | fan1 C9\|CD, fan2 CB | cpu CC/gpu CA | native | ⚠conv |

## Verdicts on mismatches
- **0x8A GPU speed (was ⚠, now RESOLVED — was OUR doc bug):** msi-ec/MCC/isw all use `0x8A` ×7
  starting `0x00`. Our doc said `0x8B` ×6. **Live-confirmed** `0x8A: 00 2d 3c 46 50 55 64`
  (`0,45,60,70,80,85,100`) → we'd read one byte late and dropped the leading silent-idle `0`.
  Fixed in `ec-register-map.md`. Does **not** affect the driver (WMI fan-table subfeature, not raw addr).
- **0xD2 turbo = 0xC0 (MCC):** Ours(live)+msi-ec = **0xC4**. **MCC raw-map bug/staleness** for 12th-gen
  (only correct when the msi-ec driver is loaded, which it delegates to).
- **0xD4 vs 0xF4 fan mode (isw):** Ours+msi-ec+MCC-probe+MSI-Center = **0xD4**. **isw hardcodes 0xF4**
  (legacy default) + low-nibble `C` → almost certainly a no-op on MS-16V5. **isw bug.**
- **0xD7 vs 0xEF charge (isw/MCC):** encoding unanimous (`percent|0x80`, start=end−10). Address:
  Ours(physically validated)+msi-ec = **0xD7**; **isw hardcodes 0xEF** (no-op here); **MCC probes 0xEF
  *before* 0xD7** → latent bug if 0xEF reads 128–228 on a 16V5. Report to both.
- **0xDB vs 0xF7 USB LED (isw):** Ours(live)+MSI-Center = **0xDB**; isw = 0xF7 tri-state → wrong/inert here.
- **temp table 7 vs 6 points:** Ours+msi-ec = 7 (top = 0x70/0x88 = 100°C); MCC+isw = 6 (top as ceiling).
  Benign convention diff.

## Action items
**Fix/verify ours:** (1) ✅ GPU speed table 0x8A ×7 (done, live-confirmed). (2) document 0xBF USB
power-share (0x08/0x28) — currently undocumented, MCC+msi-ec know it. (3) document full 0x2C bit
layout (bit2 mic-mute [ours] + bit3 kbd-BL auto-off [MCC]). (4) optional pure-EC telemetry: 0x42
capacity, 0x31 charge status, realtime fan-% 0x71/0x89.
**Report upstream:** (5) **isw** MS-16V5 uses legacy control addrs — fan mode 0xF4→0xD4, charge
0xEF→0xD7, USB LED 0xF7→0xDB (curve table addrs are correct). (6) **MControlCenter** raw turbo
0xC0→0xC4, and `detectBatteryThresholdAddress()` checks 0xEF before 0xD7.
**Re-validate live:** ✅ 0x8A confirmed. Still: probe 0xBF + 0xD3/0xF3 kbd-levels on the running
board; read 0xEF to see if MCC's 0xEF-first probe would mis-fire; pin exact CPU/GPU tach bytes in
0xC9–0xCD via a fan-spin diff.

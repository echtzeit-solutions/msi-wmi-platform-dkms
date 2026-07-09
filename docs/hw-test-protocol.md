# msi-wmi-platform — hardware test protocol

Physical + register-level verification of every driver feature on a real machine
(reference: MS-16V5 / Stealth GS66 12UHS). Each test pairs an **objective** check
(the EC register actually changed — rules out a driver bug) with a **physical**
observation (the LED lights / camera appears — rules out "writes a bit the board
ignores"). A feature that changes the EC byte but shows no physical effect means
the indicator is **not populated on this SKU** — record that; it feeds the
per-model presence list.

Fill in the **Results** table at the bottom as you go.

## 0. Setup
```sh
# Load the driver under test (dev build) and the EC-inspection helper.
cd linux-msi-ms16v5/msi-wmi-platform
sudo rmmod msi_wmi_platform 2>/dev/null; sudo insmod ./msi-wmi-platform.ko
dmesg | tail -5                       # probe OK? note the Get_Device presence bitmap line

# EC read helper (objective check). Root; ec_sys must be loaded (modprobe ec_sys).
ec() { sudo python3 -c "print('0x%02X'%open('/sys/kernel/debug/ec/ec0/io','rb').read()[$1])"; }
# Full annotated view any time:  sudo python3 msi-16v5/re/ecmon.py

L=/sys/class/leds
```
Precondition checks:
- [ ] `dmesg` shows `model matched` + `Get_Device(0x01) presence bitmap: …` and **no probe error**.
- [ ] `ls $L | grep -E 'micmute|mute|kbd_backlight|usb_backlight'` lists all four LEDs.
- [ ] `find /sys -name camera_power` returns a path (**only if** `SupportedWebCam` bit set).

---

## 1. LED tests
For each LED: silence any auto-trigger first so a manual write isn't immediately
overwritten, then toggle and watch both the EC byte and the physical indicator.

### 1a. Mic-mute LED — `platform::micmute` (EC 0x2C.0)
```sh
echo none > $L/platform::micmute/trigger      # detach audio-micmute trigger
ec 0x2C                                        # baseline (expect 0x00)
echo 1 > $L/platform::micmute/brightness ; ec 0x2C   # expect 0x01
echo 0 > $L/platform::micmute/brightness ; ec 0x2C   # expect 0x00
```
- [ ] EC 0x2C.0 follows 1/0 (objective).
- [ ] **Physical:** the mic-mute key LED lights on `1`, off on `0`.  ▸ if EC toggles but no LED → **not present on SKU**.
- [ ] Trigger behavior: `echo audio-micmute > trigger`; mute the mic (PulseAudio) → LED tracks it.

### 1b. Mute LED — `platform::mute` (EC 0x2D.0)
```sh
echo none > $L/platform::mute/trigger
ec 0x2D ; echo 1 > $L/platform::mute/brightness ; ec 0x2D ; echo 0 > $L/platform::mute/brightness ; ec 0x2D
```
- [ ] EC 0x2D.0 follows 1/0.
- [ ] **Physical:** speaker/mute LED lights.  ▸ GS66 presence unconfirmed — record result.

### 1c. USB backlight — `msi::usb_backlight` (EC 0xDB, 0–255)
```sh
for b in 0 64 128 255; do echo $b > $L/msi::usb_backlight/brightness; echo "b=$b -> $(ec 0xDB)"; sleep 1; done
echo 0 > $L/msi::usb_backlight/brightness
```
- [ ] EC 0xDB equals the written value (0x00/0x40/0x80/0xFF).
- [ ] **Physical:** a USB-port backlight glows / varies with brightness.  ▸ presence unconfirmed on GS66.

### 1d. Keyboard-backlight enable — `msi::kbd_backlight` (EC 0xEC.1)
```sh
ec 0xEC ; echo 1 > $L/msi::kbd_backlight/brightness ; ec 0xEC ; echo 0 > $L/msi::kbd_backlight/brightness ; ec 0xEC
```
- [ ] EC 0xEC.1 follows 1/0.
- [ ] **Physical:** keyboard backlight turns on/off. Caveat: per-key RGB *color/brightness* is owned
      by the SteelSeries KLC MCU — this bit is only the EC master enable. Test with RGB software idle.

---

## 2. Camera power — `camera_power` (EC 0x2E.1)  [gated on SupportedWebCam]
```sh
CP=$(find /sys -name camera_power); echo "path: $CP"
cat $CP                                  # current state (0 or 1) — matches EC 0x2E.1
ec 0x2E
# enable:
echo 1 > $CP; sleep 3
cat $CP; ls /dev/video* 2>/dev/null; lsusb | grep -i 5986:2127
# disable:
echo 0 > $CP; sleep 2
cat $CP; ls /dev/video* 2>/dev/null || echo "no /dev/video (expected)"
```
- [ ] `cat camera_power` reflects the live EC bit (read-back correct).
- [ ] **Enable (1):** `/dev/video*` appears, `5986:2127 Bison HD Camera` on USB, and a viewer
      (`cheese` / `ffmpeg -f v4l2 -i /dev/video0 -frames 1 /tmp/cam.jpg`) shows a **live image**.
- [ ] **Disable (0):** `/dev/video*` gone, camera absent from `lsusb` — a hardware kill, not a black
      frame. Confirm a running viewer loses the device.
- [ ] Restore to the state you found it in.

---

## 3. debugfs lockdown guard (patch 0013)
```sh
cat /sys/kernel/security/lockdown          # [none] integrity confidentiality
SE=$(find /sys/kernel/debug -name set_ec 2>/dev/null); echo "set_ec: $SE"
```
- [ ] **No lockdown** (`[none]`): a root write to `set_ec` still works (guard returns 0). Baseline.
- [ ] **Under lockdown:** boot with `lockdown=integrity` (or `echo integrity | sudo tee
      /sys/kernel/security/lockdown` — **irreversible until reboot**), then
      `printf '\xNN...' | sudo tee $SE` must fail with **`-EPERM` (Operation not permitted)**.
      Also confirm the *feature* attrs (LEDs / camera_power) still work under lockdown (bounded ops
      are allowed — that's the whole point).

---

## 4. Regression — pre-existing features still work
- [ ] hwmon: `sensors` shows fan RPM + temps; values sane.
- [ ] Fan curves: write `pwm1_auto_pointN_pwm/temp` under `/sys/class/hwmon/hwmonX/`; fan responds.
- [ ] platform_profile: `cat /sys/firmware/acpi/platform_profile_choices`; set each; EC 0xD2 changes,
      behavior/thermals shift.
- [ ] Charge threshold: set `charge_control_end_threshold` (power_supply); EC 0xD7 = `pct|0x80`;
      on AC, charging stops at the limit.

## 5. Suspend / resume + unload
- [ ] `systemctl suspend`, resume → LEDs restore last brightness (LED_CORE_SUSPENDRESUME); `camera_power`
      reads correctly; fan curves reapplied (per the resume hook).
- [ ] `sudo rmmod msi_wmi_platform` → all `/sys/class/leds/*` + `camera_power` disappear cleanly; `dmesg`
      no warnings; LEDs left in a sane state.

---

## Results
Objective (EC-register) column filled by `sweep.sh` on MS-16V5, 2026-07-08, dev build
srcversion `B2440AE31F2C553499301CB` (all four LEDs + `camera_power` present at probe).
The **Physical effect?** column is left for the operator's eyes.

| # | Feature | EC reg | EC toggles? | Physical effect? | Present on SKU? | Notes |
|---|---------|--------|-------------|------------------|-----------------|-------|
| 1a | mic-mute LED | 0x2C.0 | ✅ 0x00↔0x01 | ⏳ pending | ⏳ pending | clean full-byte toggle; operator saw nothing on first blink — likely absent on this SKU, confirm |
| 1b | mute LED | 0x2D.0 | ✅ bit0 0↔1 | ⏳ pending | ⏳ pending | reg 0x2D bit2 is firmware-owned (stays set); driver touches only bit0 (0x04↔0x05); operator saw nothing on first blink — likely absent, confirm |
| 1c | USB backlight | 0xDB | ✅ exact byte | ⏳ pending | ⏳ pending | 0x00/0x40/0x80/0xFF written & read back verbatim; no known USB-port illumination on GS66 — likely absent, confirm |
| 1d | kbd-bl enable | 0xEC.1 | ✅ bit1 0↔1 | ✅ **RGB keys on/off** | ✅ **yes** | eyes-on confirmed 2026-07-08: toggling KBBL switches all keyboard RGB LEDs on/off. Binary enable only — no EC brightness levels (KBBL declared 1-bit; dimming owned by SteelSeries per-key MCU over HID) |
| 2 | camera_power | 0x2E.1 | ✅ 0x0B↔0x09 | ✅ USB de/re-enumerates | yes (gated bit set) | OFF → /dev/video* + `5986:2127` gone; ON → re-enumerates (dev# 009→010 = real power cycle) |
| 3 | lockdown guard | debugfs | — | — | — | baseline `[none]`: `set_ec` node present. `-EPERM`-under-lockdown path still needs a `lockdown=integrity` boot |
| 4 | fan/profile/charge | — | | | | regression — not run this sweep |
| 5 | suspend/resume | — | | | | not run this sweep |

**Objective sweep verdict:** 5/5 feature interfaces drive their EC register correctly
(mic-mute, mute-bit0, USB-backlight byte, kbd-enable bit1, camera power).

**Physical (eyes-on) status, 2026-07-08:**
- **kbd-bl enable (0xEC.1) — CONFIRMED present**: toggles all keyboard RGB LEDs on/off.
- **camera_power (0x2E.1) — CONFIRMED present**: USB hardware kill (de/re-enumerates).
- **mic-mute (0x2C.0), mute (0x2D.0), USB-backlight (0xDB) — PENDING**: operator saw no
  physical effect on the first blink pass; strong candidates for the per-model absent list,
  but not yet a deliberate confirming look. Re-blink and confirm before adding to `deny`.

§3 lockdown-deny and §4–5 regression/suspend remain to run.

**Presence outcome:** any LED with "EC toggles = yes, Physical = no" → add to the per-model
`deny`/absent list for this EC family so the driver doesn't expose a dead indicator.

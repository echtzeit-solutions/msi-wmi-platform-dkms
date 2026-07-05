# Fan curve (ported from isw to msi-wmi-platform)

`isw` wrote fan curves via raw `ec_sys`. The `msi-wmi-platform` driver exposes the **same
hardware curve** through hwmon (`pwm1`=CPU, `pwm2`=GPU, six `pwmN_auto_point*` points), so we
just write the curve once and the EC runs it — no software polling daemon needed (unlike
lm-sensors `fancontrol`, which continuously rewrites `pwm` and would fight this model).

## Files
- `msi-fan-curve`      — apply script (reads the conf, writes the hwmon curve, sets manual mode).
- `msi-fan-curve.conf` — the curve (6 temp/speed points per fan). Ported from msi-ec `curve_apply`.
- `msi-fan-curve.service` — applies on boot.

## Resume: handled by the driver
No sleep hook is needed. Verified by full-EC diff: **s2idle keeps** the manual fan mode and the
curve, and **deep S3 / hibernate reset** them — but the `msi-wmi-platform` driver (this repo,
0.4+) snapshots the fan state on suspend and re-applies it on firmware resume
(`pm_resume_via_firmware()`), so the curve and manual mode survive every sleep mode
automatically. (A brief fan spin-up on wake settles back to the curve on its own.)
On the stock in-tree driver (no resume handler) you'd need a system-sleep hook for deep S3;
with this DKMS driver you don't.

## old (msi-ec / isw) -> new mapping
Both `msi-ec` and `isw` describe the same EC fan table as 7 speeds + 6 temperatures. The driver
exposes 6 `(temp, speed)` points that map to the EC's **speed0..speed5** — i.e. `point_i` holds
`(temp_{i-1}, speed_{i-1})`. So **point1 = (temp0, speed0)**, and `speed0` is the speed *below
the first temperature*: set it to `0` for **fans fully off at idle**. The 7th EC speed
(`speed6`, above the last temperature) has no point and is left to the EC. Speeds are percent
(the driver stores them as pwm 0-255). Do NOT drop speed0 or you lose the silent idle.

- **msi-ec** `curve` string is interleaved `speed temp speed temp ... speed` (13 values), same
  string written to `cpu/curve` and `gpu/curve`, with `fan_mode=advanced`.
- **isw** uses `*_temp_0..5` / `*_fan_speed_0..6` keys in `/etc/isw.conf`.

Shipped example values (ported from msi-ec `curve_apply`: `0 50 41 55 55 60 65 65 85 68 100 80 120`):
- CPU & GPU: temps `50 55 60 65 68 80`, speeds `0 41 55 65 85 100` (speed0..5; `0` = silent
  below the first temp point, 50 C — verified 0 RPM at idle). speed6 (120) is above the last
  point and left to the EC.

Note on units: the driver takes the temp points as **plain degrees C** (not the usual hwmon
millidegrees) and speeds as pwm 0-255; the script converts percent -> pwm for you.

## Install
```bash
sudo install -m755 msi-fan-curve            /usr/local/bin/msi-fan-curve
sudo install -m644 msi-fan-curve.conf       /etc/msi-fan-curve.conf
sudo install -m644 msi-fan-curve.service    /etc/systemd/system/msi-fan-curve.service
sudo systemctl daemon-reload
sudo systemctl enable --now msi-fan-curve.service
# (no resume hook needed — the driver re-applies the curve after deep S3/hibernate)

# and retire the old isw units:
sudo systemctl disable isw@16V5EMS1.service 2>/dev/null || true
```
Apply immediately / re-tune anytime: edit `/etc/msi-fan-curve.conf` then
`sudo systemctl restart msi-fan-curve`.

## Notes
- Manual mode (`pwm1_enable=1`) engages the tables globally (both fans). Set it back to `2` for
  EC auto control (which also restores the factory curve).
- The EC enforces a ~min RPM floor; below ~55% the fans sit at the floor, and only `0%` fully
  stops a fan. See `../docs/fan-curve-rpm.md` for measured setpoint -> RPM.

# Fan curve (ported from isw to msi-wmi-platform)

`isw` wrote fan curves via raw `ec_sys`. The `msi-wmi-platform` driver exposes the **same
hardware curve** through hwmon (`pwm1`=CPU, `pwm2`=GPU, six `pwmN_auto_point*` points), so we
just write the curve once and the EC runs it — no software polling daemon needed (unlike
lm-sensors `fancontrol`, which continuously rewrites `pwm` and would fight this model).

## Files
- `msi-fan-curve`      — apply script (reads the conf, writes the hwmon curve, sets manual mode).
- `msi-fan-curve.conf` — the curve (6 temp/speed points per fan). Ported from isw `[16V5EMS1]`.
- `msi-fan-curve.service` — applies on boot.
- `50-msi-fan-curve`   — systemd-sleep hook: re-applies on **resume** (the EC drops the manual
  fan tables across suspend, like it does shift-mode).

## old (msi-ec / isw) -> new mapping
Both `msi-ec` and `isw` describe the same EC fan table as 7 speeds + 6 temperatures. The driver
exposes 6 `(temp, speed)` points, corresponding to `(temp_i, speed_{i+1})`; the extra `speed_0`
(idle value below the first temperature) has no dedicated point — the EC's minimum-RPM floor
applies below the first point. Speeds are percent; the driver stores them as pwm 0-255.

- **msi-ec** `curve` string is interleaved `speed temp speed temp ... speed` (13 values), same
  string written to `cpu/curve` and `gpu/curve`, with `fan_mode=advanced`.
- **isw** uses `*_temp_0..5` / `*_fan_speed_0..6` keys in `/etc/isw.conf`.

Your current values (ported from msi-ec `curve_apply`: `0 50 41 55 55 60 65 65 85 68 100 80 120`):
- CPU & GPU: temps `50 55 60 65 68 80`, speeds `41 55 65 85 100 100` (your `120` clamped to 100%).

## Install
```bash
sudo install -m755 msi-fan-curve            /usr/local/bin/msi-fan-curve
sudo install -m644 msi-fan-curve.conf       /etc/msi-fan-curve.conf
sudo install -m755 50-msi-fan-curve         /usr/lib/systemd/system-sleep/50-msi-fan-curve
sudo install -m644 msi-fan-curve.service    /etc/systemd/system/msi-fan-curve.service
sudo systemctl daemon-reload
sudo systemctl enable --now msi-fan-curve.service

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

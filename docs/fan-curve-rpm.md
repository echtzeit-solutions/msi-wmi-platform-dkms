# Fan curve setpoint -> RPM characterization (MS-16V5 / GS66 12UHS)

Measured via flat curve at each level (manual mode, pwm_enable=1), idle ~45C, both fans.
"Curve value" = hwmon pwm auto-point value (0-255); driver stores it as percent (0-100) in EC.

| pwm(0-255) | EC % | CPU fan1 RPM | GPU fan2 RPM |
|---:|---:|---:|---:|
| 0   | 0%   | 0 (off) | 0 (off) |
| 36  | 14%  | 3000 | 3057 |
| 73  | 28%  | 2944 | 3018 |
| 109 | 42%  | 2944 | 3018 |
| 146 | 57%  | 3404 | 3453 |
| 182 | 71%  | 3966 | 3966 |
| 219 | 85%  | 4528 | 4173 |
| 255 | 100% | 4752 | 4173 |

Three regimes:
- 0% = fans fully OFF (hard stop).
- ~14-42% = minimum floor ~3000 RPM (cannot go slower than floor except 0).
- ~57-100% = control range, ~linear: CPU 3.4k->4.75k, GPU 3.4k->4.17k.

Notes: CPU fan max ~4750 RPM > GPU fan max ~4170 RPM (different specs). Readback rounding
(e.g. 219->216) is pwm<->percent quantization. RPM = 480000/tach. Requires pwm_enable=1
(manual) for the curve to take effect; pwm_enable=2 = EC auto + factory-curve restore.

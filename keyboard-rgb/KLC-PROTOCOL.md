# MSI SteelSeries KLC keyboard — HID protocol (for full Linux parity)

Reverse-engineered from the GE66 KLC firmware (`firmware-klc-ge66-v237.bin`, STM32 Cortex-M0) and
validated on the GS66 (`1038:113a`). **Caveat:** GS66 runs a *different firmware build* (cmd `0x10`
returns `0x0105` vs GE66 `0x0225`) — same protocol family, but byte offsets/constants may drift, so
verify each write on-device (or from a Windows USB capture) before trusting it.

## Transport
- USB-HID, interface 0 = `/dev/hidraw1`, no report IDs (report id 0). Root-only.
- Two channels into the firmware's `usb_set_report` dispatcher, gated by SET_REPORT report-type:
  - **Feature report (type 3), 524 B (`0x20C`)** = bulk per-key color payloads (`0x0C`/`0x0E`) + the
    152-B effect program (`0x0B`). Sent via `HIDIOCSFEATURE`.
  - **Output report (type 2), 64 B** = the short vendor commands below (`payload[0]=cmd`). Sent via
    `write()`. GET replies come back on the 64-B INPUT report (`read()`).

## Vendor command set (payload[0] = command)
| Cmd | Payload | Effect |
|---|---|---|
| `0x03` | `kc,R,G,B` | set ONE key live (LED+0x1c, override +0x22=1) |
| `0x50` | `xx,R,G,B` | **set ALL keys single color, live** (immediate, bypasses default/mode) |
| `0x51` | `xx,R,G,B` | **set GLOBAL DEFAULT color** (profile +0xEFC/D/E) + apply; persists with `0x09` |
| `0x52` | — | all off |
| `0x83` | `kc` | clear one key's override |
| `0x40` | — | load compiled default (resets effect/brightness machinery — RECOVERY) |
| `0x06` | `xx,mode` | set mode 0–4 (profile +0xEFF) — selects render path |
| `0x86` | — (GET) | read back mode + brightness |
| `0x0A` | 10 B/key | per-key effect config: zone id (+8) + mode-flags (+9) |
| `0x0B` | 152 B (feature) | upload effect program (see below), slot = `payload[2]` (≤18) |
| `0x0C`/`0x0E` | 524 B (feature) | bulk per-key color (0x0C=4B `[hid,R,G,B]` entries; 0x0E=12B effect entries) |
| `0x09` | — | trigger save-to-flash (`0x0800E000`, magic `0x25` + profile) |
| `0x0D` | — | show/apply (push profile → live LEDs) |
| `0x10`/`0x80`/`0x15`/`0x22` | — (GET) | device id / STM32 UID / fw CRC / status |
| `0xF1` | — | NVIC system reset (clean reboot to app; re-enumerates USB) |
| `0xFD` | — | on/off flag |

## Effect program (cmd 0x0B, 152 B / 0x98, one per zone slot)
```
0x00..0x7F : keyframe array, 8 B each:
             { u16 duration_ticks; u8 next_kf_index; u8 pad; i16 dR,dG,dB,dA }
             interpolator ramps color by delta/tick for `duration`, then jumps to next_kf_index
             (loop). next==0 => snap to default color (below) or per-key static.
0x80..0x87 : default/idle color (fixed-4 R,G,B,A)
0x88 / 0x8A: i16 wave origin X, Y (physical key-layout units)
0x8C / 0x8E: u16 per-axis scale
0x90       : wave-enable flag
0x94       : u16 wavelength/period
0x96       : u8 direction/reverse flag
```
- **Breathing/fade** = keyframes that ramp a color up then down (deltas), looping.
- **Color cycle** = multi-keyframe sequence across colors.
- **Wave/ripple** = enable wave, set origin/wavelength/direction; per-LED phase = octagonal distance
  from origin to the key's (x,y), so color propagates outward. Physical (x,y) built by
  `led_build_position_table`.
- **Blink/strobe** = separate `blink_toggle_step` engine (mode-flag bit 3).
- Mode-flags byte (per-key cfg +9): bits 0/1 static, bit 3 blink, bits 2/4 wave; else keyframe.

## Render / brightness gate (IMPORTANT)
Every channel = `color * (g_global_brightness+1) * (perLedAlpha+1) >> 16`, then gamma
(`color_gamma_mix`). `g_global_brightness` (SRAM `0x20000168`) is the master gate, driven per-frame by
the effect engine (NOT a static byte). If it's 0 → **all black regardless of color**. This caused the
black-out; recovery = `0x40` (load_default) then `0x50` (set-all).

## Persistence model
- Live per-key colors: RAM only (buffer at `0x200001ec`), wear-free for animations.
- Flash-saved *profile* (`0x20001430` → flash `0x0800E000`): default color + mode + brightness +
  per-key effect configs — NOT the live per-key RGB. Boot loads it (magic `0x25`) then `apply`
  broadcasts the DEFAULT color. So onboard persistence is effectively single-color; per-key that
  "survives reboot" is a host re-apply (what MSI Center/SteelSeries do).
- **Wear-free Linux persistence = re-apply on brightness/resume events (udev/systemd), not flash.**

## Parity roadmap
1. Static per-key + all-key + default: DONE (validated).
2. Onboard effects (breathing/wave/blink): build via `0x0B`+`0x0A`+`0x06` per the layout above —
   VERIFY exact bytes against a Windows USB capture first (GS66 build drift).
3. Brightness get/set + mode get/set: `0x86`/`0x06`.
4. Re-apply hook for persistence.
5. Ground truth: capture MSI Center / SteelSeries GG driving the GS66 (usbpcap/Wireshark) and diff
   against this spec — that removes all remaining byte-layout guesswork.

## Authoritative per-key lighting encoding (from decrypted common_lighting.lisp)
Per-key element = 12 bytes (`lighting_element_info`(10) + `lockmask`(1) + `hid`(1)), sent via the
`0x0E` feature blocks (block1=42 keys, block2=19, block3=24):
```
[ init.R, init.G, init.B,          ; static/base color
  react.R, react.G, react.B,       ; reactive (keypress) color
  react.time_lo, react.time_hi,    ; reactive fade time (uint16 ms, 0..2000)
  effect_index,                    ; 0..17, index into uploaded effect table
  settings_mask,                   ; see below
  lockmask, hid ]
```
`settings_mask` bits: `steady=1`, `effect=0`, `host-stream=2`, `override=4`, **`reactive=8`**.
(Our validated `all` uses settings_mask=1 (steady) → solid color. The keypress "trail" = settings_mask=8.)

`lighting_effect` (69B, uploaded, referenced by effect_index): `type`(disabled=0/colorshift=1/breathe=2),
`has_direction`+`direction_type`(horizontal=0/vertical=1/radial=2)+`direction_inverted` (= wave),
`focal_x/y`(u16 origin), `speed`(u16), `scale`(u16 propagation), `num_colors`(0..14), `colors[14]`(RGB),
`positions[14]`(0..100). command `0x06` = layout/layer select (layout_id 0..3).

## Named-command map (replaces `raw`)
| Command | Encoding |
|---|---|
| `all <rgb>` / `key k=rgb` / `off` | init=rgb, settings_mask=steady(1) |
| `reactive <base> <hit> [--fade ms]` | init=base, react=hit+time, settings_mask=reactive(8) |
| `effect breathe <colors..> [--speed]` | lighting_effect type=2, keys settings_mask=effect(0) |
| `effect colorshift <colors..> [--speed]` | lighting_effect type=1 |
| `effect wave <colors..> [--dir h/v/radial] [--speed]` | type + has_direction=1 |
| `effect off` | type=disabled(0) |
| `layout <0-3>` | command 0x06 layout_id |
| `brightness` / `info` | GET 0x86 / 0x10 / 0x80 / 0x15 |

## Brightness (software-owned) + DE OSD

On the validated MS-16V5 / GS66 (`1038:113a`) firmware build there is **no**
usable device brightness path from the host:

- No host HID brightness command exists.
- The `0x86` (GET mode+brightness) read-back returns zeros on this build.
- The EC brightness registers (`0xD3`/`0xEC`) are decoupled from the per-key
  RGB PWM and wrap.

So brightness is **owned in software**: the host scales the RGB it sends by a
`brightness/255` multiplier, folded into the same per-channel scale as the
model's `color_scale` correction: `effective[i] = color_scale[i] *
brightness/255`. Brightness 0 → black; 255 → unscaled; 128 → ~half. This is
the wear-free "re-apply on brightness event" model from the Persistence
section, made concrete: the last-applied logical frame (raw colors + resolved
key ids, stored **pre-brightness**) is re-rendered at the new brightness.

### uleds `*::kbd_backlight` bridge (DE OSD)
Desktop keyboard-backlight OSD integration is via the kernel `uleds`
userspace-LED module — no custom kernel driver:

- Open `/dev/uleds` `O_RDWR`, write `struct uleds_user_dev { char name[64];
  __u32 max_brightness; }` (68 bytes: 64-byte NUL-padded name + native-endian
  u32). `LED_MAX_NAME_SIZE = 64`.
- Register the name `msiklc::kbd_backlight` with `max_brightness` 255. Because
  the name contains `kbd_backlight`, UPower adopts it under
  `org.freedesktop.UPower.KbdBacklight`, and GNOME/KDE show the native
  keyboard-backlight OSD for free.
- Each brightness change from the DE arrives as a 4-byte native-endian int
  from `read(/dev/uleds)`. Closing the fd removes the LED.

Validated end-to-end on this machine: registering the LED creates
`/sys/class/leds/msiklc::kbd_backlight`; after `systemctl restart upower`,
`GetMaxBrightness` returns 255 and `SetBrightness i 200` from the DE is
delivered to the daemon's `read()`.

**Ordering constraint:** UPower only enumerates `*::kbd_backlight` LEDs at
**its own** startup — there is no hot-add. The systemd unit registering the
uleds LED must therefore be ordered `Before=upower.service` (or UPower must be
restarted once after the daemon first starts).

### `msi-klc` commands
- `msi-klc daemon` — registers the uleds LED and, on each brightness the DE
  sends, re-applies the stored frame scaled by it. Needs `modprobe uleds` +
  root.
- `msi-klc brightness <VALUE>` — `VALUE` is `0..=255` or a percentage (`50%`);
  updates the stored brightness, re-applies the stored frame, and mirrors the
  raw value into `/sys/class/leds/msiklc::kbd_backlight/brightness` (if the
  daemon is running) so the OSD stays in sync. With no `VALUE` it is the
  informational `0x86` read-back.
- State lives at `/run/msi-klc/state.json` (fallback
  `$XDG_RUNTIME_DIR/msi-klc/state.json`): `{ "brightness": 0..=255, "frame":
  <last logical frame> }`.

# msi-klc

Control MSI SteelSeries "KLC" per-key RGB laptop keyboards (USB-HID
`1038:113a` and the wider KLC family — GE73/GE66/GS66/GK70/GK80, etc.) on
Linux via `/dev/hidrawN`.

This is a Rust reimplementation of a hardware-validated Python reference
(`linux-msi-ms16v5/keyboard-rgb/{msi-nb-rgb.py,klc-cmd.py}`), reproducing its
byte layouts exactly. See `linux-msi-ms16v5/keyboard-rgb/KLC-PROTOCOL.md` for
the full protocol write-up this tool is built against.

It also embeds a per-model device table (~40 KLC USB PIDs, see `msi-klc
models`) extracted from decrypted SteelSeries per-model specs bundled with
MSI Center, so it auto-detects which model is attached and can correct
colors for that panel's LED response (see "Color-scale correction" below).

## Safety

- **Default operation is RAM-only and wear-free.** `all`, `off`, `key`,
  `reactive`, `layout`, `brightness`, `daemon`, `info`, `recover`, `query`,
  `raw`, and `effect` only touch the live LED buffer / vendor command channel
  — nothing is written to flash. (`brightness`/`daemon` additionally read and
  write a small brightness state file under `/run` or `$XDG_RUNTIME_DIR`, not
  the device flash.)
- **`persist` writes flash** (the controller's saved default-color profile)
  and is gated behind a required `--i-understand-flash-risk` flag. Wrong
  bytes have bricked the LED display before during development of the
  reference this tool is built on, and the GS66's firmware build is known to
  differ from the GE66 firmware the protocol was originally reverse
  engineered from. Only use `persist` if you understand and accept that risk.
- `effect` (onboard breathe/colorshift/wave effect programming via `0x0B`)
  uploads the transformed 151-byte device effect program: the 69-byte
  `lighting_effect` host struct (from the decrypted `common_lighting.lisp`
  spec) is run through the section-by-section keyframe/ramp transform ported
  from `get-breathe-effect-bytes`/`get-colorshift-effect-bytes` in
  `fancy_lighting_engine.lisp` (see `src/protocol.rs`'s
  `LightingEffect::to_device_bytes`), and the assigned keys are pointed at it
  (`settings_mask=EFFECT`). The transform is unit-tested against
  hand-computed byte vectors, but — like `layout` below — has **not** been
  independently cross-checked against a hardware USB capture; it prints a
  short note to that effect each time it runs (except for `effect off`,
  which is just steady black).
- `layout` (layer/layout select via vendor `0x06`) is built from the spec
  this tool was written against but its exact field layout is **not**
  independently cross-checked against a USB capture — see `src/protocol.rs`'s
  `build_layout_select`.
- `brightness`/`info` decode their GET replies best-effort — KLC-PROTOCOL.md
  documents which command returns what, but not the exact reply byte
  offsets/endianness for every field. Raw bytes are always printed alongside
  the decoded value so you can sanity-check it (or fall back to `query`).
- `raw` is an advanced escape hatch (hidden from `--help`); prefer the named
  subcommands, which cover every code path `raw` would otherwise be used for.
- hidraw nodes are root-only; every subcommand needs `sudo`.
- This tool does not expose the `0xF1` (NVIC reset) vendor command.

## Building

Requires Rust 1.85+ (edition 2024). Tested with rustc/cargo 1.96.1.

```sh
cargo build --release
cargo test        # unit tests for the frame builders, no hardware needed
cargo clippy --all-targets
```

The binary is `target/release/msi-klc`.

## Device selection

By default the tool scans `/sys/class/hidraw/*/device/uevent` for a
`HID_ID` matching vendor `0x1038` (MSI) against *every* PID in the embedded
model table (`msi-klc models`) — not just `0x113a` — so any recognized KLC
keyboard is found without extra flags. Once found, it resolves the
`vid:pid` to a model name and prints it (e.g. `detected model msi-klc496
(1038:113a)`); if the connected PID isn't in the table it falls back to
"unknown model" and applies no color-scale correction (see below).

You can override either the device path or the vid:pid to match:

```sh
sudo msi-klc --path /dev/hidraw2 all FF0000     # explicit device node
sudo msi-klc --id 1038:113a all FF0000          # explicit vid:pid to match
```

`--path` still tries to resolve the model, by reading back that node's own
`HID_ID` — pass `--id` too if you want to force which entry in the table it
resolves against (e.g. testing against a node whose kernel-reported PID
happens to not be in the table).

## Usage

```sh
# Print the embedded per-model device table (name, USB id, color_scale,
# key count, #key-coordinates known). Read-only, no device/root needed.
msi-klc models

# Set every key to one color (6-group 0x0E bulk write), then commit. RAM-only.
sudo msi-klc all FF0000

# All keys off.
sudo msi-klc off

# Set individual keys (0x0C free-form write). NAME = CLK_* from the embedded
# GE73Keys keymap (also covers the GS66), or a bare decimal/hex HID usage code.
sudo msi-klc key CLK_Escape=FF0000 CLK_Power=00FF00 CLK_W=0000FF
sudo msi-klc key 41=FF0000       # decimal HID code
sudo msi-klc key 0x29=FF0000     # hex HID code (same key)

# Onboard keypress-trail effect: keys rest at BASE, flash to HIT on keypress,
# and fade back over --fade ms (settings_mask=REACTIVE). RAM-only.
sudo msi-klc reactive 000033 00AAFF          # default 300ms fade
sudo msi-klc reactive 000000 FF0000 --fade 500
# Run `all`/`off` to return keys to steady mode (settings_mask=STEADY):
sudo msi-klc off

# Select the active keyboard layout/layer (vendor 0x06), 0..=3.
sudo msi-klc layout 1

# Read back mode + brightness (0x86, best-effort decode) and device info.
sudo msi-klc brightness   # no value -> GET mode + brightness (0x86)
sudo msi-klc info         # device id (0x10) + STM32 UID (0x80) + fw CRC (0x15)

# Software-owned brightness (see "Brightness + DE OSD" below). Sets a 0..=255
# or percentage brightness, re-applies the last-applied lighting scaled by it,
# and persists it to /run/msi-klc/state.json. Firmware/EC brightness is
# unreliable on this hardware, so brightness is a software RGB multiplier.
sudo msi-klc all 00FF00       # apply something first (stores the frame)
sudo msi-klc brightness 128   # half brightness (absolute 0..=255)
sudo msi-klc brightness 50%   # same, as a percentage

# DE-OSD bridge daemon: exposes brightness to the desktop's native
# keyboard-backlight OSD via the uleds module + UPower. Needs `modprobe uleds`.
# See packaging/ for the systemd unit (must be ordered Before=upower.service).
sudo modprobe uleds
sudo msi-klc daemon

# Vendor GET commands: print the raw 64-byte reply (prefer brightness/info above).
sudo msi-klc query 86    # mode + brightness
sudo msi-klc query 10    # device id
sudo msi-klc query 15    # firmware CRC

# Recovery: if the keyboard has gone all-black (brightness gate stuck at 0),
# this sends load_default (0x40) then set-all-live (0x50).
sudo msi-klc recover           # defaults to white
sudo msi-klc recover 00FF00

# Onboard breathe/colorshift/wave effect (0x0B transformed device-program
# upload + per-key settings_mask=EFFECT). See "Safety" above -- the byte
# transform is unit-tested against hand-computed vectors but not yet
# cross-checked against a hardware USB capture.
sudo msi-klc effect breathe FF0000 00FF00 --speed 80
sudo msi-klc effect colorshift FF0000 00FF00 0000FF
sudo msi-klc effect wave FF0000 00FF00 --dir radial
# Explicit keyframe positions (0..=100), one per color, instead of the
# default even spacing:
sudo msi-klc effect colorshift FF0000 00FF00 0000FF --positions 0,33,66
sudo msi-klc effect off

# Advanced escape hatch (hidden from --help; no reply read, no safety net).
# Prefer the named subcommands above -- they cover the same ground.
sudo msi-klc raw 09      # e.g. save-to-flash trigger, sent standalone

# Flash-persisted global default color. WRITES FLASH. Requires the flag.
sudo msi-klc persist FF0000 --i-understand-flash-risk
```

`--keymap NAME` selects an alternate embedded keymap (e.g. `GK80_US_Keys`);
default is `GE73Keys`. Run `msi-klc key --help` etc. for per-subcommand help.

## Color-scale correction

Different KLC panels have different LED color response, so the SteelSeries
spec each model ships with includes a per-channel `color_scale` (e.g. the
GS66's `msi-klc496` entry is `[1.0, 1.0, 1.0]` — no correction needed — but
most GE/GS/GT models are down-scaled on R and/or B, e.g. `[0.41, 1.0,
0.51]`).

By default (no `--raw`), `all`, `key`, `recover`, and `reactive` apply the
detected model's `color_scale` to every color you supply: `out = round(in *
scale)` per channel, e.g. `0xFF` at scale `0.41` -> `round(255 * 0.41) =
105`. This makes a given hex color look consistent across panels instead of
being sent byte-exact (and potentially over-driving or dimming a channel the
panel doesn't expect at full scale).

Pass `--raw` to skip this and send your colors unscaled — e.g. if you're
working from a capture/reference that already accounts for the panel, or
the model was detected as "unknown" and you'd rather send bytes exactly as
given than trust a wrong guess:

```sh
sudo msi-klc --raw all FF0000
```

`persist` is not scaled (out of caution — it writes flash verbatim; see its
own section above). `all`/`key`/`recover`/`reactive` colors and, since
software brightness was added, `effect` palettes all go through `color_scale`
(and, except `recover`, the software brightness fold below).

`--raw` is a diagnostic one-shot: it disables **both** the `color_scale`
correction and the software brightness fold, sending your bytes exactly as
given. The frame is still stored (with the current brightness), so a later
`brightness`/`daemon` re-apply renders it software-scaled again.

## Brightness (software-owned) + DE OSD

On the validated MS-16V5 / GS66 (`1038:113a`) the KLC keyboard has **no** host
HID brightness command, its `0x86` read-back returns zeros, and the EC
brightness registers are decoupled from the per-key RGB PWM (and wrap). So
`msi-klc` owns brightness **in software**: it folds a `brightness/255`
multiplier into the same per-channel scale `color_scale` already uses —
`effective[i] = color_scale[i] * brightness/255` — and sends the scaled RGB.

- Every apply command (`all`/`off`/`key`/`reactive`/`effect`) renders at the
  current stored brightness and saves the **pre-brightness** logical frame
  (raw colors + resolved key ids) plus the brightness to a small state file at
  `/run/msi-klc/state.json` (falling back to
  `$XDG_RUNTIME_DIR/msi-klc/state.json`).
- `msi-klc brightness <VALUE>` (`0..=255` or e.g. `50%`) updates the stored
  brightness and **re-applies the stored frame** at the new brightness, so you
  can dim/brighten whatever is currently shown without re-specifying it. With
  no value it stays the informational `0x86` read-back.
- `msi-klc daemon` registers a `uleds` userspace LED named
  `msiklc::kbd_backlight`; UPower adopts it and GNOME/KDE then drive it through
  their **native keyboard-backlight OSD**. Each brightness the DE sets arrives
  over `/dev/uleds` and re-applies the stored frame — the same path as
  `brightness <VALUE>`. `brightness <VALUE>` also mirrors the raw value back
  into the LED's sysfs so the OSD stays in sync with CLI changes.

`daemon` needs `modprobe uleds` and root. **UPower only enumerates
`*::kbd_backlight` LEDs at its own startup (no hot-add)**, so the systemd unit
must be ordered `Before=upower.service` — see `packaging/` for the unit and
full install steps.

## `settings_mask` — what mode a key is in

Every per-key `0x0E` element carries a `settings_mask` byte selecting which
of the firmware's rendering paths that key uses (from the decrypted
`common_lighting.lisp` spec):

| Value | Name | Set by |
|---|---|---|
| `0` | `EFFECT` | `effect breathe/colorshift/wave` — key follows the uploaded effect at `effect_index` |
| `1` | `STEADY` | `all`/`key`/`off`/`effect off` — key shows a fixed `init` color |
| `2` | `HOST_STREAM` | (not wired up here) live host-streamed color |
| `4` | `OVERRIDE` | (not wired up here) single-key `0x03` override |
| `8` | `REACTIVE` | `reactive` — key shows `init` at rest, flashes to `react` on keypress, fades back over `react.time` ms |

`src/protocol.rs`'s `KeyElement` struct and `settings_mask` module are the
canonical reference; `all`/`key`/`off`/`reactive`/`effect` are just
different ways of building `KeyElement`s with a given mask.

## Protocol summary

- **Feature report** (type 3, 525 bytes incl. report-id byte), sent via
  `HIDIOCSFEATURE`: bulk per-key color.
  - `0x0E` "group" format: one report per region group (`Group1_Offset` ..
    `Group6_Offset`, from `data/msi-layouts.json`), 12-byte entries.
    Sending all six groups is required to reach every key (e.g. Power,
    which a single-group approach misses).
  - `0x0C` "free" format: one report, 4-byte `[hid, R, G, B]` entries, up to
    130 keys.
- **Output report** (type 2, no fixed length beyond the report-id byte),
  sent via `write()`: short vendor commands (`[report_id=0, cmd, params...]`).
  GET replies come back on the 64-byte input report (`read()`).
  Notable commands: `0x50` set-all-live, `0x51` set-default-color, `0x40`
  load-default, `0x09` save-to-flash, `0x0D` show/commit, `0x86`/`0x10`/
  `0x80`/`0x15`/`0x22` GET variants.
- **Persistence model**: live per-key colors are RAM-only. The
  flash-saved profile only holds the default color + mode + brightness +
  per-key effect configs, not live per-key RGB — so wear-free persistence
  across reboot means re-applying colors on boot/resume (e.g. a udev/systemd
  hook calling `msi-klc all/key ...`), not `persist`.

See `KLC-PROTOCOL.md` (in the RE project this tool is derived from) for the
complete command table, the brightness-gate render pipeline, and caveats
around GS66 firmware drift.

## Layout data

`data/msi-layouts.json` is embedded at compile time (`include_str!`) from
the RE project's extracted MSI Center layout data. It provides the
`GE73Keys` (and sibling `GK70`/`GK80`) keymaps plus the `Group1_Offset` ..
`Group6_Offset` region partition used by the `0x0E` bulk-color format.

`data/msi-klc-models.json` is embedded the same way, copied from
`linux-msi-ms16v5/keyboard-rgb/msi-klc-models.json` in the RE project (itself
extracted from decrypted SteelSeries per-model specs bundled with MSI
Center). Each entry has the model's USB `vid:pid`, its `color_scale`
correction, an approximate `key_count`, and — for most models — a
`key_coords` map of HID usage code -> physical `(x, y)` position (used by
`msi-klc models`'s `COORDS` column; not yet wired into `effect`'s wave
geometry, see the TODO on `models::Model::key_coords`).

## Module layout

- `src/protocol.rs` — frame builders: the `KeyElement`/`settings_mask` model
  behind every per-key `0x0E` element (steady, reactive), `0x0C` free-form
  color, vendor command output reports, the commit/"show" frame, the
  layout-select report, and the `LightingEffect` host-struct + device-byte
  transform (`to_device_bytes`, ported from `fancy_lighting_engine.lisp`) +
  `0x0B` upload frame builder — plus unit tests asserting exact byte offsets
  and report lengths (including a byte-compat test pinning the refactored
  steady-color encoding to the original hand-rolled one, and hand-computed
  breathe/colorshift device-byte vectors).
- `src/device.rs` — hidraw device discovery (`/sys/class/hidraw` scan,
  multi-PID match) and I/O (`HIDIOCSFEATURE` ioctl, output-report write,
  input-report read/poll).
- `src/layout.rs` — embedded keymap/region-group data + key-name resolution
  and hex-color parsing.
- `src/models.rs` — embedded per-model device table (USB id -> model name,
  color-scale correction, key coordinates) and the `scale_rgb` color
  correction helper.
- `src/state.rs` — software-owned brightness + last-applied logical `Frame`,
  persisted to `/run/msi-klc/state.json` (or `$XDG_RUNTIME_DIR`); the
  `fold_brightness` scale math and the `0..=255`/`%` value parser, both
  unit-tested.
- `src/daemon.rs` — the `uleds`/UPower DE-OSD bridge (`daemon`) and the shared
  brightness re-apply path used by `daemon` and `brightness <VALUE>`.
- `src/main.rs` — `clap` derive CLI wiring the above together, including the
  single shared `apply_frame` used by initial apply, `brightness`, and
  `daemon`.

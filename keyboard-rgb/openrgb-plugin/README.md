# OpenRGB MSI KLC Keyboard plugin

A self-contained, out-of-tree [OpenRGB](https://openrgb.org) plugin that adds
device support for the **MSI GS66 SteelSeries KLC per-key RGB keyboard**
(USB `1038:113a`). It presents the keyboard as a matrix-zone keyboard with a
per-key **Direct** mode, driving it over hidraw with the same 525-byte feature
report the in-tree `MSILaptopController` uses.

This plugin lives in our tree (echtzeit-solutions) and builds against an
OpenRGB source tree as a read-only reference.

## What it does

- Detects `1038:113a` interface 0 (the SteelSeries vendor lighting collection,
  usage_page `0xFFC0` / usage `0xF1`) via `hid_enumerate`.
- Registers one `DEVICE_TYPE_KEYBOARD` controller (vendor `SteelSeries`) with a
  single `ZONE_TYPE_MATRIX` zone built from the generated GS66 layout
  (`openrgb-gs66-arrays.h`, 88 keys, 6x15 matrix map).
- **Direct mode** (`MODE_FLAG_HAS_PER_LED_COLOR | MODE_FLAG_HAS_BRIGHTNESS`,
  `MODE_COLORS_PER_LED`): every LED update sends a 525-byte feature report
  (`buf[1]=0x0C`, `buf[3]=led_count`, 4 bytes/LED `{keycode,R,G,B}` from offset
  5, unused slots padded `0xFF`).
- Software brightness + per-channel color correction: colors are scaled by
  `color_scale[i] * (brightness/255)` before being sent (`apply_correction`).
  For this unit (`msi-klc496`) the scale is `[1.0, 1.0, 1.0]`, so the color part
  is a no-op, but the mechanism exists for panels with a non-unity scale.
- Re-registers the controller after a user rescan via a detection-end callback.

### Modes implemented

- **Direct**: yes, complete.
- **Reactive / breathe / wave**: **not** implemented. The onboard-effect
  formats (`0x0E` per-key elements, `0x0B` effect programs) are documented in
  `../KLC-PROTOCOL.md` but are flagged there as unverified on the GS66 firmware
  build (byte-layout drift vs the GE66 it was reverse-engineered from). They
  are left out rather than shipped as speculative code.

## Build

Qt6 dev headers are required (Qt5 is not supported by this `.pro`).

```sh
qmake6 && make
```

This produces `libOpenRGBMSIKLCPlugin.so`.

By default the `.pro` builds against `/home/florian/src-laptop/OpenRGB`.
Point it at a different OpenRGB checkout with:

```sh
qmake6 OPENRGB_ROOT=/path/to/OpenRGB && make
```

### hidapi linkage

The `.pro` prefers `pkg-config` (`hidapi-hidraw`, then `hidapi-libusb`) when a
`-dev` package is installed. If neither pkg-config module is present (as on this
machine, which ships only the runtime `libhidapi-hidraw.so.0` with no `-dev`
symlink), it falls back to linking the SONAME directly with
`-l:libhidapi-hidraw.so.0`. The hidapi **header** comes from the OpenRGB tree's
vendored `dependencies/hidapi-win/include/hidapi.h`, which is the standard
cross-platform hidapi header.

## Install

```sh
mkdir -p ~/.config/OpenRGB/plugins
cp libOpenRGBMSIKLCPlugin.so ~/.config/OpenRGB/plugins/
```

Then start OpenRGB. The keyboard should appear as an "MSI GS66 KLC Keyboard"
device, and an "MSI KLC" entry appears under Settings > Plugins.

## Hard constraints

- **Exact-match plugin ABI**: the plugin API is `OPENRGB_PLUGIN_API_VERSION`
  and OpenRGB requires an **exact** match. The `.so` is **not** ABI-portable:
  rebuild it against the *same* OpenRGB version and the *same* Qt version you
  actually run. A mismatch makes OpenRGB refuse to load the plugin.
- **Single owner of the hidraw node**: only one process may hold the keyboard's
  hidraw device at a time. Do **not** run the `msi-klc` daemon (or any other KLC
  tool) and OpenRGB against the keyboard simultaneously — stop the daemon first.
- Access to `/dev/hidrawN` for interface 0 is root-only unless you have a udev
  rule granting your user access.

## Files

| File | Purpose |
|---|---|
| `OpenRGBMSIKLCPlugin.{h,cpp}` | Plugin class: detection, registration, rescan callback. |
| `MSIKLCController.{h,cpp}` | Raw hidraw protocol (525-byte Direct report, correction/brightness). |
| `RGBController_MSIKLC.{h,cpp}` | RGBController subclass: matrix zone, Direct mode. |
| `openrgb-gs66-arrays.h` | Generated GS66 key layout + matrix map (from `gen-openrgb-gs66.py`; do not hand-edit). |
| `OpenRGBMSIKLCPlugin.pro` | qmake6 build. |

## Note

`openrgb-gs66-arrays.h` is **generated** by `../gen-openrgb-gs66.py`. Do not
edit it by hand; regenerate and re-copy it if the layout changes. The matrix map
positions still carry a "VERIFY on real keyboard" caveat from the generator.

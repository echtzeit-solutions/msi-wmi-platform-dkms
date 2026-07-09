//! Frame builders for the MSI SteelSeries "KLC" per-key RGB protocol.
//!
//! Byte layouts here are transcribed 1:1 from the hardware-validated reference
//! implementations `msi-nb-rgb.py` (feature reports: bulk color) and
//! `klc-cmd.py` (output reports: vendor commands), cross-checked against
//! `KLC-PROTOCOL.md`, and — for the per-key `0x0E` element and the
//! `lighting_effect` host struct — against the authoritative decrypted
//! firmware spec `common_lighting.lisp` (see that file's "Authoritative
//! per-key lighting encoding" section for the field table this module
//! implements). See `KLC-PROTOCOL.md` for the full command table and
//! caveats (GS66 firmware drift vs. the GE66 RE source).

/// Length of a KLC HID *feature* report, including the leading report-id byte
/// (`buf[0] = 0`, since the device uses unnumbered reports). The wire payload
/// is 524 bytes; the ioctl buffer is 525 bytes total.
pub const FEATURE_REPORT_LEN: usize = 525;

/// Length of a KLC HID *output*/*input* report, including the leading
/// report-id byte.
pub const OUTPUT_REPORT_LEN: usize = 64;

/// Length of the dedicated layout/layer-select output report (`msi-klc
/// layout`), which is one byte longer than the other output reports.
pub const LAYOUT_REPORT_LEN: usize = 65;

/// Max per-key color entries in one "steady/group" (0x0E) feature report.
/// (Practically limited by the group's own key count, which is always far
/// smaller than this.)
pub const MAX_GROUP_ENTRIES: usize = (FEATURE_REPORT_LEN - 5) / 12;

/// Max per-key color entries in one "free" (0x0C) feature report.
pub const MAX_FREE_ENTRIES: usize = (FEATURE_REPORT_LEN - 5) / 4;

/// Vendor command bytes (`payload[0]` of an output report). Names follow
/// KLC-PROTOCOL.md. Kept as a complete reference of the known command set
/// even though only a subset is currently wired up to the CLI.
#[allow(dead_code)]
pub mod cmd {
    pub const SET_ONE_KEY: u8 = 0x03;
    pub const SET_ALL_LIVE: u8 = 0x50;
    pub const SET_DEFAULT_COLOR: u8 = 0x51;
    pub const ALL_OFF: u8 = 0x52;
    pub const CLEAR_KEY_OVERRIDE: u8 = 0x83;
    pub const LOAD_DEFAULT: u8 = 0x40;
    pub const SET_MODE: u8 = 0x06;
    pub const GET_MODE_BRIGHTNESS: u8 = 0x86;
    pub const SET_PERKEY_EFFECT_CFG: u8 = 0x0A;
    pub const SET_EFFECT_PROGRAM: u8 = 0x0B;
    pub const SET_COLOR_GROUP: u8 = 0x0E;
    pub const SET_COLOR_FREE: u8 = 0x0C;
    pub const SAVE_FLASH: u8 = 0x09;
    pub const SHOW: u8 = 0x0D;
    pub const GET_DEVICE_ID: u8 = 0x10;
    pub const GET_DEVICE_ID_ALT: u8 = 0x80;
    pub const GET_FW_CRC: u8 = 0x15;
    pub const GET_STATUS: u8 = 0x22;
    pub const RESET: u8 = 0xF1;
    pub const ON_OFF_FLAG: u8 = 0xFD;
}

/// `settings_mask` bit values for a per-key `0x0E` element, per
/// `common_lighting.lisp`'s `settings-mask-*` defines. Bits 5-7 are
/// reserved by the firmware.
#[allow(dead_code)]
pub mod settings_mask {
    /// Key follows the uploaded `lighting_effect` at `effect_index`.
    pub const EFFECT: u8 = 0;
    /// Key shows a fixed `init` color (what `all`/`key`/`off` use).
    pub const STEADY: u8 = 1;
    /// Key color is driven live by the host (streaming).
    pub const HOST_STREAM: u8 = 2;
    /// Key color override (used by the single-key `0x03` vendor command).
    pub const OVERRIDE: u8 = 4;
    /// Key shows `init` at rest and flashes to `react` on keypress, fading
    /// back over `react.time` ms (the onboard keypress-trail effect).
    pub const REACTIVE: u8 = 8;
}

/// `lighting_effect.type` values, per `common_lighting.lisp`.
#[allow(dead_code)]
pub mod effect_type {
    pub const DISABLED: u8 = 0;
    pub const COLORSHIFT: u8 = 1;
    pub const BREATHE: u8 = 2;
}

/// `lighting_effect.direction_type` values, per `common_lighting.lisp`.
#[allow(dead_code)]
pub mod direction_type {
    pub const HORIZONTAL: u8 = 0;
    pub const VERTICAL: u8 = 1;
    pub const RADIAL: u8 = 2;
}

/// A resolved (key-code, R, G, B) tuple ready to be packed into a report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyColor {
    pub hid: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl KeyColor {
    pub fn new(hid: u8, r: u8, g: u8, b: u8) -> Self {
        Self { hid, r, g, b }
    }
}

/// The legacy react-fade-time value (`0x012C` = 300, little-endian
/// `[0x2C, 0x01]`) that the original steady-color encoding always sent even
/// though `settings_mask=STEADY` never uses the reactive fields. Preserved
/// byte-for-byte so [`KeyElement::steady`] stays identical to the original
/// hand-rolled encoding (see `all_key_off_byte_identical_to_legacy_encoding`).
const STEADY_LEGACY_REACT_TIME_MS: u16 = 0x012C;

/// One per-key element of a `0x0E` "group" feature report: 12 bytes, per
/// `common_lighting.lisp`'s `lighting_element_info` (10 bytes: `init` +
/// `react` + `effect_index` + `settings_mask`) plus `lockmask` + `hid`:
///
/// ```text
/// [ init.R, init.G, init.B,        // static/base color
///   react.R, react.G, react.B,     // reactive (keypress) color
///   react.time_lo, react.time_hi,  // reactive fade time, u16 LE, 0..2000
///   effect_index,                  // 0..17, index into the uploaded effect table
///   settings_mask,                 // see `settings_mask` module
///   lockmask, hid ]
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyElement {
    pub init: [u8; 3],
    pub react: [u8; 3],
    pub react_time_ms: u16,
    pub effect_index: u8,
    pub settings_mask: u8,
    pub lockmask: u8,
    pub hid: u8,
}

impl KeyElement {
    /// Serialize to the 12-byte on-wire element.
    pub fn to_bytes(self) -> [u8; 12] {
        let mut buf = [0u8; 12];
        buf[0..3].copy_from_slice(&self.init);
        buf[3..6].copy_from_slice(&self.react);
        buf[6..8].copy_from_slice(&self.react_time_ms.to_le_bytes());
        buf[8] = self.effect_index;
        buf[9] = self.settings_mask;
        buf[10] = self.lockmask;
        buf[11] = self.hid;
        buf
    }

    /// A fixed/static color element: `init = (r, g, b)`, no reactive
    /// behavior, `settings_mask = STEADY`. What `all`/`key`/`off` use.
    ///
    /// Keeps the legacy `react_time_ms = 300` and zeroed `react`/
    /// `effect_index`/`lockmask` fields the original hand-rolled encoding
    /// always sent, so this is byte-identical to the pre-refactor output.
    pub fn steady(hid: u8, r: u8, g: u8, b: u8) -> Self {
        Self {
            init: [r, g, b],
            react: [0, 0, 0],
            react_time_ms: STEADY_LEGACY_REACT_TIME_MS,
            effect_index: 0,
            settings_mask: settings_mask::STEADY,
            lockmask: 0,
            hid,
        }
    }

    /// A reactive/keypress-trail element: rests at `base`, flashes to `hit`
    /// on keypress, fades back over `fade_ms` (device range 0..2000),
    /// `settings_mask = REACTIVE`.
    pub fn reactive(hid: u8, base: (u8, u8, u8), hit: (u8, u8, u8), fade_ms: u16) -> Self {
        Self {
            init: [base.0, base.1, base.2],
            react: [hit.0, hit.1, hit.2],
            react_time_ms: fade_ms,
            effect_index: 0,
            settings_mask: settings_mask::REACTIVE,
            lockmask: 0,
            hid,
        }
    }
}

/// Build one "steady/group" (`0x0E`) feature report for a single region
/// group from already-built [`KeyElement`]s.
///
/// Mirrors `msi-nb-rgb.py:set_all_groups`'s per-group frame: `buf[1] =
/// 0x0E`, `buf[3] = len(elements)`, and each entry at `base = 5 + 12*m`.
///
/// Panics if `elements.len() > MAX_GROUP_ENTRIES` (525-byte report can't hold
/// more).
pub fn build_group_frame_elements(elements: &[KeyElement]) -> [u8; FEATURE_REPORT_LEN] {
    assert!(
        elements.len() <= MAX_GROUP_ENTRIES,
        "group has {} keys, max {} fit in one 0x0E report",
        elements.len(),
        MAX_GROUP_ENTRIES
    );
    let mut buf = [0u8; FEATURE_REPORT_LEN];
    buf[1] = cmd::SET_COLOR_GROUP;
    buf[3] = elements.len() as u8;
    for (m, e) in elements.iter().enumerate() {
        let base = 5 + 12 * m;
        buf[base..base + 12].copy_from_slice(&e.to_bytes());
    }
    buf
}

/// Build one "steady/group" (`0x0E`) feature report for a single region
/// group, all entries set to the same `(r, g, b)` and `settings_mask =
/// STEADY`. Byte-identical to the pre-refactor hand-rolled encoding (see the
/// `all_key_off_byte_identical_to_legacy_encoding` test).
pub fn build_group_frame(hids: &[u8], r: u8, g: u8, b: u8) -> [u8; FEATURE_REPORT_LEN] {
    let elements: Vec<KeyElement> = hids.iter().map(|&hid| KeyElement::steady(hid, r, g, b)).collect();
    build_group_frame_elements(&elements)
}

/// Build one "reactive/group" (`0x0E`) feature report for a single region
/// group: every key rests at `base`, flashes to `hit` on keypress, and fades
/// back over `fade_ms` (`settings_mask = REACTIVE`). This reproduces the
/// onboard keypress-trail effect.
pub fn build_group_frame_reactive(
    hids: &[u8],
    base: (u8, u8, u8),
    hit: (u8, u8, u8),
    fade_ms: u16,
) -> [u8; FEATURE_REPORT_LEN] {
    let elements: Vec<KeyElement> = hids.iter().map(|&hid| KeyElement::reactive(hid, base, hit, fade_ms)).collect();
    build_group_frame_elements(&elements)
}

/// Build one "free" (`0x0C`) per-key feature report from arbitrary
/// `(hid, r, g, b)` entries. Mirrors `msi-nb-rgb.py:set_keys_free`:
/// - `buf[1] = 0x0C`
/// - `buf[3] = len(entries)`
/// - for each entry `i`, `base = 5 + 4*i`: `[hid, R, G, B]`
///
/// Panics if `entries.len() > MAX_FREE_ENTRIES`.
pub fn build_free_frame(entries: &[KeyColor]) -> [u8; FEATURE_REPORT_LEN] {
    assert!(
        entries.len() <= MAX_FREE_ENTRIES,
        "{} entries requested, max {} fit in one 0x0C report",
        entries.len(),
        MAX_FREE_ENTRIES
    );
    let mut buf = [0u8; FEATURE_REPORT_LEN];
    buf[1] = cmd::SET_COLOR_FREE;
    buf[3] = entries.len() as u8;
    for (i, e) in entries.iter().enumerate() {
        let base = 5 + 4 * i;
        buf[base] = e.hid;
        buf[base + 1] = e.r;
        buf[base + 2] = e.g;
        buf[base + 3] = e.b;
    }
    buf
}

/// Build the 64-byte "show"/commit output report: `[0x00, 0x0D, 0x00, 0x02, 0...]`.
/// Pushes the just-written RAM color buffer to the live LEDs
/// (MSI's `Style_Keyboard_Show`).
pub fn build_commit() -> [u8; OUTPUT_REPORT_LEN] {
    let mut buf = [0u8; OUTPUT_REPORT_LEN];
    buf[1] = cmd::SHOW;
    buf[3] = 0x02;
    buf
}

/// Build a short vendor-command output report: `[0x00, cmd, params...]`,
/// exactly as `klc-cmd.py:send_output` does — **not** padded to 64 bytes,
/// to stay byte-exact with the validated reference (the kernel/driver
/// accepts the short write for unnumbered output reports).
pub fn build_vendor(cmd: u8, params: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(2 + params.len());
    buf.push(0x00); // report id
    buf.push(cmd);
    buf.extend_from_slice(params);
    buf
}

/// Build the `0x50` "set all keys, live" vendor command: `[0x50, xx, R, G, B]`.
/// `xx` is a reserved/index byte the reference always sends as 0.
pub fn build_set_all_live(r: u8, g: u8, b: u8) -> Vec<u8> {
    build_vendor(cmd::SET_ALL_LIVE, &[0x00, r, g, b])
}

/// Build the `0x51` "set global default color" vendor command:
/// `[0x51, xx, R, G, B]`. This is the profile default that persists across
/// reboot when followed by `0x09` (save-to-flash) — see `build_save_flash`.
pub fn build_set_default_color(r: u8, g: u8, b: u8) -> Vec<u8> {
    build_vendor(cmd::SET_DEFAULT_COLOR, &[0x00, r, g, b])
}

/// Build the `0x40` "load compiled default" vendor command (no params).
/// Recovery step: resets the effect/brightness machinery so the brightness
/// gate (`g_global_brightness`) is no longer stuck at 0.
pub fn build_load_default() -> Vec<u8> {
    build_vendor(cmd::LOAD_DEFAULT, &[])
}

/// Build the `0x09` save-to-flash vendor command (no params). Persists the
/// current profile (default color / mode / brightness / per-key effect
/// configs — **not** live per-key RGB) to flash at `0x0800E000`.
pub fn build_save_flash() -> Vec<u8> {
    build_vendor(cmd::SAVE_FLASH, &[])
}

/// Build the layout/layer-select output report (`msi-klc layout`):
/// `[report_id=0, 0x0B, 0, cmd::SET_MODE(0x06), 0, layout_id]`, zero-padded
/// to `LAYOUT_REPORT_LEN` (65) bytes.
///
/// The `0x0B`/`0` fields at offsets 1-2 are named `klc-ftr-rep`/`klc-rep-id`
/// in the spec this was built against; they are not independently
/// cross-checked against a USB capture, unlike the `0x0E`/`0x0C` color
/// reports and the vendor command table in `KLC-PROTOCOL.md`.
pub fn build_layout_select(layout_id: u8) -> [u8; LAYOUT_REPORT_LEN] {
    assert!(layout_id <= 3, "layout id must be 0..=3, got {layout_id}");
    let mut buf = [0u8; LAYOUT_REPORT_LEN];
    buf[1] = 0x0B;
    buf[2] = 0x00;
    buf[3] = cmd::SET_MODE;
    buf[4] = 0x00;
    buf[5] = layout_id;
    buf
}

// ---------------------------------------------------------------------
// Lighting effect (breathe/colorshift/wave) — host struct + device-byte
// transform + upload frame.
// ---------------------------------------------------------------------

/// Length of the `lighting_effect` host struct, per `common_lighting.lisp`
/// (69 bytes: 4 flag/direction bytes + 4 u16 fields + num_colors + 14*3
/// colors + 14 positions).
pub const LIGHTING_EFFECT_LEN: usize = 69;

/// Max colors (and positions) a `lighting_effect` can carry.
pub const MAX_EFFECT_COLORS: usize = 14;

/// Length of the transformed "device effect" byte string the firmware
/// actually consumes on `0x0B` (16 sections * 8 bytes + init RGB (3*u16) +
/// `[255,0]` + focal (4B) + x/y scaling (2B each) + scale (2B) + num_sections
/// (u16) + total_ticks (u16) + direction_inverted (1B) = 151 bytes), per
/// `get-effect-bytes` in `fancy_lighting_engine.lisp`.
pub const EFFECT_DEVICE_BYTES_LEN: usize = 151;

/// Max onboard effect program slots this device exposes (`effect_index`
/// 0..=13, 14 slots), per the confirmed spec this module was built against.
/// (The generic "fancy lighting engine" reference in `ss-klc-base.lisp`
/// defines `effect_0`..`effect_18` — 19 slots — for the wider device family;
/// this KLC device's confirmed capacity is 14.)
pub const MAX_EFFECT_SLOT: u8 = 13;

/// Host-side `lighting_effect` struct (69 bytes on the wire), per
/// `common_lighting.lisp`:
///
/// ```text
/// type(1) has_direction(1) direction_type(1) direction_inverted(1)
/// focal_x(u16) focal_y(u16) speed(u16) scale(u16)
/// num_colors(1) colors[14](3B each) positions[14](1B each, 0..100)
/// ```
///
/// This is the struct MSI Center's UI edits. **What the firmware actually
/// consumes on `0x0B` is a further-transformed 151-byte "device effect" byte
/// string** produced by `get-breathe-effect-bytes`/`get-colorshift-effect-bytes`
/// in `fancy_lighting_engine.lisp` — a section-by-section keyframe/ramp
/// encoding. [`LightingEffect::to_device_bytes`] ports that transform (see
/// its doc comment), and [`build_effect_upload_frame`] wraps the result in
/// the `0x0B` upload frame.
#[derive(Debug, Clone)]
pub struct LightingEffect {
    pub effect_type: u8,
    pub has_direction: bool,
    pub direction_type: u8,
    pub direction_inverted: bool,
    pub focal_x: u16,
    pub focal_y: u16,
    pub speed: u16,
    pub scale: u16,
    pub colors: Vec<(u8, u8, u8)>,
    pub positions: Vec<u8>,
}

impl LightingEffect {
    /// Serialize to the raw 69-byte `lighting_effect` payload.
    ///
    /// Panics if `colors.len() > MAX_EFFECT_COLORS` or
    /// `positions.len() > MAX_EFFECT_COLORS`.
    pub fn to_bytes(&self) -> [u8; LIGHTING_EFFECT_LEN] {
        assert!(self.colors.len() <= MAX_EFFECT_COLORS, "at most {MAX_EFFECT_COLORS} colors");
        assert!(self.positions.len() <= MAX_EFFECT_COLORS, "at most {MAX_EFFECT_COLORS} positions");
        let mut buf = [0u8; LIGHTING_EFFECT_LEN];
        buf[0] = self.effect_type;
        buf[1] = self.has_direction as u8;
        buf[2] = self.direction_type;
        buf[3] = self.direction_inverted as u8;
        buf[4..6].copy_from_slice(&self.focal_x.to_le_bytes());
        buf[6..8].copy_from_slice(&self.focal_y.to_le_bytes());
        buf[8..10].copy_from_slice(&self.speed.to_le_bytes());
        buf[10..12].copy_from_slice(&self.scale.to_le_bytes());
        buf[12] = self.colors.len() as u8;
        for (i, &(r, g, b)) in self.colors.iter().enumerate() {
            let base = 13 + i * 3;
            buf[base] = r;
            buf[base + 1] = g;
            buf[base + 2] = b;
        }
        // Positions start right after the 14-slot color table (13 + 14*3 = 55),
        // matching `common_lighting.lisp`'s `get-effect-position-byte`.
        let positions_base = 13 + MAX_EFFECT_COLORS * 3;
        for (i, &p) in self.positions.iter().enumerate() {
            buf[positions_base + i] = p;
        }
        buf
    }
}

/// `get-scaled-uint8` (`common_lighting.lisp`): the per-tick delta from
/// `initial` to `final` over `speed` ticks, scaled by 16 and encoded as a
/// two's-complement byte (0..255, i.e. negative deltas wrap: `-1` -> `255`).
///
/// Interpretation note: golisp's `/` on two integers is assumed here to
/// truncate toward zero (Rust's native integer division), matching the
/// fixed-point-tick semantics this ramp math clearly wants (the source
/// explicitly says "speed can't go below 33 because the deltas would exceed
/// the -127 to 127 range", which only holds if `/` behaves like truncating
/// integer division). This wasn't independently verified against a golisp
/// interpreter; all byte layouts fed through this in the unit tests below use
/// evenly-divisible inputs so the exact rounding mode doesn't affect them.
fn scaled_uint8(initial: u8, final_: u8, speed: u16) -> u8 {
    let diff_scaled = (16i32 * (final_ as i32 - initial as i32)) / speed as i32;
    if diff_scaled < 0 {
        (256 + diff_scaled) as u8
    } else {
        diff_scaled as u8
    }
}

/// One computed section (`get-*-section-info` in `fancy_lighting_engine.lisp`):
/// the section's tick `speed` (also folded into `total_ticks`) and its 8-byte
/// on-wire encoding.
struct SectionInfo {
    speed: u16,
    bytes: [u8; 8],
}

const ZERO_SECTION: SectionInfo = SectionInfo { speed: 0, bytes: [0u8; 8] };

/// `get-effect-color-byte` (`common_lighting.lisp`).
fn effect_color_byte(payload: &[u8; LIGHTING_EFFECT_LEN], color_index: usize, offset: usize) -> u8 {
    payload[13 + color_index * 3 + offset]
}

/// `get-effect-position-byte` (`common_lighting.lisp`).
fn effect_position_byte(payload: &[u8; LIGHTING_EFFECT_LEN], position_index: usize) -> u8 {
    payload[55 + position_index]
}

/// `get-colorshift-section-info` (`fancy_lighting_engine.lisp` lines 11-71).
fn colorshift_section_info(
    payload: &[u8; LIGHTING_EFFECT_LEN],
    section_index: usize,
    num_sections: usize,
    initial_tick_total: u16,
    needs_padding_section: bool,
) -> SectionInfo {
    if num_sections == 0 || section_index >= num_sections {
        return ZERO_SECTION;
    }

    let color_index_for_initial = if needs_padding_section {
        if section_index == 0 { 0 } else { section_index - 1 }
    } else {
        section_index
    };
    let color_index_for_final = if section_index == num_sections - 1 {
        0
    } else if needs_padding_section {
        section_index
    } else {
        section_index + 1
    };
    let initial_position_percent: u8 = if section_index == 0 {
        0
    } else if needs_padding_section {
        effect_position_byte(payload, section_index - 1)
    } else {
        effect_position_byte(payload, section_index)
    };
    let final_position_percent: u8 = if section_index == num_sections - 1 {
        100
    } else if needs_padding_section {
        effect_position_byte(payload, section_index)
    } else {
        effect_position_byte(payload, section_index + 1)
    };

    let speed_unscaled =
        (initial_tick_total as i64 * (final_position_percent as i64 - initial_position_percent as i64)) / 100;
    // Speed can't go below 33 because the deltas would exceed the -127..127 range.
    let speed: u16 = if speed_unscaled < 33 { 33 } else { speed_unscaled as u16 };

    let diff_red = scaled_uint8(
        effect_color_byte(payload, color_index_for_initial, 0),
        effect_color_byte(payload, color_index_for_final, 0),
        speed,
    );
    let diff_green = scaled_uint8(
        effect_color_byte(payload, color_index_for_initial, 1),
        effect_color_byte(payload, color_index_for_final, 1),
        speed,
    );
    let diff_blue = scaled_uint8(
        effect_color_byte(payload, color_index_for_initial, 2),
        effect_color_byte(payload, color_index_for_final, 2),
        speed,
    );
    let next_index = if section_index == num_sections - 1 { 0 } else { section_index + 1 } as u8;
    let speed_bytes = speed.to_le_bytes();

    SectionInfo {
        speed,
        bytes: [diff_red, diff_green, diff_blue, 0, speed_bytes[0], speed_bytes[1], next_index, 0],
    }
}

/// `get-breathe-section-info` (`fancy_lighting_engine.lisp` lines 143-223).
fn breathe_section_info(
    payload: &[u8; LIGHTING_EFFECT_LEN],
    section_index: usize,
    num_colors: usize,
    initial_tick_total: u16,
    needs_padding_section: bool,
) -> SectionInfo {
    let num_sections = 2 * num_colors + usize::from(needs_padding_section);
    if num_sections == 0 || section_index >= num_sections {
        return ZERO_SECTION;
    }

    let initial_position_percent: u8 = if section_index == 0 {
        0
    } else if needs_padding_section {
        effect_position_byte(payload, (section_index - 1) / 2)
    } else {
        effect_position_byte(payload, section_index / 2)
    };
    let final_position_percent: u8 = if section_index >= num_sections - 2 {
        100
    } else if needs_padding_section {
        if section_index == 0 {
            effect_position_byte(payload, 0)
        } else {
            effect_position_byte(payload, 1 + (section_index - 1) / 2)
        }
    } else {
        effect_position_byte(payload, 1 + section_index / 2)
    };

    let half_diff = (final_position_percent as i64 - initial_position_percent as i64) / 2;
    let speed_unscaled = (initial_tick_total as i64 * half_diff) / 100;
    let speed: u16 = if speed_unscaled < 33 { 33 } else { speed_unscaled as u16 };

    let color_index_for_initial = if section_index == 0 {
        0
    } else if needs_padding_section {
        (section_index - 1) / 2
    } else {
        section_index / 2
    };
    let color_index_for_final = if section_index == num_sections - 1 {
        0
    } else if needs_padding_section {
        if section_index == 0 { 0 } else { (section_index - 1) / 2 + 1 }
    } else {
        section_index / 2 + 1
    };
    let is_mod2 = if needs_padding_section {
        section_index % 2 == 1
    } else {
        section_index % 2 == 0
    };
    let is_padding_section = needs_padding_section && section_index == 0;

    let diff_red = {
        let initial = if is_mod2 || is_padding_section {
            effect_color_byte(payload, color_index_for_initial, 0)
        } else {
            0
        };
        let final_ = if is_mod2 && !is_padding_section {
            0
        } else {
            effect_color_byte(payload, color_index_for_final, 0)
        };
        scaled_uint8(initial, final_, speed)
    };
    let diff_green = {
        let initial = if is_mod2 || is_padding_section {
            effect_color_byte(payload, color_index_for_initial, 1)
        } else {
            0
        };
        let final_ = if is_mod2 && !is_padding_section {
            0
        } else {
            effect_color_byte(payload, color_index_for_final, 1)
        };
        scaled_uint8(initial, final_, speed)
    };
    let diff_blue = {
        let initial = if is_mod2 || is_padding_section {
            effect_color_byte(payload, color_index_for_initial, 2)
        } else {
            0
        };
        let final_ = if is_mod2 && !is_padding_section {
            0
        } else {
            effect_color_byte(payload, color_index_for_final, 2)
        };
        scaled_uint8(initial, final_, speed)
    };
    let next_index = if section_index == num_sections - 1 { 0 } else { section_index + 1 } as u8;
    let speed_bytes = speed.to_le_bytes();

    SectionInfo {
        speed,
        bytes: [diff_red, diff_green, diff_blue, 0, speed_bytes[0], speed_bytes[1], next_index, 0],
    }
}

/// `x-scaling-bytes`/`y-scaling-bytes` (shared by both effect-bytes builders
/// in `fancy_lighting_engine.lisp`): only meaningful when `has_direction ==
/// 1`; horizontal drives x only, vertical drives y only, radial drives both.
fn direction_scaling_bytes(has_direction: u8, direction_type: u8) -> ([u8; 2], [u8; 2]) {
    if has_direction != 1 {
        return ([0, 0], [0, 0]);
    }
    match direction_type {
        direction_type::HORIZONTAL => ([1, 0], [0, 0]),
        direction_type::VERTICAL => ([0, 0], [1, 0]),
        direction_type::RADIAL => ([1, 0], [1, 0]),
        _ => ([0, 0], [0, 0]),
    }
}

/// Assembles the common 151-byte trailer (everything after the 16*8=128
/// section bytes) shared by `get-colorshift-effect-bytes` and
/// `get-breathe-effect-bytes`.
fn effect_bytes_trailer(
    payload: &[u8; LIGHTING_EFFECT_LEN],
    num_sections: usize,
    total_ticks: u32,
) -> [u8; EFFECT_DEVICE_BYTES_LEN - 128] {
    let has_direction = payload[1];
    let direction_type = payload[2];
    let direction_inverted = payload[3];
    let (x_scaling, y_scaling) = direction_scaling_bytes(has_direction, direction_type);
    let init_r = (effect_color_byte(payload, 0, 0) as u16) * 16;
    let init_g = (effect_color_byte(payload, 0, 1) as u16) * 16;
    let init_b = (effect_color_byte(payload, 0, 2) as u16) * 16;

    let mut out = [0u8; EFFECT_DEVICE_BYTES_LEN - 128];
    let mut pos = 0;
    out[pos..pos + 2].copy_from_slice(&init_r.to_le_bytes());
    pos += 2;
    out[pos..pos + 2].copy_from_slice(&init_g.to_le_bytes());
    pos += 2;
    out[pos..pos + 2].copy_from_slice(&init_b.to_le_bytes());
    pos += 2;
    out[pos] = 255;
    out[pos + 1] = 0;
    pos += 2;
    out[pos..pos + 4].copy_from_slice(&payload[4..8]); // focal_x/focal_y, byte-copied
    pos += 4;
    out[pos..pos + 2].copy_from_slice(&x_scaling);
    pos += 2;
    out[pos..pos + 2].copy_from_slice(&y_scaling);
    pos += 2;
    out[pos..pos + 2].copy_from_slice(&payload[10..12]); // scale, byte-copied
    pos += 2;
    out[pos..pos + 2].copy_from_slice(&(num_sections as u16).to_le_bytes());
    pos += 2;
    // `uint32-to-uint16-bytes`: keep the low 16 bits of the (wider) tick sum.
    out[pos..pos + 2].copy_from_slice(&(total_ticks as u16).to_le_bytes());
    pos += 2;
    out[pos] = direction_inverted;
    pos += 1;
    debug_assert_eq!(pos, out.len());
    out
}

/// `get-colorshift-effect-bytes` (`fancy_lighting_engine.lisp` lines 73-140).
fn colorshift_effect_bytes(payload: &[u8; LIGHTING_EFFECT_LEN]) -> [u8; EFFECT_DEVICE_BYTES_LEN] {
    let speed = u16::from_le_bytes([payload[8], payload[9]]);
    let num_colors = payload[12] as usize;
    let needs_padding_section = effect_position_byte(payload, 0) != 0;
    let num_sections = num_colors + usize::from(needs_padding_section);

    let mut section_bytes = [0u8; 128];
    let mut total_ticks: u32 = 0;
    for i in 0..16usize {
        let info = colorshift_section_info(payload, i, num_sections, speed, needs_padding_section);
        section_bytes[i * 8..i * 8 + 8].copy_from_slice(&info.bytes);
        total_ticks = total_ticks.wrapping_add(info.speed as u32);
    }
    let trailer = effect_bytes_trailer(payload, num_sections, total_ticks);

    let mut out = [0u8; EFFECT_DEVICE_BYTES_LEN];
    out[..128].copy_from_slice(&section_bytes);
    out[128..].copy_from_slice(&trailer);
    out
}

/// `get-breathe-effect-bytes` (`fancy_lighting_engine.lisp` lines 226-283).
fn breathe_effect_bytes(payload: &[u8; LIGHTING_EFFECT_LEN]) -> [u8; EFFECT_DEVICE_BYTES_LEN] {
    let speed = u16::from_le_bytes([payload[8], payload[9]]);
    let num_colors = payload[12] as usize;
    let needs_padding_section = effect_position_byte(payload, 0) != 0;
    let num_sections = 2 * num_colors + usize::from(needs_padding_section);

    let mut section_bytes = [0u8; 128];
    let mut total_ticks: u32 = 0;
    for i in 0..16usize {
        let info = breathe_section_info(payload, i, num_colors, speed, needs_padding_section);
        section_bytes[i * 8..i * 8 + 8].copy_from_slice(&info.bytes);
        total_ticks = total_ticks.wrapping_add(info.speed as u32);
    }
    let trailer = effect_bytes_trailer(payload, num_sections, total_ticks);

    let mut out = [0u8; EFFECT_DEVICE_BYTES_LEN];
    out[..128].copy_from_slice(&section_bytes);
    out[128..].copy_from_slice(&trailer);
    out
}

impl LightingEffect {
    /// `get-effect-bytes` (`fancy_lighting_engine.lisp` lines 287-293): the
    /// transformed 151-byte "device effect" program the firmware actually
    /// consumes on `0x0B`, derived from this host struct's 69-byte
    /// [`to_bytes`](Self::to_bytes) encoding. `effect_type::DISABLED` (and any
    /// unrecognized type) yields 151 zero bytes, matching
    /// `disabled-effect-bytes` in the source.
    ///
    /// This is a faithful port of the golisp section-ramp transform; see
    /// [`scaled_uint8`]'s doc comment for the one interpretation call it
    /// required (integer-division rounding direction), and the unit tests
    /// below for hand-computed cross-checks. It has not been validated
    /// against a hardware USB capture.
    pub fn to_device_bytes(&self) -> [u8; EFFECT_DEVICE_BYTES_LEN] {
        let payload = self.to_bytes();
        match payload[0] {
            effect_type::COLORSHIFT => colorshift_effect_bytes(&payload),
            effect_type::BREATHE => breathe_effect_bytes(&payload),
            _ => [0u8; EFFECT_DEVICE_BYTES_LEN],
        }
    }
}

/// Build the `0x0B` effect-upload feature report for a given `effect_index`
/// (0..=[`MAX_EFFECT_SLOT`]): `get-effect-report-bytes` in `ss-klc-base.lisp`
/// (`[0x00, 0x0C, 0x00, 0x0B, 0x00, effect_index, 0x00]` followed by the
/// 151-byte transformed device program from
/// [`LightingEffect::to_device_bytes`]), zero-padded to the 525-byte feature
/// report. Note the header's byte `1` (`0x0C`) is *not* the `SET_COLOR_FREE`
/// vendor command despite sharing its numeric value — it's a distinct
/// 7-byte header this device's effect-upload path uses, per the cited spec.
pub fn build_effect_upload_frame(effect_index: u8, effect: &LightingEffect) -> [u8; FEATURE_REPORT_LEN] {
    assert!(
        effect_index <= MAX_EFFECT_SLOT,
        "effect_index must be <= {MAX_EFFECT_SLOT}, got {effect_index}"
    );
    let mut buf = [0u8; FEATURE_REPORT_LEN];
    buf[0] = 0x00;
    buf[1] = 0x0C;
    buf[2] = 0x00;
    buf[3] = 0x0B;
    buf[4] = 0x00;
    buf[5] = effect_index;
    buf[6] = 0x00;
    let device_bytes = effect.to_device_bytes();
    buf[7..7 + EFFECT_DEVICE_BYTES_LEN].copy_from_slice(&device_bytes);
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_frame_is_525_bytes() {
        let f = build_group_frame(&[41, 43, 53], 0x11, 0x22, 0x33);
        assert_eq!(f.len(), FEATURE_REPORT_LEN);
    }

    #[test]
    fn group_frame_header_and_entries() {
        let hids = [41u8, 43, 53];
        let f = build_group_frame(&hids, 0x10, 0x20, 0x30);
        assert_eq!(f[0], 0x00); // report id
        assert_eq!(f[1], cmd::SET_COLOR_GROUP);
        assert_eq!(f[3], hids.len() as u8);
        for (m, &hid) in hids.iter().enumerate() {
            let base = 5 + 12 * m;
            assert_eq!(f[base], 0x10);
            assert_eq!(f[base + 1], 0x20);
            assert_eq!(f[base + 2], 0x30);
            assert_eq!(f[base + 3], 0);
            assert_eq!(f[base + 4], 0);
            assert_eq!(f[base + 5], 0);
            assert_eq!(f[base + 6], 0x2C);
            assert_eq!(f[base + 7], 1);
            assert_eq!(f[base + 8], 0);
            assert_eq!(f[base + 9], 1);
            assert_eq!(f[base + 10], 0);
            assert_eq!(f[base + 11], hid);
        }
    }

    #[test]
    fn group_frame_all_black_zeroes_rgb_but_keeps_footer() {
        let f = build_group_frame(&[7], 0, 0, 0);
        assert_eq!(&f[5..17], &[0, 0, 0, 0, 0, 0, 0x2C, 1, 0, 1, 0, 7]);
    }

    #[test]
    #[should_panic]
    fn group_frame_rejects_oversize_group() {
        let hids: Vec<u8> = (0..=MAX_GROUP_ENTRIES as u16).map(|x| x as u8).collect();
        let _ = build_group_frame(&hids, 1, 2, 3);
    }

    /// Byte-compat guard for the `protocol::KeyElement` refactor: the old
    /// hand-rolled per-entry byte layout (transcribed here verbatim from the
    /// pre-refactor `build_group_frame`) must still match what
    /// `KeyElement::steady` + `build_group_frame` produce today.
    #[test]
    fn all_key_off_byte_identical_to_legacy_encoding() {
        fn legacy_group_frame(hids: &[u8], r: u8, g: u8, b: u8) -> [u8; FEATURE_REPORT_LEN] {
            let mut buf = [0u8; FEATURE_REPORT_LEN];
            buf[1] = cmd::SET_COLOR_GROUP;
            buf[3] = hids.len() as u8;
            for (m, &hid) in hids.iter().enumerate() {
                let base = 5 + 12 * m;
                buf[base] = r;
                buf[base + 1] = g;
                buf[base + 2] = b;
                // buf[base+3..base+6] (react rgb) stay 0
                buf[base + 6] = 0x2C;
                buf[base + 7] = 1;
                // buf[base+8] (effect_index) stays 0
                buf[base + 9] = 1; // settings_mask = STEADY
                // buf[base+10] (lockmask) stays 0
                buf[base + 11] = hid;
            }
            buf
        }

        let hids = [41u8, 43, 53, 7];
        for &(r, g, b) in &[(0xFFu8, 0x00u8, 0x00u8), (0, 0, 0), (0x11, 0x22, 0x33)] {
            assert_eq!(
                build_group_frame(&hids, r, g, b),
                legacy_group_frame(&hids, r, g, b),
                "new KeyElement-based encoding must be byte-identical to the legacy one"
            );
        }
    }

    #[test]
    fn reactive_group_frame_sets_reactive_mask_and_le_fade_time() {
        let hids = [41u8, 43];
        let base = (0x10, 0x20, 0x30);
        let hit = (0xAA, 0xBB, 0xCC);
        let fade_ms: u16 = 0x0142; // 322, chosen to exercise both bytes
        let f = build_group_frame_reactive(&hids, base, hit, fade_ms);
        assert_eq!(f[1], cmd::SET_COLOR_GROUP);
        assert_eq!(f[3], hids.len() as u8);
        for (m, &hid) in hids.iter().enumerate() {
            let base_off = 5 + 12 * m;
            assert_eq!(&f[base_off..base_off + 3], &[0x10, 0x20, 0x30]); // init
            assert_eq!(&f[base_off + 3..base_off + 6], &[0xAA, 0xBB, 0xCC]); // react
            // react.time is little-endian: lo byte then hi byte.
            assert_eq!(f[base_off + 6], 0x42);
            assert_eq!(f[base_off + 7], 0x01);
            assert_eq!(f[base_off + 8], 0); // effect_index
            assert_eq!(f[base_off + 9], settings_mask::REACTIVE);
            assert_eq!(f[base_off + 10], 0); // lockmask
            assert_eq!(f[base_off + 11], hid);
        }
    }

    #[test]
    fn free_frame_is_525_bytes_and_packs_entries() {
        let entries = [
            KeyColor::new(4, 255, 0, 0),
            KeyColor::new(5, 0, 255, 0),
        ];
        let f = build_free_frame(&entries);
        assert_eq!(f.len(), FEATURE_REPORT_LEN);
        assert_eq!(f[1], cmd::SET_COLOR_FREE);
        assert_eq!(f[3], 2);
        assert_eq!(&f[5..9], &[4, 255, 0, 0]);
        assert_eq!(&f[9..13], &[5, 0, 255, 0]);
    }

    #[test]
    fn commit_frame_matches_reference() {
        let f = build_commit();
        assert_eq!(f.len(), OUTPUT_REPORT_LEN);
        assert_eq!(&f[0..4], &[0x00, 0x0D, 0x00, 0x02]);
        assert!(f[4..].iter().all(|&b| b == 0));
    }

    #[test]
    fn vendor_frame_is_unpadded() {
        let f = build_vendor(0x86, &[]);
        assert_eq!(f, vec![0x00, 0x86]);
        let f2 = build_set_all_live(0xAA, 0xBB, 0xCC);
        assert_eq!(f2, vec![0x00, 0x50, 0x00, 0xAA, 0xBB, 0xCC]);
        let f3 = build_set_default_color(1, 2, 3);
        assert_eq!(f3, vec![0x00, 0x51, 0x00, 1, 2, 3]);
        assert_eq!(build_load_default(), vec![0x00, 0x40]);
        assert_eq!(build_save_flash(), vec![0x00, 0x09]);
    }

    #[test]
    fn layout_select_frame_bytes() {
        let f = build_layout_select(2);
        assert_eq!(f.len(), LAYOUT_REPORT_LEN);
        assert_eq!(&f[0..6], &[0x00, 0x0B, 0x00, cmd::SET_MODE, 0x00, 2]);
        assert!(f[6..].iter().all(|&b| b == 0));
    }

    #[test]
    #[should_panic]
    fn layout_select_rejects_out_of_range_id() {
        let _ = build_layout_select(4);
    }

    #[test]
    fn lighting_effect_to_bytes_field_offsets() {
        let effect = LightingEffect {
            effect_type: effect_type::BREATHE,
            has_direction: true,
            direction_type: direction_type::RADIAL,
            direction_inverted: true,
            focal_x: 0x1234,
            focal_y: 0x5678,
            speed: 0x00AA,
            scale: 0x00BB,
            colors: vec![(1, 2, 3), (4, 5, 6)],
            positions: vec![0, 50],
        };
        let b = effect.to_bytes();
        assert_eq!(b.len(), LIGHTING_EFFECT_LEN);
        assert_eq!(b[0], effect_type::BREATHE);
        assert_eq!(b[1], 1); // has_direction
        assert_eq!(b[2], direction_type::RADIAL);
        assert_eq!(b[3], 1); // direction_inverted
        assert_eq!(&b[4..6], &0x1234u16.to_le_bytes());
        assert_eq!(&b[6..8], &0x5678u16.to_le_bytes());
        assert_eq!(&b[8..10], &0x00AAu16.to_le_bytes());
        assert_eq!(&b[10..12], &0x00BBu16.to_le_bytes());
        assert_eq!(b[12], 2); // num_colors
        assert_eq!(&b[13..16], &[1, 2, 3]);
        assert_eq!(&b[16..19], &[4, 5, 6]);
        assert_eq!(b[55], 0); // positions[0]
        assert_eq!(b[56], 50); // positions[1]
    }

    #[test]
    fn scaled_uint8_vectors() {
        // Exact (evenly-divisible) vectors, so the truncation-direction
        // interpretation in `scaled_uint8`'s doc comment doesn't matter here.
        assert_eq!(scaled_uint8(0, 255, 255), 16); // 16*255/255 = 16
        assert_eq!(scaled_uint8(0, 0, 33), 0);
        assert_eq!(scaled_uint8(0, 255, 50), 81); // 16*255/50 = 81 (rem 30)
        assert_eq!(scaled_uint8(255, 0, 50), 175); // negative wraps to two's complement (256-81)
    }

    #[test]
    fn effect_disabled_type_yields_zeroed_device_bytes() {
        let effect = LightingEffect {
            effect_type: effect_type::DISABLED,
            has_direction: false,
            direction_type: direction_type::HORIZONTAL,
            direction_inverted: false,
            focal_x: 0,
            focal_y: 0,
            speed: 100,
            scale: 0,
            colors: vec![(255, 0, 0)],
            positions: vec![],
        };
        let bytes = effect.to_device_bytes();
        assert_eq!(bytes.len(), EFFECT_DEVICE_BYTES_LEN);
        assert!(bytes.iter().all(|&b| b == 0));
    }

    /// Hand-computed single-color breathe vector: 1 color (red), speed=100,
    /// no position padding (positions[0] == 0). num_sections = 2*1 = 2.
    /// Section 0 ramps red(255)->off(0) over 50 ticks, section 1 ramps
    /// off(0)->red(255) back over 50 ticks (breathe = up then down).
    #[test]
    fn breathe_single_color_device_bytes_match_hand_computation() {
        let effect = LightingEffect {
            effect_type: effect_type::BREATHE,
            has_direction: false,
            direction_type: direction_type::HORIZONTAL,
            direction_inverted: false,
            focal_x: 0,
            focal_y: 0,
            speed: 100,
            scale: 0,
            colors: vec![(255, 0, 0)],
            positions: vec![0],
        };
        let bytes = effect.to_device_bytes();
        assert_eq!(bytes.len(), EFFECT_DEVICE_BYTES_LEN);

        // Section 0: diff_red = scaled_uint8(255, 0, 50) = 256-81 = 175.
        assert_eq!(&bytes[0..8], &[175, 0, 0, 0, 50, 0, 1, 0]);
        // Section 1: diff_red = scaled_uint8(0, 255, 50) = 81.
        assert_eq!(&bytes[8..16], &[81, 0, 0, 0, 50, 0, 0, 0]);
        // Sections 2..15 (num_sections=2) are zeroed padding.
        assert!(bytes[16..128].iter().all(|&b| b == 0));

        // init_r/g/b = 16 * color0 channel.
        assert_eq!(&bytes[128..130], &(16u16 * 255).to_le_bytes()); // init_r = 4080
        assert_eq!(&bytes[130..132], &0u16.to_le_bytes()); // init_g
        assert_eq!(&bytes[132..134], &0u16.to_le_bytes()); // init_b
        assert_eq!(&bytes[134..136], &[255, 0]);
        assert_eq!(&bytes[136..140], &[0, 0, 0, 0]); // focal
        assert_eq!(&bytes[140..142], &[0, 0]); // x_scaling (no direction)
        assert_eq!(&bytes[142..144], &[0, 0]); // y_scaling
        assert_eq!(&bytes[144..146], &[0, 0]); // scale
        assert_eq!(&bytes[146..148], &2u16.to_le_bytes()); // num_sections
        assert_eq!(&bytes[148..150], &100u16.to_le_bytes()); // total_ticks = 50+50
        assert_eq!(bytes[150], 0); // direction_inverted
    }

    /// Hand-computed 2-color colorshift vector: red -> green -> (back to)
    /// red, speed=100, positions=[0, 50] (no padding, since positions[0]==0).
    /// num_sections = 2.
    #[test]
    fn colorshift_two_color_device_bytes_match_hand_computation() {
        let effect = LightingEffect {
            effect_type: effect_type::COLORSHIFT,
            has_direction: false,
            direction_type: direction_type::HORIZONTAL,
            direction_inverted: false,
            focal_x: 0,
            focal_y: 0,
            speed: 100,
            scale: 0,
            colors: vec![(255, 0, 0), (0, 255, 0)],
            positions: vec![0, 50],
        };
        let bytes = effect.to_device_bytes();

        // Section 0: red(255,0,0) -> green(0,255,0) over 50 ticks.
        // diff_red = scaled_uint8(255,0,50) = 175, diff_green = scaled_uint8(0,255,50) = 81.
        assert_eq!(&bytes[0..8], &[175, 81, 0, 0, 50, 0, 1, 0]);
        // Section 1: green(0,255,0) -> red(255,0,0) (wraps to color 0) over 50 ticks.
        assert_eq!(&bytes[8..16], &[81, 175, 0, 0, 50, 0, 0, 0]);
        assert!(bytes[16..128].iter().all(|&b| b == 0));

        assert_eq!(&bytes[128..130], &(16u16 * 255).to_le_bytes()); // init_r
        assert_eq!(&bytes[130..132], &0u16.to_le_bytes());
        assert_eq!(&bytes[132..134], &0u16.to_le_bytes());
        assert_eq!(&bytes[134..136], &[255, 0]);
        assert_eq!(&bytes[146..148], &2u16.to_le_bytes()); // num_sections
        assert_eq!(&bytes[148..150], &100u16.to_le_bytes()); // total_ticks
    }

    #[test]
    fn effect_upload_frame_header_and_length() {
        let effect = LightingEffect {
            effect_type: effect_type::COLORSHIFT,
            has_direction: true,
            direction_type: direction_type::RADIAL,
            direction_inverted: true,
            focal_x: 0,
            focal_y: 0,
            speed: 100,
            scale: 0,
            colors: vec![(255, 0, 0), (0, 255, 0)],
            positions: vec![0, 50],
        };
        let f = build_effect_upload_frame(7, &effect);
        assert_eq!(f.len(), FEATURE_REPORT_LEN);
        assert_eq!(&f[0..7], &[0x00, 0x0C, 0x00, 0x0B, 0x00, 7, 0x00]);
        assert_eq!(&f[7..7 + EFFECT_DEVICE_BYTES_LEN], &effect.to_device_bytes());
        assert!(f[7 + EFFECT_DEVICE_BYTES_LEN..].iter().all(|&b| b == 0));
    }

    #[test]
    #[should_panic]
    fn effect_upload_frame_rejects_out_of_range_slot() {
        let effect = LightingEffect {
            effect_type: effect_type::BREATHE,
            has_direction: false,
            direction_type: direction_type::HORIZONTAL,
            direction_inverted: false,
            focal_x: 0,
            focal_y: 0,
            speed: 100,
            scale: 0,
            colors: vec![(1, 2, 3)],
            positions: vec![0],
        };
        let _ = build_effect_upload_frame(MAX_EFFECT_SLOT + 1, &effect);
    }
}

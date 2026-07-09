//! `msi-klc` — control MSI SteelSeries "KLC" per-key RGB laptop keyboards
//! (USB-HID `1038:113a` and the wider KLC family) via Linux `/dev/hidrawN`.
//!
//! Protocol details are transcribed from a hardware-validated Python
//! reference (see the `linux-msi-ms16v5/keyboard-rgb` RE project:
//! `KLC-PROTOCOL.md`, `msi-nb-rgb.py`, `klc-cmd.py`). Default operation is
//! RAM-only (wear-free); flash writes require `persist --i-understand-flash-risk`.

mod daemon;
mod device;
mod layout;
mod models;
mod protocol;
mod state;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

use layout::{Layout, parse_hex_color};
use models::ModelTable;
use protocol::KeyColor;

/// The controller drops feature reports sent back-to-back too fast; the
/// reference sleeps 10ms between the six group reports (`msi-nb-rgb.py`'s
/// `SEND_DELAY`).
const SEND_DELAY: Duration = Duration::from_millis(10);

#[derive(Parser)]
#[command(
    name = "msi-klc",
    version,
    about = "Control MSI SteelSeries KLC per-key RGB laptop keyboards via hidraw"
)]
struct Cli {
    /// Explicit hidraw device path (skips auto-detection), e.g. /dev/hidraw1
    #[arg(long, global = true)]
    path: Option<PathBuf>,

    /// Override vid:pid match in hex, e.g. 1038:113a. Default: auto-detect
    /// across every KLC PID in the embedded model table (`msi-klc models`).
    #[arg(long, global = true, value_name = "VID:PID")]
    id: Option<String>,

    /// Keymap to resolve CLK_* key names against (see msi-layouts.json)
    #[arg(long, global = true, default_value = layout::DEFAULT_KEYMAP)]
    keymap: String,

    /// Send colors unscaled, skipping the detected model's `color_scale`
    /// correction (see `models` and the README's "Color-scale correction"
    /// section). Applies to `all`, `key`, `recover`, and `reactive`.
    #[arg(long, global = true)]
    raw: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the embedded per-model KLC device table: name, USB id,
    /// color-scale correction, key count, and #key-coordinates known.
    Models,

    /// Set all keys to one color (6-group 0x0E), then commit. RAM-only.
    All { color: String },

    /// Turn all keys off (black).
    Off,

    /// Set individual keys: NAME=RRGGBB pairs (NAME = CLK_* or a decimal/hex HID code).
    Key {
        #[arg(required = true)]
        pairs: Vec<String>,
    },

    /// Set every key to a keypress-trail effect: rests at BASE_RGB, flashes
    /// to HIT_RGB on keypress, and fades back over `--fade` ms
    /// (settings_mask=REACTIVE). This reproduces the onboard "reactive"
    /// effect. Run `all` or `off` to return keys to steady mode.
    Reactive {
        /// Rest color, RRGGBB.
        base: String,
        /// Keypress-flash color, RRGGBB.
        hit: String,
        /// Reactive fade-back time in milliseconds (device range 0..2000).
        #[arg(long, default_value_t = 300)]
        fade: u16,
    },

    /// Select the active keyboard layout/layer (vendor command 0x06).
    Layout {
        /// Layout id, 0..=3.
        #[arg(value_parser = clap::value_parser!(u8).range(0..=3))]
        id: u8,
    },

    /// Get or set software-owned keyboard brightness.
    ///
    /// With no VALUE: read back mode+brightness (vendor GET 0x86, best-effort
    /// decode). With a VALUE (`0..=255`, or a percentage like `50%`): store
    /// it, re-apply the last logical frame folded at that brightness (see
    /// `state.rs` — brightness is a software multiplier on the sent RGB, the
    /// firmware/EC brightness path being unreliable on this hardware), and, if
    /// the `daemon` is running, mirror the raw value into the LED sysfs so the
    /// desktop OSD stays in sync.
    Brightness {
        /// New brightness: `0..=255` or a percentage like `50%`. Omit to read.
        value: Option<String>,
    },

    /// Run the DE-OSD bridge daemon: register a `msiklc::kbd_backlight`
    /// userspace LED via the `uleds` kernel module, then apply each brightness
    /// the desktop sets (GNOME/KDE keyboard-backlight OSD, via UPower). Needs
    /// root and `modprobe uleds`. See `packaging/` — the systemd unit must be
    /// ordered `Before=upower.service` (UPower only enumerates the LED at its
    /// own startup).
    Daemon {
        /// Advertised uleds `max_brightness` (received values are still
        /// clamped to 0..=255 internally). Default 255.
        #[arg(long, default_value_t = 255)]
        max_steps: u32,
    },

    /// Read back device id (0x10), STM32 UID (0x80), and firmware CRC
    /// (0x15), decoded.
    Info,

    /// Send a vendor GET command and print the raw 64-byte reply.
    /// (0x86 mode+brightness, 0x10/0x80 device info, 0x15 fw CRC, 0x22 status.)
    /// Prefer `brightness`/`info` for the common cases below.
    Query {
        #[arg(required = true, help = "hex command byte, then optional hex params")]
        bytes: Vec<String>,
    },

    /// Un-brick sequence: load_default (0x40) then set-all live (0x50).
    /// Fixes the "everything black" state caused by the brightness gate
    /// getting stuck at 0.
    Recover {
        /// Color to set-all to after loading defaults (default: white)
        #[arg(default_value = "FFFFFF")]
        color: String,
    },

    /// Advanced escape hatch: send an arbitrary output-report vendor command
    /// with no safety net. Prefer the named subcommands above (`all`, `key`,
    /// `off`, `reactive`, `effect`, `layout`, `recover`, `persist`) — they
    /// cover every validated/experimental code path this tool knows about.
    #[command(hide = true)]
    Raw {
        #[arg(required = true, help = "hex command byte, then optional hex params")]
        bytes: Vec<String>,
    },

    /// Onboard effect programming: breathe/colorshift/wave, or off. Uploads
    /// the transformed 151-byte device effect program (ported from
    /// `get-breathe-effect-bytes`/`get-colorshift-effect-bytes` in
    /// `fancy_lighting_engine.lisp`), then points the assigned keys at it.
    Effect {
        /// Effect mode.
        #[arg(value_enum)]
        mode: EffectMode,

        /// Colors for the effect (hex RRGGBB), up to 14. Ignored for `off`.
        colors: Vec<String>,

        /// Effect speed (device tick units; see common_lighting.lisp).
        #[arg(long, default_value_t = 100)]
        speed: u16,

        /// Wave propagation direction; enables `has_direction`. Omit for a
        /// plain breathe/colorshift with no directional wave overlay.
        /// Always enabled (defaulting to horizontal) for `wave`.
        #[arg(long, value_enum)]
        dir: Option<Direction>,

        /// Effect program slot.
        #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u8).range(0..=protocol::MAX_EFFECT_SLOT as i64))]
        slot: u8,

        /// Explicit keyframe positions (0..=100), one per color, in the same
        /// order as `colors`. Defaults to evenly-spaced positions starting
        /// at 0 (`i*100/n`). A non-zero first position adds an implicit
        /// padding section, per `common_lighting.lisp`'s
        /// `needs-padding-section` — an advanced knob most callers can leave
        /// unset.
        #[arg(long, value_delimiter = ',', num_args = 0..)]
        positions: Vec<u8>,
    },

    /// Set the flash-persisted global default color (vendor 0x51 + save 0x09).
    /// WRITES FLASH. Requires --i-understand-flash-risk.
    Persist {
        color: String,

        /// Required acknowledgement: this writes flash, and wrong bytes have
        /// bricked the LED display before (GS66 firmware differs from the
        /// RE reference this tool is built against).
        #[arg(long, required = true)]
        i_understand_flash_risk: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectMode {
    Breathe,
    Colorshift,
    Wave,
    Off,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    H,
    V,
    Radial,
}

impl Direction {
    fn to_device(self) -> u8 {
        match self {
            Direction::H => protocol::direction_type::HORIZONTAL,
            Direction::V => protocol::direction_type::VERTICAL,
            Direction::Radial => protocol::direction_type::RADIAL,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let layout = Layout::load()?;
    let table = ModelTable::load()?;

    match &cli.command {
        Command::Models => return cmd_models(&table),
        Command::Query { bytes } => {
            // Read-only vendor GET: fine to run before opening for write,
            // but we still need read+write access to the hidraw node.
            return cmd_query(&cli, &table, bytes);
        }
        Command::Raw { bytes } => return cmd_raw(&cli, &table, bytes),
        Command::Brightness { value } => return cmd_brightness(&cli, &table, &layout, value.as_deref()),
        Command::Daemon { max_steps } => return cmd_daemon(&cli, &table, &layout, *max_steps),
        Command::Info => return cmd_info(&cli, &table),
        Command::Layout { id } => return cmd_layout(&cli, &table, *id),
        _ => {}
    }

    // Validate all user input up front, before we ever touch the hidraw
    // node — a typo in a color or key name should fail fast with a parse
    // error, not get masked by a permission/open error from the device.
    validate_command(&cli.command, &layout, &cli.keymap)?;

    let (path, color_scale) = resolve_and_report(&cli, &table)?;
    let dev = device::Device::open(&path)?;

    match &cli.command {
        // Recovery / flash-persist are one-shots that don't participate in the
        // brightness/frame state model (recovery must not be dimmed to black;
        // persist writes a flash default color verbatim).
        Command::Recover { color } => {
            // `--raw` disables color-scale correction here too.
            let scale = if cli.raw { [1.0, 1.0, 1.0] } else { color_scale };
            cmd_recover(&dev, color, scale)?;
        }
        Command::Persist {
            color,
            i_understand_flash_risk,
        } => cmd_persist(&dev, color, *i_understand_flash_risk)?,
        // Every other apply command (`all`/`off`/`key`/`reactive`/`effect`)
        // goes through the shared frame model: capture the pre-brightness
        // logical frame, apply it folded at the current stored brightness, and
        // persist frame+brightness so `brightness`/`daemon` can re-apply it.
        _ => {
            let frame = command_to_frame(&cli.command, &layout, &cli.keymap)?;
            let mut st = state::load();
            // `--raw` disables BOTH color-scale correction and software
            // brightness scaling (a diagnostic one-shot). The frame is still
            // persisted with the current brightness, so a later
            // `brightness`/`daemon` re-apply renders it software-scaled.
            let scale = if cli.raw {
                [1.0, 1.0, 1.0]
            } else {
                state::fold_brightness(color_scale, st.brightness)
            };
            apply_frame(&dev, &layout, &frame, scale)?;
            if cli.raw {
                eprintln!("msi-klc: applied raw (unscaled); stored frame will re-apply software-scaled");
            } else {
                eprintln!("msi-klc: applied at brightness {} (software-scaled)", st.brightness);
            }
            st.frame = Some(frame);
            state::save(&st)?;
        }
    }

    Ok(())
}

/// Resolve the hidraw device path and report which model (if any) was
/// detected, returning the per-channel `color_scale` correction to use
/// (identity `[1,1,1]` if the model is unknown). Shared by the apply path,
/// `brightness`, and `daemon`.
fn resolve_and_report(cli: &Cli, table: &ModelTable) -> Result<(PathBuf, [f32; 3])> {
    let (path, vid_pid) = resolve_device_path(cli, table)?;
    let model = vid_pid.and_then(|(vid, pid)| table.find(vid, pid).cloned());
    match &model {
        Some(m) => eprintln!(
            "msi-klc: detected model {} ({:04x}:{:04x}), color_scale {:?}",
            m.name, m.vid, m.pid, m.color_scale
        ),
        None => eprintln!(
            "msi-klc: unknown model{} — color-scale correction disabled (scale 1.0); pass \
             --id VID:PID if you know it, or see `msi-klc models` for the known table",
            vid_pid.map(|(v, p)| format!(" ({v:04x}:{p:04x})")).unwrap_or_default()
        ),
    }
    let color_scale = model.as_ref().map_or([1.0, 1.0, 1.0], |m| m.color_scale);
    Ok((path, color_scale))
}

/// Build the pre-brightness logical [`state::Frame`] for an apply command
/// (`all`/`off`/`key`/`reactive`/`effect`). Raw user colors and resolved key
/// ids are captured so re-applying at a new brightness needs no keymap.
fn command_to_frame(command: &Command, layout: &Layout, keymap: &str) -> Result<state::Frame> {
    use state::Frame;
    let frame = match command {
        Command::All { color } => Frame::All {
            color: rgb_arr(parse_hex_color(color)?),
        },
        Command::Off => Frame::Off,
        Command::Key { pairs } => {
            let mut out = Vec::with_capacity(pairs.len());
            for pair in pairs {
                let (name, col) = pair
                    .split_once('=')
                    .with_context(|| format!("expected NAME=RRGGBB, got {pair:?}"))?;
                let hid = layout.resolve_key(keymap, name)?;
                out.push((hid, rgb_arr(parse_hex_color(col)?)));
            }
            Frame::Keys { pairs: out }
        }
        Command::Reactive { base, hit, fade } => Frame::Reactive {
            base: rgb_arr(parse_hex_color(base)?),
            hit: rgb_arr(parse_hex_color(hit)?),
            fade: *fade,
        },
        Command::Effect {
            mode,
            colors,
            speed,
            dir,
            slot,
            positions,
        } => {
            let parsed = colors
                .iter()
                .map(|c| Ok(rgb_arr(parse_hex_color(c)?)))
                .collect::<Result<Vec<[u8; 3]>>>()?;
            // Default to evenly-spaced keyframe positions (mirrors cmd_effect).
            let positions = if positions.is_empty() {
                let n = parsed.len().max(1);
                (0..parsed.len()).map(|i| ((i * 100) / n) as u8).collect()
            } else {
                positions.clone()
            };
            Frame::Effect {
                mode: *mode,
                colors: parsed,
                speed: *speed,
                dir: *dir,
                slot: *slot,
                positions,
            }
        }
        _ => unreachable!("command_to_frame only handles applyable frame commands"),
    };
    Ok(frame)
}

fn rgb_arr((r, g, b): (u8, u8, u8)) -> [u8; 3] {
    [r, g, b]
}

fn rgb_tuple([r, g, b]: [u8; 3]) -> (u8, u8, u8) {
    (r, g, b)
}

/// The single shared "render a logical frame at an effective scale" path,
/// used by the initial apply, `brightness <VALUE>`, and the `daemon`. `scale`
/// is the already-folded `color_scale * brightness/255` (or `[1,1,1]` for a
/// `--raw` one-shot).
pub fn apply_frame(
    dev: &device::Device,
    layout: &Layout,
    frame: &state::Frame,
    scale: [f32; 3],
) -> Result<()> {
    use state::Frame;
    match frame {
        Frame::All { color } => apply_solid(dev, layout, rgb_tuple(*color), scale)?,
        Frame::Off => apply_solid(dev, layout, (0, 0, 0), scale)?,
        Frame::Keys { pairs } => apply_keys(dev, layout, pairs, scale)?,
        Frame::Reactive { base, hit, fade } => {
            apply_reactive(dev, layout, rgb_tuple(*base), rgb_tuple(*hit), *fade, scale)?
        }
        Frame::Effect { .. } => apply_effect(dev, layout, frame, scale)?,
    }
    Ok(())
}

/// Pre-flight validation of the parsed command's arguments (colors, key
/// names), independent of hardware access. Called before we open the hidraw
/// device so input mistakes surface as clean parse errors.
fn validate_command(command: &Command, layout: &Layout, keymap: &str) -> Result<()> {
    match command {
        Command::Models => {}
        Command::All { color } => {
            parse_hex_color(color)?;
        }
        Command::Off => {}
        Command::Key { pairs } => {
            for pair in pairs {
                let (name, col) = pair
                    .split_once('=')
                    .with_context(|| format!("expected NAME=RRGGBB, got {pair:?}"))?;
                layout.resolve_key(keymap, name)?;
                parse_hex_color(col)?;
            }
        }
        Command::Recover { color } => {
            parse_hex_color(color)?;
        }
        Command::Persist { color, .. } => {
            parse_hex_color(color)?;
        }
        Command::Reactive { base, hit, .. } => {
            parse_hex_color(base)?;
            parse_hex_color(hit)?;
        }
        Command::Effect { mode, colors, positions, .. } => {
            if *mode == EffectMode::Off {
                // `off` ignores any (unlikely) colors passed alongside it.
            } else {
                if colors.is_empty() {
                    bail!("effect {mode:?} needs at least one color (RRGGBB); `effect off` needs none");
                }
                if colors.len() > protocol::MAX_EFFECT_COLORS {
                    bail!(
                        "effect takes at most {} colors, got {}",
                        protocol::MAX_EFFECT_COLORS,
                        colors.len()
                    );
                }
                for c in colors {
                    parse_hex_color(c)?;
                }
                if !positions.is_empty() {
                    if positions.len() != colors.len() {
                        bail!(
                            "--positions must have one entry per color: got {} positions for {} colors",
                            positions.len(),
                            colors.len()
                        );
                    }
                    for &p in positions {
                        if p > 100 {
                            bail!("--positions entries must be 0..=100, got {p}");
                        }
                    }
                }
            }
        }
        Command::Query { .. }
        | Command::Raw { .. }
        | Command::Brightness { .. }
        | Command::Daemon { .. }
        | Command::Info
        | Command::Layout { .. } => {}
    }
    Ok(())
}

/// Resolve the hidraw device path for this invocation, plus the `(vid,
/// pid)` to look the model up with in the embedded table (if resolvable).
///
/// - `--path` given: use it as-is; still try to resolve `(vid, pid)` from
///   `--id` if also given, otherwise by probing the path's sysfs `uevent`
///   (best-effort — `None` if that fails, meaning "model unknown").
/// - `--id` given (no `--path`): auto-detect restricted to that exact PID.
/// - neither given: auto-detect across every KLC PID in the embedded model
///   table (not just the original `0x113a`), so any recognized model is
///   found without needing `--id`.
fn resolve_device_path(cli: &Cli, table: &ModelTable) -> Result<(PathBuf, Option<(u16, u16)>)> {
    if let Some(p) = &cli.path {
        let vid_pid = match &cli.id {
            Some(id) => Some(parse_vid_pid(id)?),
            None => device::probe_vid_pid(p),
        };
        return Ok((p.clone(), vid_pid));
    }
    match &cli.id {
        Some(id) => {
            let (vid, pid) = parse_vid_pid(id)?;
            let (path, found_pid) = device::find_device(vid, Some(&[pid]))?;
            Ok((path, Some((vid, found_pid))))
        }
        None => {
            let pids = table.msi_pids();
            let (path, found_pid) = device::find_device(device::VID_MSI, Some(&pids))?;
            Ok((path, Some((device::VID_MSI, found_pid))))
        }
    }
}

fn parse_vid_pid(s: &str) -> Result<(u16, u16)> {
    let (v, p) = s
        .split_once(':')
        .with_context(|| format!("--id must be VID:PID in hex, e.g. 1038:113a (got {s:?})"))?;
    let vid = u16::from_str_radix(v.trim_start_matches("0x").trim_start_matches("0X"), 16)
        .with_context(|| format!("invalid VID {v:?}"))?;
    let pid = u16::from_str_radix(p.trim_start_matches("0x").trim_start_matches("0X"), 16)
        .with_context(|| format!("invalid PID {p:?}"))?;
    Ok((vid, pid))
}

fn parse_hex_byte(tok: &str) -> Result<u8> {
    let t = tok.trim_start_matches("0x").trim_start_matches("0X");
    u8::from_str_radix(t, 16).with_context(|| format!("invalid hex byte {tok:?}"))
}

/// Apply a solid color to every key (backs `all`/`off`). `rgb` is the raw
/// pre-scale color; `scale` is the effective color_scale*brightness fold.
fn apply_solid(dev: &device::Device, layout: &Layout, rgb: (u8, u8, u8), scale: [f32; 3]) -> Result<()> {
    let (r, g, b) = models::scale_rgb(scale, rgb);
    let groups = layout.groups()?;
    for group in &groups {
        if group.is_empty() {
            continue;
        }
        let frame = protocol::build_group_frame(group, r, g, b);
        dev.send_feature(&frame)?;
        sleep(SEND_DELAY);
    }
    dev.write_output(&protocol::build_commit())?;
    Ok(())
}

/// Apply per-key colors (backs `key`). `pairs` are already-resolved
/// `(hid, raw_rgb)`; `scale` is the effective color_scale*brightness fold.
fn apply_keys(
    dev: &device::Device,
    _layout: &Layout,
    pairs: &[(u8, [u8; 3])],
    scale: [f32; 3],
) -> Result<()> {
    let mut entries = Vec::with_capacity(pairs.len());
    for &(hid, rgb) in pairs {
        let (r, g, b) = models::scale_rgb(scale, rgb_tuple(rgb));
        entries.push(KeyColor::new(hid, r, g, b));
    }
    for chunk in entries.chunks(protocol::MAX_FREE_ENTRIES) {
        let frame = protocol::build_free_frame(chunk);
        dev.send_feature(&frame)?;
        sleep(SEND_DELAY);
    }
    dev.write_output(&protocol::build_commit())?;
    Ok(())
}

fn cmd_query(cli: &Cli, table: &ModelTable, bytes: &[String]) -> Result<()> {
    let (path, _) = resolve_device_path(cli, table)?;
    let dev = device::Device::open(&path)?;
    let cmd = parse_hex_byte(&bytes[0])?;
    let params = bytes[1..]
        .iter()
        .map(|s| parse_hex_byte(s))
        .collect::<Result<Vec<u8>>>()?;
    dev.drain_input()?;
    let frame = protocol::build_vendor(cmd, &params);
    dev.write_output(&frame)?;
    println!("sent OUTPUT: {}", hexdump(&frame));
    match dev.read_input(Duration::from_millis(500))? {
        Some(reply) => println!("reply INPUT ({}B): {}", reply.len(), hexdump(&reply)),
        None => println!("no reply within 500ms"),
    }
    Ok(())
}

fn cmd_raw(cli: &Cli, table: &ModelTable, bytes: &[String]) -> Result<()> {
    let (path, _) = resolve_device_path(cli, table)?;
    let dev = device::Device::open(&path)?;
    let cmd = parse_hex_byte(&bytes[0])?;
    let params = bytes[1..]
        .iter()
        .map(|s| parse_hex_byte(s))
        .collect::<Result<Vec<u8>>>()?;
    let frame = protocol::build_vendor(cmd, &params);
    dev.write_output(&frame)?;
    println!("sent OUTPUT: {}", hexdump(&frame));
    Ok(())
}

fn cmd_recover(dev: &device::Device, color: &str, scale: [f32; 3]) -> Result<()> {
    let (r, g, b) = models::scale_rgb(scale, parse_hex_color(color)?);
    eprintln!(
        "msi-klc: running recovery sequence (load_default 0x40, then set-all-live 0x50 -> #{color})"
    );
    dev.write_output(&protocol::build_load_default())?;
    sleep(SEND_DELAY);
    dev.write_output(&protocol::build_set_all_live(r, g, b))?;
    Ok(())
}

fn cmd_persist(dev: &device::Device, color: &str, ack: bool) -> Result<()> {
    if !ack {
        // clap's `required = true` on the flag already enforces this, but we
        // double-check here so the warning is unmissable regardless of how
        // this function is called.
        bail!(
            "refusing to write flash without --i-understand-flash-risk (see `msi-klc persist --help`)"
        );
    }
    let (r, g, b) = parse_hex_color(color)?;
    eprintln!(
        "msi-klc: WARNING — writing flash-persisted default color #{color}. This is a real \
         flash write to the keyboard controller. Wrong bytes have bricked the LED display \
         before; the GS66 runs different firmware than the reference this tool was built \
         against. Proceeding in 2s (Ctrl-C to abort)..."
    );
    sleep(Duration::from_secs(2));
    dev.write_output(&protocol::build_set_default_color(r, g, b))?;
    sleep(SEND_DELAY);
    dev.write_output(&protocol::build_save_flash())?;
    eprintln!("msi-klc: save-to-flash command sent.");
    Ok(())
}

fn apply_reactive(
    dev: &device::Device,
    layout: &Layout,
    base: (u8, u8, u8),
    hit: (u8, u8, u8),
    fade_ms: u16,
    scale: [f32; 3],
) -> Result<()> {
    let base_rgb = models::scale_rgb(scale, base);
    let hit_rgb = models::scale_rgb(scale, hit);
    let groups = layout.groups()?;
    for group in &groups {
        if group.is_empty() {
            continue;
        }
        let frame = protocol::build_group_frame_reactive(group, base_rgb, hit_rgb, fade_ms);
        dev.send_feature(&frame)?;
        sleep(SEND_DELAY);
    }
    dev.write_output(&protocol::build_commit())?;
    Ok(())
}

fn cmd_layout(cli: &Cli, table: &ModelTable, id: u8) -> Result<()> {
    let (path, _) = resolve_device_path(cli, table)?;
    let dev = device::Device::open(&path)?;
    let frame = protocol::build_layout_select(id);
    dev.write_output(&frame)?;
    eprintln!("msi-klc: sent layout/layer select (id={id})");
    Ok(())
}

/// Dispatch `brightness`: no VALUE reads back mode+brightness; a VALUE sets
/// software brightness and re-applies the stored frame.
fn cmd_brightness(cli: &Cli, table: &ModelTable, layout: &Layout, value: Option<&str>) -> Result<()> {
    match value {
        None => cmd_brightness_get(cli, table),
        Some(v) => cmd_brightness_set(cli, table, layout, v),
    }
}

/// Set software brightness: persist it, re-apply the stored frame at the new
/// brightness via the shared [`daemon::apply_brightness`] path, and mirror the
/// raw value into the LED sysfs (if the daemon is running) so the DE OSD stays
/// in sync. `--raw` has no effect here — brightness *is* the software-scaling
/// feature. If no frame has been applied yet, just persist the brightness.
fn cmd_brightness_set(cli: &Cli, table: &ModelTable, layout: &Layout, value: &str) -> Result<()> {
    let brightness = state::parse_brightness(value)?;
    if state::load().frame.is_some() {
        let (path, color_scale) = resolve_and_report(cli, table)?;
        let dev = device::Device::open(&path)?;
        daemon::apply_brightness(&dev, layout, color_scale, brightness)?;
        eprintln!("msi-klc: brightness set to {brightness}; re-applied stored frame");
    } else {
        let mut st = state::load();
        st.brightness = brightness;
        state::save(&st)?;
        eprintln!("msi-klc: brightness set to {brightness} (no stored frame yet; next apply uses it)");
    }
    daemon::sync_led_sysfs(brightness);
    Ok(())
}

/// Run the DE-OSD bridge daemon (see [`daemon::run`]).
fn cmd_daemon(cli: &Cli, table: &ModelTable, layout: &Layout, max_steps: u32) -> Result<()> {
    let (path, color_scale) = resolve_and_report(cli, table)?;
    let dev = device::Device::open(&path)?;
    daemon::run(&dev, layout, color_scale, max_steps)
}

fn cmd_brightness_get(cli: &Cli, table: &ModelTable) -> Result<()> {
    let (path, _) = resolve_device_path(cli, table)?;
    let dev = device::Device::open(&path)?;
    dev.drain_input()?;
    dev.write_output(&protocol::build_vendor(protocol::cmd::GET_MODE_BRIGHTNESS, &[]))?;
    match dev.read_input(Duration::from_millis(500))? {
        Some(reply) => {
            println!("raw reply ({}B): {}", reply.len(), hexdump(&reply));
            // KLC-PROTOCOL.md documents 0x86 as "GET mode + brightness" but
            // not the exact reply offsets; this is a best-effort decode
            // (byte 1 = mode, byte 2 = brightness, after the report-id
            // byte). Cross-check with `msi-klc query 86` if it looks wrong.
            match (reply.get(1), reply.get(2)) {
                (Some(&mode), Some(&brightness)) => {
                    println!("mode={mode} brightness={brightness} (best-effort decode, unverified offsets)")
                }
                _ => println!("reply too short to decode mode/brightness"),
            }
        }
        None => println!("no reply within 500ms"),
    }
    Ok(())
}

fn cmd_info(cli: &Cli, table: &ModelTable) -> Result<()> {
    let (path, _) = resolve_device_path(cli, table)?;
    let dev = device::Device::open(&path)?;
    eprintln!(
        "msi-klc: decoding device id/UID/CRC replies below is best-effort (KLC-PROTOCOL.md \
         doesn't pin down exact reply offsets/endianness); raw bytes are printed alongside."
    );

    dev.drain_input()?;
    dev.write_output(&protocol::build_vendor(protocol::cmd::GET_DEVICE_ID, &[]))?;
    match dev.read_input(Duration::from_millis(500))? {
        Some(reply) => {
            let id = reply.get(1..3).map(|b| u16::from_be_bytes([b[0], b[1]]));
            println!(
                "device id (0x10): {} (raw {})",
                id.map_or_else(|| "?".to_string(), |v| format!("0x{v:04x}")),
                hexdump(&reply)
            );
        }
        None => println!("device id (0x10): no reply within 500ms"),
    }
    sleep(SEND_DELAY);

    dev.drain_input()?;
    dev.write_output(&protocol::build_vendor(protocol::cmd::GET_DEVICE_ID_ALT, &[]))?;
    match dev.read_input(Duration::from_millis(500))? {
        Some(reply) => {
            let uid_hex: String = reply.iter().skip(1).map(|b| format!("{b:02x}")).collect();
            println!("STM32 UID (0x80): {uid_hex}");
        }
        None => println!("STM32 UID (0x80): no reply within 500ms"),
    }
    sleep(SEND_DELAY);

    dev.drain_input()?;
    dev.write_output(&protocol::build_vendor(protocol::cmd::GET_FW_CRC, &[]))?;
    match dev.read_input(Duration::from_millis(500))? {
        Some(reply) => {
            let crc = reply.get(1..5).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
            println!(
                "firmware CRC (0x15): {} (raw {})",
                crc.map_or_else(|| "?".to_string(), |v| format!("0x{v:08x}")),
                hexdump(&reply)
            );
        }
        None => println!("firmware CRC (0x15): no reply within 500ms"),
    }
    Ok(())
}

/// Onboard effect programming: breathe/colorshift/wave, or off (backs
/// `effect`). Takes the whole [`state::Frame::Effect`] so its parameter count
/// stays clippy-friendly.
///
/// Builds the 69-byte `lighting_effect` host struct (`common_lighting.lisp`),
/// transforms it to the 151-byte device effect program via
/// [`protocol::LightingEffect::to_device_bytes`] (ported from
/// `get-breathe-effect-bytes`/`get-colorshift-effect-bytes` in
/// `fancy_lighting_engine.lisp`), uploads it, then points the assigned keys
/// at it (`settings_mask=EFFECT`). The byte transform is unit-tested against
/// hand-computed vectors (see `protocol.rs`) but has not yet been validated
/// against a hardware USB capture.
///
/// Unlike the original `cmd_effect`, effect colors are folded through `scale`
/// (color_scale*brightness) so software brightness dims effects too.
fn apply_effect(
    dev: &device::Device,
    layout: &Layout,
    frame: &state::Frame,
    scale: [f32; 3],
) -> Result<()> {
    let state::Frame::Effect {
        mode,
        colors,
        speed,
        dir,
        slot,
        positions,
    } = frame
    else {
        unreachable!("apply_effect only handles Frame::Effect");
    };
    let (mode, speed, dir, slot) = (*mode, *speed, *dir, *slot);
    let groups = layout.groups()?;

    if mode == EffectMode::Off {
        // Steady/disabled counterpart: black + settings_mask=STEADY, same
        // effect as `off`.
        for group in &groups {
            if group.is_empty() {
                continue;
            }
            let frame = protocol::build_group_frame(group, 0, 0, 0);
            dev.send_feature(&frame)?;
            sleep(SEND_DELAY);
        }
        dev.write_output(&protocol::build_commit())?;
        return Ok(());
    }

    eprintln!(
        "msi-klc: uploading {mode:?} effect to slot {slot} (device-byte transform ported from \
         fancy_lighting_engine.lisp, unit-tested against hand-computed vectors, but not yet \
         validated against a hardware USB capture)."
    );

    if colors.is_empty() {
        bail!("effect {mode:?} needs at least one color");
    }
    // Fold color_scale*brightness into the effect palette so software
    // brightness dims effects too (raw one-shots pass scale [1,1,1]).
    let parsed_colors: Vec<(u8, u8, u8)> =
        colors.iter().map(|&c| models::scale_rgb(scale, rgb_tuple(c))).collect();
    let positions: Vec<u8> = if positions.is_empty() {
        let n = parsed_colors.len().max(1);
        (0..parsed_colors.len()).map(|i| ((i * 100) / n) as u8).collect()
    } else {
        positions.clone()
    };

    let (effect_type, has_direction, direction_type) = match mode {
        EffectMode::Breathe => (
            protocol::effect_type::BREATHE,
            dir.is_some(),
            dir.map_or(protocol::direction_type::HORIZONTAL, Direction::to_device),
        ),
        EffectMode::Colorshift => (
            protocol::effect_type::COLORSHIFT,
            dir.is_some(),
            dir.map_or(protocol::direction_type::HORIZONTAL, Direction::to_device),
        ),
        // "wave" = colorshift with a directional propagation always enabled.
        EffectMode::Wave => (
            protocol::effect_type::COLORSHIFT,
            true,
            dir.map_or(protocol::direction_type::HORIZONTAL, Direction::to_device),
        ),
        EffectMode::Off => unreachable!("handled above"),
    };

    let effect = protocol::LightingEffect {
        effect_type,
        has_direction,
        direction_type,
        direction_inverted: false,
        focal_x: 0,
        focal_y: 0,
        speed,
        scale: 0,
        colors: parsed_colors.clone(),
        positions,
    };
    let frame = protocol::build_effect_upload_frame(slot, &effect);
    dev.send_feature(&frame)?;
    sleep(SEND_DELAY);

    // Assign the uploaded effect to every key.
    let (first_r, first_g, first_b) = parsed_colors[0];
    for group in &groups {
        if group.is_empty() {
            continue;
        }
        let elements: Vec<protocol::KeyElement> = group
            .iter()
            .map(|&hid| protocol::KeyElement {
                init: [first_r, first_g, first_b],
                react: [0, 0, 0],
                react_time_ms: 0,
                effect_index: slot,
                settings_mask: protocol::settings_mask::EFFECT,
                lockmask: 0,
                hid,
            })
            .collect();
        let frame = protocol::build_group_frame_elements(&elements);
        dev.send_feature(&frame)?;
        sleep(SEND_DELAY);
    }
    dev.write_output(&protocol::build_commit())?;
    Ok(())
}

fn hexdump(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
}

/// Print the embedded per-model table as an aligned listing. Read-only and
/// hardware-independent — no device access, no root required.
fn cmd_models(table: &ModelTable) -> Result<()> {
    let models = table.all();
    let name_w = models.iter().map(|m| m.name.len()).max().unwrap_or(4).max(4);
    println!(
        "{:<name_w$}  {:<9}  {:<18}  {:>8}  {:>6}",
        "NAME",
        "USB ID",
        "COLOR SCALE",
        "KEYS",
        "COORDS",
        name_w = name_w
    );
    for m in models {
        let usb = format!("{:04x}:{:04x}", m.vid, m.pid);
        let scale = format!(
            "[{:.2}, {:.2}, {:.2}]",
            m.color_scale[0], m.color_scale[1], m.color_scale[2]
        );
        let key_count = m.key_count.map_or_else(|| "?".to_string(), |k| k.to_string());
        println!(
            "{:<name_w$}  {:<9}  {:<18}  {:>8}  {:>6}",
            m.name,
            usb,
            scale,
            key_count,
            m.num_key_coords,
            name_w = name_w
        );
    }
    Ok(())
}

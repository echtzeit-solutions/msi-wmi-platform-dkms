//! Software-owned brightness state + the last-applied logical *frame*,
//! persisted to a small JSON file so a later brightness change (from the
//! `brightness` command or the `daemon`'s uleds/OSD bridge) can re-apply the
//! same lighting at a new brightness.
//!
//! Why software brightness at all: on the validated MS-16V5 / GS66
//! (`1038:113a`) build the SteelSeries KLC keyboard has **no** host HID
//! brightness command, its `0x86` read-back returns zeros, and the EC
//! brightness registers (`0xD3`/`0xEC`) are decoupled from the per-key RGB
//! PWM and wrap. So brightness is owned here in software: we fold a
//! `brightness/255` multiplier into the same per-channel scale the model's
//! `color_scale` correction already uses (see [`fold_brightness`] and
//! `models::scale_rgb`). The frame is stored **pre-brightness** (raw user
//! colors + resolved key ids), so re-applying at a new brightness just
//! recomputes the scale.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::{Direction, EffectMode};

/// Default brightness when no state file exists yet: full brightness, so a
/// fresh install behaves exactly like the pre-brightness tool.
pub const DEFAULT_BRIGHTNESS: u8 = 255;

/// A logical lighting frame, captured *before* any color-scale/brightness
/// scaling so it can be re-applied at an arbitrary brightness. Colors are the
/// raw user-supplied RGB; key entries store already-resolved HID usage codes
/// (so re-apply needs no keymap).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Frame {
    /// Every key one solid color (`all`).
    All { color: [u8; 3] },
    /// Individual keys: `(hid, rgb)` pairs (`key`).
    Keys { pairs: Vec<(u8, [u8; 3])> },
    /// Keypress-trail reactive effect (`reactive`).
    Reactive {
        base: [u8; 3],
        hit: [u8; 3],
        fade: u16,
    },
    /// Onboard breathe/colorshift/wave effect program (`effect`). Positions
    /// are stored already-defaulted (one per color).
    Effect {
        mode: EffectMode,
        colors: Vec<[u8; 3]>,
        speed: u16,
        dir: Option<Direction>,
        slot: u8,
        positions: Vec<u8>,
    },
    /// All keys off (`off`).
    Off,
}

/// Persisted brightness + last logical frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// Current software brightness, 0..=255.
    pub brightness: u8,
    /// Last applied logical frame (pre-brightness), if any has been applied.
    #[serde(default)]
    pub frame: Option<Frame>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            brightness: DEFAULT_BRIGHTNESS,
            frame: None,
        }
    }
}

/// Candidate directories for the state file, most-preferred first:
/// `/run/msi-klc` (system-wide, where the root daemon writes), then
/// `$XDG_RUNTIME_DIR/msi-klc` as a per-user fallback when `/run/msi-klc`
/// isn't writable.
fn candidate_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("/run/msi-klc")];
    if let Ok(xdg) = std::env::var("XDG_RUNTIME_DIR") {
        if !xdg.is_empty() {
            dirs.push(PathBuf::from(xdg).join("msi-klc"));
        }
    }
    dirs
}

/// Load the persisted state, trying each candidate dir in order. Missing or
/// unparseable state is not an error — it falls back to [`State::default`]
/// (full brightness, no frame), which is the correct "fresh" behavior.
pub fn load() -> State {
    for dir in candidate_dirs() {
        let path = dir.join("state.json");
        if let Ok(text) = std::fs::read_to_string(&path) {
            match serde_json::from_str::<State>(&text) {
                Ok(state) => return state,
                Err(e) => eprintln!("msi-klc: ignoring unreadable state at {}: {e}", path.display()),
            }
        }
    }
    State::default()
}

/// Persist state, trying each candidate dir until one accepts the write.
/// Returns the path actually written.
pub fn save(state: &State) -> Result<PathBuf> {
    let json = serde_json::to_string_pretty(state).context("serializing state")?;
    let mut last_err: Option<String> = None;
    for dir in candidate_dirs() {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            last_err = Some(format!("mkdir {}: {e}", dir.display()));
            continue;
        }
        let path = dir.join("state.json");
        match std::fs::write(&path, &json) {
            Ok(()) => return Ok(path),
            Err(e) => last_err = Some(format!("write {}: {e}", path.display())),
        }
    }
    bail!(
        "could not write state to any of /run/msi-klc or $XDG_RUNTIME_DIR/msi-klc ({})",
        last_err.unwrap_or_else(|| "no candidate dirs".to_string())
    )
}

/// Fold a software brightness (0..=255) into a per-channel scale on top of
/// the model's `color_scale`: `effective[i] = color_scale[i] * brightness/255`.
/// The result feeds `models::scale_rgb`, so brightness dims exactly the same
/// way the color-scale correction does — one rounding step, post-color_scale.
pub fn fold_brightness(color_scale: [f32; 3], brightness: u8) -> [f32; 3] {
    let b = f32::from(brightness) / 255.0;
    [color_scale[0] * b, color_scale[1] * b, color_scale[2] * b]
}

/// Parse a user brightness argument: either an absolute `0..=255` or a
/// percentage like `50%` (0..=100%). Percentages map linearly onto 0..=255.
pub fn parse_brightness(s: &str) -> Result<u8> {
    let s = s.trim();
    if let Some(pct_str) = s.strip_suffix('%') {
        let pct: f32 = pct_str
            .trim()
            .parse()
            .with_context(|| format!("invalid percentage {s:?}"))?;
        if !(0.0..=100.0).contains(&pct) {
            bail!("percentage must be 0..=100, got {pct}");
        }
        Ok((pct / 100.0 * 255.0).round() as u8)
    } else {
        let v: u32 = s.parse().with_context(|| {
            format!("brightness must be 0..=255 or a percentage like 50%, got {s:?}")
        })?;
        if v > 255 {
            bail!("brightness must be 0..=255, got {v}");
        }
        Ok(v as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::scale_rgb;

    #[test]
    fn brightness_128_halves_an_ff_channel_post_color_scale() {
        // Identity color_scale, brightness 128 -> 0xFF channel drops to ~0x80.
        let scale = fold_brightness([1.0, 1.0, 1.0], 128);
        assert_eq!(scale_rgb(scale, (255, 255, 255)), (128, 128, 128));
    }

    #[test]
    fn brightness_zero_blacks_everything() {
        let scale = fold_brightness([1.0, 1.0, 1.0], 0);
        assert_eq!(scale_rgb(scale, (255, 200, 10)), (0, 0, 0));
    }

    #[test]
    fn brightness_255_is_unchanged_under_identity_scale() {
        let scale = fold_brightness([1.0, 1.0, 1.0], 255);
        assert_eq!(scale_rgb(scale, (12, 34, 56)), (12, 34, 56));
    }

    #[test]
    fn brightness_folds_on_top_of_color_scale() {
        // color_scale [0.5,1,1] with brightness 128 -> R channel ~0.25.
        let scale = fold_brightness([0.5, 1.0, 1.0], 128);
        // 255 * 0.5 * (128/255) = 64.0.
        assert_eq!(scale_rgb(scale, (255, 255, 255)), (64, 128, 128));
    }

    #[test]
    fn parse_brightness_absolute_and_percent() {
        assert_eq!(parse_brightness("0").unwrap(), 0);
        assert_eq!(parse_brightness("128").unwrap(), 128);
        assert_eq!(parse_brightness("255").unwrap(), 255);
        assert_eq!(parse_brightness("0%").unwrap(), 0);
        assert_eq!(parse_brightness("100%").unwrap(), 255);
        // 50% -> round(127.5) -> 128.
        assert_eq!(parse_brightness("50%").unwrap(), 128);
        assert_eq!(parse_brightness(" 40 % ".replace(' ', "").as_str()).unwrap(), 102);
    }

    #[test]
    fn parse_brightness_rejects_out_of_range_and_garbage() {
        assert!(parse_brightness("256").is_err());
        assert!(parse_brightness("101%").is_err());
        assert!(parse_brightness("-1").is_err());
        assert!(parse_brightness("nope").is_err());
        assert!(parse_brightness("").is_err());
    }

    #[test]
    fn state_json_round_trips() {
        let state = State {
            brightness: 200,
            frame: Some(Frame::Keys {
                pairs: vec![(4, [255, 0, 0]), (5, [0, 255, 0])],
            }),
        };
        let json = serde_json::to_string(&state).unwrap();
        let back: State = serde_json::from_str(&json).unwrap();
        assert_eq!(state, back);
    }

    #[test]
    fn state_frame_variants_round_trip() {
        for frame in [
            Frame::All { color: [1, 2, 3] },
            Frame::Off,
            Frame::Reactive {
                base: [10, 20, 30],
                hit: [40, 50, 60],
                fade: 300,
            },
            Frame::Effect {
                mode: EffectMode::Breathe,
                colors: vec![[255, 0, 0], [0, 0, 255]],
                speed: 100,
                dir: Some(Direction::H),
                slot: 0,
                positions: vec![0, 50],
            },
        ] {
            let state = State {
                brightness: 128,
                frame: Some(frame.clone()),
            };
            let json = serde_json::to_string(&state).unwrap();
            let back: State = serde_json::from_str(&json).unwrap();
            assert_eq!(back.frame, Some(frame));
        }
    }

    #[test]
    fn state_default_is_full_brightness_no_frame() {
        let d = State::default();
        assert_eq!(d.brightness, 255);
        assert_eq!(d.frame, None);
    }

    #[test]
    fn missing_frame_field_defaults_to_none() {
        let back: State = serde_json::from_str(r#"{"brightness":100}"#).unwrap();
        assert_eq!(back.brightness, 100);
        assert_eq!(back.frame, None);
    }
}

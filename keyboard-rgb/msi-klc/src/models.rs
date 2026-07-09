//! Per-model KLC device table: USB `vid:pid` -> physical keyboard model,
//! including the per-channel color-scale correction and (where known) the
//! physical key-coordinate map, embedded at compile time from
//! `data/msi-klc-models.json`.
//!
//! That file was extracted from decrypted SteelSeries per-model layout
//! specs bundled with MSI Center (see the RE project's
//! `linux-msi-ms16v5/keyboard-rgb/msi-klc-models.json` and
//! `KLC-PROTOCOL.md`). It covers the `1038:113a` GS66 this tool was
//! originally validated against, plus ~40 sibling KLC PIDs across the
//! GE/GS/GT/GP/P/Stealth/Z16/Z17 families.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;

use crate::device::VID_MSI;

/// Raw embedded model table, copied byte-for-byte from the RE project.
const MSI_KLC_MODELS_JSON: &str = include_str!("../data/msi-klc-models.json");

/// One entry of `msi-klc-models.json`, keyed by model name in the file.
#[derive(Debug, Deserialize)]
struct RawModel {
    usb: String,
    color_scale: [f32; 3],
    key_count: Option<u32>,
    #[serde(default)]
    num_key_coords: u32,
    #[serde(default)]
    key_coords: Option<HashMap<String, (u16, u16)>>,
    // Present in the source data for provenance/debugging but not needed
    // here: `key_coords_from` (which sibling model's coords were reused)
    // and `include_chain` (the SteelSeries spec-inheritance chain).
    #[serde(default)]
    #[allow(dead_code)]
    key_coords_from: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    include_chain: Vec<String>,
}

/// A resolved KLC keyboard model: its USB id, per-channel color-scale
/// correction, and (where the SteelSeries spec provided one) its physical
/// key layout.
#[derive(Debug, Clone)]
pub struct Model {
    pub name: String,
    pub vid: u16,
    pub pid: u16,
    /// Per-channel `(R, G, B)` multiplier applied to user-supplied colors
    /// before sending them to the panel, correcting for that panel's LED
    /// color response (see `scale_rgb`). `[1.0, 1.0, 1.0]` = no correction.
    pub color_scale: [f32; 3],
    pub key_count: Option<u32>,
    /// Number of entries the source spec provided in `key_coords` (may be
    /// less than `key_count` — not every key gets a coordinate).
    pub num_key_coords: u32,
    /// HID usage code -> physical `(x, y)` position, in the SteelSeries
    /// layout spec's coordinate space. Empty if the spec didn't provide one
    /// for this model.
    ///
    /// TODO: not yet wired into `effect`'s wave program (`wave_origin` /
    /// `wave_scale` in `protocol::EffectProgram`) — a real wave effect
    /// would want to derive those from this map instead of the current
    /// placeholder `(0, 0)`. Stored here so that's a follow-up, not a
    /// re-extraction. `#[allow(dead_code)]` until something reads it.
    #[allow(dead_code)]
    pub key_coords: HashMap<u8, (u16, u16)>,
}

/// The whole embedded model table.
pub struct ModelTable {
    models: Vec<Model>,
}

impl ModelTable {
    /// Parse the embedded `msi-klc-models.json`.
    pub fn load() -> Result<Self> {
        let raw: HashMap<String, RawModel> = serde_json::from_str(MSI_KLC_MODELS_JSON)
            .context("parsing embedded msi-klc-models.json")?;
        let mut models = Vec::with_capacity(raw.len());
        for (name, r) in raw {
            let (vid, pid) = parse_usb_id(&r.usb)
                .with_context(|| format!("model {name:?} has invalid usb id {:?}", r.usb))?;
            let mut key_coords = HashMap::new();
            for (hid_str, xy) in r.key_coords.into_iter().flatten() {
                let hid: u32 = hid_str
                    .parse()
                    .with_context(|| format!("model {name:?} has non-numeric key_coords key {hid_str:?}"))?;
                let hid = u8::try_from(hid)
                    .with_context(|| format!("model {name:?} key_coords hid {hid} out of u8 range"))?;
                key_coords.insert(hid, xy);
            }
            models.push(Model {
                name,
                vid,
                pid,
                color_scale: r.color_scale,
                key_count: r.key_count,
                num_key_coords: r.num_key_coords,
                key_coords,
            });
        }
        models.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Self { models })
    }

    /// Look up a model by exact `(vid, pid)`.
    pub fn find(&self, vid: u16, pid: u16) -> Option<&Model> {
        self.models.iter().find(|m| m.vid == vid && m.pid == pid)
    }

    /// All models, sorted by name.
    pub fn all(&self) -> &[Model] {
        &self.models
    }

    /// All PIDs under the MSI vendor id known to this table, used for
    /// multi-model auto-detection (`device::find_device`) so it recognizes
    /// every KLC PID in the table, not just the original `0x113a`.
    pub fn msi_pids(&self) -> Vec<u16> {
        self.models.iter().filter(|m| m.vid == VID_MSI).map(|m| m.pid).collect()
    }
}

/// Parse a `vvvv:pppp` hex USB id (as found in the `usb` field of
/// `msi-klc-models.json`, no `0x` prefix) into `(vid, pid)`.
fn parse_usb_id(s: &str) -> Result<(u16, u16)> {
    let (v, p) = s.split_once(':').with_context(|| format!("expected VID:PID, got {s:?}"))?;
    let vid = u16::from_str_radix(v, 16).with_context(|| format!("invalid vid {v:?}"))?;
    let pid = u16::from_str_radix(p, 16).with_context(|| format!("invalid pid {p:?}"))?;
    Ok((vid, pid))
}

/// Apply a model's per-channel color-scale correction: `out = round(in *
/// scale)`. Every channel is clamped to the `u8` range as a safety net
/// (the table's scale factors are all `<= 1.0`, so this is normally a
/// no-op).
pub fn scale_rgb(scale: [f32; 3], (r, g, b): (u8, u8, u8)) -> (u8, u8, u8) {
    (
        scale_channel(r, scale[0]),
        scale_channel(g, scale[1]),
        scale_channel(b, scale[2]),
    )
}

fn scale_channel(v: u8, scale: f32) -> u8 {
    let scaled = (f32::from(v) * scale).round();
    scaled.clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_table_parses() {
        let t = ModelTable::load().expect("embedded model table parses");
        // ~40 models as of the data extraction this was built against.
        assert!(t.all().len() > 10, "expected a healthy number of models, got {}", t.all().len());
    }

    #[test]
    fn gs66_resolves_with_identity_scale() {
        let t = ModelTable::load().unwrap();
        let m = t.find(0x1038, 0x113a).expect("1038:113a (GS66) must be in the embedded table");
        assert_eq!(m.name, "msi-klc496");
        assert_eq!(m.color_scale, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn unknown_pid_is_not_found() {
        let t = ModelTable::load().unwrap();
        assert!(t.find(0x1038, 0xffff).is_none());
    }

    #[test]
    fn msi_pids_includes_gs66_and_others() {
        let t = ModelTable::load().unwrap();
        let pids = t.msi_pids();
        assert!(pids.contains(&0x113a));
        assert!(pids.len() > 10);
    }

    #[test]
    fn color_scale_rounding_matches_reference_example() {
        // 255 * 0.41 = 104.55 -> rounds to 105.
        assert_eq!(scale_channel(255, 0.41), 105);
        assert_eq!(scale_rgb([0.41, 1.0, 0.51], (255, 255, 255)), (105, 255, 130));
    }

    #[test]
    fn color_scale_identity_is_a_no_op() {
        assert_eq!(scale_rgb([1.0, 1.0, 1.0], (12, 34, 56)), (12, 34, 56));
    }
}

//! Keymap + region-group layout data, embedded at compile time from
//! `data/msi-layouts.json` (extracted from MSI Center's
//! `MysticLight_AllDevice.dll`; see the RE project's `extract-msi-layouts.py`).
//!
//! `region_groups.GroupN_Offset` is the 6-way partition of HID usage codes
//! that the firmware's `Set_Keyboard_Color` groups keys into for the `0x0E`
//! bulk-color feature report; `keymaps.<Name>` maps symbolic key names
//! (`CLK_*`) to HID usage codes for a given physical layout (default:
//! `GE73Keys`, which also covers the GS66).

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::HashMap;

/// Raw embedded layout data (GE73Keys keymap + 6 region groups + a few
/// sibling layouts), copied byte-for-byte from the RE project.
const MSI_LAYOUTS_JSON: &str = include_str!("../data/msi-layouts.json");

pub const DEFAULT_KEYMAP: &str = "GE73Keys";

#[derive(Debug, Deserialize)]
struct LayoutsFile {
    keymaps: HashMap<String, HashMap<String, u32>>,
    region_groups: HashMap<String, Vec<u32>>,
    #[allow(dead_code)]
    #[serde(default)]
    region_group_union: Vec<u32>,
}

/// Parsed layout data plus convenience lookups.
pub struct Layout {
    file: LayoutsFile,
}

impl Layout {
    /// Parse the embedded `msi-layouts.json`.
    pub fn load() -> Result<Self> {
        let file: LayoutsFile =
            serde_json::from_str(MSI_LAYOUTS_JSON).context("parsing embedded msi-layouts.json")?;
        Ok(Self { file })
    }

    /// Names of all embedded keymaps (e.g. `GE73Keys`, `GK80_US_Keys`, ...).
    pub fn keymap_names(&self) -> Vec<&str> {
        self.file.keymaps.keys().map(String::as_str).collect()
    }

    /// The six region groups, in `Group1_Offset..Group6_Offset` order, each
    /// a list of HID usage codes. This is the partition
    /// `msi-nb-rgb.py:set_all_groups` iterates to build one `0x0E` report per
    /// group — sending all six is required to reach every key (including
    /// keys like Power that a naive single-group approach misses).
    pub fn groups(&self) -> Result<[Vec<u8>; 6]> {
        let mut out: [Vec<u8>; 6] = Default::default();
        for (i, slot) in out.iter_mut().enumerate() {
            let key = format!("Group{}_Offset", i + 1);
            let g = self
                .file
                .region_groups
                .get(&key)
                .with_context(|| format!("missing {key} in msi-layouts.json"))?;
            *slot = g
                .iter()
                .map(|&hid| hid_to_u8(hid))
                .collect::<Result<Vec<u8>>>()?;
        }
        Ok(out)
    }

    /// Resolve a key name to its HID usage code within `keymap_name`
    /// (default `GE73Keys`). Accepts either a symbolic name (`CLK_Escape`)
    /// or a bare decimal/hex HID code (`41` or `0x29`).
    pub fn resolve_key(&self, keymap_name: &str, name: &str) -> Result<u8> {
        if let Some(hid) = parse_hid_literal(name) {
            return Ok(hid);
        }
        let keymap = self
            .file
            .keymaps
            .get(keymap_name)
            .with_context(|| format!("unknown keymap {keymap_name:?} (available: {:?})", self.keymap_names()))?;
        let hid = keymap
            .get(name)
            .with_context(|| format!("unknown key {name:?} in keymap {keymap_name:?}"))?;
        hid_to_u8(*hid)
    }
}

/// Parse a bare HID literal: decimal (`41`) or `0x`-prefixed hex (`0x29`).
fn parse_hid_literal(s: &str) -> Option<u8> {
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u8::from_str_radix(hex, 16).ok()
    } else if s.chars().all(|c| c.is_ascii_digit()) && !s.is_empty() {
        s.parse::<u8>().ok()
    } else {
        None
    }
}

fn hid_to_u8(hid: u32) -> Result<u8> {
    u8::try_from(hid).map_err(|_| anyhow::anyhow!("HID usage code {hid} out of u8 range"))
}

/// Parse an `RRGGBB` (optionally `#`-prefixed) hex color into `(r, g, b)`.
pub fn parse_hex_color(s: &str) -> Result<(u8, u8, u8)> {
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() != 6 {
        bail!("expected a 6-digit hex color like FF00AA, got {s:?}");
    }
    let r = u8::from_str_radix(&s[0..2], 16).context("parsing R")?;
    let g = u8::from_str_radix(&s[2..4], 16).context("parsing G")?;
    let b = u8::from_str_radix(&s[4..6], 16).context("parsing B")?;
    Ok((r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_embedded_layout() {
        let l = Layout::load().expect("embedded layout parses");
        assert!(l.keymap_names().contains(&"GE73Keys"));
    }

    #[test]
    fn groups_cover_the_expected_key_counts() {
        let l = Layout::load().unwrap();
        let groups = l.groups().unwrap();
        let total: usize = groups.iter().map(|g| g.len()).sum();
        // msi-layouts.json's region_group_union has 106 keys (GE73Keys superset).
        assert_eq!(total, 106);
    }

    #[test]
    fn resolves_symbolic_and_literal_keys() {
        let l = Layout::load().unwrap();
        assert_eq!(l.resolve_key("GE73Keys", "CLK_A").unwrap(), 4);
        assert_eq!(l.resolve_key("GE73Keys", "41").unwrap(), 41);
        assert_eq!(l.resolve_key("GE73Keys", "0x29").unwrap(), 41);
        assert!(l.resolve_key("GE73Keys", "CLK_Nonexistent").is_err());
    }

    #[test]
    fn parses_hex_colors() {
        assert_eq!(parse_hex_color("FF00AA").unwrap(), (0xFF, 0x00, 0xAA));
        assert_eq!(parse_hex_color("#00ff00").unwrap(), (0, 0xFF, 0));
        assert!(parse_hex_color("nope").is_err());
    }
}

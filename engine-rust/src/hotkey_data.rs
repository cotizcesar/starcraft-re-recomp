//! Complete Hotkeys browser data (all races, units and buildings).
//!
//! Transcribed from the shipped `webui/dist/hotkeypanel/hotkeypanel.js`
//! (`Actions`/`Units`) and the native `OnBindingsChanged` message captured
//! from retail 1.23.10.13515 (`$default`/Brood War profile).  The fixture is
//! static data, not a screenshot-derived correction: keys, card positions and
//! icons are the ones the game sends to the CEF panel.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use crate::menu::HotkeyRace;
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyBind {
    pub text: char,
    pub code: u16,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ActionDef {
    pub label: String,
    pub icon: String,
    pub x: u8,
    pub y: u8,
    pub default_text: String,
    pub default_code: u16,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CardAction {
    pub id: u16,
    pub x: u8,
    pub y: u8,
    pub icon: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CardDef {
    pub label: String,
    pub actions: Vec<CardAction>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UnitDef {
    pub id: u16,
    pub kind: String,
    pub loc_key: String,
    pub icon: String,
    pub cards: Vec<CardDef>,
}

#[derive(Deserialize, Default)]
struct RawData {
    actions: HashMap<String, ActionDef>,
    units: HashMap<String, Vec<UnitDef>>,
}

pub struct HotkeyData {
    actions: HashMap<u16, ActionDef>,
    units: HashMap<String, Vec<UnitDef>>,
    shares: HashMap<u16, usize>,
}

static DATA: OnceLock<HotkeyData> = OnceLock::new();

impl HotkeyData {
    pub fn get() -> &'static Self {
        DATA.get_or_init(|| {
            let raw: RawData =
                serde_json::from_slice(embedded!("fixtures/hotkeys.json")).unwrap_or_default();
            let mut actions = HashMap::with_capacity(raw.actions.len());
            for (id, action) in raw.actions {
                let Ok(id) = id.parse::<u16>() else {
                    continue;
                };
                actions.insert(id, action);
            }
            let mut shares: HashMap<u16, usize> = HashMap::new();
            for units in raw.units.values() {
                for unit in units {
                    let ids: HashSet<u16> = unit
                        .cards
                        .iter()
                        .flat_map(|c| c.actions.iter().map(|a| a.id))
                        .collect();
                    for id in ids {
                        *shares.entry(id).or_default() += 1;
                    }
                }
            }
            Self {
                actions,
                units: raw.units,
                shares,
            }
        })
    }

    pub fn action(&self, id: u16) -> Option<&ActionDef> {
        self.actions.get(&id)
    }

    pub fn unit(&self, id: u16) -> Option<&UnitDef> {
        self.units.values().flatten().find(|u| u.id == id)
    }

    pub fn units(&self, race: HotkeyRace, kind: Option<&str>) -> Vec<&UnitDef> {
        let key = match race {
            HotkeyRace::Terran => "terran",
            HotkeyRace::Protoss => "protoss",
            HotkeyRace::Zerg => "zerg",
        };
        self.units
            .get(key)
            .map(|units| {
                units
                    .iter()
                    .filter(|u| kind.is_none_or(|k| u.kind == k))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn all_units(&self) -> impl Iterator<Item = &UnitDef> {
        self.units.values().flatten()
    }

    pub fn shares(&self, id: u16) -> usize {
        self.shares.get(&id).copied().unwrap_or(0)
    }

    pub fn card_title(label: &str) -> &'static str {
        match label {
            "general" => "General",
            "basicStructure" => "Basic Structure",
            "advancedStructure" => "Advanced Structure",
            "generalLifted" => "General - Lifted",
            "generalTankMode" => "General - Tank Mode",
            "generalSiegeMode" => "General - Siege Mode",
            "generalUncloaked" => "General - Uncloaked",
            "generalCloaked" => "General - Cloaked",
            "generalBurrowed" => "General - Burrowed",
            _ => "General",
        }
    }

    /// Default (`$default`/Brood War) binding for every action, indexed by id.
    pub fn default_bindings(&self) -> Vec<Option<KeyBind>> {
        let max = self.actions.keys().copied().max().unwrap_or(0) as usize;
        let mut out = vec![None; max + 1];
        for (id, action) in &self.actions {
            let Some(text) = action.default_text.chars().next() else {
                continue;
            };
            if let Some(slot) = out.get_mut(*id as usize) {
                *slot = Some(KeyBind {
                    text,
                    code: action.default_code,
                });
            }
        }
        out
    }
}

/// Apply a rebind using CEF's per-unit collision rules and return
/// `(units_with_any_unbound_action, collision_occurred)`.
pub fn apply_rebind(
    bindings: &mut [Option<KeyBind>],
    action: u16,
    text: char,
    code: u16,
) -> (Vec<u16>, bool) {
    let data = HotkeyData::get();
    if let Some(slot) = bindings.get_mut(action as usize) {
        *slot = Some(KeyBind { text, code });
    }
    let mut collision = false;
    for unit in data.all_units() {
        let contains = unit
            .cards
            .iter()
            .flat_map(|c| c.actions.iter())
            .any(|a| a.id == action);
        if !contains {
            continue;
        }
        for card in &unit.cards {
            for a in &card.actions {
                if a.id == action {
                    continue;
                }
                let same = bindings
                    .get(a.id as usize)
                    .copied()
                    .flatten()
                    .is_some_and(|k| k.code == code);
                if same {
                    if let Some(slot) = bindings.get_mut(a.id as usize) {
                        *slot = None;
                    }
                    collision = true;
                }
            }
        }
    }
    let mut unbound = Vec::new();
    for unit in data.all_units() {
        let missing = unit
            .cards
            .iter()
            .flat_map(|c| c.actions.iter())
            .any(|a| bindings.get(a.id as usize).copied().flatten().is_none());
        if missing {
            unbound.push(unit.id);
        }
    }
    let error = collision || !unbound.is_empty();
    (unbound, error)
}

const HOTKEY_MAGIC: &[u8; 4] = b"GCHK";
const HOTKEY_VERSION: u16 = 1;
const HOTKEY_MAX_BYTES: u64 = 64 * 1024;

/// Persist every binding (including `None`/unbound) with magic, version,
/// action-id bound and a SHA-256 trailer.  The retail profile is the source
/// of truth on first run; this file only stores the player's edits.
pub fn save_bindings(path: &std::path::Path, bindings: &[Option<KeyBind>]) -> std::io::Result<()> {
    let max_id = bindings.len().saturating_sub(1) as u16;
    let mut payload = Vec::with_capacity(8 + (bindings.len() * 2));
    payload.extend_from_slice(HOTKEY_MAGIC);
    payload.extend_from_slice(&HOTKEY_VERSION.to_le_bytes());
    payload.extend_from_slice(&max_id.to_le_bytes());
    for binding in bindings {
        let code = binding.map(|k| k.code).unwrap_or(0);
        payload.extend_from_slice(&code.to_le_bytes());
    }
    let digest = Sha256::digest(&payload);
    payload.extend_from_slice(&digest);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, &payload)?;
    std::fs::rename(&tmp, path)
}

/// Load a saved profile.  Anything malformed (magic, version, bounds,
/// checksum) returns `None` so the caller keeps the retail defaults.
pub fn load_bindings(path: &std::path::Path) -> Option<Vec<Option<KeyBind>>> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > HOTKEY_MAX_BYTES || meta.len() < 8 + 32 {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let split = bytes.len().checked_sub(32)?;
    let (payload, digest) = bytes.split_at(split);
    if Sha256::digest(payload).as_slice() != digest {
        return None;
    }
    if payload.get(0..4)? != HOTKEY_MAGIC {
        return None;
    }
    let version = u16::from_le_bytes([*payload.get(4)?, *payload.get(5)?]);
    if version != HOTKEY_VERSION {
        return None;
    }
    let max_id = u16::from_le_bytes([*payload.get(6)?, *payload.get(7)?]);
    if max_id > 4096 {
        return None;
    }
    let count = max_id as usize + 1;
    if payload.len() != 8 + count * 2 {
        return None;
    }
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let code = u16::from_le_bytes([*payload.get(8 + i * 2)?, *payload.get(9 + i * 2)?]);
        if code == 0 {
            out.push(None);
        } else {
            let ch = u8::try_from(code).ok()? as char;
            if !ch.is_ascii_alphabetic() {
                return None;
            }
            out.push(Some(KeyBind {
                text: ch.to_ascii_uppercase(),
                code,
            }));
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::HotkeyData;
    use crate::menu::HotkeyRace;
    use std::collections::HashSet;

    #[test]
    fn fixture_covers_every_unit_action() {
        let data = HotkeyData::get();
        for unit in data.all_units() {
            assert!(!unit.cards.is_empty(), "{} has no cards", unit.loc_key);
            for card in &unit.cards {
                for action in &card.actions {
                    assert!(
                        data.action(action.id).is_some(),
                        "{} references missing action {}",
                        unit.loc_key,
                        action.id
                    );
                }
            }
        }
    }

    #[test]
    fn fixture_has_all_three_races_and_default_keys() {
        let data = HotkeyData::get();
        for (race, units, buildings) in [
            (HotkeyRace::Terran, 13, 16),
            (HotkeyRace::Protoss, 14, 14),
            (HotkeyRace::Zerg, 15, 16),
        ] {
            let all = data.units(race, None);
            let mobile = all
                .iter()
                .filter(|u| u.kind == "ground" || u.kind == "air")
                .count();
            let fixed = all.len() - mobile;
            assert_eq!(mobile, units, "{race:?} mobile units");
            assert_eq!(fixed, buildings, "{race:?} buildings");
            for unit in all {
                for card in &unit.cards {
                    for action in &card.actions {
                        let def = data.action(action.id).expect("action");
                        assert!(
                            !def.default_text.is_empty(),
                            "action {} has no key",
                            action.id
                        );
                        assert!(def.default_code > 0, "action {} has no code", action.id);
                    }
                }
            }
        }
    }

    #[test]
    fn shared_action_counts_match_the_references() {
        let data = HotkeyData::get();
        assert_eq!(data.shares(664), 48); // Move
        assert_eq!(data.shares(665), 52); // Stop
        assert_eq!(data.shares(666), 35); // Attack
        assert_eq!(data.shares(667), 38); // Patrol
        assert_eq!(data.shares(668), 38); // Hold Position
    }

    #[test]
    fn rebinding_move_to_h_unbinds_hold_position_only() {
        let mut bindings = HotkeyData::get().default_bindings();
        let (unbound, error) = super::apply_rebind(&mut bindings, 664, 'H', 'h' as u16);
        assert!(error);
        // 38 Hold Position units plus the Drone, whose Build Hatchery uses H.
        assert_eq!(unbound.len(), 39);
        let terran: HashSet<u16> = HotkeyData::get()
            .units(HotkeyRace::Terran, None)
            .iter()
            .map(|u| u.id)
            .collect();
        assert_eq!(unbound.iter().filter(|id| terran.contains(id)).count(), 12);
        assert!(!unbound.contains(&7), "SCV has no Hold Position");
        assert!(unbound.contains(&0), "Marine carries Hold Position");
        assert!(unbound.contains(&41), "Drone Build Hatchery also uses H");
        assert!(bindings[668].is_none(), "Hold Position lost H");
        assert!(bindings[613].is_none(), "Build Hatchery lost H");
        assert_eq!(bindings[664].unwrap().text, 'H');
    }

    #[test]
    fn rebinding_move_to_m_collides_with_train_marine_when_lifted() {
        let mut bindings = HotkeyData::get().default_bindings();
        let (unbound, error) = super::apply_rebind(&mut bindings, 664, 'M', 'm' as u16);
        // Barracks' General - Lifted card contains Move, so Train Marine (M)
        // collides there; other Barracks cards do not.
        assert!(error);
        assert_eq!(unbound, vec![111]);
        assert!(bindings[586].is_none(), "Train Marine lost M");
    }

    #[test]
    fn collision_is_limited_to_units_that_contain_both_commands() {
        let mut bindings = HotkeyData::get().default_bindings();
        // SCV Gather (675) is G; rebinding SCV Move to G collides in the SCV.
        let (unbound, error) = super::apply_rebind(&mut bindings, 664, 'G', 'g' as u16);
        assert!(error);
        assert!(
            unbound.contains(&7),
            "SCV itself now has an unbound command"
        );
        assert!(bindings[675].is_none(), "Gather lost G");
    }

    #[test]
    fn binding_file_roundtrips_and_rejects_corruption() {
        let mut bindings = HotkeyData::get().default_bindings();
        let (unbound, _) = super::apply_rebind(&mut bindings, 664, 'H', 'h' as u16);
        assert!(unbound.contains(&0));
        let path = std::env::temp_dir().join("galaxycraft-hotkeys-test.bin");
        super::save_bindings(&path, &bindings).expect("save");
        let loaded = super::load_bindings(&path).expect("load");
        assert_eq!(loaded.len(), bindings.len());
        assert_eq!(loaded[664].unwrap().text, 'H');
        assert!(loaded[668].is_none());
        // Flip one payload byte: the SHA-256 trailer must reject it.
        let mut raw = std::fs::read(&path).expect("raw");
        let mid = raw.len() / 2;
        raw[mid] ^= 0x40;
        std::fs::write(&path, &raw).expect("corrupt");
        assert!(super::load_bindings(&path).is_none());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn unit_ids_are_unique() {
        let data = HotkeyData::get();
        let mut ids = HashSet::new();
        for unit in data.all_units() {
            assert!(ids.insert(unit.id), "duplicate unit id {}", unit.id);
        }
        assert_eq!(ids.len(), 88);
    }
}

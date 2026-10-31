//! Retail unit voice sets.
//!
//! `arr\sfxdata.tbl` lists every shipped voice file.  The unit folders are
//! stable (`sound\terran\marine`, `sound\protoss\zealot`, ...) and the file
//! stems encode the cue: `...Rdy00` ready/selected, `...Wht00` order
//! acknowledgement, `...Pss00` annoyed (repeated clicks), `...Yes00` confirm,
//! `...Dth00` death, `...Atk00` attack.
//!
//! The classic client selects folder + cue from the unit type.  This module
//! owns that mapping and the lazy WAV cache; [`crate::audio::SoundBank`] still
//! owns the small UI/effect bank.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::audio::Wav;
use crate::ids::unit as uid;
use crate::sfx::SfxTable;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VoiceCue {
    Ready,
    What,
    Pissed,
    Yes,
    Death,
    Attack,
    Hit,
    Special,
}

impl VoiceCue {
    pub fn from_stem(stem: &str) -> Option<Self> {
        let stem = stem.to_ascii_lowercase();
        if stem.contains("rdy") {
            Some(Self::Ready)
        } else if stem.contains("wht") {
            Some(Self::What)
        } else if stem.contains("pss") {
            Some(Self::Pissed)
        } else if stem.contains("yes") {
            Some(Self::Yes)
        } else if stem.contains("dth") || stem.contains("die") {
            Some(Self::Death)
        } else if stem.contains("atk")
            || stem.contains("att")
            || stem.contains("fir")
            || stem.contains("min")
        {
            Some(Self::Attack)
        } else if stem.contains("hit") {
            Some(Self::Hit)
        } else if stem.contains("rag")
            || stem.contains("clo")
            || stem.contains("sti")
            || stem.contains("las")
            || stem.contains("lkd")
        {
            Some(Self::Special)
        } else {
            None
        }
    }
}

/// Unit type -> retail voice folder.  Only folders that ship are listed;
/// units without voices (turrets, mines, most buildings) return `None`.
pub fn folder_for(unit_id: u16) -> Option<&'static str> {
    Some(match unit_id {
        uid::MARINE => "terran/marine",
        uid::GHOST => "terran/ghost",
        uid::VULTURE => "terran/vulture",
        uid::GOLIATH | uid::GOLIATH_TURRET => "terran/goliath",
        uid::SIEGE_TANK | uid::SIEGE_MODE | uid::SIEGE_TANK_TURRET | uid::SIEGE_MODE_TURRET => {
            "terran/tank"
        }
        uid::SCV => "terran/scv",
        uid::WRAITH => "terran/phoenix",
        uid::SCIENCE_VESSEL => "terran/vessel",
        uid::DROPSHIP => "terran/dropship",
        uid::BATTLECRUISER => "terran/battle",
        uid::FIREBAT => "terran/firebat",
        uid::MEDIC => "terran/medic",
        uid::LARVA => "zerg/larva",
        uid::EGG | uid::COCOON | uid::LURKER_EGG => "zerg/egg",
        uid::ZERGLING => "zerg/zergling",
        uid::HYDRALISK => "zerg/hydra",
        uid::ULTRALISK => "zerg/ultra",
        uid::BROODLING => "zerg/broodling",
        uid::DRONE => "zerg/drone",
        uid::OVERLORD => "zerg/overlord",
        uid::MUTALISK => "zerg/mutalid",
        uid::GUARDIAN => "zerg/guardian",
        uid::QUEEN => "zerg/queen",
        uid::DEFILER => "zerg/defiler",
        uid::SCOURGE => "zerg/bugguy",
        uid::DEVOURER => "zerg/devourer",
        uid::LURKER => "zerg/lurker",
        uid::PROBE => "protoss/probe",
        uid::ZEALOT => "protoss/zealot",
        uid::DRAGOON => "protoss/dragoon",
        uid::HIGH_TEMPLAR => "protoss/templar",
        uid::ARCHON => "protoss/archon",
        uid::SHUTTLE => "protoss/shuttle",
        uid::SCOUT => "protoss/scout",
        uid::ARBITER => "protoss/arbiter",
        uid::CARRIER => "protoss/carrier",
        uid::INTERCEPTOR => "protoss/intercep",
        uid::REAVER => "protoss/trilobyte",
        uid::OBSERVER => "protoss/witness",
        uid::CORSAIR => "protoss/corsair",
        uid::DARK_TEMPLAR => "protoss/darktemplar",
        uid::DARK_ARCHON => "protoss/darchon",
        // Buildings share the race building voice set.
        106..=129 => "terran/bldg",
        130..=153 => "zerg/bldg",
        154..=175 => "protoss/bldg",
        _ => return None,
    })
}

/// Voice id (index into `sfxdata.tbl`) for a cue, preferring the first file
/// in the shipped folder order.  `variant` cycles through the alternatives so
/// repeated orders do not play the same take.
pub fn voice_id(table: &SfxTable, unit_id: u16, cue: VoiceCue, variant: usize) -> Option<u16> {
    let folder = folder_for(unit_id)?;
    let mut ids: Vec<u16> = Vec::new();
    for id in 0..table.len() as u16 {
        if table.folder(id).as_deref() != Some(folder) {
            continue;
        }
        let Some(path) = table.cache_path(id) else {
            continue;
        };
        let stem = path
            .rsplit('/')
            .next()
            .unwrap_or("")
            .split('.')
            .next()
            .unwrap_or("");
        if VoiceCue::from_stem(stem) == Some(cue) {
            ids.push(id);
        }
    }
    if ids.is_empty() {
        return None;
    }
    Some(ids[variant % ids.len()])
}

/// Lazily decoded voice clips.  Files live under `root/<race>/<unit>/...`
/// in the materialised cache.
pub struct VoiceBank {
    root: PathBuf,
    table: SfxTable,
    cache: RefCell<HashMap<u16, Option<Vec<i16>>>>,
    hits: RefCell<Vec<u16>>,
}

impl VoiceBank {
    pub fn new(root: impl AsRef<Path>, table: SfxTable) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            table,
            cache: RefCell::new(HashMap::new()),
            hits: RefCell::new(Vec::new()),
        }
    }

    pub fn table(&self) -> &SfxTable {
        &self.table
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Load (once) the WAV for a `sfxdata.tbl` id.
    pub fn clip(&self, id: u16) -> Option<Vec<i16>> {
        if let Some(hit) = self.cache.borrow().get(&id) {
            return hit.clone();
        }
        let path = self.table.cache_path(id).map(|p| self.root.join(p));
        let decoded = path
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| Wav::parse(&b))
            .map(|w| w.to_mono_44k());
        self.cache.borrow_mut().insert(id, decoded.clone());
        if decoded.is_some() {
            self.hits.borrow_mut().push(id);
        }
        decoded
    }

    pub fn cue(&self, unit_id: u16, cue: VoiceCue, variant: usize) -> Option<Vec<i16>> {
        let id = voice_id(&self.table, unit_id, cue, variant)?;
        self.clip(id)
    }

    /// Number of voice files the bank can resolve without decoding them.
    pub fn available(&self) -> usize {
        (0..self.table.len() as u16)
            .filter(|id| {
                self.table
                    .cache_path(*id)
                    .is_some_and(|p| self.root.join(p).is_file())
            })
            .count()
    }

    /// Recently played ids, used by tests and the debug HUD.
    pub fn played(&self) -> Vec<u16> {
        self.hits.borrow().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_playable_unit_maps_to_a_shipped_voice_folder() {
        let table = SfxTable::bundled();
        let folders = table.folders();
        for id in [
            uid::MARINE,
            uid::GHOST,
            uid::VULTURE,
            uid::GOLIATH,
            uid::SIEGE_TANK,
            uid::SCV,
            uid::WRAITH,
            uid::SCIENCE_VESSEL,
            uid::DROPSHIP,
            uid::BATTLECRUISER,
            uid::FIREBAT,
            uid::MEDIC,
            uid::ZERGLING,
            uid::HYDRALISK,
            uid::ULTRALISK,
            uid::DRONE,
            uid::OVERLORD,
            uid::MUTALISK,
            uid::PROBE,
            uid::ZEALOT,
            uid::DRAGOON,
            uid::HIGH_TEMPLAR,
            uid::ARCHON,
            uid::SHUTTLE,
            uid::SCOUT,
            uid::ARBITER,
            uid::CARRIER,
            uid::REAVER,
            uid::OBSERVER,
            uid::CORSAIR,
            uid::DARK_TEMPLAR,
            uid::DARK_ARCHON,
        ] {
            let folder = folder_for(id).unwrap_or_else(|| panic!("unit {id} has no voice set"));
            assert!(folders.contains(&folder.to_string()), "unit {id}: {folder}");
        }
        assert_eq!(folder_for(uid::MINERAL_FIELD), None);
    }

    #[test]
    fn cues_resolve_to_the_shipped_takes() {
        let table = SfxTable::bundled();
        assert_eq!(
            table
                .cache_path(voice_id(&table, uid::MARINE, VoiceCue::Ready, 0).unwrap())
                .as_deref(),
            Some("terran/marine/tmardy00.wav")
        );
        assert_eq!(
            table
                .cache_path(voice_id(&table, uid::MARINE, VoiceCue::Yes, 0).unwrap())
                .as_deref(),
            Some("terran/marine/tmayes00.wav")
        );
        assert_eq!(
            table
                .cache_path(voice_id(&table, uid::MARINE, VoiceCue::Pissed, 2).unwrap())
                .as_deref(),
            Some("terran/marine/tmapss02.wav")
        );
        assert_eq!(
            table
                .cache_path(voice_id(&table, uid::ZEALOT, VoiceCue::Ready, 0).unwrap())
                .as_deref(),
            Some("protoss/zealot/pzerdy00.wav")
        );
    }

    #[test]
    fn cue_classification_covers_the_shipped_stems() {
        for (stem, cue) in [
            ("TMaRdy00", VoiceCue::Ready),
            ("TMaWht03", VoiceCue::What),
            ("TMaPss06", VoiceCue::Pissed),
            ("TMaYes02", VoiceCue::Yes),
            ("TMaDth01", VoiceCue::Death),
            ("TMaHit00", VoiceCue::Hit),
            ("ZZeAtk00", VoiceCue::Attack),
            ("PZeRag00", VoiceCue::Special),
        ] {
            assert_eq!(VoiceCue::from_stem(stem), Some(cue), "{stem}");
        }
    }
}

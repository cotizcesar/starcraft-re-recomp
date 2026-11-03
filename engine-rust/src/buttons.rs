//! Command card of a unit: the retail button set ([`crate::button_sets`])
//! filtered by its condition functions.
//!
//! The condition addresses are classified by how the table uses them
//! (INFERENCE, 2026-11-03; each line names the evidence).  Addresses not
//! classified below are shown enabled, the conservative choice for a
//! button that exists in the set.  Falsifier for any line: the retail card
//! of a unit in that state showing/hiding the button differently.

use crate::button_sets::{Button, BUTTON_SETS};
use crate::sim::{SimUnit, World};

/// Semantic class of a retail button condition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cond {
    /// `0x65f590`: Move/Stop/Attack/Patrol/Hold of mobile units (318 uses).
    Always,
    /// `0x780480`: make the unit `cond_var` (train/build/morph; 87 uses,
    /// all with `cond_var == act_var` = a unit id); greyed while its
    /// requirements are unmet (Lair before a Spawning Pool, 134905).
    MakeUnit,
    /// `0x780690`: Set Rally Point of producing buildings.
    Rally,
    /// Cancel buttons (icon 236): only while something is in progress.
    Cancel,
    /// `0x780820`: research tech `cond_var` (Burrowing at the Hatchery).
    Research,
    /// `0x780890`: upgrade `cond_var` at its building.
    Upgrade,
    /// `0x780830`: use tech `cond_var` (Stim Packs on Marines).
    UseTech,
    /// `0x780b50`: Move/Stop of a lifted Terran building.
    WhenLifted,
    /// `0x780b70`: Land (icon 283).
    Land,
    /// `0x780ba0`: Lift Off (icon 282).
    LiftOff,
    /// `0x780be0`: Burrow (icon 259, tech 11) of an unburrowed unit.
    Burrow,
    /// `0x780c50`: Unburrow (icon 260).
    Unburrow,
    Unclassified,
}

impl Cond {
    pub fn of(addr: u32) -> Cond {
        match addr {
            0x65f590 => Cond::Always,
            0x780480 => Cond::MakeUnit,
            0x780690 | 0x7806a0 => Cond::Rally,
            0x780620 | 0x780630 | 0x780640 | 0x7806f0 | 0x780710 | 0x780720 | 0x781270 => {
                Cond::Cancel
            }
            0x780820 => Cond::Research,
            0x780890 => Cond::Upgrade,
            0x780830 => Cond::UseTech,
            0x780b50 => Cond::WhenLifted,
            0x780b70 => Cond::Land,
            0x780ba0 => Cond::LiftOff,
            0x780be0 => Cond::Burrow,
            0x780c50 => Cond::Unburrow,
            _ => Cond::Unclassified,
        }
    }
}

/// How a card slot is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonState {
    Enabled,
    /// Shown greyed (its disabled tooltip applies).
    Disabled,
}

/// Visibility of one button for `unit` (`None` = hidden).
pub fn state(world: &World, unit: &SimUnit, b: &Button) -> Option<ButtonState> {
    let player = world.players.get(unit.owner as usize)?;
    let busy = !unit.completed || !unit.production.is_empty();
    let on = |shown: bool| shown.then_some(ButtonState::Enabled);
    match Cond::of(b.cond) {
        Cond::Always | Cond::Rally | Cond::Upgrade | Cond::Unclassified => on(true),
        Cond::MakeUnit => Some(if world.has_requirements(unit.owner, b.cond_var) {
            ButtonState::Enabled
        } else {
            ButtonState::Disabled
        }),
        Cond::Cancel => on(busy),
        Cond::Research => on(!player.has_tech(b.cond_var as u8)),
        Cond::UseTech => on(player.has_tech(b.cond_var as u8)),
        Cond::WhenLifted | Cond::Land => on(unit.lifted),
        Cond::LiftOff => on(!unit.lifted),
        Cond::Burrow => on(player.has_tech(b.cond_var as u8) && !unit.status.burrowed),
        Cond::Unburrow => on(unit.status.burrowed),
    }
}

/// The nine card slots (position 1..=9 -> index 0..=8): the first visible
/// button of each position, as retail stacks alternatives (Lift Off /
/// Land / Cancel all live in position 9 of the Command Center).
pub fn card(world: &World, unit: &SimUnit) -> [Option<(Button, ButtonState)>; 9] {
    let mut out = [None; 9];
    let set = BUTTON_SETS
        .get(unit.type_id as usize)
        .copied()
        .unwrap_or(&[]);
    for b in set {
        let i = (b.pos as usize).wrapping_sub(1);
        if i < 9 && out[i].is_none() {
            if let Some(s) = state(world, unit, b) {
                out[i] = Some((*b, s));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::unit as uid;
    use crate::map::Terrain;
    use crate::sim::{Race, World};
    use std::sync::Arc;

    fn world_with(race: Race, town: u16) -> (World, usize) {
        let data = Arc::new(crate::data::GameData::bundled().expect("tables"));
        let mut w = World::new(data, Terrain::open(64, 64), 2);
        w.players[0].race = race;
        let i = w.spawn_px(0, town, 512, 512).expect("spawn");
        w.units[i].completed = true;
        (w, i)
    }

    #[test]
    fn nexus_at_game_start_shows_probe_and_rally_only() {
        // 135051: Probe in slot 1, Set Rally Point in slot 6, nothing else.
        let (w, i) = world_with(Race::Protoss, uid::NEXUS);
        let c = card(&w, &w.units[i]);
        let shown: Vec<(usize, u16, ButtonState)> = c
            .iter()
            .enumerate()
            .filter_map(|(k, s)| s.map(|(b, st)| (k, b.icon, st)))
            .collect();
        assert_eq!(
            shown,
            vec![
                (0, 64, ButtonState::Enabled),
                (5, 286, ButtonState::Enabled)
            ]
        );
    }

    #[test]
    fn hatchery_greys_lair_until_a_spawning_pool_exists() {
        // 134905: Larva, Rally, Burrow research on top; Lair greyed in slot 7.
        let (mut w, i) = world_with(Race::Zerg, uid::HATCHERY);
        let c = card(&w, &w.units[i]);
        let icons: Vec<Option<u16>> = c.iter().map(|s| s.map(|(b, _)| b.icon)).collect();
        assert_eq!(icons[0..3], [Some(35), Some(286), Some(259)]);
        assert_eq!(
            c[6].map(|(b, s)| (b.icon, s)),
            Some((132, ButtonState::Disabled))
        );
        assert!(c[8].is_none(), "no cancel while idle");
        let pool = w.spawn_px(0, uid::SPAWNING_POOL, 900, 900).expect("pool");
        w.units[pool].completed = true;
        let c = card(&w, &w.units[i]);
        assert_eq!(c[6].map(|(_, s)| s), Some(ButtonState::Enabled));
    }

    #[test]
    fn every_set_uses_card_positions_one_to_nine() {
        for (uid, set) in BUTTON_SETS.iter().enumerate() {
            for b in set.iter() {
                assert!((1..=9).contains(&b.pos), "unit {uid}: pos {}", b.pos);
            }
        }
    }
}

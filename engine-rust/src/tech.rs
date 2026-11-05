//! The hard-coded build/research tree.
//!
//! `units.dat` does not carry unit prerequisites; the executable's
//! `unit_can_build`-equivalent switch contains them.  The table below is the
//! same switch recovered from the 1.23.10 runtime image (and independently
//! cross-checked against OpenBW's `unit_can_build`).  Producers, required
//! buildings and the research/upgrade buildings are all listed here so the
//! simulator can enforce the tree instead of silently ignoring it.

use crate::ids::{tech, unit, upgrade};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildInfo {
    /// The unit that produces/morphs this one.
    pub producer: u16,
    /// Completed buildings/units that must exist before production starts.
    pub required_units: &'static [(u16, u8)],
    /// A technology that must be researched (`Lurker Aspect`).
    pub required_tech: Option<u8>,
}

const fn b(producer: u16, required_units: &'static [(u16, u8)]) -> BuildInfo {
    BuildInfo {
        producer,
        required_units,
        required_tech: None,
    }
}

pub fn build_info(unit_id: u16) -> Option<BuildInfo> {
    Some(match unit_id {
        // Terran units
        unit::MARINE => b(unit::BARRACKS, &[]),
        unit::GHOST => b(unit::BARRACKS, &[(unit::COVERT_OPS, 1), (unit::ACADEMY, 1)]),
        unit::VULTURE => b(unit::FACTORY, &[]),
        unit::GOLIATH => b(unit::FACTORY, &[(unit::ARMORY, 1)]),
        unit::SIEGE_TANK => b(unit::FACTORY, &[]),
        unit::SCV => b(unit::COMMAND_CENTER, &[]),
        unit::WRAITH => b(unit::STARPORT, &[]),
        unit::SCIENCE_VESSEL => b(unit::STARPORT, &[(unit::SCIENCE_FACILITY, 1)]),
        unit::DROPSHIP => b(unit::STARPORT, &[]),
        unit::BATTLECRUISER => b(unit::STARPORT, &[(unit::PHYSICS_LAB, 1)]),
        unit::FIREBAT => b(unit::BARRACKS, &[(unit::ACADEMY, 1)]),
        unit::MEDIC => b(unit::BARRACKS, &[(unit::ACADEMY, 1)]),
        unit::VALKYRIE => b(unit::STARPORT, &[(unit::ARMORY, 1)]),
        unit::INFESTED_TERRAN => b(unit::INFESTED_COMMAND_CENTER, &[]),

        // Zerg units
        unit::ZERGLING => b(unit::LARVA, &[(unit::SPAWNING_POOL, 1)]),
        unit::HYDRALISK => b(unit::LARVA, &[(unit::HYDRALISK_DEN, 1)]),
        unit::ULTRALISK => b(unit::LARVA, &[(unit::ULTRALISK_CAVERN, 1)]),
        unit::DRONE => b(unit::LARVA, &[]),
        unit::OVERLORD => b(unit::LARVA, &[]),
        unit::MUTALISK => b(unit::LARVA, &[(unit::SPIRE, 1)]),
        unit::GUARDIAN => b(unit::MUTALISK, &[(unit::GREATER_SPIRE, 1)]),
        unit::QUEEN => b(unit::LARVA, &[(unit::QUEENS_NEST, 1)]),
        unit::DEFILER => b(unit::LARVA, &[(unit::DEFILER_MOUND, 1)]),
        unit::SCOURGE => b(unit::LARVA, &[(unit::SPIRE, 1)]),
        unit::DEVOURER => b(unit::MUTALISK, &[(unit::GREATER_SPIRE, 1)]),
        unit::LURKER => BuildInfo {
            producer: unit::HYDRALISK,
            required_units: &[],
            required_tech: Some(tech::LURKER_ASPECT),
        },

        // Protoss units
        unit::PROBE => b(unit::NEXUS, &[]),
        unit::ZEALOT => b(unit::GATEWAY, &[]),
        unit::DRAGOON => b(unit::GATEWAY, &[(unit::CYBERNETICS_CORE, 1)]),
        unit::HIGH_TEMPLAR => b(unit::GATEWAY, &[(unit::TEMPLAR_ARCHIVES, 1)]),
        unit::DARK_TEMPLAR => b(unit::GATEWAY, &[(unit::TEMPLAR_ARCHIVES, 1)]),
        unit::SHUTTLE => b(unit::ROBOTICS_FACILITY, &[]),
        unit::REAVER => b(unit::ROBOTICS_FACILITY, &[(unit::ROBOTICS_SUPPORT_BAY, 1)]),
        unit::OBSERVER => b(unit::ROBOTICS_FACILITY, &[(unit::OBSERVATORY, 1)]),
        unit::SCOUT => b(unit::STARGATE, &[]),
        unit::CARRIER => b(unit::STARGATE, &[(unit::FLEET_BEACON, 1)]),
        unit::ARBITER => b(unit::STARGATE, &[(unit::ARBITER_TRIBUNAL, 1)]),
        unit::CORSAIR => b(unit::STARGATE, &[]),

        // Terran buildings
        unit::COMMAND_CENTER => b(unit::SCV, &[]),
        unit::COMSAT_STATION => b(unit::COMMAND_CENTER, &[(unit::ACADEMY, 1)]),
        unit::NUCLEAR_SILO => b(unit::COMMAND_CENTER, &[(unit::COVERT_OPS, 1)]),
        unit::SUPPLY_DEPOT => b(unit::SCV, &[]),
        unit::REFINERY => b(unit::SCV, &[]),
        unit::BARRACKS => b(unit::SCV, &[(unit::COMMAND_CENTER, 1)]),
        unit::ACADEMY => b(unit::SCV, &[(unit::BARRACKS, 1)]),
        unit::FACTORY => b(unit::SCV, &[(unit::BARRACKS, 1)]),
        unit::STARPORT => b(unit::SCV, &[(unit::FACTORY, 1)]),
        unit::CONTROL_TOWER => b(unit::STARPORT, &[]),
        unit::SCIENCE_FACILITY => b(unit::SCV, &[(unit::STARPORT, 1)]),
        unit::COVERT_OPS => b(unit::SCIENCE_FACILITY, &[]),
        unit::PHYSICS_LAB => b(unit::SCIENCE_FACILITY, &[]),
        unit::MACHINE_SHOP => b(unit::FACTORY, &[]),
        unit::ENGINEERING_BAY => b(unit::SCV, &[(unit::COMMAND_CENTER, 1)]),
        unit::ARMORY => b(unit::SCV, &[(unit::FACTORY, 1)]),
        unit::MISSILE_TURRET => b(unit::SCV, &[(unit::ENGINEERING_BAY, 1)]),
        unit::BUNKER => b(unit::SCV, &[(unit::BARRACKS, 1)]),

        // Zerg buildings
        unit::HATCHERY => b(unit::DRONE, &[]),
        unit::LAIR => b(unit::HATCHERY, &[(unit::SPAWNING_POOL, 1)]),
        unit::HIVE => b(unit::LAIR, &[(unit::QUEENS_NEST, 1)]),
        unit::SPAWNING_POOL => b(unit::DRONE, &[(unit::HATCHERY, 1)]),
        unit::HYDRALISK_DEN => b(unit::DRONE, &[(unit::SPAWNING_POOL, 1)]),
        unit::DEFILER_MOUND => b(unit::DRONE, &[(unit::HIVE, 1)]),
        unit::GREATER_SPIRE => b(unit::SPIRE, &[(unit::HIVE, 1)]),
        unit::QUEENS_NEST => b(unit::DRONE, &[(unit::LAIR, 1)]),
        unit::EVOLUTION_CHAMBER => b(unit::DRONE, &[(unit::HATCHERY, 1)]),
        unit::ULTRALISK_CAVERN => b(unit::DRONE, &[(unit::HIVE, 1)]),
        unit::SPIRE => b(unit::DRONE, &[(unit::LAIR, 1)]),
        unit::NYDUS_CANAL => b(unit::DRONE, &[(unit::HIVE, 1)]),
        unit::CREEP_COLONY => b(unit::DRONE, &[]),
        unit::SPORE_COLONY => b(unit::CREEP_COLONY, &[(unit::EVOLUTION_CHAMBER, 1)]),
        unit::SUNKEN_COLONY => b(unit::CREEP_COLONY, &[(unit::SPAWNING_POOL, 1)]),
        unit::EXTRACTOR => b(unit::DRONE, &[]),

        // Protoss buildings
        unit::NEXUS => b(unit::PROBE, &[]),
        unit::ROBOTICS_FACILITY => b(unit::PROBE, &[(unit::CYBERNETICS_CORE, 1)]),
        unit::PYLON => b(unit::PROBE, &[]),
        unit::ASSIMILATOR => b(unit::PROBE, &[]),
        unit::OBSERVATORY => b(unit::PROBE, &[(unit::ROBOTICS_FACILITY, 1)]),
        unit::GATEWAY => b(unit::PROBE, &[(unit::NEXUS, 1)]),
        unit::PHOTON_CANNON => b(unit::PROBE, &[(unit::FORGE, 1)]),
        unit::CITADEL_OF_ADUN => b(unit::PROBE, &[(unit::CYBERNETICS_CORE, 1)]),
        unit::CYBERNETICS_CORE => b(unit::PROBE, &[(unit::GATEWAY, 1)]),
        unit::TEMPLAR_ARCHIVES => b(unit::PROBE, &[(unit::CITADEL_OF_ADUN, 1)]),
        unit::FORGE => b(unit::PROBE, &[(unit::NEXUS, 1)]),
        unit::STARGATE => b(unit::PROBE, &[(unit::CYBERNETICS_CORE, 1)]),
        unit::FLEET_BEACON => b(unit::PROBE, &[(unit::STARGATE, 1)]),
        unit::ARBITER_TRIBUNAL => b(
            unit::PROBE,
            &[(unit::STARGATE, 1), (unit::TEMPLAR_ARCHIVES, 1)],
        ),
        unit::ROBOTICS_SUPPORT_BAY => b(unit::PROBE, &[(unit::ROBOTICS_FACILITY, 1)]),
        unit::SHIELD_BATTERY => b(unit::PROBE, &[(unit::GATEWAY, 1)]),
        _ => return None,
    })
}

/// Buildings that can research a technology.
pub fn tech_buildings(tech_id: u8) -> &'static [u16] {
    match tech_id {
        tech::STIM_PACKS | tech::RESTORATION | tech::OPTICAL_FLARE => &[unit::ACADEMY],
        tech::LOCKDOWN | tech::PERSONNEL_CLOAKING => &[unit::COVERT_OPS],
        tech::SPIDER_MINES | tech::TANK_SIEGE_MODE => &[unit::MACHINE_SHOP],
        tech::EMP_SHOCKWAVE | tech::IRRADIATE => &[unit::SCIENCE_FACILITY],
        tech::YAMATO_GUN => &[unit::PHYSICS_LAB],
        tech::CLOAKING_FIELD => &[unit::CONTROL_TOWER],
        tech::BURROWING => &[unit::HATCHERY, unit::LAIR, unit::HIVE],
        tech::SPAWN_BROODLINGS | tech::PARASITE => &[unit::QUEENS_NEST],
        tech::PLAGUE | tech::CONSUME => &[unit::DEFILER_MOUND],
        tech::LURKER_ASPECT => &[unit::HYDRALISK_DEN],
        tech::PSIONIC_STORM
        | tech::HALLUCINATION
        | tech::MIND_CONTROL
        | tech::FEEDBACK
        | tech::MAELSTROM => &[unit::TEMPLAR_ARCHIVES],
        tech::RECALL | tech::STASIS_FIELD => &[unit::ARBITER_TRIBUNAL],
        tech::DISRUPTION_WEB => &[unit::FLEET_BEACON],
        _ => &[],
    }
}

/// Buildings that can research an upgrade.
pub fn upgrade_buildings(upgrade_id: u8) -> &'static [u16] {
    match upgrade_id {
        upgrade::TERRAN_INFANTRY_ARMOR | upgrade::TERRAN_INFANTRY_WEAPONS => {
            &[unit::ENGINEERING_BAY]
        }
        upgrade::TERRAN_VEHICLE_PLATING
        | upgrade::TERRAN_SHIP_PLATING
        | upgrade::TERRAN_VEHICLE_WEAPONS
        | upgrade::TERRAN_SHIP_WEAPONS => &[unit::ARMORY],
        upgrade::ZERG_CARAPACE | upgrade::ZERG_MELEE_ATTACKS | upgrade::ZERG_MISSILE_ATTACKS => {
            &[unit::EVOLUTION_CHAMBER]
        }
        upgrade::ZERG_FLYER_CARAPACE | upgrade::ZERG_FLYER_ATTACKS => {
            &[unit::SPIRE, unit::GREATER_SPIRE]
        }
        upgrade::PROTOSS_GROUND_ARMOR | upgrade::PROTOSS_GROUND_WEAPONS => &[unit::FORGE],
        upgrade::PROTOSS_AIR_ARMOR | upgrade::PROTOSS_AIR_WEAPONS | upgrade::SINGULARITY_CHARGE => {
            &[unit::CYBERNETICS_CORE]
        }
        upgrade::PROTOSS_PLASMA_SHIELDS => &[unit::FORGE],
        upgrade::U238_SHELLS | upgrade::CADUCEUS_REACTOR => &[unit::ACADEMY],
        upgrade::ION_THRUSTERS | upgrade::CHARON_BOOSTERS => &[unit::MACHINE_SHOP],
        upgrade::TITAN_REACTOR => &[unit::SCIENCE_FACILITY],
        upgrade::OCULAR_IMPLANTS | upgrade::MOEBIUS_REACTOR => &[unit::COVERT_OPS],
        upgrade::APOLLO_REACTOR => &[unit::CONTROL_TOWER],
        upgrade::COLOSSUS_REACTOR => &[unit::PHYSICS_LAB],
        upgrade::VENTRAL_SACS | upgrade::ANTENNAE | upgrade::PNEUMATIZED_CARAPACE => {
            &[unit::LAIR, unit::HIVE]
        }
        upgrade::METABOLIC_BOOST | upgrade::ADRENAL_GLANDS => &[unit::SPAWNING_POOL],
        upgrade::MUSCULAR_AUGMENTS | upgrade::GROOVED_SPINES => &[unit::HYDRALISK_DEN],
        upgrade::GAMETE_MEIOSIS => &[unit::QUEENS_NEST],
        upgrade::METASYNAPTIC_NODE => &[unit::DEFILER_MOUND],
        upgrade::LEG_ENHANCEMENTS => &[unit::CITADEL_OF_ADUN],
        upgrade::SCARAB_DAMAGE | upgrade::REAVER_CAPACITY | upgrade::GRAVITIC_DRIVE => {
            &[unit::ROBOTICS_SUPPORT_BAY]
        }
        upgrade::SENSOR_ARRAY | upgrade::GRAVITIC_BOOSTERS => &[unit::OBSERVATORY],
        upgrade::KHAYDARIN_AMULET | upgrade::ARGUS_TALISMAN => &[unit::TEMPLAR_ARCHIVES],
        upgrade::APIAL_SENSORS
        | upgrade::GRAVITIC_THRUSTERS
        | upgrade::CARRIER_CAPACITY
        | upgrade::ARGUS_JEWEL => &[unit::FLEET_BEACON],
        upgrade::KHAYDARIN_CORE => &[unit::ARBITER_TRIBUNAL],
        upgrade::CHITINOUS_PLATING | upgrade::ANABOLIC_SYNTHESIS => &[unit::ULTRALISK_CAVERN],
        _ => &[],
    }
}

/// Extra building gates for upgrade levels beyond the first.
pub fn upgrade_level_requirement(upgrade_id: u8, current_level: u8) -> &'static [u16] {
    if current_level == 0 {
        return &[];
    }
    match upgrade_id {
        upgrade::TERRAN_INFANTRY_ARMOR
        | upgrade::TERRAN_INFANTRY_WEAPONS
        | upgrade::TERRAN_VEHICLE_PLATING
        | upgrade::TERRAN_SHIP_PLATING
        | upgrade::TERRAN_VEHICLE_WEAPONS
        | upgrade::TERRAN_SHIP_WEAPONS => &[unit::SCIENCE_FACILITY],
        upgrade::ZERG_CARAPACE
        | upgrade::ZERG_FLYER_CARAPACE
        | upgrade::ZERG_MELEE_ATTACKS
        | upgrade::ZERG_MISSILE_ATTACKS
        | upgrade::ZERG_FLYER_ATTACKS => {
            if current_level >= 2 {
                &[unit::HIVE]
            } else {
                &[unit::LAIR]
            }
        }
        upgrade::PROTOSS_GROUND_ARMOR | upgrade::PROTOSS_GROUND_WEAPONS => {
            &[unit::TEMPLAR_ARCHIVES]
        }
        upgrade::PROTOSS_AIR_ARMOR
        | upgrade::PROTOSS_AIR_WEAPONS
        | upgrade::PROTOSS_PLASMA_SHIELDS => &[unit::FLEET_BEACON],
        upgrade::ADRENAL_GLANDS => &[unit::HIVE],
        _ => &[],
    }
}

/// Number of completed units of `type_id` owned by a player is tracked by the
/// caller; this module only returns the static tree.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;

    #[test]
    fn every_buildable_unit_has_a_producer() {
        let data = GameData::bundled().unwrap();
        let mut count = 0;
        for id in 0..data.units.len() as u16 {
            let u = data.unit(id);
            if (u.is_building() || u.is_worker() || matches!(id, unit::MARINE..=unit::MEDIC))
                && let Some(info) = build_info(id)
            {
                assert!(info.producer < data.units.len() as u16);
                count += 1;
            }
        }
        assert!(count > 50);
    }

    #[test]
    fn barracks_requires_command_center() {
        let info = build_info(unit::BARRACKS).unwrap();
        assert_eq!(info.producer, unit::SCV);
        assert!(info.required_units.contains(&(unit::COMMAND_CENTER, 1)));
    }

    #[test]
    fn lurker_requires_lurker_aspect() {
        let info = build_info(unit::LURKER).unwrap();
        assert_eq!(info.producer, unit::HYDRALISK);
        assert_eq!(info.required_tech, Some(tech::LURKER_ASPECT));
    }

    #[test]
    fn research_buildings_are_known() {
        assert_eq!(tech_buildings(tech::STIM_PACKS), &[unit::ACADEMY]);
        assert_eq!(
            tech_buildings(tech::PSIONIC_STORM),
            &[unit::TEMPLAR_ARCHIVES]
        );
        assert_eq!(
            upgrade_buildings(upgrade::PROTOSS_PLASMA_SHIELDS),
            &[unit::FORGE]
        );
    }
}

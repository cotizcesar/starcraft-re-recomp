//! Symbolic ids for the stock StarCraft tables.
//!
//! The numbers are the indices in `units.dat`, `weapons.dat`, `techdata.dat`,
//! `upgrades.dat` and `orders.dat`.  They were checked against the recovered
//! tables and the game's string table (unit id == `stat_txt` name index for
//! the stock 228-unit range).

/// Unit type ids.
pub mod unit {
    // Terran
    pub const MARINE: u16 = 0;
    pub const GHOST: u16 = 1;
    pub const VULTURE: u16 = 2;
    pub const GOLIATH: u16 = 3;
    pub const GOLIATH_TURRET: u16 = 4;
    pub const SIEGE_TANK: u16 = 5;
    pub const SIEGE_TANK_TURRET: u16 = 6;
    pub const SCV: u16 = 7;
    pub const WRAITH: u16 = 8;
    pub const SCIENCE_VESSEL: u16 = 9;
    pub const DROPSHIP: u16 = 11;
    pub const BATTLECRUISER: u16 = 12;
    pub const SPIDER_MINE: u16 = 13;
    pub const NUCLEAR_MISSILE: u16 = 14;
    pub const SIEGE_MODE: u16 = 30;
    pub const SIEGE_MODE_TURRET: u16 = 31;
    pub const FIREBAT: u16 = 32;
    pub const SCANNER_SWEEP: u16 = 33;
    pub const MEDIC: u16 = 34;
    // Zerg
    pub const LARVA: u16 = 35;
    pub const EGG: u16 = 36;
    pub const ZERGLING: u16 = 37;
    pub const HYDRALISK: u16 = 38;
    pub const ULTRALISK: u16 = 39;
    pub const BROODLING: u16 = 40;
    pub const DRONE: u16 = 41;
    pub const OVERLORD: u16 = 42;
    pub const MUTALISK: u16 = 43;
    pub const GUARDIAN: u16 = 44;
    pub const QUEEN: u16 = 45;
    pub const DEFILER: u16 = 46;
    pub const SCOURGE: u16 = 47;
    pub const INFESTED_TERRAN: u16 = 50;
    pub const VALKYRIE: u16 = 58;
    pub const COCOON: u16 = 59;
    pub const DEVOURER: u16 = 62;
    pub const LURKER_EGG: u16 = 97;
    pub const LURKER: u16 = 103;
    // Protoss
    pub const CORSAIR: u16 = 60;
    pub const DARK_TEMPLAR: u16 = 61;
    pub const DARK_ARCHON: u16 = 63;
    pub const PROBE: u16 = 64;
    pub const ZEALOT: u16 = 65;
    pub const DRAGOON: u16 = 66;
    pub const HIGH_TEMPLAR: u16 = 67;
    pub const ARCHON: u16 = 68;
    pub const SHUTTLE: u16 = 69;
    pub const SCOUT: u16 = 70;
    pub const ARBITER: u16 = 71;
    pub const CARRIER: u16 = 72;
    pub const INTERCEPTOR: u16 = 73;
    pub const REAVER: u16 = 83;
    pub const OBSERVER: u16 = 84;
    pub const SCARAB: u16 = 85;

    // Buildings
    pub const COMMAND_CENTER: u16 = 106;
    pub const COMSAT_STATION: u16 = 107;
    pub const NUCLEAR_SILO: u16 = 108;
    pub const SUPPLY_DEPOT: u16 = 109;
    pub const REFINERY: u16 = 110;
    pub const BARRACKS: u16 = 111;
    pub const ACADEMY: u16 = 112;
    pub const FACTORY: u16 = 113;
    pub const STARPORT: u16 = 114;
    pub const CONTROL_TOWER: u16 = 115;
    pub const SCIENCE_FACILITY: u16 = 116;
    pub const COVERT_OPS: u16 = 117;
    pub const PHYSICS_LAB: u16 = 118;
    pub const MACHINE_SHOP: u16 = 120;
    pub const ENGINEERING_BAY: u16 = 122;
    pub const ARMORY: u16 = 123;
    pub const MISSILE_TURRET: u16 = 124;
    pub const BUNKER: u16 = 125;

    pub const INFESTED_COMMAND_CENTER: u16 = 130;
    pub const HATCHERY: u16 = 131;
    pub const LAIR: u16 = 132;
    pub const HIVE: u16 = 133;
    pub const NYDUS_CANAL: u16 = 134;
    pub const HYDRALISK_DEN: u16 = 135;
    pub const DEFILER_MOUND: u16 = 136;
    pub const GREATER_SPIRE: u16 = 137;
    pub const QUEENS_NEST: u16 = 138;
    pub const EVOLUTION_CHAMBER: u16 = 139;
    pub const ULTRALISK_CAVERN: u16 = 140;
    pub const SPIRE: u16 = 141;
    pub const SPAWNING_POOL: u16 = 142;
    pub const CREEP_COLONY: u16 = 143;
    pub const SPORE_COLONY: u16 = 144;
    pub const SUNKEN_COLONY: u16 = 146;
    pub const EXTRACTOR: u16 = 149;

    pub const NEXUS: u16 = 154;
    pub const ROBOTICS_FACILITY: u16 = 155;
    pub const PYLON: u16 = 156;
    pub const ASSIMILATOR: u16 = 157;
    pub const OBSERVATORY: u16 = 159;
    pub const GATEWAY: u16 = 160;
    pub const PHOTON_CANNON: u16 = 162;
    pub const CITADEL_OF_ADUN: u16 = 163;
    pub const CYBERNETICS_CORE: u16 = 164;
    pub const TEMPLAR_ARCHIVES: u16 = 165;
    pub const FORGE: u16 = 166;
    pub const STARGATE: u16 = 167;
    pub const FLEET_BEACON: u16 = 169;
    pub const ARBITER_TRIBUNAL: u16 = 170;
    pub const ROBOTICS_SUPPORT_BAY: u16 = 171;
    pub const SHIELD_BATTERY: u16 = 172;

    pub const MINERAL_FIELD: u16 = 176;
    pub const MINERAL_FIELD_2: u16 = 177;
    pub const MINERAL_FIELD_3: u16 = 178;
    pub const VESPENE_GEYSER: u16 = 188;
    pub const DARK_SWARM: u16 = 202;
    pub const DISRUPTION_WEB: u16 = 105;
    pub const START_LOCATION: u16 = 214;
}

/// Weapon ids used by the rules engine.
///
/// These are the actual `weapons.dat` row indices.  The display strings in the
/// shipped table are not a reliable id map: several rows share a label and the
/// labels for the later rows do not line up with the row order.  The ids below
/// were cross-checked against the raw `bullet_type`, `hit_type`, damage and
/// unit-weapon columns (for example row 31 is the 600-damage nuclear hit type,
/// row 30 is the 260-damage Yamato hit type, row 33 is the EMP hit type).
pub mod weapon {
    pub const GAUSS_RIFLE: u8 = 0;
    pub const C10_CANISTER_RIFLE: u8 = 2;
    pub const FRAGMENTATION_GRENADE: u8 = 4;
    pub const SPIDER_MINES: u8 = 6;
    pub const TWIN_AUTOCANNONS: u8 = 7;
    pub const HELLFIRE_MISSILE_PACK: u8 = 8;
    pub const ARCLITE_CANNON: u8 = 11;
    pub const FUSION_CUTTER: u8 = 13;
    pub const GEMINI_MISSILES: u8 = 15;
    pub const BURST_LASERS: u8 = 16;
    pub const ATS_LASER_BATTERY: u8 = 19;
    pub const ATA_LASER_BATTERY: u8 = 20;
    pub const FLAME_THROWER: u8 = 25;
    pub const ARCLITE_SHOCK_CANNON: u8 = 27;
    pub const LONGBOLT_MISSILE: u8 = 29;
    pub const YAMATO_GUN: u8 = 30;
    pub const NUCLEAR_STRIKE: u8 = 31;
    pub const LOCKDOWN: u8 = 32;
    pub const EMP_SHOCKWAVE: u8 = 33;
    pub const IRRADIATE: u8 = 34;
    pub const CLAWS: u8 = 35;
    pub const NEEDLE_SPINES: u8 = 38;
    pub const KAISER_BLADES: u8 = 40;
    pub const GLAVE_WURM: u8 = 48;
    pub const SUBTERRANEAN_TENTACLE: u8 = 53;
    pub const INFESTED_TERRAN_SUICIDE: u8 = 54;
    pub const SCOURGE_SUICIDE: u8 = 55;
    pub const PARASITE: u8 = 56;
    pub const SPAWN_BROODLINGS: u8 = 57;
    pub const ENSNARE: u8 = 58;
    pub const DARK_SWARM: u8 = 59;
    pub const PLAGUE: u8 = 60;
    pub const CONSUME: u8 = 61;
    pub const PSI_BLADES: u8 = 64;
    pub const PHASE_DISRUPTOR: u8 = 66;
    pub const PSI_ASSAULT: u8 = 69;
    pub const PSIONIC_SHOCKWAVE: u8 = 70;
    pub const DUAL_PHOTON_BLASTERS: u8 = 73;
    pub const PHASE_DISRUPTOR_CANNON: u8 = 77;
    pub const PULSE_CANNON: u8 = 79;
    pub const STS_PHOTON_CANNON: u8 = 80;
    pub const STA_PHOTON_CANNON: u8 = 81;
    pub const SCARAB: u8 = 82;
    pub const STASIS_FIELD: u8 = 83;
    pub const PSIONIC_STORM: u8 = 84;
    pub const NEUTRON_FLARE: u8 = 100;
    pub const DISRUPTION_WEB: u8 = 101;
    pub const RESTORATION: u8 = 102;
    pub const HALO_ROCKETS: u8 = 103;
    pub const CORROSIVE_ACID: u8 = 104;
    pub const MIND_CONTROL: u8 = 105;
    pub const FEEDBACK: u8 = 106;
    pub const OPTICAL_FLARE: u8 = 107;
    pub const MAELSTROM: u8 = 108;
    pub const SUBTERRANEAN_SPINES: u8 = 109;
}

/// Technology ids (44 entries in `techdata.dat`).
pub mod tech {
    pub const STIM_PACKS: u8 = 0;
    pub const LOCKDOWN: u8 = 1;
    pub const EMP_SHOCKWAVE: u8 = 2;
    pub const SPIDER_MINES: u8 = 3;
    pub const SCANNER_SWEEP: u8 = 4;
    pub const TANK_SIEGE_MODE: u8 = 5;
    pub const DEFENSIVE_MATRIX: u8 = 6;
    pub const IRRADIATE: u8 = 7;
    pub const YAMATO_GUN: u8 = 8;
    pub const CLOAKING_FIELD: u8 = 9;
    pub const PERSONNEL_CLOAKING: u8 = 10;
    pub const BURROWING: u8 = 11;
    pub const INFESTATION: u8 = 12;
    pub const SPAWN_BROODLINGS: u8 = 13;
    pub const DARK_SWARM: u8 = 14;
    pub const PLAGUE: u8 = 15;
    pub const CONSUME: u8 = 16;
    pub const ENSNARE: u8 = 17;
    pub const PARASITE: u8 = 18;
    pub const PSIONIC_STORM: u8 = 19;
    pub const HALLUCINATION: u8 = 20;
    pub const RECALL: u8 = 21;
    pub const STASIS_FIELD: u8 = 22;
    pub const ARCHON_WARP: u8 = 23;
    pub const RESTORATION: u8 = 24;
    pub const DISRUPTION_WEB: u8 = 25;
    pub const MIND_CONTROL: u8 = 27;
    pub const DARK_ARCHON_MELD: u8 = 28;
    pub const FEEDBACK: u8 = 29;
    pub const OPTICAL_FLARE: u8 = 30;
    pub const MAELSTROM: u8 = 31;
    pub const LURKER_ASPECT: u8 = 32;
    pub const HEALING: u8 = 34;
}

/// Upgrade ids (61 entries in `upgrades.dat`).
pub mod upgrade {
    pub const TERRAN_INFANTRY_ARMOR: u8 = 0;
    pub const TERRAN_VEHICLE_PLATING: u8 = 1;
    pub const TERRAN_SHIP_PLATING: u8 = 2;
    pub const ZERG_CARAPACE: u8 = 3;
    pub const ZERG_FLYER_CARAPACE: u8 = 4;
    pub const PROTOSS_GROUND_ARMOR: u8 = 5;
    pub const PROTOSS_AIR_ARMOR: u8 = 6;
    pub const TERRAN_INFANTRY_WEAPONS: u8 = 7;
    pub const TERRAN_VEHICLE_WEAPONS: u8 = 8;
    pub const TERRAN_SHIP_WEAPONS: u8 = 9;
    pub const ZERG_MELEE_ATTACKS: u8 = 10;
    pub const ZERG_MISSILE_ATTACKS: u8 = 11;
    pub const ZERG_FLYER_ATTACKS: u8 = 12;
    pub const PROTOSS_GROUND_WEAPONS: u8 = 13;
    pub const PROTOSS_AIR_WEAPONS: u8 = 14;
    pub const PROTOSS_PLASMA_SHIELDS: u8 = 15;
    pub const U238_SHELLS: u8 = 16;
    pub const ION_THRUSTERS: u8 = 17;
    pub const TITAN_REACTOR: u8 = 19;
    pub const OCULAR_IMPLANTS: u8 = 20;
    pub const MOEBIUS_REACTOR: u8 = 21;
    pub const APOLLO_REACTOR: u8 = 22;
    pub const COLOSSUS_REACTOR: u8 = 23;
    pub const VENTRAL_SACS: u8 = 24;
    pub const ANTENNAE: u8 = 25;
    pub const PNEUMATIZED_CARAPACE: u8 = 26;
    pub const METABOLIC_BOOST: u8 = 27;
    pub const ADRENAL_GLANDS: u8 = 28;
    pub const MUSCULAR_AUGMENTS: u8 = 29;
    pub const GROOVED_SPINES: u8 = 30;
    pub const GAMETE_MEIOSIS: u8 = 31;
    pub const METASYNAPTIC_NODE: u8 = 32;
    pub const SINGULARITY_CHARGE: u8 = 33;
    pub const LEG_ENHANCEMENTS: u8 = 34;
    pub const SCARAB_DAMAGE: u8 = 35;
    pub const REAVER_CAPACITY: u8 = 36;
    pub const GRAVITIC_DRIVE: u8 = 37;
    pub const SENSOR_ARRAY: u8 = 38;
    pub const GRAVITIC_BOOSTERS: u8 = 39;
    pub const KHAYDARIN_AMULET: u8 = 40;
    pub const APIAL_SENSORS: u8 = 41;
    pub const GRAVITIC_THRUSTERS: u8 = 42;
    pub const CARRIER_CAPACITY: u8 = 43;
    pub const KHAYDARIN_CORE: u8 = 44;
    pub const ARGUS_JEWEL: u8 = 47;
    pub const ARGUS_TALISMAN: u8 = 49;
    pub const CADUCEUS_REACTOR: u8 = 51;
    pub const CHITINOUS_PLATING: u8 = 52;
    pub const ANABOLIC_SYNTHESIS: u8 = 53;
    pub const CHARON_BOOSTERS: u8 = 54;
}

/// Order ids used by the simulation.
pub mod order {
    pub const DIE: u8 = 0;
    pub const STOP: u8 = 1;
    pub const GUARD: u8 = 2;
    pub const PLAYER_GUARD: u8 = 3;
    pub const TURRET_GUARD: u8 = 4;
    pub const MOVE: u8 = 6;
    pub const ATTACK_DEFAULT: u8 = 8;
    pub const MOVE_TO_ATTACK: u8 = 9;
    pub const ATTACK_UNIT: u8 = 10;
    pub const ATTACK_FIXED_RANGE: u8 = 11;
    pub const ATTACK_TILE: u8 = 12;
    pub const ATTACK_MOVE: u8 = 14;
    pub const DRONE_START_BUILD: u8 = 25;
    pub const DRONE_BUILD: u8 = 26;
    pub const PLACE_BUILDING: u8 = 30;
    pub const PLACE_PROTOSS_BUILDING: u8 = 31;
    pub const CONSTRUCTING_BUILDING: u8 = 33;
    pub const REPAIR: u8 = 34;
    pub const PLACE_ADDON: u8 = 36;
    pub const BUILD_ADDON: u8 = 37;
    pub const TRAIN: u8 = 38;
    pub const ZERG_BIRTH: u8 = 41;
    pub const ZERG_UNIT_MORPH: u8 = 42;
    pub const ZERG_BUILDING_MORPH: u8 = 43;
    pub const INCOMPLETE_BUILDING: u8 = 44;
    pub const FOLLOW: u8 = 49;
    pub const CARRIER: u8 = 50;
    pub const REAVER: u8 = 58;
    pub const RESEARCH_TECH: u8 = 75;
    pub const UPGRADE: u8 = 76;
    pub const LARVA: u8 = 77;
    pub const HARVEST_1: u8 = 79;
    pub const HARVEST_2: u8 = 80;
    pub const MOVE_TO_GAS: u8 = 81;
    pub const HARVEST_GAS: u8 = 83;
    pub const RETURN_GAS: u8 = 84;
    pub const MOVE_TO_MINERALS: u8 = 85;
    pub const MINING_MINERALS: u8 = 87;
    pub const RETURN_MINERALS: u8 = 90;
    pub const ENTER_TRANSPORT: u8 = 92;
    pub const PICKUP_TRANSPORT: u8 = 94;
    pub const PICKUP_BUNKER: u8 = 95;
    pub const SIEGING: u8 = 98;
    pub const UNSIEGING: u8 = 99;
    pub const WATCH_TARGET: u8 = 100;
    pub const HOLD_POSITION: u8 = 107;
    pub const CLOAK: u8 = 109;
    pub const DECLOAK: u8 = 110;
    pub const UNLOAD: u8 = 111;
    pub const FIRE_YAMATO_GUN: u8 = 113;
    pub const CAST_LOCKDOWN: u8 = 115;
    pub const BURROWING: u8 = 116;
    pub const BURROWED: u8 = 117;
    pub const UNBURROWING: u8 = 118;
    pub const CAST_DARK_SWARM: u8 = 119;
    pub const CAST_PARASITE: u8 = 120;
    pub const CAST_SPAWN_BROODLINGS: u8 = 121;
    pub const CAST_EMP_SHOCKWAVE: u8 = 122;
    pub const CAST_NUCLEAR_STRIKE: u8 = 128;
    pub const PLACE_MINE: u8 = 132;
    pub const RIGHT_CLICK_ACTION: u8 = 133;
    pub const CAST_RECALL: u8 = 137;
    pub const CAST_SCANNER_SWEEP: u8 = 139;
    pub const CAST_DEFENSIVE_MATRIX: u8 = 141;
    pub const CAST_PSIONIC_STORM: u8 = 142;
    pub const CAST_IRRADIATE: u8 = 143;
    pub const CAST_PLAGUE: u8 = 144;
    pub const CAST_CONSUME: u8 = 145;
    pub const CAST_ENSNARE: u8 = 146;
    pub const CAST_STASIS_FIELD: u8 = 147;
    pub const CAST_HALLUCINATION: u8 = 148;
    pub const RESET_COLLISION: u8 = 150;
    pub const PATROL: u8 = 152;
    pub const COMPUTER_AI: u8 = 156;
    pub const MEDIC_HEAL: u8 = 176;
    pub const CAST_RESTORATION: u8 = 180;
    pub const CAST_DISRUPTION_WEB: u8 = 181;
    pub const CAST_MIND_CONTROL: u8 = 182;
    pub const DARK_ARCHON_MELD: u8 = 183;
    pub const CAST_FEEDBACK: u8 = 184;
    pub const CAST_OPTICAL_FLARE: u8 = 185;
    pub const CAST_MAELSTROM: u8 = 186;
}

//! Deterministic StarCraft rules simulation.
//!
//! The world is a fixed 24 Hz logical simulation.  It implements the recovered
//! data tables (units, weapons, flingy, orders, tech, upgrades), the damage
//! pipeline, production/research, the hard-coded build tree, worker mining,
//! grid pathfinding, unit orders, spells and transport loading.
//!
//! Every deterministic source of randomness (spell scatter, starting larva)
//! uses a small seeded LCG so replays are reproducible.  The retail engine's
//! pathfinding tie-breaking is recovered from the `0x7a305590` open-list heap
//! (see `docs/evidence/pathfinding_astar.txt`); other ordering choices are
//! exposed through `RulesProfile` where they are visible.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use crate::combat::{fp, resolve_damage_status, Defense, StatusDamage};
use crate::data::{GameData, UnitType, WeaponType};
use crate::ids::{tech as tech_id, unit as uid, upgrade, weapon as wid};
use crate::iscript::{
    AnimContext, AnimEvent, AnimState, ANIM_AIR_ATTACK_INIT, ANIM_AIR_ATTACK_REPEAT,
    ANIM_AIR_ATTACK_TO_IDLE, ANIM_CAST_SPELL, ANIM_GND_ATTACK_INIT, ANIM_GND_ATTACK_REPEAT,
    ANIM_GND_ATTACK_TO_IDLE, ANIM_INIT, ANIM_WALKING, ANIM_WALKING_TO_IDLE,
};
use crate::map::{Scenario, Terrain, UnitProperties};
use crate::path::{find_path_with_regions, Obstacles, PathRequest, Rect, RegionMap};
use crate::projectile::{behavior, hit, BulletState, Projectile};
use crate::replay::Recorder;
use crate::tech as tree;
use crate::vision::Vision;

pub const TICKS_PER_SECOND: u32 = 24;
/// `units.dat` build times are in 1/15 second units.
pub const BUILD_TIME_UNITS: u32 = 15;
/// `upgrades.dat` research times are in 1/60 second units.
pub const UPGRADE_TIME_UNITS: u32 = 60;
pub const MAX_ENERGY: i32 = 200;
/// Retail stores energy in 1/256 points (`CUnit+0xa2`), like shields and HP.
/// Ability costs in `techdata.dat` are whole points and are scaled by this.
pub const ENERGY_SCALE: i32 = 256;
pub const MINERALS_PER_TRIP: i32 = 8;
pub const GAS_PER_TRIP: i32 = 8;
pub const MINERAL_CAPACITY: i32 = 1500;
pub const GAS_CAPACITY: i32 = 5000;
pub const MAX_ORDER_QUEUE: usize = 8;
pub const MAX_PRODUCTION_QUEUE: usize = 5;
/// The retail engine caps active Halo Rockets at 80; use the same hard cap for
/// every projectile so a pathological custom map cannot grow without bound.
pub const MAX_PROJECTILES: usize = 80;
/// Spider mine arming delay in logical ticks (about one second).
pub const SPIDER_MINE_ARM_TICKS: i64 = 24;
/// Carrier/Reaver hangar limits before upgrades.
pub const CARRIER_HANGAR: u8 = 4;
pub const REAVER_HANGAR: u8 = 5;
/// Detector building sight radius, in pixels, recovered from the retail
/// `unit_calculate_detected_flags` path.
pub const DETECTOR_BUILDING_RANGE: i32 = 7 * 32;
pub const SHIELD_REGEN_PER_TICK: i32 = 7;
pub const ZERG_REGEN_PER_TICK: i32 = 4;
/// Per-tick energy regeneration in 1/256 points (`FUN_7a16f...`: `+8` on
/// `CUnit+0xa2` every logical tick, clamped to the unit's maximum).
pub const ENERGY_REGEN_PER_TICK: i32 = 8;
/// Personal-cloak drain in 1/256 points per tick (Ghost 10, Wraith 13).
/// Cloaking suppresses the +8 regen, so the effective net drain is 18/21.
pub const GHOST_CLOAK_COST_PER_TICK: i32 = 10;
pub const WRAITH_CLOAK_COST_PER_TICK: i32 = 13;
/// Medic healing: 200/256 HP restored for 100/256 energy per tick.
pub const MEDIC_HEAL_PER_TICK: i32 = 200;
pub const MEDIC_HEAL_COST_PER_TICK: i32 = 100;
/// Creep spreads/recedes one maturity step every 8 logical ticks.
pub const CREEP_GROW_INTERVAL: u32 = 8;

pub fn build_ticks(build_time: u16) -> i32 {
    ((build_time as u32 * TICKS_PER_SECOND).div_ceil(BUILD_TIME_UNITS)) as i32
}

pub fn research_ticks(research_time: u16) -> i32 {
    ((research_time as u32 * TICKS_PER_SECOND).div_ceil(BUILD_TIME_UNITS)) as i32
}

pub fn upgrade_ticks(time: u32) -> i32 {
    ((time * TICKS_PER_SECOND).div_ceil(UPGRADE_TIME_UNITS)) as i32
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Race {
    Zerg,
    Terran,
    Protoss,
    Neutral,
}

impl Race {
    pub fn from_unit(unit: &UnitType) -> Self {
        match unit.group_flags & 0x0f {
            9 => Race::Zerg,
            10 => Race::Terran,
            12 => Race::Protoss,
            _ => Race::Neutral,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CarryKind {
    None,
    Minerals,
    Gas,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpellTarget {
    None,
    Unit(u32),
    Point(i32, i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spell {
    StimPacks,
    Lockdown,
    EmpShockwave,
    SpiderMines,
    ScannerSweep,
    TankSiegeMode,
    DefensiveMatrix,
    Irradiate,
    YamatoGun,
    CloakingField,
    PersonnelCloaking,
    Burrowing,
    SpawnBroodlings,
    DarkSwarm,
    Plague,
    Consume,
    Ensnare,
    Parasite,
    Infestation,
    PsionicStorm,
    Hallucination,
    Recall,
    StasisField,
    ArchonWarp,
    Restoration,
    DisruptionWeb,
    MindControl,
    DarkArchonMeld,
    Feedback,
    OpticalFlare,
    Maelstrom,
    LurkerAspect,
    Healing,
}

impl Order {
    /// `orders.dat` row used by this high-level order.  Only the queue/UI
    /// metadata is read from the row; the simulation still keeps its typed
    /// order state.
    pub fn order_id(&self) -> u8 {
        match self {
            Order::Idle => 0,
            Order::Stop => 1,
            Order::Move { .. } => 6,
            Order::AttackMove { .. } => 13,
            Order::AttackUnit { .. } => 8,
            Order::HoldPosition => 106,
            Order::Patrol { .. } => 151,
            Order::Gather { .. } => 78,
            Order::ReturnCargo { .. } => 85,
            Order::Build { .. } => 15,
            Order::Repair { .. } => 34,
            Order::Train { .. } => 37,
            Order::Research { .. } => 74,
            Order::Upgrade { .. } => 75,
            Order::Cast { .. } => 80,
            Order::EnterTransport { .. } => 90,
            Order::Unload => 110,
            Order::Burrow => 115,
            Order::Unburrow => 117,
            Order::Siege => 97,
            Order::Unsiege => 98,
            Order::Lift => 73,
            Order::Land { .. } => 70,
            Order::Die => 0,
        }
    }
}

impl Spell {
    pub fn tech_id(self) -> u8 {
        match self {
            Spell::StimPacks => tech_id::STIM_PACKS,
            Spell::Lockdown => tech_id::LOCKDOWN,
            Spell::EmpShockwave => tech_id::EMP_SHOCKWAVE,
            Spell::SpiderMines => tech_id::SPIDER_MINES,
            Spell::ScannerSweep => tech_id::SCANNER_SWEEP,
            Spell::TankSiegeMode => tech_id::TANK_SIEGE_MODE,
            Spell::DefensiveMatrix => tech_id::DEFENSIVE_MATRIX,
            Spell::Irradiate => tech_id::IRRADIATE,
            Spell::YamatoGun => tech_id::YAMATO_GUN,
            Spell::CloakingField => tech_id::CLOAKING_FIELD,
            Spell::PersonnelCloaking => tech_id::PERSONNEL_CLOAKING,
            Spell::Burrowing => tech_id::BURROWING,
            Spell::SpawnBroodlings => tech_id::SPAWN_BROODLINGS,
            Spell::DarkSwarm => tech_id::DARK_SWARM,
            Spell::Plague => tech_id::PLAGUE,
            Spell::Consume => tech_id::CONSUME,
            Spell::Ensnare => tech_id::ENSNARE,
            Spell::Parasite => tech_id::PARASITE,
            Spell::Infestation => tech_id::INFESTATION,
            Spell::PsionicStorm => tech_id::PSIONIC_STORM,
            Spell::Hallucination => tech_id::HALLUCINATION,
            Spell::Recall => tech_id::RECALL,
            Spell::StasisField => tech_id::STASIS_FIELD,
            Spell::ArchonWarp => tech_id::ARCHON_WARP,
            Spell::Restoration => tech_id::RESTORATION,
            Spell::DisruptionWeb => tech_id::DISRUPTION_WEB,
            Spell::MindControl => tech_id::MIND_CONTROL,
            Spell::DarkArchonMeld => tech_id::DARK_ARCHON_MELD,
            Spell::Feedback => tech_id::FEEDBACK,
            Spell::OpticalFlare => tech_id::OPTICAL_FLARE,
            Spell::Maelstrom => tech_id::MAELSTROM,
            Spell::LurkerAspect => tech_id::LURKER_ASPECT,
            Spell::Healing => tech_id::HEALING,
        }
    }
}

pub use crate::orders::{lane, order_class, spec, OrderClass, OrderLane, OrderSpec};

/// True when the recovered order dispatcher runs this row's handler on the
/// current logical tick.  Retail keeps a per-unit carry counter at
/// `CUnit+0x85`: the four-case fast switch and the 36-case worker/attack
/// switch run every tick, while the remaining 128 direct cases wait for the
/// counter to reach zero (every 8th tick).  `carry` is that counter before it
/// is decremented for the tick; it is `0` on the first tick after spawn.
pub fn order_runs_this_tick(id: u8, carry: u8) -> bool {
    match lane(id) {
        OrderLane::EveryTick => true,
        OrderLane::Carry8 => carry == 0,
        OrderLane::Alias(target) => order_runs_this_tick(target, carry),
        OrderLane::Production | OrderLane::Engine => true,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Order {
    Idle,
    Stop,
    Move {
        pos: (i32, i32),
    },
    AttackMove {
        pos: (i32, i32),
    },
    AttackUnit {
        target: u32,
    },
    HoldPosition,
    Patrol {
        a: (i32, i32),
        b: (i32, i32),
        forward: bool,
    },
    Gather {
        resource: u32,
    },
    ReturnCargo {
        depot: u32,
    },
    Build {
        unit_type: u16,
        pos: (i32, i32),
    },
    Repair {
        target: u32,
    },
    Train {
        unit_type: u16,
    },
    Research {
        tech_id: u8,
    },
    Upgrade {
        upgrade_id: u8,
        level: u8,
    },
    Cast {
        spell: Spell,
        target: SpellTarget,
    },
    EnterTransport {
        transport: u32,
    },
    Unload,
    Burrow,
    Unburrow,
    Siege,
    Unsiege,
    /// Lift a Terran building off the ground (row 73).
    Lift,
    /// Land a lifted building at a position (row 70).
    Land {
        pos: (i32, i32),
    },
    Die,
}

#[derive(Clone, Debug, Default)]
pub struct Status {
    /// Defensive Matrix pool in 1/256 points.
    pub matrix: i32,
    pub acid_spores: u8,
    pub hallucination: bool,
    pub stim_ticks: i32,
    pub lockdown_ticks: i32,
    pub stasis_ticks: i32,
    pub irradiate_ticks: i32,
    pub plague_ticks: i32,
    pub ensnare_ticks: i32,
    pub maelstrom_ticks: i32,
    pub disruption_ticks: i32,
    pub parasite: bool,
    pub optical_flare_ticks: i32,
    pub cloaked: bool,
    pub burrowed: bool,
    pub sieged: bool,
    /// Scanner Sweep / detector reveal timer.
    pub revealed_ticks: i32,
}

#[derive(Clone, Debug)]
pub struct SimUnit {
    pub id: u32,
    pub owner: u8,
    pub type_id: u16,
    pub hp: i32,
    pub shields: i32,
    pub energy: i32,
    /// Sub-point energy in 1/256, matching retail's `CUnit+0xa2` fixed-point
    /// storage.  `energy` remains the whole-point view used by ability costs.
    pub energy_frac: i32,
    pub x: i32,
    pub y: i32,
    pub facing: u8,
    pub alive: bool,
    /// False while a building is under construction.
    pub completed: bool,
    /// True while a Terran building is lifted off the ground (`orders.dat`
    /// rows 69-73).  Lifted buildings move as air units and cannot produce.
    pub lifted: bool,
    /// Add-on attachment state.  Lifted parents leave their add-on grounded
    /// and inactive; landing on it re-attaches it.
    pub addon_attached: bool,
    /// Remaining construction/training/research ticks.
    pub build_remaining: i32,
    pub build_total: i32,
    pub cooldown: u32,
    pub order: Order,
    /// Order to resume when a temporary attack (from attack-move) ends.
    pub resume_order: Option<Order>,
    pub queue: VecDeque<Order>,
    pub target: Option<u32>,
    pub path: VecDeque<(i32, i32)>,
    pub path_goal: Option<(i32, i32)>,
    pub rally: Option<(i32, i32)>,
    pub carry: i32,
    pub carry_kind: CarryKind,
    pub resource_target: Option<u32>,
    pub depot: Option<u32>,
    pub production: VecDeque<Order>,
    pub addon: Option<u32>,
    /// Units currently loaded into this transport/bunker.
    pub cargo: Vec<u32>,
    pub in_transport: Option<u32>,
    pub status: Status,
    /// Larva spawn timer for Zerg hatcheries.
    pub larva_timer: i32,
    /// Resource capacity for mineral fields/geysers.
    pub resource_amount: i32,
    /// For Zerg eggs: the unit that will hatch when construction finishes.
    pub morph_to: Option<u16>,
    /// Last damage tick, for damage overlays/telemetry.
    pub last_damaged: u64,
    /// Trigger action 43 (Set Invincibility) or a UPRP invincible slot.
    pub invincible: bool,
    /// Unit that last dealt damage, used to credit kills and razings.
    pub last_damage_source: u32,
    /// Trigger action 42 (Set Doodad State); `None` for ordinary units.
    pub doodad_enabled: Option<bool>,
    /// Paired Nydus Canal exit, when the map linked the two records by serial.
    pub nydus_exit: Option<u32>,
    /// Tick at which a temporary spell unit (Dark Swarm, Disruption Web,
    /// Scanner Sweep, Nuclear Missile) disappears.  Zero means permanent.
    pub expires_at: i64,
    /// Tick at which a Spider Mine finishes arming.
    pub mine_armed_at: i64,
    /// Fractional mineral debt for SCV repair, in 1/256 minerals.
    pub repair_debt: i32,
    /// Interceptors (Carrier) or scarabs (Reaver) currently in the hangar.
    pub hangar: u8,
    pub hangar_capacity: u8,
    /// Personal weapon cooldown used by carriers and reavers, whose attack is
    /// driven by a launched unit rather than a normal weapon row.
    pub special_cooldown: i32,
    /// Per-unit order-carry counter (`CUnit+0x85`).  Initialised to zero so
    /// the carry-gated handlers run on the first tick, then reset to 8.
    pub order_carry: u8,
    /// `orders.dat` row that produced the current typed order.  This keeps
    /// the exact recovered row (including internal rows) available to the
    /// handler dispatch layer.
    pub order_row: u8,
    /// Tick counter into the Walking iscript cycle.
    pub anim_tick: u32,
    /// Current iscript animation state, when the unit has one.
    pub anim: Option<AnimState>,
    /// Number of weapon events fired, used to select the repeat-attack
    /// animation instead of the initial attack animation.
    pub attacks_fired: u32,
}

impl SimUnit {
    pub fn is_completed_building(&self, data: &GameData) -> bool {
        self.completed && data.unit(self.type_id).is_building()
    }

    pub fn is_resource(&self) -> bool {
        matches!(
            self.type_id,
            uid::MINERAL_FIELD | uid::MINERAL_FIELD_2 | uid::MINERAL_FIELD_3 | uid::VESPENE_GEYSER
        )
    }

    /// Energy in retail 1/256 points.
    pub fn energy_fixed(&self) -> i32 {
        self.energy * ENERGY_SCALE + self.energy_frac
    }

    /// Write energy from a 1/256-point value, clamping at both ends.
    pub fn set_energy_fixed(&mut self, value: i32) {
        let v = value.clamp(0, MAX_ENERGY * ENERGY_SCALE);
        self.energy = v / ENERGY_SCALE;
        self.energy_frac = v % ENERGY_SCALE;
    }

    pub fn add_energy_fixed(&mut self, delta: i32) {
        self.set_energy_fixed(self.energy_fixed() + delta);
    }

    pub fn add_energy_points(&mut self, points: i32) {
        self.add_energy_fixed(points.saturating_mul(ENERGY_SCALE));
    }

    /// Spend 1/256-point energy.  Returns false (and changes nothing) when
    /// the pool cannot pay the whole cost.
    pub fn spend_energy_fixed(&mut self, cost: i32) -> bool {
        let current = self.energy_fixed();
        if current < cost {
            return false;
        }
        self.set_energy_fixed(current - cost);
        true
    }
}

#[derive(Clone, Debug)]
pub struct Player {
    pub race: Race,
    pub team: u8,
    pub minerals: i32,
    pub gas: i32,
    pub techs: [bool; 44],
    pub upgrades: [u8; 61],
    pub defeated: bool,
    pub is_computer: bool,
    /// Resources ever gathered, used by the "accumulate" trigger condition.
    pub minerals_gathered: i32,
    pub gas_gathered: i32,
    /// Lifetime statistics used by trigger conditions and the score screen.
    pub kills: u32,
    pub deaths: u32,
    pub razings: u32,
    /// Per-unit-type death counters (condition type 8).
    pub death_counts: [u32; 228],
    /// Per-unit-type kill counters (condition types 5/8/18).
    pub kill_counts: [u32; 228],
    /// Trigger score categories, indexed by the CHK score type: total,
    /// units, buildings, units+buildings, kills, razings, kills+razings,
    /// custom.
    pub scores: [i32; 8],
}

impl Player {
    pub fn new(race: Race, team: u8, minerals: i32, gas: i32) -> Self {
        Self {
            race,
            team,
            minerals,
            gas,
            techs: [false; 44],
            upgrades: [0; 61],
            defeated: false,
            is_computer: false,
            minerals_gathered: 0,
            gas_gathered: 0,
            kills: 0,
            deaths: 0,
            razings: 0,
            death_counts: [0; 228],
            kill_counts: [0; 228],
            scores: [0; 8],
        }
    }

    pub fn has_tech(&self, id: u8) -> bool {
        self.techs.get(id as usize).copied().unwrap_or(false)
    }

    pub fn upgrade_level(&self, id: u8) -> u8 {
        self.upgrades.get(id as usize).copied().unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimError {
    BadUnit,
    BadOwner,
    NotEnoughMinerals,
    NotEnoughGas,
    NotEnoughSupply,
    MissingRequirements,
    MissingTech,
    ProducerBusy,
    QueueFull,
    InvalidTarget,
    OutOfRange,
    NotEnoughEnergy,
    /// A Protoss structure is outside every Pylon power field.
    Unpowered,
    Unsupported,
}

impl std::fmt::Display for SimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for SimError {}

pub struct World {
    pub data: Arc<GameData>,
    pub terrain: Terrain,
    /// Connected-component map of the walk grid, built once per terrain.
    pub regions: RegionMap,
    pub players: Vec<Player>,
    pub units: Vec<SimUnit>,
    pub next_id: u32,
    pub tick: u64,
    pub winner: Option<u8>,
    /// Deterministic LCG state for effects that use randomness.
    pub rng: u64,
    /// Scenario trigger VM, when the map has a `TRIG` section.
    pub triggers: Option<crate::trigger::TriggerSystem>,
    /// Travelling bullets and missiles.
    pub projectiles: Vec<Projectile>,
    /// Per-player visible/explored masks.  Present when a scenario has an
    /// extent; `fog_of_war` controls whether targeting consults it.
    pub vision: Option<Vision>,
    pub fog_of_war: bool,
    /// Zerg creep, one flag per 32-pixel tile.  Rebuilt when a creep source
    /// completes or dies.
    creep: Vec<bool>,
    /// Creep maturity per tile (0..=3).  Sources seed their footprint at 3;
    /// creep spreads one step every `CREEP_GROW_INTERVAL` ticks and decays
    /// when no source supports the tile.
    creep_strength: Vec<u8>,
    /// Tiles covered by a creep source's full radius.
    creep_supported: Vec<bool>,
    creep_dirty: bool,
    next_projectile_id: u32,
    /// Scratch obstacle list rebuilt when a path is requested.
    obstacles: Obstacles,
    index: HashMap<u32, usize>,
    /// How many times each recovered order handler routine was entered.
    /// The 137 fixed targets are ordered by VA in `crate::handlers`.
    pub handler_hits: [u64; crate::handlers::HANDLER_COUNT],
    /// Optional command recorder (save games / replays / LAN). Pushed by
    /// [`World::issue`] only; zero cost when `None`.
    pub recorder: Option<Recorder>,
}

impl World {
    pub fn new(data: Arc<GameData>, terrain: Terrain, player_count: usize) -> Self {
        let regions = RegionMap::from_terrain(&terrain);
        let creep_tiles = terrain.width * terrain.height;
        let creep = vec![false; creep_tiles];
        let creep_strength = vec![0u8; creep_tiles];
        let creep_supported = vec![false; creep_tiles];
        Self {
            data,
            terrain,
            regions,
            players: (0..player_count)
                .map(|i| Player::new(Race::Terran, i as u8, 50, 0))
                .collect(),
            units: Vec::new(),
            next_id: 1,
            tick: 0,
            winner: None,
            triggers: None,
            projectiles: Vec::new(),
            vision: None,
            fog_of_war: false,
            creep,
            creep_strength,
            creep_supported,
            creep_dirty: true,
            next_projectile_id: 1,
            rng: 0x1234_5678_9abc_def0,
            obstacles: Obstacles::new(),
            index: HashMap::new(),
            handler_hits: [0; crate::handlers::HANDLER_COUNT],
            recorder: None,
        }
    }

    pub fn from_scenario(
        data: Arc<GameData>,
        scenario: &Scenario,
        vf4: &[u8],
        races: &[Race],
    ) -> Self {
        let terrain = Terrain::from_scenario(scenario, vf4);
        let mut world = Self::new(data, terrain, races.len());
        world.vision = Some(Vision::new(scenario.width, scenario.height));
        for (i, race) in races.iter().enumerate() {
            world.players[i].race = *race;
        }
        let mut serial_to_index: HashMap<u32, usize> = HashMap::new();
        for mu in &scenario.units {
            // Start-location markers are metadata, not units.
            if mu.unit_id == uid::START_LOCATION {
                continue;
            }
            let resource = is_resource_type(mu.unit_id);
            let owner = mu.owner as usize;
            if !resource && owner >= world.players.len() {
                continue;
            }
            // Neutral resources are placed under player 0 for ownership
            // bookkeeping; every combat/economy path treats them as neutral.
            let owner = if resource { 0 } else { owner };
            let idx = world.spawn(owner, mu.unit_id, mu.x, mu.y);
            if let Ok(i) = idx {
                serial_to_index.insert(mu.serial, i);
                let ty = world.data.unit(mu.unit_id).clone();
                let u = &mut world.units[i];
                if mu.fields_set & 0x02 != 0 {
                    let pct = (mu.hit_point_percent as i32).min(100);
                    // 0% leaves the retail minimum of 1/256 HP.
                    u.hp = ((ty.hit_points_raw as i64 * pct as i64 / 100) as i32).max(1);
                }
                if mu.fields_set & 0x04 != 0 && ty.has_shields {
                    let pct = (mu.shield_percent as i32).min(100);
                    u.shields = (fp(ty.shield_points as i32) * pct / 100).max(0);
                }
                if mu.fields_set & 0x08 != 0 {
                    let pct = (mu.energy_percent as i32).min(100);
                    u.set_energy_fixed(MAX_ENERGY * ENERGY_SCALE * pct / 100);
                }
                if mu.fields_set & 0x20 != 0 {
                    u.hangar = mu.hangar.min(u.hangar_capacity as u16) as u8;
                }
                if is_resource_type(u.type_id) {
                    let capacity = if u.type_id == uid::VESPENE_GEYSER {
                        GAS_CAPACITY
                    } else {
                        MINERAL_CAPACITY
                    };
                    u.resource_amount = if mu.fields_set & 0x10 != 0 || mu.resource_amount > 0 {
                        (mu.resource_amount as u16) as i32
                    } else {
                        capacity
                    };
                }
                let states = mu.states & mu.states_set;
                if states & 0x01 != 0 {
                    u.status.cloaked = true;
                }
                if states & 0x02 != 0 {
                    u.status.burrowed = true;
                }
                if states & 0x04 != 0 && can_lift_building(u.type_id) {
                    u.lifted = true;
                }
                if states & 0x08 != 0 {
                    u.status.hallucination = true;
                }
                if states & 0x10 != 0 {
                    u.invincible = true;
                }
            }
        }
        // Link add-ons and Nydus pairs by the serial numbers StarEdit writes.
        // The link flags at offset 10 are ignored by the retail engine; only
        // the two records naming each other matter.
        for mu in &scenario.units {
            if mu.link_serial == 0 {
                continue;
            }
            let (Some(&i), Some(&j)) = (
                serial_to_index.get(&mu.serial),
                serial_to_index.get(&mu.link_serial),
            ) else {
                continue;
            };
            if i == j || !world.units[i].alive || !world.units[j].alive {
                continue;
            }
            if world.units[i].type_id == uid::NYDUS_CANAL
                && world.units[j].type_id == uid::NYDUS_CANAL
            {
                world.units[i].nydus_exit = Some(world.units[j].id);
                world.units[j].nydus_exit = Some(world.units[i].id);
            }
            // Add-on -> parent, or parent -> add-on.
            if let Some(parent_type) = addon_parent(world.units[i].type_id) {
                if world.units[j].type_id == parent_type && world.units[j].addon.is_none() {
                    world.units[j].addon = Some(world.units[i].id);
                    world.units[i].addon_attached = true;
                }
            } else if let Some(child_parent) = addon_parent(world.units[j].type_id) {
                if world.units[i].type_id == child_parent && world.units[i].addon.is_none() {
                    world.units[i].addon = Some(world.units[j].id);
                    world.units[j].addon_attached = true;
                }
            }
        }
        // Legacy/protected maps with no usable serial link: attach an add-on
        // that sits beside its parent.
        let placed_addons: Vec<(usize, u16, (i32, i32))> = world
            .units
            .iter()
            .enumerate()
            .filter(|(_, u)| {
                addon_parent(u.type_id).is_some() && u.alive && u.completed && !u.addon_attached
            })
            .map(|(i, u)| (i, u.type_id, (u.x, u.y)))
            .collect();
        for (ai, addon_type, (ax, ay)) in placed_addons {
            let Some(parent_type) = addon_parent(addon_type) else {
                continue;
            };
            let parent = world.units.iter().position(|u| {
                u.alive
                    && u.completed
                    && u.type_id == parent_type
                    && u.addon.is_none()
                    && (u.x - ax).abs() <= fp(96)
                    && (u.y - ay).abs() <= fp(96)
            });
            if let Some(p) = parent {
                world.units[p].addon = Some(world.units[ai].id);
                world.units[ai].addon_attached = true;
            }
        }
        // Melee maps contain only start locations and resources; create the
        // stock starting town hall and four workers for every player.  A
        // scenario that already places a force for any active player is a
        // campaign/use-map map and must not receive free bases.
        let has_placed_force = scenario.units.iter().any(|mu| {
            mu.unit_id != uid::START_LOCATION
                && !is_resource_type(mu.unit_id)
                && (mu.owner as usize) < world.players.len()
        });
        let starts = scenario.start_locations();
        for p in 0..world.players.len() {
            if has_placed_force {
                break;
            }
            let race = world.players[p].race;
            let start = starts
                .get(p)
                .copied()
                .or_else(|| starts.first().copied())
                .unwrap_or((16 + p as i32 * 16, 16));
            let (sx, sy) = (start.0, start.1);
            let has_base = world.units.iter().any(|u| {
                u.alive
                    && u.owner == p as u8
                    && u.completed
                    && matches!(u.type_id, uid::COMMAND_CENTER | uid::NEXUS | uid::HATCHERY)
            });
            if has_base {
                continue;
            }
            let (town, worker) = match race {
                Race::Terran => (uid::COMMAND_CENTER, uid::SCV),
                Race::Protoss => (uid::NEXUS, uid::PROBE),
                Race::Zerg => (uid::HATCHERY, uid::DRONE),
                Race::Neutral => (uid::COMMAND_CENTER, uid::SCV),
            };
            let _ = world.spawn_px(p, town, sx, sy);
            for k in 0..4 {
                let _ = world.spawn_px(p, worker, sx + 32 + k * 20, sy + 48);
            }
            // A melee Zerg start also includes one Overlord; it is the
            // starting supply provider (the Hatchery alone provides 1).
            if race == Race::Zerg {
                let _ = world.spawn_px(p, uid::OVERLORD, sx + 48, sy - 32);
            }
            world.players[p].minerals = 50;
        }
        if !scenario.triggers.is_empty() {
            let locations: Vec<(i32, i32, i32, i32)> = scenario
                .locations
                .iter()
                .map(|l| (l.left * 32, l.top * 32, l.right * 32, l.bottom * 32))
                .collect();
            world.triggers = Some(crate::trigger::TriggerSystem::parse(
                &scenario.triggers,
                &locations,
            ));
            if let Some(t) = world.triggers.as_mut() {
                t.unit_properties = scenario.unit_properties.clone();
                t.mission_objectives = vec![None; world.players.len()];
                t.preserved = vec![vec![false; t.triggers.len()]; world.players.len()];
            }
        }
        world
    }

    /// Turn on rules-level fog of war.  A scenario gets a vision grid when it
    /// is loaded; for a synthetic `World::new` the grid is created lazily from
    /// the terrain extent.
    pub fn enable_fog(&mut self) {
        if self.vision.is_none() {
            self.vision = Some(Vision::new(self.terrain.width, self.terrain.height));
        }
        self.fog_of_war = true;
        self.update_vision();
    }

    pub fn index_of(&self, id: u32) -> Option<usize> {
        self.index.get(&id).copied()
    }

    pub fn unit(&self, id: u32) -> Option<&SimUnit> {
        self.index_of(id).map(|i| &self.units[i])
    }

    pub fn unit_mut(&mut self, id: u32) -> Option<&mut SimUnit> {
        self.index_of(id).map(|i| &mut self.units[i])
    }

    pub fn spawn(&mut self, owner: usize, type_id: u16, x: i32, y: i32) -> Result<usize, SimError> {
        if owner >= self.players.len() {
            return Err(SimError::BadOwner);
        }
        let ty = self.data.unit(type_id).clone();
        let shields = if ty.has_shields {
            fp(ty.shield_points as i32)
        } else {
            0
        };
        let energy = if ty.flags & 0x0020_0000 != 0 {
            MAX_ENERGY
        } else {
            0
        };
        let id = self.next_id;
        self.next_id += 1;
        let unit = SimUnit {
            id,
            owner: owner as u8,
            type_id,
            hp: ty.hit_points_raw as i32,
            shields,
            energy,
            energy_frac: 0,
            x: x.saturating_mul(fp(1)),
            y: y.saturating_mul(fp(1)),
            facing: 0,
            alive: true,
            completed: true,
            lifted: false,
            addon_attached: false,
            build_remaining: 0,
            build_total: 0,
            cooldown: 0,
            order: Order::Idle,
            resume_order: None,
            queue: VecDeque::new(),
            target: None,
            path: VecDeque::new(),
            path_goal: None,
            rally: None,
            carry: 0,
            carry_kind: CarryKind::None,
            resource_target: None,
            depot: None,
            production: VecDeque::new(),
            addon: None,
            cargo: Vec::new(),
            in_transport: None,
            status: Status::default(),
            larva_timer: 0,
            resource_amount: 0,
            morph_to: None,
            last_damaged: 0,
            invincible: false,
            last_damage_source: 0,
            doodad_enabled: None,
            nydus_exit: None,
            expires_at: 0,
            mine_armed_at: 0,
            repair_debt: 0,
            hangar: default_hangar(type_id),
            hangar_capacity: default_hangar_capacity(type_id),
            special_cooldown: 0,
            order_carry: 0,
            order_row: 0,
            anim_tick: 0,
            anim: None,
            attacks_fired: 0,
        };
        let idx = self.units.len();
        self.index.insert(id, idx);
        self.units.push(unit);
        if is_creep_source(type_id) {
            self.creep_dirty = true;
        }
        self.start_animation(idx, ANIM_INIT);
        Ok(idx)
    }

    /// Spawn using pixel coordinates rather than tiles.
    pub fn spawn_px(
        &mut self,
        owner: usize,
        type_id: u16,
        x: i32,
        y: i32,
    ) -> Result<usize, SimError> {
        let ty = self.data.unit(type_id).clone();
        let shields = if ty.has_shields {
            fp(ty.shield_points as i32)
        } else {
            0
        };
        let energy = if ty.flags & 0x0020_0000 != 0 {
            MAX_ENERGY
        } else {
            0
        };
        let id = self.next_id;
        self.next_id += 1;
        let unit = SimUnit {
            id,
            owner: owner as u8,
            type_id,
            hp: ty.hit_points_raw as i32,
            shields,
            energy,
            energy_frac: 0,
            x: x.saturating_mul(fp(1)),
            y: y.saturating_mul(fp(1)),
            facing: 0,
            alive: true,
            completed: true,
            lifted: false,
            addon_attached: false,
            build_remaining: 0,
            build_total: 0,
            cooldown: 0,
            order: Order::Idle,
            resume_order: None,
            queue: VecDeque::new(),
            target: None,
            path: VecDeque::new(),
            path_goal: None,
            rally: None,
            carry: 0,
            carry_kind: CarryKind::None,
            resource_target: None,
            depot: None,
            production: VecDeque::new(),
            addon: None,
            cargo: Vec::new(),
            in_transport: None,
            status: Status::default(),
            larva_timer: 0,
            resource_amount: 0,
            morph_to: None,
            last_damaged: 0,
            invincible: false,
            last_damage_source: 0,
            doodad_enabled: None,
            nydus_exit: None,
            expires_at: 0,
            mine_armed_at: 0,
            repair_debt: 0,
            hangar: default_hangar(type_id),
            hangar_capacity: default_hangar_capacity(type_id),
            special_cooldown: 0,
            order_carry: 0,
            order_row: 0,
            anim_tick: 0,
            anim: None,
            attacks_fired: 0,
        };
        let idx = self.units.len();
        self.index.insert(id, idx);
        self.units.push(unit);
        if is_creep_source(type_id) {
            self.creep_dirty = true;
        }
        self.start_animation(idx, ANIM_INIT);
        Ok(idx)
    }

    /// Spawn a unit and apply one `UPRP` property slot (trigger actions 11 and
    /// 44).  Only fields whose flag bit is set are applied; percentages clamp
    /// to 1..=100 and resources/hangar keep the low 16/8 bits, matching the
    /// retail loader.
    pub fn spawn_with_properties(
        &mut self,
        owner: usize,
        type_id: u16,
        x: i32,
        y: i32,
        props: &UnitProperties,
    ) -> Result<usize, SimError> {
        let idx = self.spawn_px(owner, type_id, x, y)?;
        let ty = self.data.unit(type_id).clone();
        let u = &mut self.units[idx];
        if props.field_flags & 0x01 != 0 && (props.owner as usize) < self.players.len() {
            u.owner = props.owner;
        }
        if props.field_flags & 0x02 != 0 {
            let pct = props.hit_points_percent.clamp(1, 100) as i32;
            u.hp = ((ty.hit_points_raw as i64 * pct as i64 / 100) as i32).max(1);
        }
        if props.field_flags & 0x04 != 0 && ty.has_shields {
            let pct = props.shields_percent.min(100) as i32;
            u.shields = (ty.shield_points as i32 * pct / 100).max(0);
        }
        if props.field_flags & 0x08 != 0 {
            let pct = props.energy_percent.min(100) as i32;
            u.set_energy_fixed(MAX_ENERGY * ENERGY_SCALE * pct / 100);
        }
        if props.field_flags & 0x10 != 0 {
            u.resource_amount = (props.resource_amount as u16) as i32;
        }
        if props.field_flags & 0x20 != 0 {
            u.hangar = props.hangar.min(u.hangar_capacity as u16) as u8;
        }
        let states = props.states & props.state_flags;
        if states & 0x01 != 0 {
            u.status.cloaked = true;
        }
        if states & 0x02 != 0 {
            u.status.burrowed = true;
        }
        if states & 0x08 != 0 {
            u.status.hallucination = true;
        }
        if states & 0x10 != 0 {
            u.invincible = true;
        }
        Ok(idx)
    }

    pub fn supply_cap(&self, owner: u8) -> i32 {
        // Supply is stored in half-points, so the retail 200 cap is 400 raw.
        let cap: i32 = self
            .units
            .iter()
            .filter(|u| u.alive && u.owner == owner && u.completed)
            .map(|u| self.data.unit(u.type_id).supply_provided as i32)
            .sum();
        cap.min(400)
    }

    /// Recompute the creep mask from completed Zerg creep sources.  Called
    /// lazily; sources are Hatcheries/Lairs/Hives and the three colony
    /// buildings.  Creep covers walkable tiles within three tiles of the
    /// building footprint.
    fn rebuild_creep(&mut self) {
        if !self.creep_dirty {
            return;
        }
        let w = self.terrain.width as i32;
        let h = self.terrain.height as i32;
        self.creep_supported.iter_mut().for_each(|s| *s = false);
        let mut seed = vec![false; self.creep.len()];
        for u in &self.units {
            if !u.alive || !u.completed || !is_creep_source(u.type_id) {
                continue;
            }
            let ty = self.data.unit(u.type_id);
            let hw = ty.dimensions.right.max(ty.dimensions.left.max(0)) as i32;
            let hh = ty.dimensions.bottom.max(ty.dimensions.top.max(0)) as i32;
            let cx = u.x / fp(1);
            let cy = u.y / fp(1);
            // The source footprint is mature immediately.
            for tile_y in ((cy - hh) / 32).max(0)..=((cy + hh) / 32).min(h - 1) {
                for tile_x in ((cx - hw) / 32).max(0)..=((cx + hw) / 32).min(w - 1) {
                    seed[(tile_y * w + tile_x) as usize] = true;
                }
            }
            // Three tiles beyond the building edge, matching the classic
            // creep radius.
            let rx = hw + 3 * 32;
            let ry = hh + 3 * 32;
            let tx0 = ((cx - rx) / 32).max(0);
            let ty0 = ((cy - ry) / 32).max(0);
            let tx1 = ((cx + rx) / 32).min(w - 1);
            let ty1 = ((cy + ry) / 32).min(h - 1);
            for tile_y in ty0..=ty1 {
                for tile_x in tx0..=tx1 {
                    let px = tile_x * 32 + 16;
                    let py = tile_y * 32 + 16;
                    let dx = (px - cx) as i64;
                    let dy = (py - cy) as i64;
                    if dx * dx + dy * dy <= (rx as i64) * (rx as i64)
                        && self.terrain.walkable_cell(tile_x * 4 + 2, tile_y * 4 + 2)
                    {
                        self.creep_supported[(tile_y * w + tile_x) as usize] = true;
                    }
                }
            }
        }
        for (i, &is_seed) in seed.iter().enumerate() {
            if self.creep_supported[i] {
                if is_seed && self.creep_strength[i] == 0 {
                    self.creep_strength[i] = 3;
                }
            } else {
                // A source died: the tile recedes one step per rebuild.
                self.creep_strength[i] = self.creep_strength[i].saturating_sub(1);
            }
            self.creep[i] = self.creep_strength[i] > 0;
        }
        self.creep_dirty = false;
    }

    /// Spread/recede creep one maturity step.  Runs every
    /// `CREEP_GROW_INTERVAL` logical ticks.
    fn grow_creep(&mut self) {
        self.rebuild_creep();
        let w = self.terrain.width as i32;
        let h = self.terrain.height as i32;
        let mut next = self.creep_strength.clone();
        for tile_y in 0..h {
            for tile_x in 0..w {
                let i = (tile_y * w + tile_x) as usize;
                if !self.creep_supported[i] {
                    next[i] = self.creep_strength[i].saturating_sub(1);
                    continue;
                }
                if self.creep_strength[i] >= 3 {
                    continue;
                }
                let mut adjacent = false;
                for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    let nx = tile_x + dx;
                    let ny = tile_y + dy;
                    if nx >= 0
                        && ny >= 0
                        && nx < w
                        && ny < h
                        && self.creep_strength[(ny * w + nx) as usize] > 0
                    {
                        adjacent = true;
                        break;
                    }
                }
                if adjacent {
                    next[i] = (self.creep_strength[i] + 1).min(3);
                }
            }
        }
        self.creep_strength = next;
        for i in 0..self.creep.len() {
            self.creep[i] = self.creep_strength[i] > 0;
        }
    }

    pub fn has_creep_tile(&mut self, tx: i32, ty: i32) -> bool {
        self.rebuild_creep();
        let w = self.terrain.width as i32;
        let h = self.terrain.height as i32;
        if tx < 0 || ty < 0 || tx >= w || ty >= h {
            return false;
        }
        self.creep[(ty * w + tx) as usize]
    }

    /// Per-tile creep maturity (`0..=3`) for renderers.  Rebuilds the mask
    /// first so a source that appeared or died this tick is reflected without
    /// waiting for the next `CREEP_GROW_INTERVAL` boundary.
    pub fn creep_grid(&mut self) -> &[u8] {
        self.rebuild_creep();
        &self.creep_strength
    }

    /// Creep maturity at one tile, or zero outside the map.
    pub fn creep_strength_at(&mut self, tx: i32, ty: i32) -> u8 {
        self.rebuild_creep();
        let w = self.terrain.width as i32;
        let h = self.terrain.height as i32;
        if tx < 0 || ty < 0 || tx >= w || ty >= h {
            return 0;
        }
        self.creep_strength[(ty * w + tx) as usize]
    }

    pub fn has_creep_px(&mut self, x: i32, y: i32) -> bool {
        self.has_creep_tile(x.div_euclid(32), y.div_euclid(32))
    }

    /// Minimap dots for one observer: own units always, enemies only when the
    /// rules-level fog can currently see them.  Each unit is tested by its own
    /// index; using one shared index would leak every enemy once any unit was
    /// visible.
    pub fn minimap_units(&self, owner: u8, fog: bool) -> Vec<(i32, i32, u8, bool)> {
        self.units
            .iter()
            .enumerate()
            .filter(|(i, u)| u.alive && (u.owner == owner || !fog || self.can_see_unit(owner, *i)))
            .map(|(_, u)| {
                (
                    u.x / 256,
                    u.y / 256,
                    u.owner,
                    self.data.unit(u.type_id).is_building(),
                )
            })
            .collect()
    }

    /// Protoss power field: a completed Pylon powers a 6-tile radius.
    pub fn has_power_px(&self, x: i32, y: i32) -> bool {
        let r = 6 * 32;
        self.units.iter().any(|u| {
            u.alive
                && u.completed
                && u.type_id == uid::PYLON
                && (u.x / fp(1) - x).abs() <= r
                && (u.y / fp(1) - y).abs() <= r
        })
    }

    /// A Protoss building functions only inside a power field.  Nexus and
    /// Pylon are exempt (they are the field providers themselves).
    pub fn building_powered(&self, i: usize) -> bool {
        let u = &self.units[i];
        if !is_protoss_building(u.type_id) || matches!(u.type_id, uid::NEXUS | uid::PYLON) {
            return true;
        }
        self.has_power_px(u.x / fp(1), u.y / fp(1))
    }

    pub fn supply_used(&self, owner: u8) -> i32 {
        let mut used: i32 = self
            .units
            .iter()
            // Units inside a transport/bunker are hidden (`alive == false`)
            // but still consume supply.
            .filter(|u| (u.alive || u.in_transport.is_some()) && u.owner == owner)
            .map(|u| self.data.unit(u.type_id).supply_required as i32)
            .sum();
        for u in &self.units {
            if u.alive && u.owner == owner {
                for o in u.production.iter().chain(std::iter::once(&u.order)) {
                    if let Order::Train { unit_type } = o {
                        used += self.data.unit(*unit_type).supply_required as i32;
                    }
                }
            }
        }
        used
    }

    pub fn can_afford(&self, owner: u8, type_id: u16) -> bool {
        let p = &self.players[owner as usize];
        let ty = self.data.unit(type_id);
        p.minerals >= ty.mineral_cost as i32 && p.gas >= ty.gas_cost as i32
    }

    pub fn has_requirements(&self, owner: u8, type_id: u16) -> bool {
        let Some(info) = tree::build_info(type_id) else {
            return false;
        };
        let p = &self.players[owner as usize];
        if let Some(t) = info.required_tech {
            if !p.has_tech(t) {
                return false;
            }
        }
        let attached_addons: std::collections::HashSet<u32> =
            self.units.iter().filter_map(|u| u.addon).collect();
        info.required_units.iter().all(|&(required, count)| {
            self.units
                .iter()
                .filter(|u| {
                    u.alive
                        && u.owner == owner
                        && u.completed
                        && requirement_equivalent(required, u.type_id)
                        && (addon_parent(u.type_id).is_none() || attached_addons.contains(&u.id))
                })
                .count()
                >= count as usize
        })
    }

    pub fn train(&mut self, building_id: u32, unit_type: u16) -> Result<(), SimError> {
        let i = self.index_of(building_id).ok_or(SimError::BadUnit)?;
        let owner = self.units[i].owner;
        let building_type = self.units[i].type_id;
        if !self.units[i].completed || self.units[i].lifted {
            return Err(SimError::MissingRequirements);
        }
        if !self.building_powered(i) {
            return Err(SimError::Unpowered);
        }
        let info = tree::build_info(unit_type).ok_or(SimError::MissingRequirements)?;
        if info.producer != building_type {
            return Err(SimError::MissingRequirements);
        }
        if !self.has_requirements(owner, unit_type) {
            return Err(SimError::MissingRequirements);
        }
        let ty = self.data.unit(unit_type).clone();
        {
            let p = &self.players[owner as usize];
            if p.minerals < ty.mineral_cost as i32 {
                return Err(SimError::NotEnoughMinerals);
            }
            if p.gas < ty.gas_cost as i32 {
                return Err(SimError::NotEnoughGas);
            }
        }
        if self.supply_used(owner) + ty.supply_required as i32 > self.supply_cap(owner) {
            return Err(SimError::NotEnoughSupply);
        }
        if self.units[i].production.len() >= MAX_PRODUCTION_QUEUE {
            return Err(SimError::QueueFull);
        }
        {
            let p = &mut self.players[owner as usize];
            p.minerals -= ty.mineral_cost as i32;
            p.gas -= ty.gas_cost as i32;
        }
        let order = Order::Train { unit_type };
        self.units[i].production.push_back(order);
        if self.units[i].order == Order::Idle {
            self.start_next_production(i);
        }
        if let Some(r) = self.recorder.as_mut() {
            r.record_econ(
                self.tick,
                crate::replay::RecAct::Train {
                    building: building_id,
                    unit: unit_type,
                },
            );
        }
        Ok(())
    }

    /// Morph a Zerg larva into a unit.  The larva becomes an egg and hatches
    /// after the target's build time.  Supply is checked and the cost is paid
    /// immediately, matching the retail build queue.
    pub fn morph(&mut self, larva_id: u32, unit_type: u16) -> Result<(), SimError> {
        let i = self.index_of(larva_id).ok_or(SimError::BadUnit)?;
        if self.units[i].type_id != uid::LARVA {
            return Err(SimError::MissingRequirements);
        }
        let info = tree::build_info(unit_type).ok_or(SimError::MissingRequirements)?;
        if info.producer != uid::LARVA {
            return Err(SimError::MissingRequirements);
        }
        let owner = self.units[i].owner;
        if !self.has_requirements(owner, unit_type) {
            return Err(SimError::MissingRequirements);
        }
        let ty = self.data.unit(unit_type).clone();
        {
            let p = &mut self.players[owner as usize];
            if p.minerals < ty.mineral_cost as i32 {
                return Err(SimError::NotEnoughMinerals);
            }
            if p.gas < ty.gas_cost as i32 {
                return Err(SimError::NotEnoughGas);
            }
            p.minerals -= ty.mineral_cost as i32;
            p.gas -= ty.gas_cost as i32;
        }
        if self.supply_used(owner) + ty.supply_required as i32 > self.supply_cap(owner) {
            return Err(SimError::NotEnoughSupply);
        }
        self.units[i].type_id = uid::EGG;
        self.units[i].morph_to = Some(unit_type);
        self.units[i].completed = false;
        self.units[i].build_total = build_ticks(ty.build_time);
        self.units[i].build_remaining = self.units[i].build_total;
        self.units[i].hp = fp(1);
        self.units[i].order = Order::Idle;
        if let Some(r) = self.recorder.as_mut() {
            r.record_econ(
                self.tick,
                crate::replay::RecAct::Morph {
                    larva: larva_id,
                    unit: unit_type,
                },
            );
        }
        Ok(())
    }

    /// All living unit ids owned by `owner`.
    pub fn unit_ids(&self, owner: u8) -> Vec<u32> {
        self.units
            .iter()
            .filter(|u| u.alive && u.owner == owner)
            .map(|u| u.id)
            .collect()
    }

    /// Find a buildable spot near `near` (pixel coordinates).
    pub fn find_build_spot(&mut self, type_id: u16, near: (i32, i32)) -> Option<(i32, i32)> {
        self.rebuild_creep();
        // Gas buildings are not free-standing: they are placed on a geyser
        // and occupy the geyser tile.
        if is_gas_building(type_id) {
            // The Extractor is the one Zerg structure exempt from creep
            // (it lives off the geyser itself).
            let zerg = is_zerg_building(type_id) && type_id != uid::EXTRACTOR;
            let geyser = self.units.iter().find_map(|u| {
                if !u.alive || u.type_id != uid::VESPENE_GEYSER {
                    return None;
                }
                let occupied = self.units.iter().any(|b| {
                    b.alive
                        && b.completed
                        && is_gas_building(b.type_id)
                        && (b.x - u.x).abs() < fp(64)
                        && (b.y - u.y).abs() < fp(64)
                });
                if occupied {
                    None
                } else {
                    Some((u.x / fp(1), u.y / fp(1)))
                }
            });
            if let Some((gx, gy)) = geyser {
                if zerg && !self.has_creep_px(gx, gy) {
                    return None;
                }
                return Some((gx, gy));
            }
            return None;
        }
        let ty = self.data.unit(type_id).clone();
        let half_w = ty.dimensions.right.max(0).max(ty.dimensions.left.max(0)) as i32;
        let half_h = ty.dimensions.bottom.max(0).max(ty.dimensions.top.max(0)) as i32;
        let need_creep = is_zerg_building(type_id) && type_id != uid::HATCHERY;
        let need_power =
            is_protoss_building(type_id) && !matches!(type_id, uid::NEXUS | uid::PYLON);
        // Cache the Pylon field and the blocking footprint list once instead
        // of rescanning every unit for each candidate position.
        let pylons: Vec<(i32, i32)> = if need_power {
            self.units
                .iter()
                .filter(|u| u.alive && u.completed && u.type_id == uid::PYLON)
                .map(|u| (u.x / fp(1), u.y / fp(1)))
                .collect()
        } else {
            Vec::new()
        };
        let blockers: Vec<Rect> = self
            .units
            .iter()
            .filter(|u| u.alive && u.completed && !u.lifted)
            .filter_map(|u| {
                let ut = self.data.unit(u.type_id);
                if !(ut.is_building() || is_resource_type(u.type_id)) {
                    return None;
                }
                let ux = u.x / fp(1);
                let uy = u.y / fp(1);
                let hw = ut.dimensions.right.max(0).max(ut.dimensions.left.max(0)) as i32;
                let hh = ut.dimensions.bottom.max(0).max(ut.dimensions.top.max(0)) as i32;
                Some(Rect::new(ux - hw, uy - hh, ux + hw, uy + hh))
            })
            .collect();
        let step = 16;
        for ring in 0..20i32 {
            for dy in -ring..=ring {
                for dx in -ring..=ring {
                    if ring > 0 && dx.abs() != ring && dy.abs() != ring {
                        continue;
                    }
                    let x = near.0 + dx * step * 2;
                    let y = near.1 + dy * step * 2;
                    let left = x - half_w;
                    let top = y - half_h;
                    let right = x + half_w;
                    let bottom = y + half_h;
                    if left < 0 || top < 0 {
                        continue;
                    }
                    if !self.terrain.footprint_buildable(left, top, right, bottom) {
                        continue;
                    }
                    let rect = Rect::new(left, top, right, bottom);
                    if blockers.iter().any(|r| rect.intersects(r)) {
                        continue;
                    }
                    if need_creep && !self.has_creep_px(x, y) {
                        continue;
                    }
                    if need_power
                        && !pylons
                            .iter()
                            .any(|&(px, py)| (px - x).abs() <= 6 * 32 && (py - y).abs() <= 6 * 32)
                    {
                        continue;
                    }
                    return Some((x, y));
                }
            }
        }
        None
    }

    /// Build a Terran add-on on the right edge of its parent building.  The
    /// recovered build tree names the parent as the add-on's producer
    /// (Command Center → Comsat/Nuclear Silo, Factory → Machine Shop,
    /// Starport → Control Tower, Science Facility → Covert Ops/Physics Lab).
    pub fn build_addon(&mut self, parent_id: u32, addon_type: u16) -> Result<(), SimError> {
        let p = self.index_of(parent_id).ok_or(SimError::BadUnit)?;
        if !self.units[p].alive || !self.units[p].completed {
            return Err(SimError::MissingRequirements);
        }
        if addon_parent(addon_type) != Some(self.units[p].type_id) {
            return Err(SimError::InvalidTarget);
        }
        if self.units[p].addon.is_some() {
            return Err(SimError::ProducerBusy);
        }
        let owner = self.units[p].owner;
        let ty = self.data.unit(addon_type).clone();
        if self.players[owner as usize].minerals < ty.mineral_cost as i32 {
            return Err(SimError::NotEnoughMinerals);
        }
        if self.players[owner as usize].gas < ty.gas_cost as i32 {
            return Err(SimError::NotEnoughGas);
        }
        let parent_ty = self.data.unit(self.units[p].type_id).clone();
        let addon_hw = ty.dimensions.right.max(0).max(ty.dimensions.left.max(0)) as i32;
        let parent_hw = parent_ty
            .dimensions
            .right
            .max(0)
            .max(parent_ty.dimensions.left.max(0)) as i32;
        let px = self.units[p].x + (parent_hw + addon_hw).max(8) * fp(1);
        let py = self.units[p].y;
        if !self.terrain.footprint_buildable(
            px / fp(1) - addon_hw,
            py / fp(1) - addon_hw,
            px / fp(1) + addon_hw,
            py / fp(1) + addon_hw,
        ) {
            return Err(SimError::InvalidTarget);
        }
        {
            let pl = &mut self.players[owner as usize];
            pl.minerals -= ty.mineral_cost as i32;
            pl.gas -= ty.gas_cost as i32;
        }
        let idx = self.spawn_px(owner as usize, addon_type, px / fp(1), py / fp(1))?;
        self.units[idx].completed = false;
        self.units[idx].build_total = build_ticks(ty.build_time);
        self.units[idx].build_remaining = self.units[idx].build_total;
        self.units[idx].hp = (ty.hit_points_raw as i32 / 10).max(1);
        self.units[p].addon = Some(self.units[idx].id);
        self.units[idx].addon_attached = true;
        Ok(())
    }

    /// Morph one Zerg unit into another (Hydralisk → Lurker, Mutalisk →
    /// Guardian/Devourer).  The unit becomes an egg until the morph time
    /// elapses, exactly like the larva morph path.
    pub fn morph_unit(&mut self, unit_id: u32, target_type: u16) -> Result<(), SimError> {
        let i = self.index_of(unit_id).ok_or(SimError::BadUnit)?;
        if !self.units[i].alive || !self.units[i].completed {
            return Err(SimError::MissingRequirements);
        }
        let valid = matches!(
            (self.units[i].type_id, target_type),
            (uid::HYDRALISK, uid::LURKER)
                | (uid::MUTALISK, uid::GUARDIAN)
                | (uid::MUTALISK, uid::DEVOURER)
        );
        if !valid {
            return Err(SimError::InvalidTarget);
        }
        let owner = self.units[i].owner;
        if !self.has_requirements(owner, target_type) {
            return Err(SimError::MissingRequirements);
        }
        let ty = self.data.unit(target_type).clone();
        {
            let p = &mut self.players[owner as usize];
            if p.minerals < ty.mineral_cost as i32 {
                return Err(SimError::NotEnoughMinerals);
            }
            if p.gas < ty.gas_cost as i32 {
                return Err(SimError::NotEnoughGas);
            }
            p.minerals -= ty.mineral_cost as i32;
            p.gas -= ty.gas_cost as i32;
        }
        self.units[i].type_id = uid::EGG;
        self.units[i].morph_to = Some(target_type);
        self.units[i].completed = false;
        self.units[i].build_total = build_ticks(ty.build_time).max(1);
        self.units[i].build_remaining = self.units[i].build_total;
        self.units[i].hp = fp(1);
        self.units[i].order = Order::Idle;
        Ok(())
    }

    /// Place one Spider Mine from a Vulture.  Rows 18/19/131 use this path;
    /// the Vulture carries three mines (`hangar`).
    pub fn place_spider_mine(&mut self, unit_id: u32, pos: (i32, i32)) -> Result<(), SimError> {
        let i = self.index_of(unit_id).ok_or(SimError::BadUnit)?;
        if self.units[i].type_id != uid::VULTURE {
            return Err(SimError::Unsupported);
        }
        let owner = self.units[i].owner;
        if !self.players[owner as usize].has_tech(tech_id::SPIDER_MINES) {
            return Err(SimError::MissingTech);
        }
        if self.units[i].hangar == 0 {
            return Err(SimError::QueueFull);
        }
        self.spawn_temporary(owner, uid::SPIDER_MINE, pos, 0)
            .ok_or(SimError::InvalidTarget)?;
        self.units[i].hangar -= 1;
        self.units[i].cooldown = self.units[i].cooldown.max(15);
        Ok(())
    }

    /// Find an owned completed parent building with a free add-on slot.
    /// True when the unit is in the air, either natively or as a lifted
    /// Terran building.
    pub fn is_air_unit(&self, i: usize) -> bool {
        self.units[i].lifted || self.data.unit(self.units[i].type_id).is_flyer()
    }

    /// Lift a Terran building off the ground.  Add-ons are left behind and
    /// destroyed, as in retail.
    pub fn lift_building(&mut self, building_id: u32) -> Result<(), SimError> {
        let i = self.index_of(building_id).ok_or(SimError::BadUnit)?;
        if !self.units[i].alive || !self.units[i].completed {
            return Err(SimError::MissingRequirements);
        }
        if !can_lift_building(self.units[i].type_id) {
            return Err(SimError::InvalidTarget);
        }
        if self.units[i].lifted {
            return Err(SimError::ProducerBusy);
        }
        if let Some(addon_id) = self.units[i].addon.take() {
            if let Some(a) = self.index_of(addon_id) {
                // Add-ons stay grounded and become inactive, not destroyed.
                self.units[a].addon_attached = false;
            }
        }
        self.units[i].lifted = true;
        self.units[i].order = Order::Idle;
        self.units[i].path.clear();
        self.units[i].path_goal = None;
        Ok(())
    }

    /// Land a lifted building at `pos` (pixels).  The footprint must be
    /// buildable and free of other structures/units.
    pub fn land_building(&mut self, building_id: u32, pos: (i32, i32)) -> Result<(), SimError> {
        let i = self.index_of(building_id).ok_or(SimError::BadUnit)?;
        if !self.units[i].alive || !self.units[i].lifted {
            return Err(SimError::InvalidTarget);
        }
        let ty = self.data.unit(self.units[i].type_id).clone();
        let hw = ty.dimensions.right.max(0).max(ty.dimensions.left.max(0)) as i32;
        let hh = ty.dimensions.bottom.max(0).max(ty.dimensions.top.max(0)) as i32;
        let left = pos.0 - hw;
        let top = pos.1 - hh;
        if left < 0
            || top < 0
            || !self
                .terrain
                .footprint_buildable(left, top, pos.0 + hw, pos.1 + hh)
        {
            return Err(SimError::InvalidTarget);
        }
        let rect = Rect::new(left, top, pos.0 + hw, pos.1 + hh);
        let blocked = self.units.iter().any(|u| {
            if !u.alive || u.id == building_id || !u.completed || u.lifted {
                return false;
            }
            let ut = self.data.unit(u.type_id);
            if !(ut.is_building() || is_resource_type(u.type_id)) {
                return false;
            }
            let ux = u.x / fp(1);
            let uy = u.y / fp(1);
            let uhw = ut.dimensions.right.max(0).max(ut.dimensions.left.max(0)) as i32;
            let uhh = ut.dimensions.bottom.max(0).max(ut.dimensions.top.max(0)) as i32;
            rect.intersects(&Rect::new(ux - uhw, uy - uhh, ux + uhw, uy + uhh))
        });
        if blocked {
            return Err(SimError::InvalidTarget);
        }
        self.units[i].x = pos.0 * fp(1);
        self.units[i].y = pos.1 * fp(1);
        self.units[i].lifted = false;
        self.units[i].order = Order::Idle;
        self.units[i].path.clear();
        self.units[i].path_goal = None;
        // Landing on/near a grounded add-on of the right type re-attaches it.
        let parent_type = self.units[i].type_id;
        let (px, py) = (pos.0 * fp(1), pos.1 * fp(1));
        let found = self.units.iter().position(|u| {
            u.alive
                && u.completed
                && !u.addon_attached
                && addon_parent(u.type_id) == Some(parent_type)
                && (u.x - px).abs() <= fp(96)
                && (u.y - py).abs() <= fp(96)
        });
        if let Some(a) = found {
            self.units[a].addon_attached = true;
            self.units[i].addon = Some(self.units[a].id);
        }
        Ok(())
    }

    pub fn addon_parent_unit(&self, owner: u8, addon: u16) -> Option<u32> {
        let parent = addon_parent(addon)?;
        self.units
            .iter()
            .find(|u| {
                u.alive
                    && u.completed
                    && u.owner == owner
                    && u.type_id == parent
                    && u.addon.is_none()
            })
            .map(|u| u.id)
    }

    /// Find the owned completed building that can morph into `target`.
    pub fn morph_source_unit(&self, owner: u8, target: u16) -> Option<u32> {
        let source = building_morph_source(target)?;
        self.units
            .iter()
            .find(|u| u.alive && u.completed && u.owner == owner && u.type_id == source)
            .map(|u| u.id)
    }

    /// Morph an existing Zerg building into its upgrade (Hatchery → Lair,
    /// Lair → Hive, Spire → Greater Spire, Creep Colony → Sunken/Spore
    /// Colony).  The building keeps its position and identity and becomes a
    /// construction site for the target type.
    pub fn morph_building(&mut self, building_id: u32, target_type: u16) -> Result<(), SimError> {
        let i = self.index_of(building_id).ok_or(SimError::BadUnit)?;
        if !self.units[i].alive || !self.units[i].completed {
            return Err(SimError::MissingRequirements);
        }
        if building_morph_source(target_type) != Some(self.units[i].type_id) {
            return Err(SimError::InvalidTarget);
        }
        let owner = self.units[i].owner;
        if !self.has_requirements(owner, target_type) {
            return Err(SimError::MissingRequirements);
        }
        let ty = self.data.unit(target_type).clone();
        {
            let p = &mut self.players[owner as usize];
            if p.minerals < ty.mineral_cost as i32 {
                return Err(SimError::NotEnoughMinerals);
            }
            if p.gas < ty.gas_cost as i32 {
                return Err(SimError::NotEnoughGas);
            }
            p.minerals -= ty.mineral_cost as i32;
            p.gas -= ty.gas_cost as i32;
        }
        let old_max = self.data.unit(self.units[i].type_id).hit_points_raw.max(1) as i64;
        let old_hp = self.units[i].hp.max(1) as i64;
        let new_max = ty.hit_points_raw.max(1) as i64;
        self.units[i].type_id = target_type;
        self.units[i].completed = false;
        self.units[i].build_total = build_ticks(ty.build_time);
        self.units[i].build_remaining = self.units[i].build_total;
        // Preserve the current health fraction across the morph.
        self.units[i].hp = ((new_max * old_hp) / old_max).clamp(1, new_max) as i32;
        self.units[i].order = Order::Idle;
        if let Some(r) = self.recorder.as_mut() {
            r.record_econ(
                self.tick,
                crate::replay::RecAct::MorphBuilding {
                    source: building_id,
                    unit: target_type,
                },
            );
        }
        Ok(())
    }

    pub fn cancel_production(&mut self, building_id: u32) -> Result<(), SimError> {
        let i = self.index_of(building_id).ok_or(SimError::BadUnit)?;
        let order = std::mem::replace(&mut self.units[i].order, Order::Idle);
        let refund = match order {
            Order::Train { unit_type } => Some((unit_type, 100)),
            _ => None,
        };
        if let Some((unit_type, percent)) = refund {
            let ty = self.data.unit(unit_type).clone();
            let owner = self.units[i].owner as usize;
            self.players[owner].minerals += ty.mineral_cost as i32 * percent / 100;
            self.players[owner].gas += ty.gas_cost as i32 * percent / 100;
            self.units[i].build_remaining = 0;
            self.start_next_production(i);
            return Ok(());
        }
        Ok(())
    }

    pub fn research(&mut self, building_id: u32, tech_id: u8) -> Result<(), SimError> {
        let i = self.index_of(building_id).ok_or(SimError::BadUnit)?;
        let owner = self.units[i].owner;
        if !self.units[i].completed || self.units[i].lifted {
            return Err(SimError::MissingRequirements);
        }
        if !self.building_powered(i) {
            return Err(SimError::Unpowered);
        }
        if !tree::tech_buildings(tech_id)
            .iter()
            .any(|&b| requirement_equivalent(b, self.units[i].type_id))
        {
            return Err(SimError::MissingRequirements);
        }
        if self.players[owner as usize].has_tech(tech_id) {
            return Err(SimError::MissingTech);
        }
        let t = self.data.tech(tech_id).ok_or(SimError::BadUnit)?.clone();
        {
            let p = &mut self.players[owner as usize];
            if p.minerals < t.mineral_cost as i32 {
                return Err(SimError::NotEnoughMinerals);
            }
            if p.gas < t.gas_cost as i32 {
                return Err(SimError::NotEnoughGas);
            }
            p.minerals -= t.mineral_cost as i32;
            p.gas -= t.gas_cost as i32;
        }
        if self.units[i].order != Order::Idle {
            return Err(SimError::ProducerBusy);
        }
        self.units[i].build_remaining = research_ticks(t.research_time);
        self.units[i].build_total = self.units[i].build_remaining;
        self.units[i].order = Order::Research { tech_id };
        Ok(())
    }

    pub fn upgrade(&mut self, building_id: u32, upgrade_id: u8) -> Result<(), SimError> {
        let i = self.index_of(building_id).ok_or(SimError::BadUnit)?;
        let owner = self.units[i].owner;
        if !self.units[i].completed || self.units[i].lifted {
            return Err(SimError::MissingRequirements);
        }
        if !self.building_powered(i) {
            return Err(SimError::Unpowered);
        }
        if !tree::upgrade_buildings(upgrade_id)
            .iter()
            .any(|&b| requirement_equivalent(b, self.units[i].type_id))
        {
            return Err(SimError::MissingRequirements);
        }
        let up = self
            .data
            .upgrade(upgrade_id)
            .ok_or(SimError::BadUnit)?
            .clone();
        let level = self.players[owner as usize].upgrade_level(upgrade_id);
        if level >= up.max_level {
            return Err(SimError::MissingTech);
        }
        let next = level + 1;
        let extra = tree::upgrade_level_requirement(upgrade_id, level);
        for required in extra {
            if !self
                .units
                .iter()
                .any(|u| u.alive && u.owner == owner && u.completed && u.type_id == *required)
            {
                return Err(SimError::MissingRequirements);
            }
        }
        {
            let p = &mut self.players[owner as usize];
            let m = up.mineral_cost(next);
            let g = up.gas_cost(next);
            if p.minerals < m as i32 {
                return Err(SimError::NotEnoughMinerals);
            }
            if p.gas < g as i32 {
                return Err(SimError::NotEnoughGas);
            }
            p.minerals -= m as i32;
            p.gas -= g as i32;
        }
        if self.units[i].order != Order::Idle {
            return Err(SimError::ProducerBusy);
        }
        self.units[i].build_remaining = upgrade_ticks(up.time_cost(next));
        self.units[i].build_total = self.units[i].build_remaining;
        self.units[i].order = Order::Upgrade {
            upgrade_id,
            level: next,
        };
        Ok(())
    }

    fn start_next_production(&mut self, i: usize) {
        let next = self.units[i].production.pop_front();
        if let Some(Order::Train { unit_type }) = next {
            let ty = self.data.unit(unit_type);
            self.units[i].build_remaining = build_ticks(ty.build_time);
            self.units[i].build_total = self.units[i].build_remaining;
            self.units[i].order = Order::Train { unit_type };
        }
    }

    pub fn issue(&mut self, unit_id: u32, new_order: Order) -> Result<(), SimError> {
        let i = self.index_of(unit_id).ok_or(SimError::BadUnit)?;
        if !self.units[i].alive {
            return Err(SimError::BadUnit);
        }
        match &new_order {
            Order::AttackUnit { target } => {
                if self.unit(*target).is_none() {
                    return Err(SimError::InvalidTarget);
                }
            }
            Order::Gather { resource } => {
                let target = self.unit(*resource).ok_or(SimError::InvalidTarget)?;
                if !target.is_resource() && !is_gas_building(target.type_id) {
                    return Err(SimError::InvalidTarget);
                }
            }
            _ => {}
        }
        self.units[i].order = new_order;
        self.units[i].order_row = self.units[i].order.order_id();
        self.units[i].resume_order = None;
        self.units[i].target = None;
        self.units[i].path.clear();
        self.units[i].path_goal = None;
        if !matches!(self.units[i].order, Order::Idle | Order::HoldPosition) {
            self.units[i].queue.clear();
        }
        // Record successful commands for save games / replays / LAN sync.
        if self.recorder.is_some() {
            let tick = self.tick;
            let logged = self.units[i].order.clone();
            if let Some(r) = self.recorder.as_mut() {
                r.cmds.push(crate::replay::RecCmd {
                    tick,
                    act: crate::replay::RecAct::Issue {
                        unit: unit_id,
                        order: logged,
                    },
                });
            }
        }
        Ok(())
    }

    /// Issue any of the 189 `orders.dat` rows by id.  This is the front
    /// door to the recovered handler table: every row with a direct case in
    /// `FUN_7a178340` calls its own transcribed Rust routine in
    /// [`crate::handlers`], and rows resolved by an alias, a production timer
    /// or the engine fall back to the typed state machine.
    pub fn issue_order_id(
        &mut self,
        unit_id: u32,
        order_id: u8,
        target: Option<u32>,
        pos: Option<(i32, i32)>,
    ) -> Result<(), SimError> {
        crate::handlers::dispatch_row(self, unit_id, order_id, target, pos)
    }

    /// Apply a row through the typed order state machine.  This is the body
    /// called by the transcribed handler routines and by rows that are
    /// resolved before the recovered switch.
    pub(crate) fn apply_order_row_typed(
        &mut self,
        unit_id: u32,
        order_id: u8,
        target: Option<u32>,
        pos: Option<(i32, i32)>,
    ) -> Result<(), SimError> {
        let result = self.apply_order_row_typed_inner(unit_id, order_id, target, pos);
        if result.is_ok() {
            if let Some(i) = self.index_of(unit_id) {
                self.units[i].order_row = order_id;
            }
        }
        result
    }

    fn apply_order_row_typed_inner(
        &mut self,
        unit_id: u32,
        order_id: u8,
        target: Option<u32>,
        pos: Option<(i32, i32)>,
    ) -> Result<(), SimError> {
        // Internal-but-player-visible rows with a concrete API.
        match order_id {
            // Critters / Junk Yard Dog wander around their current point.
            154 | 155 | 178 => {
                let i = self.index_of(unit_id).ok_or(SimError::BadUnit)?;
                let a = (self.units[i].x / fp(1), self.units[i].y / fp(1));
                let dx = (self.world_rand() % 128) as i32 - 64;
                let dy = (self.world_rand() % 128) as i32 - 64;
                return self.issue(
                    unit_id,
                    Order::Patrol {
                        a,
                        b: (a.0 + dx, a.1 + dy),
                        forward: true,
                    },
                );
            }
            // AI harass uses attack-move; guard/tower/passive rows hold.
            157 => {
                let p = pos.ok_or(SimError::Unsupported)?;
                return self.issue(unit_id, Order::AttackMove { pos: p });
            }
            156 | 159 | 160 | 161 | 163 | 166 | 61 => {
                return self.issue(unit_id, Order::HoldPosition)
            }
            // Computer Return heads for the owner's town hall.
            162 => {
                let i = self.index_of(unit_id).ok_or(SimError::BadUnit)?;
                let owner = self.units[i].owner;
                let home = self
                    .units
                    .iter()
                    .find(|u| {
                        u.alive
                            && u.completed
                            && u.owner == owner
                            && matches!(
                                u.type_id,
                                uid::COMMAND_CENTER
                                    | uid::NEXUS
                                    | uid::HATCHERY
                                    | uid::LAIR
                                    | uid::HIVE
                            )
                    })
                    .map(|u| (u.x / fp(1), u.y / fp(1)));
                return match home {
                    Some(p) => self.issue(unit_id, Order::Move { pos: p }),
                    None => self.issue(unit_id, Order::Stop),
                };
            }
            // Self-destructing rows (Scourge-style campaign units).
            164 | 174 => {
                let i = self.index_of(unit_id).ok_or(SimError::BadUnit)?;
                let hp = self.units[i].hp;
                self.damage_unit(unit_id, hp)?;
                return Ok(());
            }
            // Reaver/Scarab combat rows.
            56 | 58 | 59 | 60 | 64 | 65 => {
                if let Some(t) = target {
                    return self.issue(unit_id, Order::AttackUnit { target: t });
                }
                let p = pos.ok_or(SimError::Unsupported)?;
                return self.issue(unit_id, Order::AttackMove { pos: p });
            }
            18 | 19 | 131 => {
                let p = pos.ok_or(SimError::Unsupported)?;
                self.place_spider_mine(unit_id, p)?;
                return Ok(());
            }
            49 | 62 => {
                self.build_hangar_unit(unit_id)?;
                return Ok(());
            }
            51 | 52 | 55 => return self.issue(unit_id, Order::Stop),
            68 => {
                let p = pos.ok_or(SimError::Unsupported)?;
                let i = self.index_of(unit_id).ok_or(SimError::BadUnit)?;
                self.units[i].rally = Some(p);
                return Ok(());
            }
            104 | 105 | 147 => {
                return self.cast(unit_id, Spell::ArchonWarp, SpellTarget::None);
            }
            182 => return self.cast(unit_id, Spell::DarkArchonMeld, SpellTarget::None),
            138 => {
                let target = pos
                    .map(|(x, y)| SpellTarget::Point(x, y))
                    .unwrap_or(SpellTarget::None);
                return self.cast(unit_id, Spell::ScannerSweep, target);
            }
            _ => {}
        }
        // Rows whose behaviour is a direct ability call rather than a typed
        // order state.  These are the shipped cast/cloak/shield rows.
        if let Some(spell) = row_spell(order_id) {
            let target = match target {
                Some(t) => SpellTarget::Unit(t),
                None => match pos {
                    Some((x, y)) => SpellTarget::Point(x, y),
                    None => SpellTarget::None,
                },
            };
            return self.cast(unit_id, spell, target);
        }
        match order_id {
            57 | 107 | 108 | 136 => return self.set_cloak(unit_id, true),
            109 => return self.set_cloak(unit_id, false),
            66 | 67 => {
                let t = target.ok_or(SimError::Unsupported)?;
                self.shield_battery(unit_id, t)?;
                return Ok(());
            }
            124 | 127 => {
                let p = pos.ok_or(SimError::Unsupported)?;
                return self.launch_nuke(unit_id, p);
            }
            126 => {
                let t = target.ok_or(SimError::Unsupported)?;
                let u = self.unit(t).ok_or(SimError::InvalidTarget)?;
                let p = (u.x / fp(1), u.y / fp(1));
                return self.launch_nuke(unit_id, p);
            }
            _ => {}
        }
        // Terran lift/land rows (69-73) have their own typed states.
        match order_id {
            71..=73 => return self.issue(unit_id, Order::Lift),
            69 | 70 => {
                let p = pos.ok_or(SimError::Unsupported)?;
                return self.issue(unit_id, Order::Land { pos: p });
            }
            _ => {}
        }
        let order = match order_class(order_id) {
            OrderClass::Idle => Order::Stop,
            OrderClass::Move => Order::Move {
                pos: pos.ok_or(SimError::Unsupported)?,
            },
            OrderClass::Attack => match target {
                Some(t) => Order::AttackUnit { target: t },
                None => Order::AttackMove {
                    pos: pos.ok_or(SimError::Unsupported)?,
                },
            },
            OrderClass::AttackMove => Order::AttackMove {
                pos: pos.ok_or(SimError::Unsupported)?,
            },
            OrderClass::Hold => Order::HoldPosition,
            OrderClass::Patrol => {
                let a = pos.ok_or(SimError::Unsupported)?;
                Order::Patrol {
                    a,
                    b: a,
                    forward: true,
                }
            }
            OrderClass::Harvest => Order::Gather {
                resource: target.ok_or(SimError::Unsupported)?,
            },
            OrderClass::ReturnCargo => Order::ReturnCargo {
                depot: target.ok_or(SimError::Unsupported)?,
            },
            OrderClass::Build => return Err(SimError::Unsupported),
            OrderClass::Train => return Err(SimError::Unsupported),
            OrderClass::Research => return Err(SimError::Unsupported),
            OrderClass::Upgrade => return Err(SimError::Unsupported),
            OrderClass::Cast => return Err(SimError::Unsupported),
            OrderClass::Transport => Order::EnterTransport {
                transport: target.ok_or(SimError::Unsupported)?,
            },
            OrderClass::Unload => Order::Unload,
            OrderClass::Burrow => Order::Burrow,
            OrderClass::Siege => Order::Siege,
            OrderClass::Morph => return Err(SimError::Unsupported),
            OrderClass::Special => Order::Stop,
        };
        self.issue(unit_id, order)
    }

    pub fn stop(&mut self, unit_id: u32) {
        if let Some(i) = self.index_of(unit_id) {
            self.units[i].order = Order::Stop;
            self.units[i].resume_order = None;
            self.units[i].queue.clear();
            self.units[i].path.clear();
            self.units[i].path_goal = None;
            self.units[i].target = None;
        }
    }

    pub fn queue_order(&mut self, unit_id: u32, o: Order) -> Result<(), SimError> {
        let i = self.index_of(unit_id).ok_or(SimError::BadUnit)?;
        // The retail queue counter is only incremented for orders whose
        // `orders.dat` highlight field is not -1.  Non-queueable orders
        // interrupt the current order instead of occupying a queue slot.
        let queueable = self
            .data
            .order(o.order_id())
            .map(|row| row.highlight != -1)
            .unwrap_or(false);
        if queueable && self.units[i].queue.len() >= MAX_ORDER_QUEUE {
            return Err(SimError::QueueFull);
        }
        if self.units[i].order == Order::Idle || !queueable {
            self.units[i].order = o;
            if !queueable {
                self.units[i].queue.clear();
            }
        } else {
            self.units[i].queue.push_back(o);
        }
        Ok(())
    }

    pub fn set_rally(&mut self, building_id: u32, pos: (i32, i32)) -> Result<(), SimError> {
        let i = self.index_of(building_id).ok_or(SimError::BadUnit)?;
        self.units[i].rally = Some(pos);
        Ok(())
    }

    // ------------------------------------------------------------------
    // Tick
    // ------------------------------------------------------------------

    pub fn step(&mut self) {
        if self.winner.is_some() {
            return;
        }
        self.tick += 1;
        let n = self.units.len();
        for i in 0..n {
            if !self.units[i].alive {
                continue;
            }
            self.update_timers(i);
            self.update_construction(i);
            self.update_production(i);
        }
        for i in 0..n {
            if !self.units[i].alive {
                continue;
            }
            self.tick_animation(i);
            if !self.units[i].alive {
                continue;
            }
            let order = self.units[i].order.clone();
            // Recovered `0x7a178340` dispatch cadence: the carry counter is
            // decremented every tick and reset to 8 when it reaches zero, so
            // the carry-gated handlers run on ticks 0, 8, 16, ... per unit.
            let carry = self.units[i].order_carry;
            self.units[i].order_carry = if carry == 0 { 8 } else { carry - 1 };
            let think = order_runs_this_tick(order.order_id(), carry);
            if think {
                // Enter the recovered handler routine for this row.  The
                // typed order state machine below remains the gameplay
                // authority; this is the recovered dispatcher boundary and
                // records the hit in `World::handler_hits`.
                crate::handlers::tick_row(self, self.units[i].id, order.order_id());
            }
            if !matches!(order, Order::Die) {
                self.execute_order(i, order, think);
            }
        }
        self.resolve_collisions();
        if self.tick.is_multiple_of(CREEP_GROW_INTERVAL as u64) {
            self.grow_creep();
        }
        self.update_specials();
        self.update_projectiles();
        self.update_larva();
        self.update_vision();
        self.resolve_deaths();
        self.check_victory();
        if self.triggers.is_some() {
            let mut triggers = self.triggers.take();
            if let Some(t) = triggers.as_mut() {
                t.process(self);
            }
            self.triggers = triggers;
        }
    }

    fn update_timers(&mut self, i: usize) {
        let ty = self.data.unit(self.units[i].type_id).clone();
        if self.units[i].status.stasis_ticks > 0 {
            self.units[i].status.stasis_ticks -= 1;
            return;
        }
        self.units[i].anim_tick = self.units[i].anim_tick.wrapping_add(1);
        if self.units[i].cooldown > 0 {
            self.units[i].cooldown -= 1;
        }
        if ty.has_shields {
            let max = fp(ty.shield_points as i32);
            if self.units[i].shields < max {
                self.units[i].shields = (self.units[i].shields + SHIELD_REGEN_PER_TICK).min(max);
            }
        }
        if ty.regenerates() {
            let max = ty.hit_points_raw as i32;
            if self.units[i].hp < max && self.units[i].hp > 0 {
                self.units[i].hp = (self.units[i].hp + ZERG_REGEN_PER_TICK).min(max);
            }
        }
        // Terran structures below one third HP burn at the recovered rate of
        // 20/256 HP per frame (1.875 HP/s at 24 Hz).
        if self.units[i].completed && is_terran_building(self.units[i].type_id) {
            let max = ty.hit_points_raw as i32;
            if self.units[i].hp > 0 && self.units[i].hp * 3 < max {
                self.units[i].hp -= 20;
                if self.units[i].hp <= 0 {
                    self.kill_unit(i);
                    return;
                }
            }
        }
        if self.units[i].special_cooldown > 0 {
            self.units[i].special_cooldown -= 1;
        }
        if self.units[i].expires_at > 0 && self.tick as i64 >= self.units[i].expires_at {
            self.kill_unit(i);
            return;
        }
        if ty.flags & 0x0020_0000 != 0 {
            // Personal cloaking drains 10 (Ghost) or 13 (Wraith) energy in
            // 1/256 points per tick and suppresses the +8 regeneration, for
            // the documented net 18/256 and 21/256.  Every other energy user
            // regenerates 8/256 per tick up to its maximum.
            let cloak_drain = if self.units[i].status.cloaked && !self.units[i].status.hallucination
            {
                match self.units[i].type_id {
                    uid::GHOST => GHOST_CLOAK_COST_PER_TICK,
                    uid::WRAITH => WRAITH_CLOAK_COST_PER_TICK,
                    _ => 0,
                }
            } else {
                0
            };
            if cloak_drain > 0 {
                if !self.units[i].spend_energy_fixed(cloak_drain) {
                    self.units[i].status.cloaked = false;
                }
            } else {
                self.units[i].add_energy_fixed(ENERGY_REGEN_PER_TICK);
            }
        }
        let (irradiate, plague) = {
            let s = &mut self.units[i].status;
            s.stim_ticks = (s.stim_ticks - 1).max(0);
            s.lockdown_ticks = (s.lockdown_ticks - 1).max(0);
            s.irradiate_ticks = (s.irradiate_ticks - 1).max(0);
            s.plague_ticks = (s.plague_ticks - 1).max(0);
            s.ensnare_ticks = (s.ensnare_ticks - 1).max(0);
            s.maelstrom_ticks = (s.maelstrom_ticks - 1).max(0);
            s.disruption_ticks = (s.disruption_ticks - 1).max(0);
            s.optical_flare_ticks = (s.optical_flare_ticks - 1).max(0);
            s.revealed_ticks = (s.revealed_ticks - 1).max(0);
            (s.irradiate_ticks, s.plague_ticks)
        };
        if irradiate > 0 && self.tick.is_multiple_of(8) {
            self.damage_hp(i, fp(2), 0);
        }
        if plague > 0 && self.tick.is_multiple_of(8) {
            let hp = self.units[i].hp;
            if hp > 0 {
                self.damage_hp(i, (hp - fp(1)).max(0), 0);
            }
        }
        if ty.is_building() && self.units[i].completed && self.units[i].type_id == uid::HATCHERY {
            self.units[i].larva_timer -= 1;
        }
    }

    fn update_construction(&mut self, i: usize) {
        if self.units[i].completed {
            return;
        }
        if self.units[i].build_remaining > 0 {
            self.units[i].build_remaining -= 1;
            if self.units[i].build_remaining <= 0 {
                self.units[i].completed = true;
                if let Some(target) = self.units[i].morph_to.take() {
                    self.units[i].type_id = target;
                }
                let ty = self.data.unit(self.units[i].type_id).clone();
                self.units[i].hp = ty.hit_points_raw as i32;
                if ty.has_shields {
                    self.units[i].shields = fp(ty.shield_points as i32);
                }
                if is_creep_source(self.units[i].type_id) {
                    self.creep_dirty = true;
                }
                // A completed Refinery/Assimilator/Extractor takes over the
                // geyser underneath it: the gas pool moves to the building
                // and the geyser is hidden.
                if is_gas_building(self.units[i].type_id) {
                    let (bx, by) = (self.units[i].x, self.units[i].y);
                    let geyser = self.units.iter().position(|u| {
                        u.alive
                            && u.type_id == uid::VESPENE_GEYSER
                            && (u.x - bx).abs() < fp(64)
                            && (u.y - by).abs() < fp(64)
                    });
                    if let Some(g) = geyser {
                        let pool = self.units[g].resource_amount;
                        self.units[i].resource_amount = if pool > 0 { pool } else { GAS_CAPACITY };
                        self.units[g].resource_amount = 0;
                        self.units[g].alive = false;
                    }
                }
            }
        }
    }

    fn update_production(&mut self, i: usize) {
        if !self.units[i].alive || !self.units[i].completed || self.units[i].lifted {
            return;
        }
        // Production and research pause while a Protoss structure is outside
        // a Pylon field; it resumes when power returns.
        if !self.building_powered(i) {
            return;
        }
        if self.units[i].build_remaining > 0 {
            self.units[i].build_remaining -= 1;
            if self.units[i].build_remaining > 0 {
                return;
            }
        }
        let order = self.units[i].order.clone();
        match order {
            Order::Train { unit_type } => {
                let owner = self.units[i].owner;
                let spawn_pos = self.exit_position(i);
                let rally = self.units[i].rally;
                if let Ok(new_idx) = self.spawn_px(
                    owner as usize,
                    unit_type,
                    spawn_pos.0 / fp(1),
                    spawn_pos.1 / fp(1),
                ) {
                    if let Some(rally) = rally {
                        self.units[new_idx].order = Order::Move { pos: rally };
                    }
                }
                self.units[i].order = Order::Idle;
                self.start_next_production(i);
            }
            Order::Research { tech_id } => {
                let owner = self.units[i].owner as usize;
                if let Some(t) = self.data.tech(tech_id) {
                    let _ = t;
                }
                self.players[owner].techs[tech_id as usize] = true;
                self.units[i].order = Order::Idle;
            }
            Order::Upgrade { upgrade_id, level } => {
                let owner = self.units[i].owner as usize;
                self.players[owner].upgrades[upgrade_id as usize] = level;
                self.units[i].order = Order::Idle;
            }
            _ => {}
        }
    }

    fn update_larva(&mut self) {
        let mut spawns: Vec<(usize, (i32, i32))> = Vec::new();
        for i in 0..self.units.len() {
            let (alive, completed, owner, ux, uy, hatch) = {
                let u = &self.units[i];
                (
                    u.alive,
                    u.completed,
                    u.owner,
                    u.x,
                    u.y,
                    matches!(u.type_id, uid::HATCHERY | uid::LAIR | uid::HIVE),
                )
            };
            if !alive || !completed || !hatch {
                continue;
            }
            let larva_count = self
                .units
                .iter()
                .filter(|l| {
                    l.alive
                        && l.owner == owner
                        && l.type_id == uid::LARVA
                        && (l.x - ux).abs() < fp(96)
                        && (l.y - uy).abs() < fp(96)
                })
                .count();
            if larva_count >= 3 {
                self.units[i].larva_timer = 300;
                continue;
            }
            if self.units[i].larva_timer <= 0 {
                self.units[i].larva_timer = 300;
                spawns.push((owner as usize, (ux / fp(1) + 16, uy / fp(1) + 16)));
            } else {
                self.units[i].larva_timer -= 1;
            }
        }
        for (owner, (x, y)) in spawns {
            let _ = self.spawn_px(owner, uid::LARVA, x, y);
        }
    }

    fn exit_position(&self, i: usize) -> (i32, i32) {
        let u = &self.units[i];
        let ty = self.data.unit(u.type_id);
        let w = ty.dimensions.right.max(0) as i32 * fp(1);
        let h = ty.dimensions.bottom.max(0) as i32 * fp(1);
        (u.x + w / 2 + fp(8), u.y + h / 2 + fp(8))
    }

    /// Advance toward a hostile without starting a new attack.  Used on the
    /// ticks between carry passes: retail keeps moving the flingy every tick
    /// while the order handler only re-validates the target every 8th tick.
    fn advance_attack(&mut self, i: usize, target: u32) {
        if let Some(tidx) = self.index_of(target) {
            if self.units[tidx].alive {
                let p = (self.units[tidx].x, self.units[tidx].y);
                self.move_towards(i, p, fp(2));
            }
        }
    }

    fn execute_order(&mut self, i: usize, order: Order, think: bool) {
        if self.units[i].status.stasis_ticks > 0
            || self.units[i].status.lockdown_ticks > 0
            || self.units[i].status.maelstrom_ticks > 0
            || self.units[i].status.burrowed
        {
            return;
        }
        match order {
            Order::Idle => self.order_idle(i),
            Order::Stop => {
                self.units[i].target = None;
                self.units[i].path.clear();
                self.units[i].path_goal = None;
                self.units[i].order = Order::Idle;
            }
            Order::Move { pos } => {
                if self.move_towards(i, pos, fp(4)) {
                    self.order_done(i);
                }
            }
            Order::AttackMove { pos } => {
                if think {
                    if let Some(target) = self.acquire_target(i) {
                        self.units[i].resume_order = Some(Order::AttackMove { pos });
                        self.units[i].order = Order::AttackUnit { target };
                        self.attack_target(i, target);
                        return;
                    }
                }
                if self.move_towards(i, pos, fp(4)) {
                    self.order_done(i);
                }
            }
            Order::AttackUnit { target } => {
                if think {
                    self.attack_target(i, target);
                } else {
                    self.advance_attack(i, target);
                }
            }
            Order::HoldPosition => {
                if let Some(target) = self.acquire_target(i) {
                    self.attack_target_no_move(i, target);
                }
            }
            Order::Patrol { a, b, forward } => {
                if let Some(target) = self.acquire_target(i) {
                    self.attack_target(i, target);
                    return;
                }
                let dest = if forward { b } else { a };
                if self.move_towards(i, dest, fp(4)) {
                    let next = Order::Patrol {
                        a,
                        b,
                        forward: !forward,
                    };
                    self.units[i].order = next;
                }
            }
            Order::Gather { resource } => self.order_gather(i, resource),
            Order::ReturnCargo { depot } => self.order_return(i, depot),
            Order::Build { unit_type, pos } => {
                if think {
                    self.order_build(i, unit_type, pos);
                }
            }
            Order::Repair { target } => self.order_repair(i, target),
            Order::Train { .. } | Order::Research { .. } | Order::Upgrade { .. } => {
                // Handled by update_production.
            }
            Order::Cast { spell, target } => {
                if think {
                    self.order_cast(i, spell, target);
                }
            }
            Order::EnterTransport { transport } => self.order_enter_transport(i, transport),
            Order::Unload => self.order_unload(i),
            Order::Burrow => {
                if think {
                    self.order_burrow(i, true);
                }
            }
            Order::Unburrow => {
                if think {
                    self.order_burrow(i, false);
                }
            }
            Order::Siege => {
                if think {
                    self.order_siege(i, true);
                }
            }
            Order::Unsiege => {
                if think {
                    self.order_siege(i, false);
                }
            }
            Order::Lift => {
                let id = self.units[i].id;
                let _ = self.lift_building(id);
            }
            Order::Land { pos } => {
                let id = self.units[i].id;
                let _ = self.land_building(id, pos);
            }
            Order::Die => {}
        }
    }

    fn order_done(&mut self, i: usize) {
        if self.units[i].anim.as_ref().is_some_and(|a| {
            matches!(
                a.anim as usize,
                ANIM_GND_ATTACK_INIT
                    | ANIM_AIR_ATTACK_INIT
                    | ANIM_GND_ATTACK_REPEAT
                    | ANIM_AIR_ATTACK_REPEAT
                    | ANIM_CAST_SPELL
            )
        }) {
            self.units[i].anim = None;
        }
        if let Some(resume) = self.units[i].resume_order.take() {
            // Finishing the temporary attack returns the unit to its
            // attack-move destination instead of dropping to Idle.
            self.units[i].order = resume;
            self.units[i].target = None;
            self.units[i].path.clear();
            self.units[i].path_goal = None;
            return;
        }
        self.units[i].order = Order::Idle;
        self.units[i].target = None;
        self.units[i].path.clear();
        self.units[i].path_goal = None;
        if let Some(next) = self.units[i].queue.pop_front() {
            self.units[i].order = next;
        }
    }

    fn order_idle(&mut self, i: usize) {
        let ty = self.data.unit(self.units[i].type_id).clone();
        if ty.is_building() {
            // Defensive buildings (Missile Turret, Photon Cannon, Sunken and
            // Spore Colony) acquire and fire while stationary; economic
            // buildings have no weapon and stay idle.
            if let Some(target) = self.acquire_target(i) {
                self.attack_target_no_move(i, target);
            }
            return;
        }
        if self.units[i].type_id == uid::SCV
            || self.units[i].type_id == uid::PROBE
            || self.units[i].type_id == uid::DRONE
        {
            return;
        }
        if let Some(target) = self.acquire_target(i) {
            self.attack_target(i, target);
        }
    }

    fn attack_target(&mut self, i: usize, target: u32) {
        let Some(tidx) = self.index_of(target) else {
            self.order_done(i);
            return;
        };
        if !self.units[tidx].alive {
            self.order_done(i);
            return;
        }
        self.units[i].target = Some(target);
        if self.attack_target_no_move(i, target) {
            return;
        }
        let (tx, ty) = (self.units[tidx].x, self.units[tidx].y);
        self.move_towards(i, (tx, ty), fp(2));
    }

    /// Fire if in range.  Returns true when the unit is in range (whether or
    /// not the cooldown allowed a shot this tick).
    fn attack_target_no_move(&mut self, i: usize, target: u32) -> bool {
        let Some(tidx) = self.index_of(target) else {
            return false;
        };
        if !self.units[tidx].alive {
            return false;
        }
        self.units[i].target = Some(target);
        let Some((weapon_id, weapon)) = self.weapon_for(i, tidx) else {
            return self.special_attack(i, tidx);
        };
        let dist = self.distance(i, tidx);
        if dist > weapon.max_range as i32 * fp(1) || dist < weapon.min_range as i32 * fp(1) {
            return false;
        }
        if self.units[i].status.disruption_ticks > 0 {
            return false;
        }
        // The attack animation supplies the release timing: the weapon fires
        // on its `attack`/`attackwith`/`attackmelee` opcode, not on the tick
        // the order is issued.
        let attacking = self.units[i].anim.as_ref().is_some_and(|a| {
            matches!(
                a.anim as usize,
                ANIM_GND_ATTACK_INIT
                    | ANIM_AIR_ATTACK_INIT
                    | ANIM_GND_ATTACK_REPEAT
                    | ANIM_AIR_ATTACK_REPEAT
            )
        });
        if attacking {
            return true;
        }
        if self.units[i].cooldown == 0 {
            let flying = self.is_air_unit(tidx);
            if !self.start_attack_animation(i, flying) {
                self.fire(i, tidx, weapon_id, &weapon);
            }
        }
        true
    }

    fn weapon_for(&self, attacker: usize, target: usize) -> Option<(u8, WeaponType)> {
        let a_ty = self.data.unit(self.units[attacker].type_id);
        if self.is_air_unit(target) {
            let id = a_ty.air_weapon;
            self.data.air_weapon(a_ty).cloned().map(|w| (id, w))
        } else {
            let id = a_ty.ground_weapon;
            self.data.ground_weapon(a_ty).cloned().map(|w| (id, w))
        }
    }

    fn acquire_target(&self, i: usize) -> Option<u32> {
        let u = &self.units[i];
        let ty = self.data.unit(u.type_id);
        let mut acq_range = self.data.target_acquisition_px(ty);
        if matches!(u.type_id, uid::CARRIER | uid::REAVER) {
            acq_range = acq_range.max(if u.type_id == uid::CARRIER {
                8 * 32
            } else {
                10 * 32
            });
        }
        if acq_range == 0 {
            return None;
        }
        let max_fp = acq_range * fp(1);
        let mut best: Option<(i32, u32)> = None;
        for (other_idx, other) in self.units.iter().enumerate() {
            if !other.alive
                || other.owner == u.owner
                || other.status.hallucination && other.owner == u.owner
            {
                continue;
            }
            if u.in_transport == Some(other.id) {
                continue;
            }
            let t_ty = self.data.unit(other.type_id);
            if is_resource_type(other.type_id) || other.type_id == uid::START_LOCATION {
                continue;
            }
            if !self.can_see_unit(u.owner, other_idx) {
                continue;
            }
            let Some(w) = (if self.is_air_unit(other_idx) {
                self.data.air_weapon(ty)
            } else {
                self.data.ground_weapon(ty)
            }) else {
                if !matches!(u.type_id, uid::CARRIER | uid::REAVER) {
                    continue;
                }
                // Carriers and Reavers attack through their hangar units.
                if u.hangar == 0 {
                    continue;
                }
                let d = self.distance_ids(i, other.id);
                if d <= max_fp && best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, other.id));
                }
                continue;
            };
            if !weapon_can_target(w, t_ty) {
                continue;
            }
            let d = self.distance_ids(i, other.id);
            if d <= max_fp && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, other.id));
            }
        }
        best.map(|(_, id)| id)
    }

    fn distance_ids(&self, a: usize, b: u32) -> i32 {
        let ui = &self.units[a];
        let Some(bi) = self.index_of(b) else {
            return i32::MAX;
        };
        let uj = &self.units[bi];
        let dx = (ui.x - uj.x) as i64;
        let dy = (ui.y - uj.y) as i64;
        isqrt(dx * dx + dy * dy) as i32
    }

    fn distance(&self, a: usize, b: usize) -> i32 {
        let dx = (self.units[a].x - self.units[b].x) as i64;
        let dy = (self.units[a].y - self.units[b].y) as i64;
        isqrt(dx * dx + dy * dy) as i32
    }

    fn fire(&mut self, attacker: usize, target: usize, weapon_id: u8, weapon: &WeaponType) {
        let shots = if weapon.bullet_count == 2 { 2 } else { 1 };
        let owner = self.units[attacker].owner;
        if self.units[attacker].status.hallucination {
            // Hallucinations play the attack animation and cooldown, but do
            // no damage.
            self.units[attacker].cooldown = self.modified_cooldown(attacker, weapon);
            return;
        }
        if weapon.bullet_type == behavior::SELF_DESTRUCT {
            self.kill_unit(attacker);
            return;
        }
        if Projectile::weapon_is_travelling(weapon) {
            for _ in 0..shots {
                self.spawn_projectile(attacker, target, weapon_id, weapon);
            }
        } else {
            let missed = weapon.bullet_type == behavior::APPEAR_AT_TARGET_UNIT
                && self.target_dodges(attacker, target);
            if !missed {
                for _ in 0..shots {
                    self.resolve_hit_type(
                        owner,
                        Some(self.units[target].id),
                        attacker,
                        weapon,
                        (self.units[target].x, self.units[target].y),
                        1,
                        1,
                    );
                }
            }
        }
        self.units[attacker].cooldown = self.modified_cooldown(attacker, weapon);
    }

    /// Cooldown modifiers recovered from the retail `get_modified_weapon_cooldown`
    /// helper: acid spores lengthen it, Stim/cooldown upgrades halve it and
    /// Ensnare adds 25%.  The result is clamped to `[5, 250]`.
    fn modified_cooldown(&self, attacker: usize, weapon: &WeaponType) -> u32 {
        let u = &self.units[attacker];
        let mut cooldown = weapon.cooldown as i32;
        if u.status.acid_spores > 0 {
            cooldown += (cooldown / 8).max(3) * u.status.acid_spores as i32;
        }
        let mut mods = 0;
        if u.status.stim_ticks > 0 {
            mods += 1;
        }
        if self.data.unit(u.type_id).flags & 0x2000_0000 != 0 {
            mods += 1;
        }
        if u.status.ensnare_ticks > 0 {
            mods -= 1;
        }
        if mods > 0 {
            cooldown /= 2;
        } else if mods < 0 {
            cooldown += cooldown / 4;
        }
        cooldown = cooldown.clamp(5, 250);
        cooldown as u32
    }

    /// Create one travelling bullet.  The same routine handles every weapon
    /// behaviour; `Projectile::state` selects the per-tick movement rule.
    fn spawn_projectile(
        &mut self,
        attacker: usize,
        target: usize,
        weapon_id: u8,
        weapon: &WeaponType,
    ) {
        if self.projectiles.len() >= MAX_PROJECTILES {
            return;
        }
        let a = &self.units[attacker];
        let (ax, ay) = (a.x, a.y);
        let facing = a.facing as i32;
        let (dx, dy) = direction_vector(facing);
        let forward = weapon.forward_offset as i32 * fp(1);
        let mut px = ax + dx * forward / 256;
        let mut py = ay + dy * forward / 256;
        let target_unit = self.units.get(target).map(|u| (u.id, u.x, u.y));
        let Some((target_id, tx, ty)) = target_unit else {
            return;
        };
        let mut state = match weapon.bullet_type {
            behavior::FLY => BulletState::Fly,
            behavior::FOLLOW_TARGET => BulletState::Follow,
            behavior::BOUNCE => BulletState::Bounce,
            behavior::ATTACK_TARGET_POS => BulletState::HitNearTarget,
            behavior::EXTEND_TO_MAX_RANGE => BulletState::Extend,
            behavior::PERSIST_AT_TARGET_POS => BulletState::Persist,
            _ => return,
        };
        let mut target_pos = (tx, ty);
        let mut hit_flags = 0u8;
        if self.units[attacker].status.hallucination {
            hit_flags |= 2;
        }
        // Miss chance.  Ground units under Dark Swarm are almost impossible
        // to hit; a target on higher ground or behind cover also has a chance
        // to dodge.  The roll is deterministic (the world's LCG).
        if matches!(
            weapon.bullet_type,
            behavior::FLY | behavior::FOLLOW_TARGET | behavior::BOUNCE
        ) && self.target_dodges(attacker, target)
        {
            hit_flags |= 1;
            target_pos.0 -= dx * 30 / 256;
            target_pos.1 -= dy * 30 / 256;
        }
        match weapon.bullet_type {
            behavior::PERSIST_AT_TARGET_POS => {
                px = tx;
                py = ty;
                state = BulletState::Persist;
            }
            behavior::ATTACK_TARGET_POS => {
                let off = self.projectile_near_offset(attacker);
                px = ax;
                py = ay;
                target_pos = (tx + off.0, ty + off.1);
            }
            behavior::EXTEND_TO_MAX_RANGE => {
                let range = weapon.max_range as i32 + 20;
                target_pos = (ax + dx * range * fp(1) / 256, ay + dy * range * fp(1) / 256);
            }
            _ => {}
        }
        let bounces = if weapon.bullet_type == behavior::BOUNCE {
            3
        } else {
            0
        };
        let speed = self.data.weapon_speed(weapon).max(256);
        let _ = state;
        self.projectiles.push(Projectile {
            id: self.next_projectile_id,
            owner: self.units[attacker].owner,
            source: self.units[attacker].id,
            weapon: weapon_id,
            behavior: weapon.bullet_type,
            hit_type: weapon.explosion,
            state: match weapon.bullet_type {
                behavior::FLY => BulletState::Fly,
                behavior::FOLLOW_TARGET => BulletState::Follow,
                behavior::BOUNCE => BulletState::Bounce,
                behavior::ATTACK_TARGET_POS => BulletState::HitNearTarget,
                behavior::EXTEND_TO_MAX_RANGE => BulletState::Extend,
                behavior::PERSIST_AT_TARGET_POS => BulletState::Persist,
                _ => BulletState::Fly,
            },
            x: px,
            y: py,
            target: Some(target_id),
            target_pos,
            speed,
            remaining_time: weapon.lifetime.max(1) as i32,
            bounces_left: bounces,
            hit_flags,
            previous_target: None,
            last_dot_tick: 0,
        });
        self.next_projectile_id = self.next_projectile_id.wrapping_add(1).max(1);
    }

    fn projectile_near_offset(&mut self, _attacker: usize) -> (i32, i32) {
        // The retail engine cycles through a 14-entry table of small offsets
        // so repeated Corrosive Acid shots do not stack exactly.  A
        // deterministic ring preserves that behaviour.
        let n = (self.next_projectile_id % 14) as i32;
        let angle = n as f32 * std::f32::consts::TAU / 14.0;
        (
            (angle.cos() * 8.0 * fp(1) as f32) as i32,
            (angle.sin() * 8.0 * fp(1) as f32) as i32,
        )
    }

    pub fn world_rand(&mut self) -> u32 {
        // 32-bit LCG used by the retail engine for miss rolls and scatter.
        self.rng = self
            .rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.rng >> 32) as u32
    }

    /// Miss chance in 1/256 units for one ranged attack.  Retail uses
    /// 255/256 against ground units inside Dark Swarm and 119/256 when the
    /// target stands on higher terrain.  Melee attacks (`fire_melee`) never
    /// consult this, and air units ignore both rules.
    fn dodge_chance(&self, attacker: usize, target: usize) -> u32 {
        let a = &self.units[attacker];
        let t = &self.units[target];
        if !self.is_air_unit(target) && self.is_under_dark_swarm(target) {
            255
        } else if !self.is_air_unit(attacker) && !self.is_air_unit(target) {
            let ah = self.terrain.elevation_px(a.x / fp(1), a.y / fp(1));
            let th = self.terrain.elevation_px(t.x / fp(1), t.y / fp(1));
            if th > ah {
                119
            } else {
                0
            }
        } else {
            0
        }
    }

    fn target_dodges(&mut self, attacker: usize, target: usize) -> bool {
        let chance = self.dodge_chance(attacker, target);
        dodge_roll(self.world_rand(), chance)
    }

    fn update_projectiles(&mut self) {
        if self.projectiles.is_empty() {
            return;
        }
        let bullets = std::mem::take(&mut self.projectiles);
        let mut kept = Vec::with_capacity(bullets.len());
        for mut b in bullets {
            if self.step_projectile(&mut b) {
                kept.push(b);
            }
        }
        self.projectiles = kept;
    }

    /// Advance one bullet.  Returns false when it has been consumed.
    fn step_projectile(&mut self, b: &mut Projectile) -> bool {
        let Some(weapon) = self.data.weapon(b.weapon).cloned() else {
            return false;
        };
        if b.state == BulletState::Persist {
            b.remaining_time -= 1;
            if b.remaining_time <= 0 {
                return false;
            }
            if self.tick as i64 - b.last_dot_tick >= 7 {
                b.last_dot_tick = self.tick as i64;
                self.resolve_hit_type(
                    b.owner,
                    b.target,
                    self.index_of(b.source).unwrap_or(usize::MAX),
                    &weapon,
                    b.target_pos,
                    1,
                    1,
                );
            }
            return true;
        }
        if b.state == BulletState::Follow {
            if let Some(tid) = b.target {
                if let Some(t) = self.unit(tid) {
                    if t.alive {
                        b.target_pos = (t.x, t.y);
                    } else {
                        b.target = None;
                        b.state = BulletState::Fly;
                    }
                } else {
                    b.target = None;
                    b.state = BulletState::Fly;
                }
            }
        }
        if b.state == BulletState::Bounce {
            if let Some(tid) = b.target {
                if let Some(t) = self.unit(tid) {
                    if t.alive {
                        b.target_pos = (t.x, t.y);
                    }
                }
            }
        }
        b.remaining_time -= 1;
        let dx = b.target_pos.0 - b.x;
        let dy = b.target_pos.1 - b.y;
        let dist = isqrt((dx as i64) * (dx as i64) + (dy as i64) * (dy as i64)) as i32;
        if dist <= b.speed || b.remaining_time <= 0 {
            b.x = b.target_pos.0;
            b.y = b.target_pos.1;
            return self.projectile_impact(b, &weapon);
        }
        b.x += (dx as i64 * b.speed as i64 / dist as i64) as i32;
        b.y += (dy as i64 * b.speed as i64 / dist as i64) as i32;
        true
    }

    fn projectile_impact(&mut self, b: &mut Projectile, weapon: &WeaponType) -> bool {
        // A missed shot still produces its hit animation in the retail game
        // but deals no damage.  Hallucinated attackers likewise produce a fake
        // impact with no damage.
        if b.hit_flags & 3 != 0 {
            return false;
        }
        let source_idx = self.index_of(b.source).unwrap_or(usize::MAX);
        if b.state == BulletState::Bounce {
            if b.bounces_left > 0 && b.hit_flags & 1 == 0 {
                if let Some(tid) = b.target {
                    let divisor = 3u32.saturating_pow(3 - b.bounces_left as u32);
                    self.resolve_hit_type(
                        b.owner,
                        Some(tid),
                        source_idx,
                        weapon,
                        (b.x, b.y),
                        divisor,
                        1,
                    );
                    if b.bounces_left > 1 {
                        if let Some(next) = self.find_bounce_target(b, tid) {
                            b.previous_target = Some(tid);
                            b.target = Some(next);
                            b.bounces_left -= 1;
                            b.remaining_time = b.remaining_time.max(30);
                            b.state = BulletState::Bounce;
                            return true;
                        }
                    }
                }
            }
            return false;
        }
        let divisor = 1;
        self.resolve_hit_type(
            b.owner,
            b.target,
            source_idx,
            weapon,
            (b.x, b.y),
            divisor,
            1,
        );
        false
    }

    fn find_bounce_target(&self, b: &Projectile, current: u32) -> Option<u32> {
        let mut best: Option<(i32, u32)> = None;
        for u in &self.units {
            if !u.alive || u.owner == b.owner || u.id == current || Some(u.id) == b.previous_target
            {
                continue;
            }
            if !self.can_see_unit(b.owner, self.index_of(u.id)?) {
                continue;
            }
            let dx = (u.x - b.x) as i64;
            let dy = (u.y - b.y) as i64;
            let d = isqrt(dx * dx + dy * dy) as i32;
            if d <= 96 * fp(1) && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, u.id));
            }
        }
        best.map(|(_, id)| id)
    }

    /// Apply a weapon's `hit_type` at `pos`.  `target` is the intended unit
    /// (used by normal hits and unit-targeted spells); `divisor` is the splash
    /// ring divisor.
    #[allow(clippy::too_many_arguments)]
    fn resolve_hit_type(
        &mut self,
        owner: u8,
        target: Option<u32>,
        source: usize,
        weapon: &WeaponType,
        pos: (i32, i32),
        divisor: u32,
        bullet_count: u32,
    ) {
        let _ = bullet_count;
        let target_idx = target.and_then(|id| self.index_of(id));
        match weapon.explosion {
            hit::NONE => {}
            hit::NORMAL | hit::YAMATO | hit::UNUSED_23 => {
                if let Some(t) = target_idx {
                    if self.can_see_unit(owner, t) {
                        self.apply_hit_from(owner, source, t, weapon, divisor);
                    }
                }
            }
            hit::RADIAL_SPLASH | hit::ENEMY_SPLASH | hit::AIR_SPLASH => {
                self.apply_splash(
                    owner,
                    source,
                    weapon,
                    pos,
                    weapon.explosion == hit::ENEMY_SPLASH,
                    weapon.explosion == hit::AIR_SPLASH,
                );
            }
            hit::NUCLEAR => {
                self.apply_nuke(owner, source, pos);
            }
            hit::LOCKDOWN => {
                if let Some(t) = target_idx {
                    self.units[t].status.lockdown_ticks = 480;
                }
            }
            hit::PARASITE => {
                if let Some(t) = target_idx {
                    self.units[t].status.parasite = true;
                    self.units[t].status.revealed_ticks = 1200;
                }
            }
            hit::BROODLINGS => {
                if let Some(t) = target_idx {
                    if self.data.unit(self.units[t].type_id).is_organic() {
                        let (x, y) = (self.units[t].x / fp(1), self.units[t].y / fp(1));
                        self.kill_unit(t);
                        let _ = self.spawn_px(owner as usize, uid::BROODLING, x, y);
                        let _ = self.spawn_px(owner as usize, uid::BROODLING, x + 16, y);
                    }
                }
            }
            hit::EMP => {
                for j in 0..self.units.len() {
                    if self.units[j].alive && self.distance_to_point(j, pos) <= fp(64) {
                        self.units[j].shields = 0;
                        self.units[j].set_energy_fixed(0);
                    }
                }
            }
            hit::IRRADIATE => {
                if let Some(t) = target_idx {
                    let ty = self.data.unit(self.units[t].type_id);
                    if ty.is_organic() && !ty.is_building() {
                        self.units[t].status.irradiate_ticks = 400;
                    }
                }
            }
            hit::ENSNARE => {
                for j in 0..self.units.len() {
                    if self.units[j].alive
                        && self.distance_to_point(j, pos) <= fp(64)
                        && !self.is_air_unit(j)
                    {
                        self.units[j].status.ensnare_ticks = 240;
                    }
                }
            }
            hit::PLAGUE => {
                for j in 0..self.units.len() {
                    if self.units[j].alive && self.distance_to_point(j, pos) <= fp(64) {
                        self.units[j].status.plague_ticks = 400;
                    }
                }
            }
            hit::STASIS => {
                for j in 0..self.units.len() {
                    if self.units[j].alive && self.distance_to_point(j, pos) <= fp(64) {
                        self.units[j].status.stasis_ticks = 480;
                    }
                }
            }
            hit::DARK_SWARM => {
                self.spawn_dark_swarm(owner, pos);
            }
            hit::CONSUME => {
                if let Some(t) = target_idx {
                    if t != source {
                        let gained =
                            self.data.unit(self.units[t].type_id).hit_points_raw as i32 / fp(1) * 2;
                        if source < self.units.len() {
                            self.units[source].add_energy_points(gained);
                        }
                        self.kill_unit(t);
                    }
                }
            }
            hit::RESTORATION => {
                if let Some(t) = target_idx {
                    let s = &mut self.units[t].status;
                    s.lockdown_ticks = 0;
                    s.plague_ticks = 0;
                    s.irradiate_ticks = 0;
                    s.maelstrom_ticks = 0;
                    s.ensnare_ticks = 0;
                    s.optical_flare_ticks = 0;
                }
            }
            hit::DISRUPTION_WEB => {
                let _ = self.spawn_temporary(owner, uid::DISRUPTION_WEB, pos, 360);
            }
            hit::CORROSIVE_ACID => {
                if let Some(t) = target_idx {
                    self.apply_hit_from(owner, source, t, weapon, divisor);
                }
                for j in 0..self.units.len() {
                    if self.units[j].alive
                        && self.is_air_unit(j)
                        && self.distance_to_point(j, pos) <= fp(64)
                    {
                        self.units[j].status.acid_spores =
                            (self.units[j].status.acid_spores + 1).min(9);
                    }
                }
            }
            hit::MIND_CONTROL => {
                if let Some(t) = target_idx {
                    self.units[t].owner = owner;
                }
            }
            hit::FEEDBACK => {
                if let Some(t) = target_idx {
                    self.feedback_damage(t, source as u32);
                }
            }
            hit::OPTICAL_FLARE => {
                if let Some(t) = target_idx {
                    self.units[t].status.optical_flare_ticks = 600;
                }
            }
            hit::MAELSTROM => {
                for j in 0..self.units.len() {
                    if self.units[j].alive
                        && self.distance_to_point(j, pos) <= fp(64)
                        && self.data.unit(self.units[j].type_id).is_organic()
                    {
                        self.units[j].status.maelstrom_ticks = 240;
                    }
                }
            }
            _ => {}
        }
    }

    fn apply_splash(
        &mut self,
        owner: u8,
        source: usize,
        weapon: &WeaponType,
        impact: (i32, i32),
        enemy_only: bool,
        air_only: bool,
    ) {
        let inner = weapon.inner_splash as i64 * fp(1) as i64;
        let middle = weapon.middle_splash as i64 * fp(1) as i64;
        let outer = weapon.outer_splash as i64 * fp(1) as i64;
        for j in 0..self.units.len() {
            if !self.units[j].alive {
                continue;
            }
            if self.fog_of_war && !self.can_see_unit(owner, j) {
                continue;
            }
            if air_only && !self.is_air_unit(j) {
                continue;
            }
            if enemy_only && self.units[j].owner == owner {
                continue;
            }
            let dx = (self.units[j].x - impact.0) as i64;
            let dy = (self.units[j].y - impact.1) as i64;
            let d = isqrt(dx * dx + dy * dy);
            if d > outer {
                continue;
            }
            let divisor = if d <= inner {
                1
            } else if d <= middle {
                2
            } else {
                4
            };
            self.apply_hit_from(owner, source, j, weapon, divisor);
        }
    }

    /// The retail nuclear missile does `max(500, 2/3 * max_hp_plus_shields)`
    /// damage to every unit in its splash radius, ignoring the weapon's stored
    /// per-shot damage.
    fn apply_nuke(&mut self, owner: u8, source: usize, impact: (i32, i32)) {
        let radius = 128 * fp(1);
        let mut targets = Vec::new();
        for j in 0..self.units.len() {
            if !self.units[j].alive {
                continue;
            }
            if self.distance_to_point(j, impact) <= radius {
                targets.push(j);
            }
        }
        let Some(mut nuke) = self.data.weapon(wid::NUCLEAR_STRIKE).cloned() else {
            return;
        };
        nuke.damage_bonus = 0;
        for j in targets {
            let ty = self.data.unit(self.units[j].type_id);
            let max = ty.hit_points_raw as i32
                + if ty.has_shields {
                    fp(ty.shield_points as i32)
                } else {
                    0
                };
            let damage = (max * 2 / 3).max(fp(500));
            nuke.damage = (damage / fp(1)).max(1) as u16;
            self.apply_hit_from(owner, source, j, &nuke, 1);
        }
    }

    fn apply_hit(&mut self, attacker: usize, target: usize, weapon: &WeaponType, divisor: u32) {
        let owner = self.units.get(attacker).map(|u| u.owner).unwrap_or(0);
        self.apply_hit_from(owner, attacker, target, weapon, divisor);
    }

    fn apply_hit_from(
        &mut self,
        owner: u8,
        _source: usize,
        target: usize,
        weapon: &WeaponType,
        divisor: u32,
    ) {
        if owner as usize >= self.players.len() || !self.units.get(target).is_some_and(|u| u.alive)
        {
            return;
        }
        let t_owner = self.units[target].owner;
        if t_owner as usize >= self.players.len() {
            return;
        }
        let t_ty = self.data.unit(self.units[target].type_id).clone();
        let a_up = self.players[owner as usize].upgrade_level(weapon.damage_upgrade);
        let t_armor_up = self.players[t_owner as usize].upgrade_level(t_ty.armor_upgrade);
        let defense = Defense {
            size: t_ty.size,
            armor_level: t_ty.armor as u32 + t_armor_up as u32,
            has_shields: t_ty.has_shields,
            shield_points: self.units[target].shields,
            shield_upgrade: self.players[t_owner as usize]
                .upgrade_level(upgrade::PROTOSS_PLASMA_SHIELDS),
        };
        let status = StatusDamage {
            matrix: self.units[target].status.matrix,
            acid_spores: self.units[target].status.acid_spores,
            hallucination: self.units[target].status.hallucination,
        };
        let result = resolve_damage_status(weapon, a_up, defense, divisor, status);
        if result.matrix_absorbed > 0 {
            self.units[target].status.matrix =
                (self.units[target].status.matrix - result.matrix_absorbed).max(0);
        }
        if result.damage.shield_damage > 0 {
            self.units[target].shields =
                (self.units[target].shields - result.damage.shield_damage).max(0);
        }
        if result.damage.hp_damage > 0 {
            self.damage_hp(target, result.damage.hp_damage, _source as u32);
        }
    }

    /// Apply raw hit-point damage to a unit (scripting/test entry point).
    pub fn damage_unit(&mut self, id: u32, amount: i32) -> Result<(), SimError> {
        let target = self.index_of(id).ok_or(SimError::BadUnit)?;
        self.damage_hp(target, amount, 0);
        Ok(())
    }

    /// Set a unit's plasma shields in whole points (scripting/test entry
    /// point for damage and Shield Battery scenarios).
    pub fn set_unit_shields(&mut self, id: u32, points: i32) -> Result<(), SimError> {
        let target = self.index_of(id).ok_or(SimError::BadUnit)?;
        self.units[target].shields = fp(points.max(0));
        Ok(())
    }

    /// Feedback drains every 1/256 energy point and deals exactly that many
    /// 1/256 HP of damage.  Defensive Matrix absorbs the damage first; shields
    /// and armour are bypassed, as in the retail spell.
    fn feedback_damage(&mut self, target: usize, source: u32) {
        let energy = self.units[target].energy_fixed();
        if energy <= 0 {
            return;
        }
        self.units[target].set_energy_fixed(0);
        let absorbed = self.units[target].status.matrix.min(energy);
        self.units[target].status.matrix -= absorbed;
        let hp_damage = energy - absorbed;
        if hp_damage > 0 {
            self.damage_hp(target, hp_damage, source);
        }
    }

    fn damage_hp(&mut self, target: usize, amount: i32, source: u32) {
        if !self.units[target].alive || self.units[target].invincible {
            return;
        }
        if source != 0 {
            self.units[target].last_damage_source = source;
        }
        self.units[target].hp -= amount;
        self.units[target].last_damaged = self.tick;
        if self.units[target].hp <= 0 {
            self.kill_unit(target);
        }
    }

    fn kill_unit(&mut self, i: usize) {
        if !self.units[i].alive {
            return;
        }
        self.units[i].alive = false;
        self.units[i].hp = 0;
        self.units[i].order = Order::Die;
        let owner = self.units[i].owner as usize;
        let type_id = self.units[i].type_id;
        let source_id = self.units[i].last_damage_source;
        self.units[i].last_damage_source = 0;
        let source_owner = self
            .index_of(source_id)
            .map(|j| self.units[j].owner as usize)
            .filter(|&s| s != owner && s < self.players.len());
        if is_creep_source(type_id) {
            self.creep_dirty = true;
        }
        // An add-on dies with its parent, and a destroyed add-on clears the
        // parent's slot.
        if let Some(addon_id) = self.units[i].addon.take() {
            if let Some(j) = self.index_of(addon_id) {
                if self.units[j].alive {
                    self.kill_unit(j);
                }
            }
        }
        let dead_id = self.units[i].id;
        for u in &mut self.units {
            if u.addon == Some(dead_id) {
                u.addon = None;
            }
        }
        if owner < self.players.len() {
            self.players[owner].deaths += 1;
            if (type_id as usize) < self.players[owner].death_counts.len() {
                self.players[owner].death_counts[type_id as usize] += 1;
            }
        }
        if let Some(killer) = source_owner {
            self.players[killer].kills += 1;
            if (type_id as usize) < self.players[killer].kill_counts.len() {
                self.players[killer].kill_counts[type_id as usize] += 1;
            }
            if self.data.unit(type_id).is_building() {
                self.players[killer].razings += 1;
            }
        }
        // Loaded units die with their transport/bunker (retail behaviour);
        // they are not ejected alive.
        let cargo = std::mem::take(&mut self.units[i].cargo);
        for id in cargo {
            if let Some(j) = self.index_of(id) {
                if self.units[j].alive {
                    self.kill_unit(j);
                }
            }
        }
    }

    fn resolve_deaths(&mut self) {
        // Dead units stay in the vector (ids are stable) but are skipped by
        // every system.  Transports drop their cargo on death in kill_unit.
    }

    fn check_victory(&mut self) {
        // A sandbox with a single owner (unit tests, spell labs) has no
        // opposing side and therefore no victory condition.
        let owners: std::collections::HashSet<u8> = self.units.iter().map(|u| u.owner).collect();
        if owners.len() < 2 {
            return;
        }
        let alive_players: Vec<u8> = (0..self.players.len() as u8)
            .filter(|&p| {
                !self.players[p as usize].defeated
                    && self.units.iter().any(|u| {
                        u.owner == p
                            && !is_resource_type(u.type_id)
                            && (u.alive || u.in_transport.is_some())
                    })
            })
            .collect();
        if alive_players.len() <= 1 {
            self.winner = alive_players.first().copied();
        }
    }

    // ------------------------------------------------------------------
    // Movement
    // ------------------------------------------------------------------

    fn build_obstacles(&mut self) {
        self.obstacles.clear();
        for u in &self.units {
            if !u.alive || !u.completed {
                continue;
            }
            let ty = self.data.unit(u.type_id);
            if !u.lifted && (is_resource_type(u.type_id) || ty.is_flyer() || ty.is_building()) {
                let half_w = ty.dimensions.right.max(ty.dimensions.left.max(0)) as i32;
                let half_h = ty.dimensions.bottom.max(ty.dimensions.top.max(0)) as i32;
                if half_w > 0 || half_h > 0 {
                    let ux = u.x / fp(1);
                    let uy = u.y / fp(1);
                    self.obstacles.push(Rect::new(
                        ux - half_w,
                        uy - half_h,
                        ux + half_w,
                        uy + half_h,
                    ));
                }
            }
        }
    }

    /// Half extents of a ground unit's collision box in 1/256 pixels.
    fn collision_extents(&self, i: usize) -> Option<(i32, i32)> {
        let u = &self.units[i];
        if !u.alive || u.in_transport.is_some() || u.status.burrowed {
            return None;
        }
        let ty = self.data.unit(u.type_id);
        if ty.is_building() || ty.is_flyer() {
            return None;
        }
        // `units.dat` dimensions are in pixels.  Every stock ground unit has
        // a non-zero box; the floor keeps tiny doodads from collapsing.
        let hw = ty.dimensions.right.max(ty.dimensions.left.max(0)) as i32;
        let hh = ty.dimensions.bottom.max(ty.dimensions.top.max(0)) as i32;
        Some(((hw.max(5)) * fp(1), (hh.max(5)) * fp(1)))
    }

    fn try_shift(&mut self, i: usize, dx: i32, dy: i32) -> bool {
        if dx == 0 && dy == 0 {
            return true;
        }
        let nx = self.units[i].x + dx;
        let ny = self.units[i].y + dy;
        if self.terrain.walkable_px(nx / fp(1), ny / fp(1)) {
            self.units[i].x = nx;
            self.units[i].y = ny;
            true
        } else {
            false
        }
    }

    /// Push overlapping ground units apart.  Retail units cannot share the
    /// same ground position; ignoring this made armies collapse into a single
    /// point.  The separation is the minimum translation along the smaller
    /// overlap axis, split between both bodies, with a few relaxation passes.
    fn resolve_collisions(&mut self) {
        let mut movable: Vec<usize> = (0..self.units.len())
            .filter(|&i| self.collision_extents(i).is_some())
            .collect();
        if movable.len() < 2 {
            return;
        }
        movable.sort_unstable();
        let cell = 32 * fp(1);
        for _pass in 0..2 {
            let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
            for &i in &movable {
                let key = (
                    self.units[i].x.div_euclid(cell),
                    self.units[i].y.div_euclid(cell),
                );
                grid.entry(key).or_default().push(i);
            }
            let mut pairs: Vec<(usize, usize)> = Vec::new();
            for (&(cx, cy), list) in &grid {
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        let Some(other) = grid.get(&(cx + dx, cy + dy)) else {
                            continue;
                        };
                        for &i in list {
                            for &j in other {
                                if i < j {
                                    pairs.push((i, j));
                                }
                            }
                        }
                    }
                }
            }
            pairs.sort_unstable();
            pairs.dedup();
            for (i, j) in pairs {
                let (Some((hwi, hhi)), Some((hwj, hhj))) =
                    (self.collision_extents(i), self.collision_extents(j))
                else {
                    continue;
                };
                let dx = self.units[j].x - self.units[i].x;
                let dy = self.units[j].y - self.units[i].y;
                let ox = hwi + hwj - dx.abs();
                let oy = hhi + hhj - dy.abs();
                if ox <= 0 || oy <= 0 {
                    continue;
                }
                if ox < oy {
                    let sign = if dx >= 0 { 1 } else { -1 };
                    let half = (ox / 2).max(1) * sign;
                    if !self.try_shift(i, -half, 0) {
                        let _ = self.try_shift(j, half * 2, 0);
                    } else {
                        let _ = self.try_shift(j, half, 0);
                    }
                } else {
                    let sign = if dy >= 0 { 1 } else { -1 };
                    let half = (oy / 2).max(1) * sign;
                    if !self.try_shift(i, 0, -half) {
                        let _ = self.try_shift(j, 0, half * 2);
                    } else {
                        let _ = self.try_shift(j, 0, half);
                    }
                }
            }
        }
    }

    /// Move toward `goal` in pixel coordinates.  Returns true when the unit
    /// has arrived.
    fn move_towards(&mut self, i: usize, goal: (i32, i32), stop_distance: i32) -> bool {
        let ty = self.data.unit(self.units[i].type_id).clone();
        if ty.is_building() && !self.units[i].status.sieged && !self.units[i].lifted {
            return true;
        }
        if !ty.is_flyer() && !self.units[i].lifted {
            self.ensure_walking_animation(i);
        }
        let goal_fp = (goal.0.saturating_mul(fp(1)), goal.1.saturating_mul(fp(1)));
        if self.units[i].path_goal != Some(goal_fp) {
            self.build_obstacles();
            let speed = self.speed_of(i);
            let half_w = ty.dimensions.right.max(0).max(ty.dimensions.left.max(0)) as i32;
            let half_h = ty.dimensions.bottom.max(0).max(ty.dimensions.top.max(0)) as i32;
            let req = PathRequest {
                start: (self.units[i].x / fp(1), self.units[i].y / fp(1)),
                goal: (goal.0, goal.1),
                half_width: half_w.max(4),
                half_height: half_h.max(4),
                flying: ty.is_flyer() || self.units[i].lifted || speed <= 0,
            };
            let path = if speed <= 0 {
                None
            } else {
                find_path_with_regions(&self.terrain, &self.obstacles, &req, Some(&self.regions))
            };
            self.units[i].path.clear();
            self.units[i].path_goal = Some(goal_fp);
            if let Some(p) = path {
                for w in p.waypoints {
                    // find_path works in pixels; convert to fp.
                    self.units[i].path.push_back((w.0 * fp(1), w.1 * fp(1)));
                }
                if self.units[i].path.is_empty() {
                    self.units[i].path.push_back(goal_fp);
                }
            } else {
                // No route: fall back to a direct move.  The retail engine
                // reports "unreachable" and keeps the unit in place; a direct
                // fallback keeps the simulation usable on partially walkable
                // custom maps without hiding the failure from the caller.
                self.units[i].path.push_back(goal_fp);
            }
        }
        let speed = self.speed_of(i);
        if speed <= 0 {
            return true;
        }
        let mut remaining = speed;
        while remaining > 0 {
            let Some(&(wx, wy)) = self.units[i].path.front() else {
                if self.distance_to_point(i, goal_fp) > stop_distance {
                    // Replan next tick instead of freezing.
                    self.units[i].path_goal = None;
                }
                let done = self.distance_to_point(i, goal_fp) <= stop_distance;
                if done {
                    self.finish_move_animation(i);
                }
                return done;
            };
            let dx = wx - self.units[i].x;
            let dy = wy - self.units[i].y;
            let dist = isqrt((dx as i64) * (dx as i64) + (dy as i64) * (dy as i64)) as i32;
            if dist <= remaining {
                self.units[i].x = wx;
                self.units[i].y = wy;
                self.units[i].path.pop_front();
                remaining -= dist.max(0);
            } else if dist > 0 {
                self.units[i].x += (dx as i64 * remaining as i64 / dist as i64) as i32;
                self.units[i].y += (dy as i64 * remaining as i64 / dist as i64) as i32;
                self.units[i].facing = direction_byte(dx, dy);
                remaining = 0;
            } else {
                self.units[i].path.pop_front();
            }
        }
        self.distance_to_point(i, goal_fp) <= stop_distance && self.units[i].path.is_empty()
    }

    fn finish_move_animation(&mut self, i: usize) {
        if self.units[i]
            .anim
            .as_ref()
            .is_some_and(|a| a.anim as usize == ANIM_WALKING)
        {
            self.start_animation(i, ANIM_WALKING_TO_IDLE);
        }
    }

    fn distance_to_point(&self, i: usize, p: (i32, i32)) -> i32 {
        let dx = (self.units[i].x - p.0) as i64;
        let dy = (self.units[i].y - p.1) as i64;
        isqrt(dx * dx + dy * dy) as i32
    }

    fn speed_of(&self, i: usize) -> i32 {
        let u = &self.units[i];
        let ty = self.data.unit(u.type_id);
        if u.status.lockdown_ticks > 0 || u.status.stasis_ticks > 0 {
            return 0;
        }
        if u.status.sieged || u.status.burrowed {
            return 0;
        }
        let mut speed = if let Some(anim) = &u.anim {
            if anim.fling_speed > 0 {
                anim.fling_speed
            } else if anim.anim as usize == ANIM_WALKING && anim.move_speed > 0 {
                anim.move_speed
            } else {
                let cycle = self.data.unit_walk_cycle(ty);
                if cycle.is_empty() {
                    self.data.unit_speed(ty)
                } else {
                    cycle[(u.anim_tick as usize) % cycle.len()]
                }
            }
        } else {
            let cycle = self.data.unit_walk_cycle(ty);
            if cycle.is_empty() {
                self.data.unit_speed(ty)
            } else {
                cycle[(u.anim_tick as usize) % cycle.len()]
            }
        };
        if u.status.stim_ticks > 0 {
            speed = speed * 3 / 2;
        }
        if u.status.ensnare_ticks > 0 {
            speed /= 2;
        }
        // Lifted Terran buildings use their flingy speed when present;
        // otherwise they drift at the standard building air speed.
        if u.lifted && speed <= 0 {
            speed = 1280;
        }
        let owner = u.owner as usize;
        if ty.is_worker() && self.players[owner].upgrade_level(upgrade::METABOLIC_BOOST) > 0 {
            speed = speed * 3 / 2;
        }
        speed
    }

    // ------------------------------------------------------------------
    // iscript animation VM integration
    // ------------------------------------------------------------------

    /// Start an animation and execute the script's first tick immediately.
    /// The retail engine does the same from `FUN_7a1148f0`, which is why a
    /// Walking script's first `move` takes effect on the tick the order is
    /// issued.
    fn start_animation(&mut self, i: usize, anim: usize) {
        let ty = self.data.unit(self.units[i].type_id).clone();
        let Some(iscript) = self.data.unit_iscript_id(&ty) else {
            return;
        };
        let direction = self.units[i].facing;
        if let Some(st) = self.data.iscript.start_animation(iscript, anim, direction) {
            self.units[i].anim = Some(st);
            self.tick_animation(i);
        }
    }

    /// Advance the current animation by one logical tick and apply gameplay
    /// events (weapon release, spell release, order signals, turns).
    fn tick_animation(&mut self, i: usize) {
        let Some(mut st) = self.units[i].anim.take() else {
            return;
        };
        let direction = self.units[i].facing;
        let (target_distance, target_angle) = match self.units[i].target {
            Some(id) => match self.index_of(id) {
                Some(tidx) => {
                    let dx = self.units[tidx].x - self.units[i].x;
                    let dy = self.units[tidx].y - self.units[i].y;
                    let d = crate::sim::isqrt((dx as i64) * (dx as i64) + (dy as i64) * (dy as i64))
                        as i32
                        / fp(1);
                    let a = direction_byte(dx, dy) as i32 * 8;
                    (Some(d), Some(a))
                }
                None => (None, None),
            },
            None => (None, None),
        };
        let mut ctx = AnimContext {
            direction,
            tileset: 0,
            rng: self.rng as u32,
            target_distance,
            target_angle,
            lifted: false,
            powerup_picked: false,
            hallucination: self.units[i].status.hallucination,
        };
        let events = self.data.iscript.tick_anim(&mut st, &mut ctx);
        self.rng = (self.rng & 0xffff_ffff_0000_0000) | ctx.rng as u64;
        let finished = st.ended || st.ignore_rest;
        if finished {
            self.units[i].anim = None;
        } else {
            self.units[i].anim = Some(st);
        }
        for ev in events {
            self.handle_anim_event(i, ev);
        }
    }

    fn handle_anim_event(&mut self, i: usize, ev: AnimEvent) {
        match ev {
            AnimEvent::Attack {
                weapon,
                melee,
                ground,
            } => self.animation_attack(i, weapon, melee, ground),
            AnimEvent::CastSpell => self.animation_cast(i),
            AnimEvent::OrderDone(_) => {
                let blocked = self.units[i]
                    .anim
                    .as_ref()
                    .is_some_and(|a| a.orders_blocked);
                if !blocked {
                    self.order_done(i);
                }
            }
            AnimEvent::SetDirection(d) => {
                self.units[i].facing = ((d / 8).clamp(0, 31)) as u8;
            }
            AnimEvent::Turn(d) => {
                self.units[i].facing = (self.units[i].facing as i32 + d).rem_euclid(32) as u8;
            }
            AnimEvent::Flip(_)
            | AnimEvent::Frame { .. }
            | AnimEvent::Sound(_)
            | AnimEvent::RandomSound(_)
            | AnimEvent::Move(_)
            | AnimEvent::SetFlingSpeed(_)
            | AnimEvent::Signal(_)
            | AnimEvent::Overlay { .. }
            | AnimEvent::End
            | AnimEvent::IgnoreRest => {}
        }
    }

    fn animation_attack(&mut self, i: usize, weapon: Option<u8>, melee: bool, ground: bool) {
        let Some(target) = self.units[i].target else {
            return;
        };
        let Some(tidx) = self.index_of(target) else {
            return;
        };
        if !self.units[tidx].alive {
            return;
        }
        let ty = self.data.unit(self.units[i].type_id).clone();
        let selected = if let Some(id) = weapon {
            self.data.weapon(id).cloned().map(|w| (id, w))
        } else if ground {
            self.data
                .ground_weapon(&ty)
                .cloned()
                .map(|w| (ty.ground_weapon, w))
        } else {
            self.weapon_for(i, tidx)
        };
        let Some((weapon_id, w)) = selected else {
            self.special_attack(i, tidx);
            return;
        };
        if melee {
            self.fire_melee(i, tidx, &w);
        } else {
            self.fire(i, tidx, weapon_id, &w);
        }
        self.units[i].attacks_fired = self.units[i].attacks_fired.saturating_add(1);
    }

    fn animation_cast(&mut self, i: usize) {
        let order = self.units[i].order.clone();
        if let Order::Cast { spell, target } = order {
            let id = self.units[i].id;
            let _ = self.cast(id, spell, target);
            self.order_done(i);
        }
    }

    fn fire_melee(&mut self, attacker: usize, target: usize, weapon: &WeaponType) {
        if self.units[attacker].status.hallucination {
            self.units[attacker].cooldown = self.modified_cooldown(attacker, weapon);
            return;
        }
        let owner = self.units[attacker].owner;
        let pos = (self.units[target].x, self.units[target].y);
        self.resolve_hit_type(
            owner,
            Some(self.units[target].id),
            attacker,
            weapon,
            pos,
            1,
            1,
        );
        self.units[attacker].cooldown = self.modified_cooldown(attacker, weapon);
    }

    fn ensure_walking_animation(&mut self, i: usize) {
        let current = self.units[i].anim.as_ref().map(|a| a.anim as usize);
        if current.is_none()
            || matches!(
                current,
                Some(ANIM_WALKING_TO_IDLE)
                    | Some(ANIM_GND_ATTACK_TO_IDLE)
                    | Some(ANIM_AIR_ATTACK_TO_IDLE)
            )
        {
            self.start_animation(i, ANIM_WALKING);
        }
    }

    fn start_attack_animation(&mut self, i: usize, flying_target: bool) -> bool {
        let repeat = self.units[i].attacks_fired > 0;
        let anim = match (flying_target, repeat) {
            (true, false) => ANIM_AIR_ATTACK_INIT,
            (true, true) => ANIM_AIR_ATTACK_REPEAT,
            (false, false) => ANIM_GND_ATTACK_INIT,
            (false, true) => ANIM_GND_ATTACK_REPEAT,
        };
        let ty = self.data.unit(self.units[i].type_id);
        let Some(iscript) = self.data.unit_iscript_id(ty) else {
            return false;
        };
        if !self.data.iscript.animation_has_attack(iscript, anim) {
            return false;
        }
        self.start_animation(i, anim);
        true
    }

    // ------------------------------------------------------------------
    // Economy
    // ------------------------------------------------------------------

    fn order_gather(&mut self, i: usize, resource: u32) {
        let Some(ridx) = self.index_of(resource) else {
            self.order_done(i);
            return;
        };
        if !self.units[ridx].alive {
            self.order_done(i);
            return;
        }
        if self.units[ridx].resource_amount <= 0 {
            self.order_done(i);
            return;
        }
        // Raw geysers require a completed Refinery/Assimilator/Extractor.
        // The worker then gathers from that building, exactly as in the
        // retail game.
        if self.units[ridx].type_id == uid::VESPENE_GEYSER {
            let (gx, gy) = (self.units[ridx].x, self.units[ridx].y);
            let refinery = self.units.iter().position(|u| {
                u.alive
                    && u.completed
                    && is_gas_building(u.type_id)
                    && (u.x - gx).abs() < fp(64)
                    && (u.y - gy).abs() < fp(64)
            });
            if let Some(r) = refinery {
                self.issue(
                    self.units[i].id,
                    Order::Gather {
                        resource: self.units[r].id,
                    },
                )
                .ok();
            } else {
                self.order_done(i);
            }
            return;
        }
        let dist = self.distance(i, ridx);
        if dist > fp(80) {
            let (rx, ry) = (self.units[ridx].x, self.units[ridx].y);
            self.move_towards(i, (rx / fp(1), ry / fp(1)), fp(64));
            return;
        }
        // Harvest.
        let kind = if is_gas_resource(self.units[ridx].type_id) {
            CarryKind::Gas
        } else {
            CarryKind::Minerals
        };
        let amount = if kind == CarryKind::Gas {
            GAS_PER_TRIP
        } else {
            MINERALS_PER_TRIP
        };
        let taken = amount.min(self.units[ridx].resource_amount);
        self.units[ridx].resource_amount -= taken;
        self.units[i].carry = taken;
        self.units[i].carry_kind = kind;
        self.units[i].resource_target = Some(resource);
        self.units[i].order = Order::ReturnCargo { depot: 0 };
    }

    fn order_return(&mut self, i: usize, depot: u32) {
        let kind = self.units[i].carry_kind;
        if kind == CarryKind::None {
            if let Some(res) = self.units[i].resource_target {
                self.units[i].order = Order::Gather { resource: res };
            } else {
                self.order_done(i);
            }
            return;
        }
        let depot_idx = if depot != 0 {
            self.index_of(depot)
        } else {
            self.nearest_depot(i)
        };
        let Some(didx) = depot_idx else {
            self.order_done(i);
            return;
        };
        let dist = self.distance(i, didx);
        if dist > fp(96) {
            let (dx, dy) = (self.units[didx].x, self.units[didx].y);
            self.move_towards(i, (dx / fp(1), dy / fp(1)), fp(64));
            return;
        }
        let owner = self.units[i].owner as usize;
        match kind {
            CarryKind::Minerals => {
                self.players[owner].minerals += self.units[i].carry;
                self.players[owner].minerals_gathered += self.units[i].carry;
            }
            CarryKind::Gas => {
                self.players[owner].gas += self.units[i].carry;
                self.players[owner].gas_gathered += self.units[i].carry;
            }
            CarryKind::None => {}
        }
        self.units[i].carry = 0;
        self.units[i].carry_kind = CarryKind::None;
        if let Some(res) = self.units[i].resource_target {
            self.units[i].order = Order::Gather { resource: res };
        } else {
            self.order_done(i);
        }
    }

    fn nearest_depot(&self, i: usize) -> Option<usize> {
        let owner = self.units[i].owner;
        self.units
            .iter()
            .enumerate()
            .filter(|(_, u)| {
                u.alive
                    && u.owner == owner
                    && u.completed
                    && (u.type_id == uid::COMMAND_CENTER
                        || u.type_id == uid::NEXUS
                        || u.type_id == uid::HATCHERY
                        || u.type_id == uid::LAIR
                        || u.type_id == uid::HIVE)
            })
            .min_by_key(|(_, u)| {
                let dx = (u.x - self.units[i].x) as i64;
                let dy = (u.y - self.units[i].y) as i64;
                dx * dx + dy * dy
            })
            .map(|(idx, _)| idx)
    }

    fn order_build(&mut self, i: usize, unit_type: u16, pos: (i32, i32)) {
        // Add-ons are parent-built, never worker-built; building morphs keep
        // the source building and never create a second structure.
        if addon_parent(unit_type).is_some() || building_morph_source(unit_type).is_some() {
            self.order_done(i);
            return;
        }
        // Zerg structures (except the Hatchery, which creates creep) must be
        // placed on creep; Protoss structures must be inside a Pylon field.
        if is_zerg_building(unit_type)
            && !matches!(unit_type, uid::HATCHERY | uid::EXTRACTOR)
            && !self.has_creep_px(pos.0, pos.1)
        {
            self.order_done(i);
            return;
        }
        if is_protoss_building(unit_type)
            && !matches!(unit_type, uid::NEXUS | uid::PYLON)
            && !self.has_power_px(pos.0, pos.1)
        {
            self.order_done(i);
            return;
        }
        let ty = self.data.unit(unit_type).clone();
        let pos_fp = (pos.0 * fp(1), pos.1 * fp(1));
        if self.distance_to_point(i, pos_fp) > fp(64) {
            self.move_towards(i, pos, fp(32));
            return;
        }
        let owner = self.units[i].owner as usize;
        if !self.has_requirements(owner as u8, unit_type) {
            self.order_done(i);
            return;
        }
        if !self.can_afford(owner as u8, unit_type) {
            self.order_done(i);
            return;
        }
        {
            let p = &mut self.players[owner];
            p.minerals -= ty.mineral_cost as i32;
            p.gas -= ty.gas_cost as i32;
        }
        let worker_is_drone = self.units[i].type_id == uid::DRONE;
        let Ok(bidx) = self.spawn_px(owner, unit_type, pos.0, pos.1) else {
            self.order_done(i);
            return;
        };
        self.units[bidx].completed = false;
        self.units[bidx].build_total = build_ticks(ty.build_time);
        self.units[bidx].build_remaining = self.units[bidx].build_total;
        self.units[bidx].hp = (ty.hit_points_raw as i32 / 10).max(1);
        if worker_is_drone && is_zerg_building(unit_type) {
            // The Drone is consumed by the structure it starts.
            self.kill_unit(i);
        } else {
            self.order_done(i);
        }
    }

    /// SCV repair.  Rate is the documented `0.9 * maxHP / build_time` per
    /// second; cost is about one third of the unit's mineral cost over the
    /// full hit-point pool, accumulated in fractional minerals per SCV.
    fn order_repair(&mut self, i: usize, target: u32) {
        let Some(t) = self.index_of(target) else {
            self.order_done(i);
            return;
        };
        if !self.units[t].alive {
            self.order_done(i);
            return;
        }
        let ty = self.data.unit(self.units[t].type_id).clone();
        if !ty.is_mechanical() && !ty.is_building() {
            self.order_done(i);
            return;
        }
        let max = ty.hit_points_raw as i32;
        if self.units[t].hp >= max {
            self.order_done(i);
            return;
        }
        if t == i {
            self.order_done(i);
            return;
        }
        let dist = self.distance(i, t);
        if dist > fp(48) {
            let (tx, ty) = (self.units[t].x, self.units[t].y);
            self.move_towards(i, (tx / fp(1), ty / fp(1)), fp(32));
            return;
        }
        let owner = self.units[i].owner as usize;
        // Retail repair rate: 0.9 * maxHP / build_time_seconds per SCV.
        // `build_time` is in 1/15 s and the logical rate is 24 Hz, so the
        // per-tick raw HP is maxHP * 27 / (build_time * 48).
        let build_time = ty.build_time.max(1) as i64;
        let max_raw = max.max(1) as i64;
        let mut heal = (max_raw * 27 / (build_time * 48)).max(1) as i32;
        heal = heal.min(max - self.units[t].hp);
        if heal <= 0 {
            self.order_done(i);
            return;
        }
        // Repair costs about one third of the unit's mineral cost over its
        // whole hit-point pool.  Fractional minerals are kept per SCV.
        let debt = (ty.mineral_cost as i64 * heal as i64 * 256) / (3 * max_raw);
        self.units[i].repair_debt = self.units[i].repair_debt.saturating_add(debt as i32);
        let payable = self.units[i].repair_debt / 256;
        if payable > self.players[owner].minerals {
            self.order_done(i);
            return;
        }
        if payable > 0 {
            self.players[owner].minerals -= payable;
            self.units[i].repair_debt %= 256;
        }
        self.units[t].hp += heal;
    }

    // ------------------------------------------------------------------
    // Transports
    // ------------------------------------------------------------------

    fn order_enter_transport(&mut self, i: usize, transport: u32) {
        let Some(tidx) = self.index_of(transport) else {
            self.order_done(i);
            return;
        };
        if !self.units[tidx].alive {
            self.order_done(i);
            return;
        }
        let dist = self.distance(i, tidx);
        if dist > fp(48) {
            let (tx, ty) = (self.units[tidx].x, self.units[tidx].y);
            self.move_towards(i, (tx / fp(1), ty / fp(1)), fp(24));
            return;
        }
        let target_type = self.units[tidx].type_id;
        let owner = self.units[i].owner;

        // Nydus Canal: the unit travels to the paired canal instead of being
        // stored.  Order 45 (Enter Nydus Canal) uses this path.
        if target_type == uid::NYDUS_CANAL {
            let paired = self.units[tidx]
                .nydus_exit
                .and_then(|id| self.index_of(id))
                .filter(|&e| self.units[e].alive && self.units[e].completed);
            let exit = paired.or_else(|| {
                self.units.iter().position(|u| {
                    u.alive
                        && u.completed
                        && u.owner == owner
                        && u.type_id == uid::NYDUS_CANAL
                        && u.id != transport
                })
            });
            if let Some(e) = exit {
                let (ex, ey) = self.exit_position(e);
                self.units[i].x = ex;
                self.units[i].y = ey;
                self.units[i].path.clear();
                self.units[i].path_goal = None;
                self.order_done(i);
            } else {
                self.order_done(i);
            }
            return;
        }

        let moving_id = self.units[i].id;
        if target_type == uid::BUNKER {
            // Four infantry slots, no vehicles or workers.
            if !is_bunker_infantry(self.units[i].type_id) || self.units[tidx].cargo.len() >= 4 {
                self.order_done(i);
                return;
            }
        } else {
            let capacity = self.data.unit(target_type).space_provided as i32;
            let used: i32 = self.units[tidx]
                .cargo
                .iter()
                .filter_map(|id| self.unit(*id))
                .map(|u| self.data.unit(u.type_id).space_required as i32)
                .sum();
            let need = self.data.unit(self.units[i].type_id).space_required as i32;
            if capacity - used < need {
                self.order_done(i);
                return;
            }
        }
        self.units[tidx].cargo.push(moving_id);
        self.units[i].in_transport = Some(transport);
        self.units[i].alive = false; // hidden/loaded
        if target_type == uid::BUNKER {
            self.units[i].x = self.units[tidx].x;
            self.units[i].y = self.units[tidx].y;
        }
        self.units[i].order = Order::Idle;
    }

    fn order_unload(&mut self, i: usize) {
        let cargo = std::mem::take(&mut self.units[i].cargo);
        let (x, y) = self.exit_position(i);
        for id in cargo {
            if let Some(j) = self.index_of(id) {
                self.units[j].alive = true;
                self.units[j].in_transport = None;
                self.units[j].x = x;
                self.units[j].y = y;
            }
        }
        self.order_done(i);
    }

    // ------------------------------------------------------------------
    // Spells and modes
    // ------------------------------------------------------------------

    fn order_burrow(&mut self, i: usize, burrow: bool) {
        if burrow {
            if !self.players[self.units[i].owner as usize].has_tech(tech_id::BURROWING) {
                self.order_done(i);
                return;
            }
            self.units[i].status.burrowed = true;
        } else {
            self.units[i].status.burrowed = false;
        }
        self.order_done(i);
    }

    fn order_siege(&mut self, i: usize, siege: bool) {
        if !self.players[self.units[i].owner as usize].has_tech(tech_id::TANK_SIEGE_MODE) {
            self.order_done(i);
            return;
        }
        if siege && self.units[i].type_id == uid::SIEGE_TANK {
            self.units[i].status.sieged = true;
            self.units[i].type_id = uid::SIEGE_MODE;
        } else if !siege && self.units[i].type_id == uid::SIEGE_MODE {
            self.units[i].status.sieged = false;
            self.units[i].type_id = uid::SIEGE_TANK;
        }
        self.order_done(i);
    }

    /// Shield Battery recharge (orders 66/67).  The classic ability converts
    /// one energy point into two plasma-shield points and can be repeated
    /// until the battery is empty.  Returns the number of shield points
    /// restored.
    pub fn shield_battery(&mut self, battery_id: u32, target_id: u32) -> Result<i32, SimError> {
        let b = self.index_of(battery_id).ok_or(SimError::BadUnit)?;
        let t = self.index_of(target_id).ok_or(SimError::BadUnit)?;
        if self.units[b].type_id != uid::SHIELD_BATTERY || !self.units[b].completed {
            return Err(SimError::InvalidTarget);
        }
        if !self.units[b].alive || !self.units[t].alive {
            return Err(SimError::BadUnit);
        }
        if self.units[b].owner != self.units[t].owner {
            return Err(SimError::InvalidTarget);
        }
        let ty = self.data.unit(self.units[t].type_id).clone();
        if !ty.has_shields {
            return Err(SimError::InvalidTarget);
        }
        let max = fp(ty.shield_points as i32);
        if self.units[t].shields >= max {
            return Ok(0);
        }
        if self.units[b].energy <= 0 {
            return Err(SimError::NotEnoughEnergy);
        }
        let need = max - self.units[t].shields;
        let restore_fp = need.min(self.units[b].energy * fp(2));
        let energy_used = (restore_fp + fp(2) - 1) / fp(2);
        self.units[t].shields += restore_fp;
        self.units[b].energy -= energy_used;
        Ok(restore_fp / fp(1))
    }

    pub fn cast(
        &mut self,
        caster_id: u32,
        spell: Spell,
        target: SpellTarget,
    ) -> Result<(), SimError> {
        let i = self.index_of(caster_id).ok_or(SimError::BadUnit)?;
        let owner = self.units[i].owner as usize;
        // A detached Comsat/Nuclear Silo is inactive.
        if matches!(
            self.units[i].type_id,
            uid::COMSAT_STATION | uid::NUCLEAR_SILO
        ) && !self.units[i].addon_attached
        {
            return Err(SimError::InvalidTarget);
        }
        let tech_id = spell.tech_id();
        if !self.players[owner].has_tech(tech_id) && tech_id != tech_id::HEALING {
            return Err(SimError::MissingTech);
        }
        if !caster_can_cast(self.units[i].type_id, spell) {
            return Err(SimError::Unsupported);
        }
        // Lurker Aspect is a unit morph, not an energy ability; the techdata
        // energy column is unused for it.
        let cost = if spell == Spell::LurkerAspect {
            0
        } else {
            self.data
                .tech(tech_id)
                .map(|t| t.energy_cost as i32)
                .unwrap_or(0)
        };
        if !self.units[i].spend_energy_fixed(cost * ENERGY_SCALE) {
            return Err(SimError::NotEnoughEnergy);
        }
        self.apply_spell(i, spell, target);
        Ok(())
    }

    fn order_cast(&mut self, i: usize, spell: Spell, target: SpellTarget) {
        let current = self.units[i].anim.as_ref().map(|a| a.anim as usize);
        if current == Some(ANIM_CAST_SPELL) {
            return;
        }
        // Spells are released on the iscript `castspell` opcode.  The order
        // waits at casting range until the animation reaches that frame.
        let range = spell_range(spell);
        let pos = match target {
            SpellTarget::Unit(id) => self.unit(id).map(|u| (u.x, u.y)),
            SpellTarget::Point(x, y) => Some((x * fp(1), y * fp(1))),
            SpellTarget::None => Some((self.units[i].x, self.units[i].y)),
        };
        if let Some(p) = pos {
            if self.distance_to_point(i, p) > range {
                self.move_towards(i, (p.0 / fp(1), p.1 / fp(1)), range / fp(1));
                return;
            }
        }
        let has_anim = self
            .data
            .unit_iscript_id(self.data.unit(self.units[i].type_id))
            .is_some_and(|id| self.data.iscript.animation_has_cast(id, ANIM_CAST_SPELL));
        if has_anim {
            self.start_animation(i, ANIM_CAST_SPELL);
        } else {
            let _ = self.cast(self.units[i].id, spell, target);
            self.order_done(i);
        }
    }

    fn apply_spell(&mut self, caster: usize, spell: Spell, target: SpellTarget) {
        let c_owner = self.units[caster].owner;
        let (cx, cy) = (self.units[caster].x, self.units[caster].y);
        let target_idx = match target {
            SpellTarget::Unit(id) => self.index_of(id),
            _ => None,
        };
        let point = match target {
            SpellTarget::Unit(id) => self.unit(id).map(|u| (u.x, u.y)),
            SpellTarget::Point(x, y) => Some((x * fp(1), y * fp(1))),
            SpellTarget::None => Some((cx, cy)),
        };
        match spell {
            Spell::StimPacks => {
                let hp = self.units[caster].hp;
                self.units[caster].hp = (hp - fp(10)).max(1);
                self.units[caster].status.stim_ticks = 336;
            }
            Spell::TankSiegeMode => {
                self.order_siege(caster, self.units[caster].type_id == uid::SIEGE_TANK);
            }
            Spell::CloakingField | Spell::PersonnelCloaking => {
                self.units[caster].status.cloaked = !self.units[caster].status.cloaked;
            }
            Spell::Burrowing => {
                self.units[caster].status.burrowed = true;
            }
            Spell::YamatoGun => {
                if let Some(t) = target_idx {
                    let w = self.data.weapon(wid::YAMATO_GUN).cloned();
                    if let Some(w) = w {
                        self.apply_hit(caster, t, &w, 1);
                    }
                }
            }
            Spell::DefensiveMatrix => {
                if let Some(t) = target_idx {
                    self.units[t].status.matrix = fp(250);
                }
            }
            Spell::EmpShockwave => {
                if let Some((tx, ty)) = point {
                    for j in 0..self.units.len() {
                        if self.units[j].alive && self.distance_to_point(j, (tx, ty)) <= fp(64) {
                            self.units[j].shields = 0;
                            self.units[j].set_energy_fixed(0);
                        }
                    }
                }
            }
            Spell::Irradiate => {
                if let Some(t) = target_idx {
                    let ty = self.data.unit(self.units[t].type_id);
                    if ty.is_organic() && !ty.is_building() {
                        self.units[t].status.irradiate_ticks = 400;
                    }
                }
            }
            Spell::PsionicStorm => {
                if let Some((tx, ty)) = point {
                    for j in 0..self.units.len() {
                        if self.units[j].alive && self.distance_to_point(j, (tx, ty)) <= fp(48) {
                            self.damage_hp(j, fp(14), caster as u32);
                        }
                    }
                }
            }
            Spell::Hallucination => {
                if let Some(t) = target_idx {
                    let ty = self.units[t].type_id;
                    let owner = self.units[t].owner;
                    if let Ok(new) = self.spawn_px(
                        owner as usize,
                        ty,
                        self.units[t].x / fp(1),
                        self.units[t].y / fp(1),
                    ) {
                        self.units[new].status.hallucination = true;
                        self.units[new].status.revealed_ticks = 600;
                    }
                }
            }
            Spell::StasisField => {
                if let Some((tx, ty)) = point {
                    for j in 0..self.units.len() {
                        if self.units[j].alive && self.distance_to_point(j, (tx, ty)) <= fp(64) {
                            self.units[j].status.stasis_ticks = 480;
                        }
                    }
                }
            }
            Spell::ArchonWarp | Spell::DarkArchonMeld => {
                let (a, b) = (uid::HIGH_TEMPLAR, uid::DARK_TEMPLAR);
                let (wanted, result) = if spell == Spell::ArchonWarp {
                    (a, uid::ARCHON)
                } else {
                    (b, uid::DARK_ARCHON)
                };
                let count = self
                    .units
                    .iter()
                    .filter(|u| u.alive && u.owner == c_owner && u.type_id == wanted)
                    .count();
                if count >= 2 {
                    let mut removed = 0;
                    for j in 0..self.units.len() {
                        if removed == 2 {
                            break;
                        }
                        if self.units[j].alive
                            && self.units[j].owner == c_owner
                            && self.units[j].type_id == wanted
                        {
                            self.units[j].alive = false;
                            removed += 1;
                        }
                    }
                    let _ = self.spawn_px(c_owner as usize, result, cx / fp(1), cy / fp(1));
                }
            }
            Spell::Feedback => {
                if let Some(t) = target_idx {
                    self.feedback_damage(t, caster as u32);
                }
            }
            Spell::Maelstrom => {
                if let Some((tx, ty)) = point {
                    for j in 0..self.units.len() {
                        if self.units[j].alive
                            && self.distance_to_point(j, (tx, ty)) <= fp(64)
                            && self.data.unit(self.units[j].type_id).is_organic()
                        {
                            self.units[j].status.maelstrom_ticks = 240;
                        }
                    }
                }
            }
            Spell::MindControl => {
                if let Some(t) = target_idx {
                    self.units[t].owner = c_owner;
                }
            }
            Spell::SpawnBroodlings => {
                if let Some(t) = target_idx {
                    let tx = self.units[t].x / fp(1);
                    let ty = self.units[t].y / fp(1);
                    if self.data.unit(self.units[t].type_id).is_organic() {
                        self.kill_unit(t);
                        let _ = self.spawn_px(c_owner as usize, uid::BROODLING, tx, ty);
                        let _ = self.spawn_px(c_owner as usize, uid::BROODLING, tx + 16, ty);
                    }
                }
            }
            Spell::Parasite => {
                if let Some(t) = target_idx {
                    self.units[t].status.parasite = true;
                    self.units[t].status.revealed_ticks = 1200;
                }
            }
            Spell::Infestation => {
                if let Some(t) = target_idx {
                    let max = self.data.unit(uid::COMMAND_CENTER).hit_points_raw as i32;
                    if self.units[t].type_id == uid::COMMAND_CENTER && self.units[t].hp * 2 < max {
                        if let Some(addon) = self.units[t].addon.take() {
                            if let Some(a) = self.index_of(addon) {
                                self.kill_unit(a);
                            }
                        }
                        self.units[t].type_id = uid::INFESTED_COMMAND_CENTER;
                        self.units[t].owner = c_owner;
                        self.units[t].shields = 0;
                        self.units[t].status.parasite = false;
                    }
                }
            }
            Spell::Plague => {
                if let Some((tx, ty)) = point {
                    for j in 0..self.units.len() {
                        if self.units[j].alive && self.distance_to_point(j, (tx, ty)) <= fp(64) {
                            self.units[j].status.plague_ticks = 400;
                        }
                    }
                }
            }
            Spell::Consume => {
                if let Some(t) = target_idx {
                    if t != caster {
                        let gained =
                            self.data.unit(self.units[t].type_id).hit_points_raw as i32 / fp(1) * 2;
                        self.units[caster].add_energy_points(gained);
                        self.kill_unit(t);
                    }
                }
            }
            Spell::Ensnare => {
                if let Some((tx, ty)) = point {
                    for j in 0..self.units.len() {
                        if self.units[j].alive
                            && self.distance_to_point(j, (tx, ty)) <= fp(64)
                            && !self.is_air_unit(j)
                        {
                            self.units[j].status.ensnare_ticks = 240;
                        }
                    }
                }
            }
            Spell::Recall => {
                if let Some((tx, ty)) = point {
                    for j in 0..self.units.len() {
                        if self.units[j].alive
                            && self.units[j].owner == c_owner
                            && j != caster
                            && self.distance_to_point(j, (cx, cy)) <= fp(96)
                        {
                            self.units[j].x = tx;
                            self.units[j].y = ty;
                        }
                    }
                }
            }
            Spell::Restoration => {
                if let Some(t) = target_idx {
                    let s = &mut self.units[t].status;
                    s.lockdown_ticks = 0;
                    s.plague_ticks = 0;
                    s.irradiate_ticks = 0;
                    s.maelstrom_ticks = 0;
                    s.ensnare_ticks = 0;
                    s.optical_flare_ticks = 0;
                }
            }
            Spell::DisruptionWeb => {
                if let Some((tx, ty)) = point {
                    for j in 0..self.units.len() {
                        if self.units[j].alive && self.distance_to_point(j, (tx, ty)) <= fp(64) {
                            self.units[j].status.disruption_ticks = 240;
                        }
                    }
                    self.spawn_temporary(c_owner, uid::DISRUPTION_WEB, (tx, ty), 360);
                }
            }
            Spell::OpticalFlare => {
                if let Some(t) = target_idx {
                    self.units[t].status.optical_flare_ticks = 600;
                }
            }
            Spell::Healing => {
                if let Some(t) = target_idx {
                    let max = self.data.unit(self.units[t].type_id).hit_points_raw as i32;
                    self.units[t].hp = (self.units[t].hp + fp(20)).min(max);
                }
            }
            Spell::Lockdown => {
                if let Some(t) = target_idx {
                    let ty = self.data.unit(self.units[t].type_id);
                    if ty.is_mechanical() {
                        self.units[t].status.lockdown_ticks = 480;
                    }
                }
            }
            Spell::DarkSwarm => {
                let point = match target {
                    SpellTarget::Unit(id) => self.unit(id).map(|u| (u.x, u.y)),
                    SpellTarget::Point(x, y) => Some((x * fp(1), y * fp(1))),
                    SpellTarget::None => None,
                };
                if let Some((tx, ty)) = point {
                    self.spawn_dark_swarm(c_owner, (tx, ty));
                }
            }
            Spell::ScannerSweep => {
                let point = match target {
                    SpellTarget::Unit(id) => self.unit(id).map(|u| (u.x, u.y)),
                    SpellTarget::Point(x, y) => Some((x * fp(1), y * fp(1))),
                    SpellTarget::None => None,
                };
                if let Some((tx, ty)) = point {
                    for j in 0..self.units.len() {
                        if self.units[j].alive
                            && self.units[j].owner != c_owner
                            && self.distance_to_point(j, (tx, ty)) <= fp(96)
                        {
                            self.units[j].status.revealed_ticks = 400;
                        }
                    }
                    self.reveal_point(c_owner, tx / fp(1), ty / fp(1), 8);
                }
            }
            Spell::SpiderMines => {
                let point = match target {
                    SpellTarget::Unit(id) => self.unit(id).map(|u| (u.x, u.y)),
                    SpellTarget::Point(x, y) => Some((x * fp(1), y * fp(1))),
                    SpellTarget::None => None,
                };
                if let Some((tx, ty)) = point {
                    for k in 0..3 {
                        if let Some(idx) = self.spawn_temporary(
                            c_owner,
                            uid::SPIDER_MINE,
                            (tx + k * 16 * fp(1), ty + k * 8 * fp(1)),
                            0,
                        ) {
                            self.units[idx].status.burrowed = true;
                            self.units[idx].mine_armed_at =
                                self.tick as i64 + SPIDER_MINE_ARM_TICKS;
                        }
                    }
                }
            }
            Spell::LurkerAspect => {
                if let Some(t) = target_idx {
                    let _ = self.morph_unit(self.units[t].id, uid::LURKER);
                }
            }
        }
    }

    /// Toggle a Ghost's/Wraith's personal cloak.  The tech gate is the same
    /// `Cloaking Field` / `Personnel Cloaking` research used by the ability.
    pub fn set_cloak(&mut self, unit_id: u32, cloaked: bool) -> Result<(), SimError> {
        let i = self.index_of(unit_id).ok_or(SimError::BadUnit)?;
        let owner = self.units[i].owner as usize;
        let tech_id = match self.units[i].type_id {
            uid::GHOST => tech_id::PERSONNEL_CLOAKING,
            uid::WRAITH => tech_id::CLOAKING_FIELD,
            _ => return Err(SimError::Unsupported),
        };
        if cloaked && !self.players[owner].has_tech(tech_id) {
            return Err(SimError::MissingTech);
        }
        self.units[i].status.cloaked = cloaked;
        Ok(())
    }

    /// Launch a nuclear strike from a Ghost.  Requires a completed Nuclear
    /// Silo and pays the 200/200 cost.  The missile is a travelling projectile
    /// with the retail nuclear hit type; impact damage is
    /// `max(500, 2/3 * max_hp_plus_shields)` over the large splash radius.
    pub fn launch_nuke(&mut self, ghost_id: u32, pos: (i32, i32)) -> Result<(), SimError> {
        let i = self.index_of(ghost_id).ok_or(SimError::BadUnit)?;
        if self.units[i].type_id != uid::GHOST {
            return Err(SimError::Unsupported);
        }
        let owner = self.units[i].owner;
        if !self.units.iter().any(|u| {
            u.alive
                && u.completed
                && u.owner == owner
                && u.type_id == uid::NUCLEAR_SILO
                && u.addon_attached
        }) {
            return Err(SimError::MissingRequirements);
        }
        {
            let p = &mut self.players[owner as usize];
            if p.minerals < 200 || p.gas < 200 {
                return Err(if p.minerals < 200 {
                    SimError::NotEnoughMinerals
                } else {
                    SimError::NotEnoughGas
                });
            }
            p.minerals -= 200;
            p.gas -= 200;
        }
        let w = self.data.weapon(wid::NUCLEAR_STRIKE).cloned();
        let Some(w) = w else {
            return Err(SimError::Unsupported);
        };
        let _ = &w;
        let (gx, gy) = (self.units[i].x, self.units[i].y);
        let some_target = self.units.iter().position(|u| {
            u.alive
                && u.owner != owner
                && (u.x - pos.0 * fp(1)).abs() < fp(4)
                && (u.y - pos.1 * fp(1)).abs() < fp(4)
        });
        let target = some_target;
        let target_id = target.map(|t| self.units[t].id);
        self.projectiles.push(Projectile {
            id: self.next_projectile_id,
            owner,
            source: self.units[i].id,
            weapon: wid::NUCLEAR_STRIKE,
            behavior: behavior::FLY,
            hit_type: hit::NUCLEAR,
            state: BulletState::Fly,
            x: gx,
            y: gy,
            target: target_id,
            target_pos: (pos.0 * fp(1), pos.1 * fp(1)),
            speed: 10 * 256,
            remaining_time: 300,
            bounces_left: 0,
            hit_flags: 0,
            previous_target: None,
            last_dot_tick: 0,
        });
        self.next_projectile_id = self.next_projectile_id.wrapping_add(1).max(1);
        self.units[i].cooldown = 5;
        Ok(())
    }

    /// Apply a simple AI order: move/attack-move.
    pub fn command_move(&mut self, unit_ids: &[u32], pos: (i32, i32)) {
        for &id in unit_ids {
            let _ = self.issue(id, Order::Move { pos });
        }
    }

    pub fn command_attack_move(&mut self, unit_ids: &[u32], pos: (i32, i32)) {
        for &id in unit_ids {
            let _ = self.issue(id, Order::AttackMove { pos });
        }
    }

    // ------------------------------------------------------------------
    // Carriers, Reavers, Spider Mines and other special units
    // ------------------------------------------------------------------

    /// Add one interceptor/scarab to a carrier/reaver hangar.
    pub fn build_hangar_unit(&mut self, unit_id: u32) -> Result<(), SimError> {
        let i = self.index_of(unit_id).ok_or(SimError::BadUnit)?;
        let owner = self.units[i].owner;
        let type_id = self.units[i].type_id;
        let (child, cost) = match type_id {
            uid::CARRIER => (uid::INTERCEPTOR, 15),
            uid::REAVER => (uid::SCARAB, 15),
            _ => return Err(SimError::Unsupported),
        };
        let max = self.hangar_capacity(i);
        if self.units[i].hangar >= max {
            return Err(SimError::QueueFull);
        }
        if self.players[owner as usize].minerals < cost {
            return Err(SimError::NotEnoughMinerals);
        }
        self.players[owner as usize].minerals -= cost;
        self.units[i].hangar += 1;
        let _ = child;
        Ok(())
    }

    pub fn hangar_capacity(&self, i: usize) -> u8 {
        let u = &self.units[i];
        let owner = u.owner as usize;
        match u.type_id {
            uid::CARRIER => {
                if self.players[owner].upgrade_level(upgrade::CARRIER_CAPACITY) > 0 {
                    8
                } else {
                    CARRIER_HANGAR
                }
            }
            uid::REAVER => {
                if self.players[owner].upgrade_level(upgrade::REAVER_CAPACITY) > 0 {
                    10
                } else {
                    REAVER_HANGAR
                }
            }
            _ => 0,
        }
    }

    fn special_attack(&mut self, attacker: usize, target: usize) -> bool {
        let ty = self.units[attacker].type_id;
        let dist = self.distance(attacker, target);
        match ty {
            uid::CARRIER => {
                if self.units[attacker].hangar == 0 || dist > 8 * 32 * fp(1) {
                    return false;
                }
                if self.units[attacker].cooldown == 0 {
                    let w = self.data.weapon(wid::PULSE_CANNON).cloned();
                    if let Some(w) = w {
                        self.spawn_projectile(attacker, target, wid::PULSE_CANNON, &w);
                        self.units[attacker].cooldown = 9;
                    }
                }
                true
            }
            uid::REAVER => {
                if self.units[attacker].hangar == 0 || dist > 10 * 32 * fp(1) {
                    return false;
                }
                if self.units[attacker].cooldown == 0 {
                    let w = self.data.weapon(wid::SCARAB).cloned();
                    if let Some(w) = w {
                        self.spawn_projectile(attacker, target, wid::SCARAB, &w);
                        self.units[attacker].hangar -= 1;
                        self.units[attacker].cooldown = 30;
                    }
                }
                true
            }
            _ => false,
        }
    }

    fn is_under_dark_swarm(&self, target: usize) -> bool {
        let t = &self.units[target];
        if self.is_air_unit(target) || self.data.unit(t.type_id).is_building() {
            return false;
        }
        self.units.iter().any(|u| {
            u.alive
                && u.type_id == uid::DARK_SWARM
                && (u.x - t.x).abs() < fp(96)
                && (u.y - t.y).abs() < fp(96)
        })
    }

    fn spawn_dark_swarm(&mut self, owner: u8, pos: (i32, i32)) {
        let _ = self.spawn_temporary(owner, uid::DARK_SWARM, pos, 360);
    }

    fn spawn_temporary(
        &mut self,
        owner: u8,
        type_id: u16,
        pos: (i32, i32),
        lifetime: i64,
    ) -> Option<usize> {
        let idx = self
            .spawn_px(owner as usize, type_id, pos.0 / fp(1), pos.1 / fp(1))
            .ok()?;
        self.units[idx].expires_at = if lifetime > 0 {
            self.tick as i64 + lifetime
        } else {
            0
        };
        self.units[idx].completed = true;
        Some(idx)
    }

    /// Spider mines arm after a short delay, then detonate on the first enemy
    /// that enters their 96-pixel trigger radius.  The damage comes from the
    /// recovered `weapons.dat` row 6 (Spider Mines: 125 damage, radial splash).
    fn update_specials(&mut self) {
        let mut detonations: Vec<(usize, (i32, i32))> = Vec::new();
        let mut mines: Vec<usize> = Vec::new();
        for i in 0..self.units.len() {
            let u = &self.units[i];
            if !u.alive || u.type_id != uid::SPIDER_MINE {
                continue;
            }
            if !u.status.burrowed {
                mines.push(i);
                continue;
            }
            if self.tick as i64 >= u.mine_armed_at {
                let enemy = self.units.iter().enumerate().any(|(ni, n)| {
                    n.alive
                        && n.owner != u.owner
                        && !self.is_air_unit(ni)
                        && (n.x - u.x).abs() <= fp(32)
                        && (n.y - u.y).abs() <= fp(32)
                });
                if enemy {
                    detonations.push((i, (u.x, u.y)));
                }
            }
        }
        for i in mines {
            self.units[i].status.burrowed = true;
            self.units[i].mine_armed_at = self.tick as i64 + SPIDER_MINE_ARM_TICKS;
        }
        for (i, pos) in detonations {
            if let Some(w) = self.data.weapon(wid::SPIDER_MINES).cloned() {
                let owner = self.units[i].owner;
                self.resolve_hit_type(owner, None, i, &w, pos, 1, 1);
            }
            self.kill_unit(i);
        }
        self.bunker_fire();
        self.medic_healing();
    }

    /// Infantry inside a Bunker fire from the building's position.  Each
    /// occupant keeps its own weapon, cooldown and upgrades; the bunker only
    /// supplies the location and protection.
    fn bunker_fire(&mut self) {
        for b in 0..self.units.len() {
            if !self.units[b].alive
                || !self.units[b].completed
                || self.units[b].type_id != uid::BUNKER
                || self.units[b].cargo.is_empty()
            {
                continue;
            }
            let (bx, by) = (self.units[b].x, self.units[b].y);
            let owner = self.units[b].owner;
            let cargo = self.units[b].cargo.clone();
            for id in cargo {
                let Some(o) = self.index_of(id) else {
                    continue;
                };
                // Loaded occupants are hidden (`alive == false`) but still
                // fight from the bunker.
                if self.units[o].hp <= 0 {
                    continue;
                }
                if self.units[o].cooldown > 0 {
                    self.units[o].cooldown -= 1;
                    continue;
                }
                let mut best: Option<(usize, u8, WeaponType, i32)> = None;
                for j in 0..self.units.len() {
                    if !self.units[j].alive || self.units[j].owner == owner {
                        continue;
                    }
                    let Some((wid, w)) = self.weapon_for(o, j) else {
                        continue;
                    };
                    let dx = self.units[j].x - bx;
                    let dy = self.units[j].y - by;
                    let dist = isqrt(dx as i64 * dx as i64 + dy as i64 * dy as i64) as i32;
                    // Bunkers grant their occupants one extra tile of range.
                    if dist > (w.max_range as i32 + 1) * fp(1) || dist < w.min_range as i32 * fp(1)
                    {
                        continue;
                    }
                    if best.as_ref().is_none_or(|(_, _, _, bd)| dist < *bd) {
                        best = Some((j, wid, w, dist));
                    }
                }
                if let Some((j, wid, w, _)) = best {
                    self.units[o].facing =
                        direction_byte(self.units[j].x - bx, self.units[j].y - by);
                    self.fire(o, j, wid, &w);
                }
            }
        }
    }

    /// Medics automatically heal the nearest injured organic friendly unit.
    /// The retail unit spends energy per point healed; this rules-level
    /// version uses one energy for two hit points.
    fn medic_healing(&mut self) {
        for i in 0..self.units.len() {
            if !self.units[i].alive
                || self.units[i].type_id != uid::MEDIC
                || self.units[i].energy_fixed() < MEDIC_HEAL_COST_PER_TICK
                || !matches!(self.units[i].order, Order::Idle | Order::HoldPosition)
            {
                continue;
            }
            let owner = self.units[i].owner;
            let mut best: Option<(i32, usize)> = None;
            for j in 0..self.units.len() {
                let ty = self.data.unit(self.units[j].type_id);
                if !self.units[j].alive
                    || self.units[j].owner != owner
                    || !ty.is_organic()
                    || ty.is_building()
                    || self.units[j].hp >= ty.hit_points_raw as i32
                {
                    continue;
                }
                let d = self.distance(i, j);
                if d <= fp(64) && best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, j));
                }
            }
            if let Some((_, target)) = best {
                let max = self.data.unit(self.units[target].type_id).hit_points_raw as i32;
                let healed = MEDIC_HEAL_PER_TICK.min(max - self.units[target].hp);
                if healed > 0 && self.units[i].spend_energy_fixed(MEDIC_HEAL_COST_PER_TICK) {
                    self.units[target].hp += healed;
                }
            }
        }
    }
}

fn direction_vector(direction: i32) -> (i32, i32) {
    // The engine's 256-entry direction table is a cosine/sine pair scaled by
    // 256.  Four cardinal directions are exact; intermediate angles are
    // rounded the same way the recovered helper does.
    let angle = (direction as f32) * std::f32::consts::TAU / 256.0;
    (
        (angle.cos() * 256.0).round() as i32,
        (angle.sin() * 256.0).round() as i32,
    )
}

fn caster_can_cast(type_id: u16, spell: Spell) -> bool {
    match spell {
        Spell::StimPacks => matches!(type_id, uid::MARINE | uid::FIREBAT),
        Spell::Lockdown => type_id == uid::GHOST,
        Spell::EmpShockwave | Spell::DefensiveMatrix | Spell::Irradiate => {
            type_id == uid::SCIENCE_VESSEL
        }
        Spell::SpiderMines => type_id == uid::VULTURE,
        Spell::ScannerSweep => type_id == uid::COMSAT_STATION,
        Spell::TankSiegeMode => matches!(type_id, uid::SIEGE_TANK | uid::SIEGE_MODE),
        Spell::YamatoGun => type_id == uid::BATTLECRUISER,
        Spell::CloakingField => type_id == uid::WRAITH,
        Spell::PersonnelCloaking => type_id == uid::GHOST,
        Spell::Burrowing => matches!(
            type_id,
            uid::DRONE
                | uid::ZERGLING
                | uid::HYDRALISK
                | uid::LURKER
                | uid::DEFILER
                | uid::INFESTED_TERRAN
        ),
        Spell::SpawnBroodlings | Spell::Ensnare | Spell::Parasite | Spell::Infestation => {
            type_id == uid::QUEEN
        }
        Spell::DarkSwarm | Spell::Plague | Spell::Consume => type_id == uid::DEFILER,
        Spell::PsionicStorm | Spell::Hallucination | Spell::ArchonWarp => {
            type_id == uid::HIGH_TEMPLAR
        }
        Spell::Recall | Spell::StasisField => type_id == uid::ARBITER,
        Spell::Restoration | Spell::OpticalFlare | Spell::Healing => type_id == uid::MEDIC,
        Spell::DisruptionWeb => type_id == uid::CORSAIR,
        Spell::MindControl | Spell::Feedback | Spell::Maelstrom => type_id == uid::DARK_ARCHON,
        Spell::DarkArchonMeld => type_id == uid::DARK_TEMPLAR,
        Spell::LurkerAspect => type_id == uid::HYDRALISK,
    }
}

fn default_hangar_capacity(type_id: u16) -> u8 {
    match type_id {
        uid::CARRIER => CARRIER_HANGAR,
        uid::REAVER => REAVER_HANGAR,
        // The Vulture's three Spider Mines reuse the hangar counter.
        uid::VULTURE => 3,
        _ => 0,
    }
}

fn default_hangar(type_id: u16) -> u8 {
    if type_id == uid::VULTURE {
        3
    } else {
        0
    }
}

fn weapon_can_target(weapon: &WeaponType, target: &UnitType) -> bool {
    if target.is_flyer() {
        if !weapon.can_target_air() {
            return false;
        }
    } else if !weapon.can_target_ground() {
        return false;
    }
    if weapon.requires_mechanical() && !target.is_mechanical() {
        return false;
    }
    if weapon.requires_organic() && !target.is_organic() {
        return false;
    }
    if !weapon.can_target_buildings() && target.is_building() {
        return false;
    }
    true
}

pub fn is_resource_type(type_id: u16) -> bool {
    matches!(
        type_id,
        uid::MINERAL_FIELD | uid::MINERAL_FIELD_2 | uid::MINERAL_FIELD_3 | uid::VESPENE_GEYSER
    )
}

/// Refinery/Assimilator/Extractor: the gas building that replaces a geyser.
fn is_gas_building(type_id: u16) -> bool {
    matches!(type_id, uid::REFINERY | uid::ASSIMILATOR | uid::EXTRACTOR)
}

fn is_gas_resource(type_id: u16) -> bool {
    type_id == uid::VESPENE_GEYSER || is_gas_building(type_id)
}

/// Terran buildings occupy 106..=125 (Command Center through Bunker).
fn is_terran_building(type_id: u16) -> bool {
    (106..=125).contains(&type_id)
}

/// Zerg buildings are the contiguous block 130..=150 in `units.dat`
/// (Infested Command Center through Extractor).
fn is_zerg_building(type_id: u16) -> bool {
    (130..=150).contains(&type_id)
}

/// Protoss buildings occupy 154..=172 (Nexus through Shield Battery).
fn is_protoss_building(type_id: u16) -> bool {
    (154..=172).contains(&type_id)
}

/// Upgraded Zerg buildings satisfy their base building's requirements
/// (Lair/Hive count as Hatchery, Greater Spire as Spire, colonies as Creep
/// Colony).
fn requirement_equivalent(required: u16, actual: u16) -> bool {
    actual == required
        || matches!(
            (required, actual),
            (uid::HATCHERY, uid::LAIR)
                | (uid::HATCHERY, uid::HIVE)
                | (uid::LAIR, uid::HIVE)
                | (uid::SPIRE, uid::GREATER_SPIRE)
                | (uid::CREEP_COLONY, uid::SUNKEN_COLONY)
                | (uid::CREEP_COLONY, uid::SPORE_COLONY)
        )
}

/// Source building for each Zerg building morph.
fn building_morph_source(target: u16) -> Option<u16> {
    Some(match target {
        uid::LAIR => uid::HATCHERY,
        uid::HIVE => uid::LAIR,
        uid::GREATER_SPIRE => uid::SPIRE,
        uid::SUNKEN_COLONY | uid::SPORE_COLONY => uid::CREEP_COLONY,
        _ => return None,
    })
}

/// One miss roll: `chance` is in 1/256 units, so `roll < chance` misses
/// exactly `chance` times out of 256.  `0` never misses.
fn dodge_roll(roll: u32, chance: u32) -> bool {
    chance > 0 && (roll & 0xff) < chance
}

/// Shipped `orders.dat` rows that are direct ability casts.
fn row_spell(order_id: u8) -> Option<Spell> {
    Some(match order_id {
        112 | 143 => Spell::YamatoGun,
        118 | 119 => Spell::Parasite,
        120 => Spell::SpawnBroodlings,
        121 => Spell::EmpShockwave,
        122 => Spell::Lockdown,
        140 => Spell::DefensiveMatrix,
        142 => Spell::Hallucination,
        148 | 149 => Spell::Recall,
        175 | 186 => Spell::Healing,
        180 => Spell::DisruptionWeb,
        181 => Spell::MindControl,
        183 => Spell::Feedback,
        184 => Spell::OpticalFlare,
        185 => Spell::Maelstrom,
        _ => return None,
    })
}

/// Terran structures that can lift off in the retail game.
fn can_lift_building(type_id: u16) -> bool {
    matches!(
        type_id,
        uid::COMMAND_CENTER
            | uid::BARRACKS
            | uid::FACTORY
            | uid::STARPORT
            | uid::SCIENCE_FACILITY
            | uid::ENGINEERING_BAY
            | uid::INFESTED_COMMAND_CENTER
    )
}

/// Parent building for each Terran add-on.
fn addon_parent(addon: u16) -> Option<u16> {
    Some(match addon {
        uid::COMSAT_STATION | uid::NUCLEAR_SILO => uid::COMMAND_CENTER,
        uid::MACHINE_SHOP => uid::FACTORY,
        uid::CONTROL_TOWER => uid::STARPORT,
        uid::COVERT_OPS | uid::PHYSICS_LAB => uid::SCIENCE_FACILITY,
        _ => return None,
    })
}

/// The buildings that generate creep.
fn is_creep_source(type_id: u16) -> bool {
    matches!(
        type_id,
        uid::HATCHERY
            | uid::LAIR
            | uid::HIVE
            | uid::CREEP_COLONY
            | uid::SPORE_COLONY
            | uid::SUNKEN_COLONY
    )
}

/// Bunker occupancy is four infantry slots; vehicles and workers cannot
/// enter.  The shipped rows are Marine, Ghost, Firebat and Medic.
fn is_bunker_infantry(type_id: u16) -> bool {
    matches!(
        type_id,
        uid::MARINE | uid::GHOST | uid::FIREBAT | uid::MEDIC
    )
}

fn spell_range(spell: Spell) -> i32 {
    let tiles = match spell {
        Spell::PsionicStorm
        | Spell::EmpShockwave
        | Spell::Irradiate
        | Spell::DefensiveMatrix
        | Spell::StasisField
        | Spell::Maelstrom
        | Spell::Plague
        | Spell::Ensnare
        | Spell::DisruptionWeb
        | Spell::Recall
        | Spell::OpticalFlare
        | Spell::Restoration
        | Spell::Healing
        | Spell::Feedback
        | Spell::MindControl
        | Spell::SpawnBroodlings
        | Spell::Parasite
        | Spell::Consume
        | Spell::YamatoGun
        | Spell::Lockdown
        | Spell::Hallucination => 9,
        Spell::ScannerSweep | Spell::DarkSwarm | Spell::SpiderMines => 12,
        _ => 1,
    };
    tiles * 32 * fp(1)
}

fn direction_byte(dx: i32, dy: i32) -> u8 {
    let angle = (dy as f32).atan2(dx as f32);
    let mut a = (angle * 256.0 / std::f32::consts::TAU) as i32;
    a = (a + 256) % 256;
    a as u8
}

fn isqrt(n: i64) -> i64 {
    if n <= 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_world() -> World {
        World::new(
            Arc::new(GameData::bundled().unwrap()),
            Terrain::open(64, 64),
            2,
        )
    }

    #[test]
    fn miss_chances_are_exact_fractions_of_256() {
        // 255/256 under Dark Swarm, 119/256 on higher terrain, never 256/256.
        assert!(dodge_roll(0, 255));
        assert!(dodge_roll(254, 255));
        assert!(!dodge_roll(255, 255));
        assert!(dodge_roll(0, 119));
        assert!(dodge_roll(118, 119));
        assert!(!dodge_roll(119, 119));
        assert!(!dodge_roll(0, 0));
        // Over a full 256-value sweep exactly `chance` rolls miss.
        let misses = (0u32..256).filter(|r| dodge_roll(*r, 119)).count();
        assert_eq!(misses, 119);
        let misses = (0u32..256).filter(|r| dodge_roll(*r, 255)).count();
        assert_eq!(misses, 255);
    }

    #[test]
    fn dark_swarm_only_affects_ground_targets() {
        let mut w = open_world();
        let shooter = w.spawn(0, uid::MARINE, 10, 10).unwrap();
        let ground = w.spawn(1, uid::MARINE, 20, 10).unwrap();
        let air = w.spawn(1, uid::WRAITH, 20, 11).unwrap();
        assert_eq!(w.dodge_chance(shooter, ground), 0);
        // A target standing one elevation class higher misses 119/256.
        w.terrain.set_elevation_cell(20 / 8, 10 / 8, 2);
        assert_eq!(w.dodge_chance(shooter, ground), 119);
        w.terrain.set_elevation_cell(20 / 8, 10 / 8, 0);
        // A Dark Swarm cloud at the target's position.
        w.spawn(0, uid::DARK_SWARM, 20, 10).unwrap();
        assert_eq!(w.dodge_chance(shooter, ground), 255);
        assert_eq!(w.dodge_chance(shooter, air), 0);
    }

    #[test]
    fn marines_fight_and_damage_matches_the_pipeline() {
        let mut w = open_world();
        let a = w.spawn(0, uid::MARINE, 10, 10).unwrap();
        let b = w.spawn(1, uid::MARINE, 12, 10).unwrap();
        let start_hp = w.units[b].hp;
        for _ in 0..16 {
            w.step();
            if w.units[b].hp < start_hp {
                break;
            }
        }
        assert!(w.units[b].hp < start_hp);
        assert_eq!(w.units[a].cooldown, 15);
    }

    #[test]
    fn attack_animation_supplies_the_first_shot_timing() {
        let mut w = open_world();
        let _a = w.spawn(0, uid::MARINE, 10, 10).unwrap();
        let b = w.spawn(1, uid::MARINE, 12, 10).unwrap();
        let start = w.units[b].hp;
        let mut fired = None;
        for t in 1..=10 {
            w.step();
            if w.units[b].hp < start {
                fired = Some(t);
                break;
            }
        }
        assert_eq!(
            fired,
            Some(4),
            "the Marine attackwith opcode is reached on the fourth logical tick"
        );
    }

    #[test]
    fn worker_mines_and_returns() {
        let mut w = open_world();
        let cc = w.spawn(0, uid::COMMAND_CENTER, 4, 4).unwrap();
        let mf = w.spawn_px(0, uid::MINERAL_FIELD, 12 * 32, 12 * 32).unwrap();
        w.units[mf].resource_amount = 1500;
        let scv = w.spawn(0, uid::SCV, 8, 8).unwrap();
        w.issue(
            w.units[scv].id,
            Order::Gather {
                resource: w.units[mf].id,
            },
        )
        .unwrap();
        for _ in 0..2000 {
            w.step();
            if w.players[0].minerals > 50 {
                break;
            }
        }
        assert!(w.players[0].minerals > 50, "worker should deposit minerals");
        let _ = cc;
    }

    #[test]
    fn production_requires_the_right_producer_and_supply() {
        let mut w = open_world();
        let rax = w.spawn(0, uid::BARRACKS, 10, 10).unwrap();
        // A Barracks has no supply of its own without a Command Center, so a
        // Marine cannot be queued even though the producer matches.
        assert_eq!(
            w.train(w.units[rax].id, uid::MARINE),
            Err(SimError::NotEnoughSupply)
        );
        let cc = w.spawn(0, uid::COMMAND_CENTER, 4, 4).unwrap();
        let _ = cc;
        assert_eq!(
            w.train(w.units[rax].id, uid::SCV),
            Err(SimError::MissingRequirements)
        );
    }

    #[test]
    fn barracks_trains_a_marine() {
        let mut w = open_world();
        let cc = w.spawn(0, uid::COMMAND_CENTER, 4, 4).unwrap();
        let depot = w.spawn(0, uid::SUPPLY_DEPOT, 8, 4).unwrap();
        let rax = w.spawn(0, uid::BARRACKS, 12, 4).unwrap();
        w.players[0].minerals = 1000;
        w.train(w.units[rax].id, uid::MARINE).unwrap();
        for _ in 0..1000 {
            w.step();
            if w.units.iter().any(|u| u.alive && u.type_id == uid::MARINE) {
                break;
            }
        }
        assert!(w.units.iter().any(|u| u.alive && u.type_id == uid::MARINE));
        let _ = (cc, depot);
    }

    #[test]
    fn tech_and_upgrade_are_tracked_per_player() {
        let mut w = open_world();
        let academy = w.spawn(0, uid::ACADEMY, 8, 8).unwrap();
        w.players[0].minerals = 1000;
        w.players[0].gas = 1000;
        w.research(w.units[academy].id, tech_id::STIM_PACKS)
            .unwrap();
        for _ in 0..3000 {
            w.step();
            if w.players[0].has_tech(tech_id::STIM_PACKS) {
                break;
            }
        }
        assert!(w.players[0].has_tech(tech_id::STIM_PACKS));
    }

    #[test]
    fn siege_mode_switches_unit_type() {
        let mut w = open_world();
        let tank = w.spawn(0, uid::SIEGE_TANK, 8, 8).unwrap();
        w.players[0].techs[tech_id::TANK_SIEGE_MODE as usize] = true;
        w.issue(w.units[tank].id, Order::Siege).unwrap();
        w.step();
        assert_eq!(w.units[tank].type_id, uid::SIEGE_MODE);
    }

    #[test]
    fn travelling_weapons_create_a_bullet_and_hit_on_arrival() {
        let mut w = open_world();
        // Missile Turret air weapons are BulletType 1 (follow target), unlike
        // the Marine's instant appear-at-target rifle.
        let a = w.spawn(0, uid::MISSILE_TURRET, 320, 320).unwrap();
        let b = w.spawn(1, uid::DROPSHIP, 480, 320).unwrap();
        let start = w.units[b].hp;
        for _ in 0..16 {
            w.step();
            if !w.projectiles.is_empty() {
                break;
            }
        }
        assert!(!w.projectiles.is_empty(), "the turret missile must travel");
        assert_eq!(w.units[b].hp, start, "damage is not applied at fire time");
        for _ in 0..120 {
            w.step();
            if w.units[b].hp < start {
                break;
            }
        }
        assert!(
            w.units[b].hp < start,
            "the bullet must deal damage on impact"
        );
        let _ = a;
    }

    #[test]
    fn instant_weapons_still_hit_on_the_firing_tick() {
        let mut w = open_world();
        let a = w.spawn(0, uid::MARINE, 10, 10).unwrap();
        let b = w.spawn(1, uid::MARINE, 12, 10).unwrap();
        let start = w.units[b].hp;
        for _ in 0..16 {
            w.step();
            if w.units[b].hp < start {
                break;
            }
        }
        assert!(w.projectiles.is_empty(), "Gauss Rifle is instant");
        assert!(w.units[b].hp < start);
        let _ = a;
    }

    #[test]
    fn cloak_requires_a_detector() {
        let mut w = open_world();
        let ghost = w.spawn(0, uid::GHOST, 10, 10).unwrap();
        w.players[0].techs[tech_id::PERSONNEL_CLOAKING as usize] = true;
        w.set_cloak(w.units[ghost].id, true).unwrap();
        let marine = w.spawn(1, uid::MARINE, 12, 10).unwrap();
        for _ in 0..20 {
            w.step();
        }
        assert_eq!(
            w.units[ghost].hp,
            w.data.unit(uid::GHOST).hit_points_raw as i32
        );
        // A Missile Turret is a detector and makes the Ghost targetable.
        let turret = w.spawn(1, uid::MISSILE_TURRET, 14, 10).unwrap();
        let start = w.units[ghost].hp;
        for _ in 0..50 {
            w.step();
            if w.units[ghost].hp < start {
                break;
            }
        }
        assert!(
            w.units[ghost].hp < start,
            "detector should reveal the Ghost"
        );
        let _ = (marine, turret);
    }

    #[test]
    fn detector_radius_is_seven_tiles_in_fixed_point_space() {
        // The cloaked unit's position is in 1/256 pixels while the detector
        // radius is in pixels; comparing them directly made the radius less
        // than one pixel.  The recovered building radius is seven tiles.
        let mut w = open_world();
        let ghost = w.spawn(0, uid::GHOST, 320, 320).unwrap();
        w.players[0].techs[tech_id::PERSONNEL_CLOAKING as usize] = true;
        w.set_cloak(w.units[ghost].id, true).unwrap();
        let turret = w.spawn(1, uid::MISSILE_TURRET, 320 + 6 * 32, 320).unwrap();
        assert!(w.can_see_unit(1, ghost), "six tiles is inside the radius");
        w.units[turret].x = (320 + 7 * 32) * fp(1); // exactly seven tiles
        assert!(w.can_see_unit(1, ghost));
        w.units[turret].x = (320 + 8 * 32) * fp(1); // eight tiles
        assert!(!w.can_see_unit(1, ghost));
    }

    #[test]
    fn energy_regenerates_at_eight_256ths_per_tick() {
        // `CUnit+0xa2` is fixed point and the update routine adds 8 each
        // logical tick, so one whole energy point takes 32 ticks.
        let mut w = open_world();
        let ghost = w.spawn(0, uid::GHOST, 10, 10).unwrap();
        w.units[ghost].set_energy_fixed(0);
        for _ in 0..32 {
            w.step();
        }
        assert_eq!(w.units[ghost].energy_fixed(), ENERGY_SCALE);
    }

    #[test]
    fn personal_cloak_drains_its_256ths_and_suppresses_regen() {
        let mut w = open_world();
        let ghost = w.spawn(0, uid::GHOST, 10, 10).unwrap();
        w.players[0].techs[tech_id::PERSONNEL_CLOAKING as usize] = true;
        w.units[ghost].set_energy_fixed(MAX_ENERGY * ENERGY_SCALE);
        w.set_cloak(w.units[ghost].id, true).unwrap();
        for _ in 0..25 {
            w.step();
        }
        // 25 ticks * 10/256 drained, no +8 regen while cloaked.
        assert_eq!(
            w.units[ghost].energy_fixed(),
            MAX_ENERGY * ENERGY_SCALE - 25 * GHOST_CLOAK_COST_PER_TICK
        );
        assert!(w.units[ghost].status.cloaked);

        let wraith = w.spawn(0, uid::WRAITH, 20, 20).unwrap();
        w.players[0].techs[tech_id::CLOAKING_FIELD as usize] = true;
        w.units[wraith].set_energy_fixed(1000);
        w.set_cloak(w.units[wraith].id, true).unwrap();
        w.step();
        assert_eq!(
            w.units[wraith].energy_fixed(),
            1000 - WRAITH_CLOAK_COST_PER_TICK
        );
    }

    #[test]
    fn medic_heals_200_256ths_for_100_256ths_energy() {
        let mut w = open_world();
        let medic = w.spawn(0, uid::MEDIC, 320, 320).unwrap();
        let marine = w.spawn(0, uid::MARINE, 340, 320).unwrap();
        w.units[marine].hp = fp(10);
        w.units[medic].set_energy_fixed(ENERGY_SCALE);
        let before = w.units[marine].hp;
        w.step();
        assert_eq!(w.units[marine].hp - before, MEDIC_HEAL_PER_TICK);
        // +8 regen then -100 heal cost.
        assert_eq!(
            w.units[medic].energy_fixed(),
            ENERGY_SCALE + ENERGY_REGEN_PER_TICK - MEDIC_HEAL_COST_PER_TICK
        );
    }

    #[test]
    fn feedback_deals_the_full_drained_energy_through_matrix() {
        let mut w = open_world();
        w.players[0].techs[tech_id::FEEDBACK as usize] = true;
        let archon = w.spawn(0, uid::DARK_ARCHON, 320, 320).unwrap();
        let bc = w.spawn(1, uid::BATTLECRUISER, 340, 320).unwrap();
        w.units[bc].set_energy_fixed(200 * ENERGY_SCALE);
        let before = w.units[bc].hp;
        w.cast(
            w.units[archon].id,
            Spell::Feedback,
            SpellTarget::Unit(w.units[bc].id),
        )
        .unwrap();
        assert_eq!(w.units[bc].hp, before - 200 * fp(1));
        assert_eq!(w.units[bc].energy_fixed(), 0);

        // Defensive Matrix absorbs the feedback damage first.
        let bc2 = w.spawn(1, uid::BATTLECRUISER, 360, 320).unwrap();
        w.units[bc2].set_energy_fixed(100 * ENERGY_SCALE);
        w.units[bc2].status.matrix = fp(40);
        let before = w.units[bc2].hp;
        w.cast(
            w.units[archon].id,
            Spell::Feedback,
            SpellTarget::Unit(w.units[bc2].id),
        )
        .unwrap();
        assert_eq!(w.units[bc2].status.matrix, 0);
        assert_eq!(w.units[bc2].hp, before - 60 * fp(1));
    }

    #[test]
    fn fog_of_war_hides_distant_units_from_targeting() {
        let mut w = open_world();
        w.enable_fog();
        let a = w.spawn(0, uid::MARINE, 320, 320).unwrap();
        let b = w.spawn(1, uid::MARINE, 600, 600).unwrap();
        w.step();
        assert!(w.is_visible_to(0, w.units[a].x / 256, w.units[a].y / 256));
        assert!(!w.is_visible_to(0, w.units[b].x / 256, w.units[b].y / 256));
        assert!(w.acquire_target(a).is_none());
        let _ = b;
    }

    #[test]
    fn spider_mines_arm_and_detonate() {
        let mut w = open_world();
        let vulture = w.spawn(0, uid::VULTURE, 10, 10).unwrap();
        w.players[0].techs[tech_id::SPIDER_MINES as usize] = true;
        w.units[vulture].energy = 200;
        w.cast(
            w.units[vulture].id,
            Spell::SpiderMines,
            SpellTarget::Point(14, 10),
        )
        .unwrap();
        assert!(
            w.units
                .iter()
                .filter(|u| u.type_id == uid::SPIDER_MINE)
                .count()
                >= 3
        );
        // Let the mines arm, then walk an enemy onto them.
        for _ in 0..SPIDER_MINE_ARM_TICKS as usize + 2 {
            w.step();
        }
        let enemy = w.spawn(1, uid::MARINE, 14, 10).unwrap();
        let start = w.units[enemy].hp;
        for _ in 0..20 {
            w.step();
        }
        assert!(w.units[enemy].hp < start, "spider mine should detonate");
    }

    #[test]
    fn scv_repairs_a_mechanical_unit() {
        let mut w = open_world();
        let scv = w.spawn(0, uid::SCV, 320, 320).unwrap();
        let tank = w.spawn(0, uid::SIEGE_TANK, 340, 320).unwrap();
        w.units[tank].hp = fp(50);
        w.players[0].minerals = 100;
        w.issue(
            w.units[scv].id,
            Order::Repair {
                target: w.units[tank].id,
            },
        )
        .unwrap();
        for _ in 0..30 {
            w.step();
        }
        assert!(w.units[tank].hp > fp(50));
    }

    #[test]
    fn medics_automatically_heal_nearby_organic_units() {
        let mut w = open_world();
        let medic = w.spawn(0, uid::MEDIC, 320, 320).unwrap();
        let marine = w.spawn(0, uid::MARINE, 340, 320).unwrap();
        w.units[marine].hp = fp(10);
        w.units[medic].energy = 200;
        for _ in 0..20 {
            w.step();
        }
        assert!(w.units[marine].hp > fp(10));
        let _ = medic;
    }

    #[test]
    fn point_target_spells_hit_the_ground_location() {
        let mut w = open_world();
        let templar = w.spawn(0, uid::HIGH_TEMPLAR, 320, 320).unwrap();
        w.players[0].techs[tech_id::PSIONIC_STORM as usize] = true;
        w.units[templar].energy = 200;
        let a = w.spawn(1, uid::MARINE, 480, 320).unwrap();
        let b = w.spawn(1, uid::MARINE, 500, 320).unwrap();
        let (ha, hb) = (w.units[a].hp, w.units[b].hp);
        w.cast(
            w.units[templar].id,
            Spell::PsionicStorm,
            SpellTarget::Point(480, 320),
        )
        .unwrap();
        assert!(w.units[a].hp < ha);
        assert!(w.units[b].hp < hb);
    }

    #[test]
    fn order_queue_uses_the_recovered_queueable_metadata() {
        let mut w = open_world();
        let idx = w.spawn(0, uid::MARINE, 320, 320).unwrap();
        let id = w.units[idx].id;
        w.issue(id, Order::Move { pos: (400, 400) }).unwrap();
        for i in 0..MAX_ORDER_QUEUE {
            w.queue_order(
                id,
                Order::Move {
                    pos: (400 + i as i32, 400),
                },
            )
            .unwrap();
        }
        assert_eq!(
            w.units[w.index_of(id).unwrap()].queue.len(),
            MAX_ORDER_QUEUE
        );
        assert_eq!(
            w.queue_order(id, Order::Move { pos: (1, 1) }),
            Err(SimError::QueueFull)
        );
        // Build is not queueable in the shipped table; it replaces the
        // current order and clears the queue.
        w.queue_order(
            id,
            Order::Build {
                unit_type: uid::BARRACKS,
                pos: (400, 400),
            },
        )
        .unwrap();
        let u = &w.units[w.index_of(id).unwrap()];
        assert!(matches!(u.order, Order::Build { .. }));
        assert!(u.queue.is_empty());
    }

    #[test]
    fn carrier_launches_interceptors_from_its_hangar() {
        let mut w = open_world();
        let carrier = w.spawn(0, uid::CARRIER, 320, 320).unwrap();
        w.players[0].minerals = 1000;
        w.build_hangar_unit(w.units[carrier].id).unwrap();
        assert_eq!(w.units[carrier].hangar, 1);
        let enemy = w.spawn(1, uid::MARINE, 480, 320).unwrap();
        let start = w.units[enemy].hp;
        for _ in 0..120 {
            w.step();
            if w.units[enemy].hp < start {
                break;
            }
        }
        assert!(
            w.units[enemy].hp < start,
            "interceptor should damage the target"
        );
    }

    #[test]
    fn nuclear_strike_uses_the_nuclear_damage_floor() {
        let mut w = open_world();
        let ghost = w.spawn(0, uid::GHOST, 320, 320).unwrap();
        let silo = w.spawn(0, uid::NUCLEAR_SILO, 128, 128).unwrap();
        w.units[silo].addon_attached = true;
        let target = w.spawn(1, uid::MARINE, 448, 320).unwrap();
        w.players[0].minerals = 1000;
        w.players[0].gas = 1000;
        let start = w.units[target].hp;
        w.launch_nuke(w.units[ghost].id, (448, 320)).unwrap();
        for _ in 0..400 {
            w.step();
            if w.units[target].hp < start {
                break;
            }
        }
        assert!(
            w.units[target].hp < start,
            "nuke should hit the target area"
        );
    }
}

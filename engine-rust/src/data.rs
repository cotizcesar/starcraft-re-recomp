//! Data tables recovered from StarCraft 1.23.10.13515.
//!
//! The shipped `arr\units.dat`, `arr\weapons.dat` and `arr\flingy.dat` files
//! are column-major tables.  The exact layout below was cross-checked against
//! the executable's data loader (see `docs/EVIDENCE.md`) and against the
//! documented 1.16 layout; 1.23 keeps the same layout.
//!
//! Every accessor is bounds checked.  Parse failures are reported, never
//! papered over with defaults.

use std::fmt;

use crate::iscript::Iscript;
use crate::tables::{
    self, IMAGE_COUNT, ImageType, ORDER_COUNT, OrderType, SPRITE_COUNT, SpriteType, StringTable,
    TECH_COUNT, TechType, UPGRADE_COUNT, UpgradeType,
};
pub const UNIT_COUNT: usize = 228;
pub const WEAPON_COUNT: usize = 130;
pub const FLINGY_COUNT: usize = 209;
pub const UNIT_COUNT_BUILDINGS: usize = 96;
pub const UNIT_COUNT_SPEAKERS: usize = 106;

/// The data tables that drive combat and production.
#[derive(Clone, Debug)]
pub struct GameData {
    pub units: Vec<UnitType>,
    pub weapons: Vec<WeaponType>,
    pub flingies: Vec<FlingyType>,
    pub orders: Vec<OrderType>,
    pub techs: Vec<TechType>,
    pub upgrades: Vec<UpgradeType>,
    pub sprites: Vec<SpriteType>,
    pub images: Vec<ImageType>,
    pub strings: StringTable,
    pub iscript: Iscript,
    /// Campaign AI programs from `scripts\aiscript.bin`.
    pub aiscripts: crate::aiscript::AiScriptSet,
    /// Brood War campaign AI programs from `scripts\bwscript.bin`.
    pub bwscripts: crate::aiscript::AiScriptSet,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitType {
    /// Index into `flingy.dat`.
    pub flingy: u16,
    /// Unit id of the attached turret, or `0xffff` when there is none.
    pub turret: u16,
    pub has_shields: bool,
    /// Maximum shields in whole points.  Units without shields carry a dummy
    /// value (normally 100); use `has_shields` to decide whether it is live.
    pub shield_points: u16,
    /// Maximum hit points in 1/256 points.
    pub hit_points_raw: u32,
    /// Z-sorting level.
    pub elevation: u8,
    pub armor: u8,
    pub size: UnitSize,
    /// Armour upgrade id (`armor_upgrade` in units.dat).
    pub armor_upgrade: u8,
    pub ground_weapon: u8,
    pub air_weapon: u8,
    pub max_ground_hits: u8,
    pub max_air_hits: u8,
    pub sight_range: u8,
    pub target_acquisition_range: u8,
    pub flags: u32,
    pub group_flags: u8,
    pub mineral_cost: u16,
    pub gas_cost: u16,
    /// Training/construction time in game frames (`sim::build_ticks`).
    pub build_time: u16,
    /// Supply requirement in half-points (2 == one supply).
    pub supply_required: u8,
    /// Supply provided in half-points.
    pub supply_provided: u8,
    /// Space required in a transport.
    pub space_required: u8,
    /// Space provided as a transport.
    pub space_provided: u8,
    pub build_score: u16,
    pub destroy_score: u16,
    pub placement: Point16,
    pub dimensions: Box16,
    pub portrait: u16,
    /// `units.dat` rank / sublabel: the selection panel shows `stat_txt`
    /// string `1302 + rank` (SCV 1 and Marine 2 -> "Private", Firebat 4 ->
    /// "Corporal"; 0 = no rank line).
    pub rank: u8,
    /// `units.dat` construction image (`images.dat` id, 0 = none): the
    /// Terran scaffold `TBldLrg` 325 / `tBldMed` 327 / `TBldSml` 329-330.
    pub construction_image: u16,
}

impl UnitType {
    pub fn hit_points(&self) -> f32 {
        self.hit_points_raw as f32 / 256.0
    }

    pub fn is_building(&self) -> bool {
        self.flags & 0x0000_0001 != 0
    }

    pub fn is_flyer(&self) -> bool {
        self.flags & 0x0000_0004 != 0
    }

    pub fn is_worker(&self) -> bool {
        self.flags & 0x0000_0008 != 0
    }

    pub fn is_subunit(&self) -> bool {
        self.flags & 0x0000_0010 != 0
    }

    pub fn regenerates(&self) -> bool {
        self.flags & 0x0000_0080 != 0
    }

    pub fn is_detector(&self) -> bool {
        self.flags & 0x0000_8000 != 0
    }

    pub fn is_mechanical(&self) -> bool {
        self.flags & 0x4000_0000 != 0
    }

    pub fn is_organic(&self) -> bool {
        self.flags & 0x0001_0000 != 0
    }

    /// Supply in whole points, as displayed by the game (half-points / 2).
    pub fn supply(&self) -> f32 {
        self.supply_required as f32 / 2.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponType {
    pub label: u16,
    pub flingy: u32,
    pub target_flags: u16,
    pub min_range: u32,
    pub max_range: u32,
    pub damage_upgrade: u8,
    pub damage_type: DamageType,
    pub bullet_type: u8,
    pub lifetime: u8,
    pub explosion: u8,
    pub inner_splash: u16,
    pub middle_splash: u16,
    pub outer_splash: u16,
    pub damage: u16,
    pub damage_bonus: u16,
    pub cooldown: u8,
    pub bullet_count: u8,
    pub attack_angle: u8,
    pub launch_spin: i8,
    pub forward_offset: i8,
    pub vertical_offset: i8,
    pub target_error: u16,
    pub icon: u16,
}

impl WeaponType {
    pub fn can_target_air(&self) -> bool {
        self.target_flags & 0x0001 != 0
    }

    pub fn can_target_ground(&self) -> bool {
        self.target_flags & 0x0002 != 0
    }

    pub fn requires_mechanical(&self) -> bool {
        self.target_flags & 0x0004 != 0
    }

    pub fn requires_organic(&self) -> bool {
        self.target_flags & 0x0008 != 0
    }

    pub fn can_target_buildings(&self) -> bool {
        self.target_flags & 0x0010 == 0
    }

    /// Damage in whole points for a given upgrade level.
    pub fn damage_for_upgrade(&self, upgrade_level: u8) -> u32 {
        self.damage as u32 + self.damage_bonus as u32 * upgrade_level as u32
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlingyType {
    pub sprite: u16,
    /// Top speed in 1/256 pixels per logical tick.
    pub top_speed: i32,
    /// Acceleration in 1/256 pixels per logical tick squared.
    pub acceleration: i16,
    /// Halt distance in 1/256 pixels.
    pub halt_distance: i32,
    pub turn_rate: u8,
    pub movement_type: MovementType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum MovementType {
    Flingy = 0,
    FlingyNoDeceleration = 1,
    Iscript = 2,
    Unknown(u8),
}

impl From<u8> for MovementType {
    fn from(v: u8) -> Self {
        match v {
            0 => MovementType::Flingy,
            1 => MovementType::FlingyNoDeceleration,
            2 => MovementType::Iscript,
            other => MovementType::Unknown(other),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum UnitSize {
    None = 0,
    Small = 1,
    Medium = 2,
    Large = 3,
}

impl UnitSize {
    pub fn from_raw(v: u8) -> Self {
        match v {
            0 => UnitSize::None,
            1 => UnitSize::Small,
            2 => UnitSize::Medium,
            3 => UnitSize::Large,
            other => panic!("invalid unit size {other}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum DamageType {
    None = 0,
    Explosive = 1,
    Concussive = 2,
    Normal = 3,
    IgnoreArmor = 4,
}

impl DamageType {
    pub fn from_raw(v: u8) -> Option<Self> {
        match v {
            0 => Some(DamageType::None),
            1 => Some(DamageType::Explosive),
            2 => Some(DamageType::Concussive),
            3 => Some(DamageType::Normal),
            4 => Some(DamageType::IgnoreArmor),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Point16 {
    pub x: i16,
    pub y: i16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Box16 {
    pub left: i16,
    pub top: i16,
    pub right: i16,
    pub bottom: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataError {
    ShortFile {
        which: &'static str,
        expected: usize,
        actual: usize,
    },
    InvalidDamageType {
        weapon: usize,
        value: u8,
    },
    InvalidUnitSize {
        unit: usize,
        value: u8,
    },
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataError::ShortFile {
                which,
                expected,
                actual,
            } => write!(
                f,
                "{which} is too short: expected at least {expected} bytes, got {actual}"
            ),
            DataError::InvalidDamageType { weapon, value } => {
                write!(
                    f,
                    "weapons.dat weapon {weapon} has invalid damage type {value}"
                )
            }
            DataError::InvalidUnitSize { unit, value } => {
                write!(f, "units.dat unit {unit} has invalid size {value}")
            }
        }
    }
}

impl std::error::Error for DataError {}

impl GameData {
    /// Parse the three column-major tables.
    pub fn parse(
        units_dat: &[u8],
        weapons_dat: &[u8],
        flingy_dat: &[u8],
    ) -> Result<Self, DataError> {
        Ok(Self {
            units: parse_units(units_dat)?,
            weapons: parse_weapons(weapons_dat)?,
            flingies: parse_flingies(flingy_dat)?,
            orders: Vec::new(),
            techs: Vec::new(),
            upgrades: Vec::new(),
            sprites: Vec::new(),
            images: Vec::new(),
            strings: StringTable::default(),
            iscript: Iscript::default(),
            aiscripts: Default::default(),
            bwscripts: Default::default(),
        })
    }

    /// Parse every table the game loads through `arr\*.dat` plus the string
    /// table.
    #[allow(clippy::too_many_arguments)]
    pub fn parse_all(
        units_dat: &[u8],
        weapons_dat: &[u8],
        flingy_dat: &[u8],
        orders_dat: &[u8],
        techdata_dat: &[u8],
        upgrades_dat: &[u8],
        sprites_dat: &[u8],
        images_dat: &[u8],
        stat_txt_tbl: &[u8],
    ) -> Result<Self, DataError> {
        Ok(Self {
            units: parse_units(units_dat)?,
            weapons: parse_weapons(weapons_dat)?,
            flingies: parse_flingies(flingy_dat)?,
            orders: tables::parse_orders(orders_dat)?,
            techs: tables::parse_techs(techdata_dat)?,
            upgrades: tables::parse_upgrades(upgrades_dat)?,
            sprites: tables::parse_sprites(sprites_dat)?,
            images: tables::parse_images(images_dat)?,
            strings: StringTable::parse(stat_txt_tbl)?,
            iscript: Iscript::default(),
            aiscripts: Default::default(),
            bwscripts: Default::default(),
        })
    }

    /// Parse the fixture files bundled with this crate.
    pub fn bundled() -> Result<Self, DataError> {
        let mut data = Self::parse_all(
            embedded!("fixtures/units.dat"),
            embedded!("fixtures/weapons.dat"),
            embedded!("fixtures/flingy.dat"),
            embedded!("fixtures/orders.dat"),
            embedded!("fixtures/techdata.dat"),
            embedded!("fixtures/upgrades.dat"),
            embedded!("fixtures/sprites.dat"),
            embedded!("fixtures/images.dat"),
            embedded!("fixtures/stat_txt.tbl"),
        )?;
        data.iscript = Iscript::parse(embedded!("fixtures/iscript.bin"))?;
        data.aiscripts = crate::aiscript::parse_aiscript(embedded!("fixtures/aiscript.bin"));
        data.bwscripts = crate::aiscript::parse_bwscript(embedded!("fixtures/bwscript.bin"));
        Ok(data)
    }

    pub fn unit(&self, id: u16) -> &UnitType {
        &self.units[id as usize]
    }

    pub fn weapon(&self, id: u8) -> Option<&WeaponType> {
        self.weapons.get(id as usize)
    }

    pub fn flingy(&self, id: u16) -> Option<&FlingyType> {
        self.flingies.get(id as usize)
    }

    /// Ground weapon of a unit, or `None` for the sentinel 130.
    pub fn ground_weapon(&self, unit: &UnitType) -> Option<&WeaponType> {
        self.weapon(unit.ground_weapon)
    }

    /// Air weapon of a unit, or `None` for the sentinel 130.
    pub fn air_weapon(&self, unit: &UnitType) -> Option<&WeaponType> {
        self.weapon(unit.air_weapon)
    }

    pub fn order(&self, id: u8) -> Option<&OrderType> {
        self.orders.get(id as usize)
    }

    pub fn tech(&self, id: u8) -> Option<&TechType> {
        self.techs.get(id as usize)
    }

    pub fn upgrade(&self, id: u8) -> Option<&UpgradeType> {
        self.upgrades.get(id as usize)
    }

    pub fn sprite(&self, id: u16) -> Option<&SpriteType> {
        self.sprites.get(id as usize)
    }

    pub fn image(&self, id: u16) -> Option<&ImageType> {
        self.images.get(id as usize)
    }

    /// Id of the Remastered `anim/main_NNN.anim` holding image `id`'s art.
    /// The CASC ships one container per GRP, under the first image that
    /// uses it (131 of 999 ids have none: the Command Center's 275 shares
    /// `terran\control.grp` with 63, the Nexus glow 180 with 179).
    pub fn hd_anim_id(&self, id: u16) -> u16 {
        let Some(grp) = self.image(id).map(|i| i.grp_filename_index) else {
            return id;
        };
        self.images
            .iter()
            .position(|i| i.grp_filename_index == grp)
            .map_or(id, |p| p as u16)
    }

    /// Walking speed of a unit in 1/256 pixels per logical tick.
    ///
    /// Walking units carry the sentinel speed `1` in `flingy.dat`; the real
    /// value is the mean `move` opcode of the iscript `Walking` animation.
    /// Air units and flingy-driven units use `flingy.dat` directly.
    /// Target-acquisition range in pixels.  The shipped helper at runtime
    /// `0x7a150d80` computes `max(target_acquisition_range, ground_range/32,
    /// air_range/32)` and stores the result in a per-unit byte table.
    pub fn target_acquisition_px(&self, unit: &UnitType) -> i32 {
        let mut range = unit.target_acquisition_range as i32 * 32;
        if let Some(w) = self.ground_weapon(unit) {
            range = range.max(w.max_range as i32);
        }
        if let Some(w) = self.air_weapon(unit) {
            range = range.max(w.max_range as i32);
        }
        range
    }

    /// Speed of the flingy attached to a weapon, in 1/256 pixels per tick.
    ///
    /// `weapons.dat` stores a flingy id, and `flingy.dat` stores the speed used
    /// by travelling bullets.  Instant/appearance weapons still carry a flingy
    /// id but never move, so the value is only used by travelling behaviours.
    pub fn weapon_speed(&self, weapon: &WeaponType) -> i32 {
        self.flingy(weapon.flingy as u16)
            .map(|f| f.top_speed)
            .unwrap_or(0)
            .max(0)
    }

    /// iscript id for a unit's main image, if the chain is present.
    pub fn unit_iscript_id(&self, unit: &UnitType) -> Option<u16> {
        let flingy = self.flingy(unit.flingy)?;
        let sprite = self.sprite(flingy.sprite)?;
        let image = self.image(sprite.image)?;
        Some(image.iscript_id as u16)
    }

    /// Per-tick Walking speed cycle for a unit, if its iscript has one.
    pub fn unit_walk_cycle(&self, unit: &UnitType) -> &[i32] {
        if unit.is_building() {
            return &[];
        }
        let Some(flingy) = self.flingy(unit.flingy) else {
            return &[];
        };
        let Some(sprite) = self.sprite(flingy.sprite) else {
            return &[];
        };
        let Some(image) = self.image(sprite.image) else {
            return &[];
        };
        self.iscript.walk_cycle(image.iscript_id as u16)
    }

    pub fn unit_speed(&self, unit: &UnitType) -> i32 {
        if unit.is_building() {
            return 0;
        }
        let Some(flingy) = self.flingy(unit.flingy) else {
            return 0;
        };
        if let Some(sprite) = self.sprite(flingy.sprite)
            && let Some(image) = self.image(sprite.image)
        {
            let speed = self.iscript.walk_speed(image.iscript_id as u16);
            if speed > 0 {
                return speed;
            }
        }
        flingy.top_speed.max(0)
    }

    /// Localised/display name for a unit type.
    pub fn unit_name(&self, id: u16) -> &str {
        self.strings.text(id)
    }

    pub fn order_name(&self, id: u8) -> &str {
        self.order(id)
            .map(|o| self.strings.text(o.label))
            .unwrap_or("")
    }

    pub fn tech_name(&self, id: u8) -> &str {
        // Tech/upgrade labels are stored one greater than the `stat_txt.tbl`
        // index; the game subtracts one before lookup.
        self.tech(id)
            .map(|t| self.strings.text(t.label.saturating_sub(1)))
            .unwrap_or("")
    }

    pub fn upgrade_name(&self, id: u8) -> &str {
        self.upgrade(id)
            .map(|u| self.strings.text(u.label.saturating_sub(1)))
            .unwrap_or("")
    }

    /// Sanity-check the inventory sizes.  Used by tests and by loaders that
    /// accept a directory of extracted files.
    pub fn table_counts(&self) -> [usize; 8] {
        [
            self.units.len(),
            self.weapons.len(),
            self.flingies.len(),
            self.orders.len(),
            self.techs.len(),
            self.upgrades.len(),
            self.sprites.len(),
            self.images.len(),
        ]
    }

    /// Expected stock counts; `GameData::parse` deliberately leaves the
    /// optional tables empty.
    pub fn has_full_tables(&self) -> bool {
        self.units.len() == UNIT_COUNT
            && self.weapons.len() == WEAPON_COUNT
            && self.flingies.len() == FLINGY_COUNT
            && self.orders.len() == ORDER_COUNT
            && self.techs.len() == TECH_COUNT
            && self.upgrades.len() == UPGRADE_COUNT
            && self.sprites.len() == SPRITE_COUNT
            && self.images.len() == IMAGE_COUNT
    }
}

// ---------------------------------------------------------------------------
// units.dat
// ---------------------------------------------------------------------------

#[allow(dead_code)]
mod unit_off {
    pub const FLINGY: usize = 0;
    pub const TURRET: usize = 228;
    pub const SUBUNIT2: usize = 684;
    pub const INFESTATION: usize = 1140;
    pub const CONSTRUCTION: usize = 1332;
    pub const DIRECTION: usize = 2244;
    pub const HAS_SHIELD: usize = 2472;
    pub const SHIELD: usize = 2700;
    pub const HP: usize = 3156;
    pub const ELEVATION: usize = 4068;
    pub const MOVEMENT_FLAGS: usize = 4296;
    pub const SUBLABEL: usize = 4524;
    pub const AI_IDLE: usize = 4752;
    pub const HUMAN_IDLE: usize = 4980;
    pub const RETURN_IDLE: usize = 5208;
    pub const ATTACK_UNIT: usize = 5436;
    pub const ATTACK_MOVE: usize = 5664;
    pub const GROUND_WEAPON: usize = 5892;
    pub const MAX_GROUND_HITS: usize = 6120;
    pub const AIR_WEAPON: usize = 6348;
    pub const MAX_AIR_HITS: usize = 6576;
    pub const AI_INTERNAL: usize = 6804;
    pub const FLAGS: usize = 7032;
    pub const TARGET_ACQ: usize = 7944;
    pub const SIGHT: usize = 8172;
    pub const ARMOR_UPGRADE: usize = 8400;
    pub const SIZE: usize = 8628;
    pub const ARMOR: usize = 8856;
    pub const RIGHT_CLICK: usize = 9084;
    pub const READY: usize = 9312;
    pub const FIRST_WHAT: usize = 9524;
    pub const LAST_WHAT: usize = 9980;
    pub const FIRST_PISSED: usize = 10436;
    pub const LAST_PISSED: usize = 10648;
    pub const FIRST_YES: usize = 10860;
    pub const LAST_YES: usize = 11072;
    pub const PLACEMENT: usize = 11284;
    pub const ADDON: usize = 12196;
    pub const DIMENSIONS: usize = 12580;
    pub const PORTRAIT: usize = 14404;
    pub const MINERAL: usize = 14860;
    pub const GAS: usize = 15316;
    pub const BUILD_TIME: usize = 15772;
    pub const DAT_REQS: usize = 16228;
    pub const GROUP_FLAGS: usize = 16684;
    pub const SUPPLY_PROVIDED: usize = 16912;
    pub const SUPPLY_REQUIRED: usize = 17140;
    pub const SPACE_REQUIRED: usize = 17368;
    pub const SPACE_PROVIDED: usize = 17596;
    pub const BUILD_SCORE: usize = 17824;
    pub const DESTROY_SCORE: usize = 18280;
    pub const MAP_STRING: usize = 18736;
    pub const IS_BROODWAR: usize = 19192;
    pub const STAREDIT: usize = 19420;
}

fn parse_units(d: &[u8]) -> Result<Vec<UnitType>, DataError> {
    let expected = unit_off::STAREDIT + UNIT_COUNT * 2;
    if d.len() < expected {
        return Err(DataError::ShortFile {
            which: "units.dat",
            expected,
            actual: d.len(),
        });
    }
    let mut units = Vec::with_capacity(UNIT_COUNT);
    for i in 0..UNIT_COUNT {
        let size_raw = u8_at(d, unit_off::SIZE, i);
        let size = match size_raw {
            0..=3 => UnitSize::from_raw(size_raw),
            value => return Err(DataError::InvalidUnitSize { unit: i, value }),
        };
        units.push(UnitType {
            flingy: u8_at(d, unit_off::FLINGY, i) as u16,
            turret: u16_at(d, unit_off::TURRET, i),
            has_shields: u8_at(d, unit_off::HAS_SHIELD, i) != 0,
            shield_points: u16_at(d, unit_off::SHIELD, i),
            hit_points_raw: u32_at(d, unit_off::HP, i),
            elevation: u8_at(d, unit_off::ELEVATION, i),
            armor: u8_at(d, unit_off::ARMOR, i),
            size,
            armor_upgrade: u8_at(d, unit_off::ARMOR_UPGRADE, i),
            ground_weapon: u8_at(d, unit_off::GROUND_WEAPON, i),
            air_weapon: u8_at(d, unit_off::AIR_WEAPON, i),
            max_ground_hits: u8_at(d, unit_off::MAX_GROUND_HITS, i),
            max_air_hits: u8_at(d, unit_off::MAX_AIR_HITS, i),
            sight_range: u8_at(d, unit_off::SIGHT, i),
            target_acquisition_range: u8_at(d, unit_off::TARGET_ACQ, i),
            flags: u32_at(d, unit_off::FLAGS, i),
            group_flags: u8_at(d, unit_off::GROUP_FLAGS, i),
            mineral_cost: u16_at(d, unit_off::MINERAL, i),
            gas_cost: u16_at(d, unit_off::GAS, i),
            build_time: u16_at(d, unit_off::BUILD_TIME, i),
            supply_required: u8_at(d, unit_off::SUPPLY_REQUIRED, i),
            supply_provided: u8_at(d, unit_off::SUPPLY_PROVIDED, i),
            space_required: u8_at(d, unit_off::SPACE_REQUIRED, i),
            space_provided: u8_at(d, unit_off::SPACE_PROVIDED, i),
            build_score: u16_at(d, unit_off::BUILD_SCORE, i),
            destroy_score: u16_at(d, unit_off::DESTROY_SCORE, i),
            placement: Point16 {
                x: i16_at(d, unit_off::PLACEMENT, i * 2),
                y: i16_at(d, unit_off::PLACEMENT, i * 2 + 1),
            },
            dimensions: Box16 {
                left: i16_at(d, unit_off::DIMENSIONS, i * 4),
                top: i16_at(d, unit_off::DIMENSIONS, i * 4 + 1),
                right: i16_at(d, unit_off::DIMENSIONS, i * 4 + 2),
                bottom: i16_at(d, unit_off::DIMENSIONS, i * 4 + 3),
            },
            portrait: u16_at(d, unit_off::PORTRAIT, i),
            rank: u8_at(d, unit_off::SUBLABEL, i),
            construction_image: u32_at(d, unit_off::CONSTRUCTION, i) as u16,
        });
    }
    Ok(units)
}

// ---------------------------------------------------------------------------
// weapons.dat
// ---------------------------------------------------------------------------

#[allow(dead_code)]
mod weapon_off {
    pub const LABEL: usize = 0;
    pub const FLINGY: usize = 260;
    pub const TECH: usize = 780;
    pub const TARGET_FLAGS: usize = 910;
    pub const MIN_RANGE: usize = 1170;
    pub const MAX_RANGE: usize = 1690;
    pub const DAMAGE_UPGRADE: usize = 2210;
    pub const DAMAGE_TYPE: usize = 2340;
    pub const BEHAVIOR: usize = 2470;
    pub const LIFETIME: usize = 2600;
    pub const EXPLOSION: usize = 2730;
    pub const INNER_SPLASH: usize = 2860;
    pub const MIDDLE_SPLASH: usize = 3120;
    pub const OUTER_SPLASH: usize = 3380;
    pub const DAMAGE: usize = 3640;
    pub const DAMAGE_BONUS: usize = 3900;
    pub const COOLDOWN: usize = 4160;
    pub const DAMAGE_FACTOR: usize = 4290;
    pub const ATTACK_ANGLE: usize = 4420;
    pub const LAUNCH_SPIN: usize = 4550;
    pub const FORWARD_OFFSET: usize = 4680;
    pub const VERTICAL_OFFSET: usize = 4810;
    pub const TARGET_ERROR: usize = 4940;
    pub const ICON: usize = 5200;
}

fn parse_weapons(d: &[u8]) -> Result<Vec<WeaponType>, DataError> {
    let expected = weapon_off::ICON + WEAPON_COUNT * 2;
    if d.len() < expected {
        return Err(DataError::ShortFile {
            which: "weapons.dat",
            expected,
            actual: d.len(),
        });
    }
    let mut weapons = Vec::with_capacity(WEAPON_COUNT);
    for i in 0..WEAPON_COUNT {
        let raw_type = u8_at(d, weapon_off::DAMAGE_TYPE, i);
        let damage_type = DamageType::from_raw(raw_type).ok_or(DataError::InvalidDamageType {
            weapon: i,
            value: raw_type,
        })?;
        weapons.push(WeaponType {
            label: u16_at(d, weapon_off::LABEL, i),
            flingy: u32_at(d, weapon_off::FLINGY, i),
            target_flags: u16_at(d, weapon_off::TARGET_FLAGS, i),
            min_range: u32_at(d, weapon_off::MIN_RANGE, i),
            max_range: u32_at(d, weapon_off::MAX_RANGE, i),
            damage_upgrade: u8_at(d, weapon_off::DAMAGE_UPGRADE, i),
            damage_type,
            bullet_type: u8_at(d, weapon_off::BEHAVIOR, i),
            lifetime: u8_at(d, weapon_off::LIFETIME, i),
            explosion: u8_at(d, weapon_off::EXPLOSION, i),
            inner_splash: u16_at(d, weapon_off::INNER_SPLASH, i),
            middle_splash: u16_at(d, weapon_off::MIDDLE_SPLASH, i),
            outer_splash: u16_at(d, weapon_off::OUTER_SPLASH, i),
            damage: u16_at(d, weapon_off::DAMAGE, i),
            damage_bonus: u16_at(d, weapon_off::DAMAGE_BONUS, i),
            cooldown: u8_at(d, weapon_off::COOLDOWN, i),
            bullet_count: u8_at(d, weapon_off::DAMAGE_FACTOR, i),
            attack_angle: u8_at(d, weapon_off::ATTACK_ANGLE, i),
            launch_spin: i8_at(d, weapon_off::LAUNCH_SPIN, i),
            forward_offset: i8_at(d, weapon_off::FORWARD_OFFSET, i),
            vertical_offset: i8_at(d, weapon_off::VERTICAL_OFFSET, i),
            target_error: u16_at(d, weapon_off::TARGET_ERROR, i),
            icon: u16_at(d, weapon_off::ICON, i),
        });
    }
    Ok(weapons)
}

// ---------------------------------------------------------------------------
// flingy.dat
// ---------------------------------------------------------------------------

#[allow(dead_code)]
mod flingy_off {
    pub const SPRITE: usize = 0;
    pub const TOP_SPEED: usize = 418;
    pub const ACCELERATION: usize = 1254;
    pub const HALT_DISTANCE: usize = 1672;
    pub const TURN_RATE: usize = 2508;
    pub const UNUSED: usize = 2717;
    pub const MOVEMENT_TYPE: usize = 2926;
}

fn parse_flingies(d: &[u8]) -> Result<Vec<FlingyType>, DataError> {
    let expected = flingy_off::MOVEMENT_TYPE + FLINGY_COUNT;
    if d.len() < expected {
        return Err(DataError::ShortFile {
            which: "flingy.dat",
            expected,
            actual: d.len(),
        });
    }
    let mut flingies = Vec::with_capacity(FLINGY_COUNT);
    for i in 0..FLINGY_COUNT {
        flingies.push(FlingyType {
            sprite: u16_at(d, flingy_off::SPRITE, i),
            top_speed: i32_at(d, flingy_off::TOP_SPEED, i),
            acceleration: i16_at(d, flingy_off::ACCELERATION, i),
            halt_distance: i32_at(d, flingy_off::HALT_DISTANCE, i),
            turn_rate: u8_at(d, flingy_off::TURN_RATE, i),
            movement_type: MovementType::from(u8_at(d, flingy_off::MOVEMENT_TYPE, i)),
        });
    }
    Ok(flingies)
}

// ---------------------------------------------------------------------------
// small, checked column readers
// ---------------------------------------------------------------------------

#[inline]
fn u8_at(d: &[u8], base: usize, index: usize) -> u8 {
    d[base + index]
}

#[inline]
fn u16_at(d: &[u8], base: usize, index: usize) -> u16 {
    u16::from_le_bytes([d[base + index * 2], d[base + index * 2 + 1]])
}

#[inline]
fn i16_at(d: &[u8], base: usize, index: usize) -> i16 {
    i16::from_le_bytes([d[base + index * 2], d[base + index * 2 + 1]])
}

#[inline]
fn u32_at(d: &[u8], base: usize, index: usize) -> u32 {
    let o = base + index * 4;
    u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

#[inline]
fn i32_at(d: &[u8], base: usize, index: usize) -> i32 {
    let o = base + index * 4;
    i32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

#[inline]
fn i8_at(d: &[u8], base: usize, index: usize) -> i8 {
    d[base + index] as i8
}

/// Unit ids used by the tests and examples.  They are the stock StarCraft
/// ordering, verified against the recovered tables (HP, shields and weapons).
pub mod unit_ids {
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
    pub const FIREBAT: u16 = 33;
    pub const DROPSHIP: u16 = 11;
    pub const BATTLECRUISER: u16 = 12;
    pub const ZERGLING: u16 = 37;
    pub const HYDRALISK: u16 = 38;
    pub const ULTRALISK: u16 = 39;
    pub const DRONE: u16 = 41;
    pub const OVERLORD: u16 = 42;
    pub const MUTALISK: u16 = 43;
    pub const LURKER: u16 = 103;
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
}

/// Weapon ids used by the examples.
pub mod weapon_ids {
    pub const GAUSS_RIFLE: u8 = 0;
    pub const C10_CANISTER_RIFLE: u8 = 2;
    pub const FRAGMENTATION_GRENADE: u8 = 4;
    pub const TWIN_AUTOCANNONS: u8 = 8;
    pub const ARCLITE_SHOCK_CANNON: u8 = 11;
    pub const SIEGE_MODE_SHOCK_CANNON: u8 = 12;
    pub const FUSION_CUTTER: u8 = 13;
    pub const BURST_LASERS: u8 = 16;
    pub const GEMINI_MISSILES: u8 = 15;
    pub const FLAME_THROWER: u8 = 25;
    pub const PSIONIC_BLADE: u8 = 64;
    pub const PHASE_DISRUPTOR: u8 = 66;
    pub const PHOTON_CANNON: u8 = 32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hd_art_is_looked_up_by_the_first_image_sharing_the_grp() {
        let g = GameData::bundled().expect("bundled tables parse");
        // Command Center image 275 reuses terran\control.grp of image 63.
        assert_eq!(g.hd_anim_id(275), 63);
        assert_eq!(g.hd_anim_id(63), 63);
        // Nexus glow 180 shares nexus.grp with 179.
        assert_eq!(g.hd_anim_id(180), 179);
        assert_eq!(g.hd_anim_id(179), 179);
    }

    #[test]
    fn parses_every_table() {
        let g = GameData::bundled().expect("bundled tables parse");
        assert_eq!(g.units.len(), UNIT_COUNT);
        assert_eq!(g.weapons.len(), WEAPON_COUNT);
        assert_eq!(g.flingies.len(), FLINGY_COUNT);
    }

    #[test]
    fn marine_stats_match_shipped_table() {
        let g = GameData::bundled().unwrap();
        let u = g.unit(unit_ids::MARINE);
        assert_eq!(u.hit_points(), 40.0);
        assert_eq!(u.shield_points, 100);
        assert!(!u.has_shields);
        assert_eq!(u.armor, 0);
        assert_eq!(u.size, UnitSize::Small);
        assert_eq!(u.ground_weapon, weapon_ids::GAUSS_RIFLE);
        assert_eq!(u.mineral_cost, 50);
        assert_eq!(u.gas_cost, 0);
        assert_eq!(u.build_time, 360);
        assert_eq!(u.supply_required, 2);
        assert_eq!(u.sight_range, 7);
    }

    #[test]
    fn zealot_stats_match_shipped_table() {
        let g = GameData::bundled().unwrap();
        let u = g.unit(unit_ids::ZEALOT);
        assert_eq!(u.hit_points(), 100.0);
        assert!(u.has_shields);
        assert_eq!(u.shield_points, 60);
        assert_eq!(u.armor, 1);
        assert_eq!(u.size, UnitSize::Small);
        assert_eq!(u.ground_weapon, weapon_ids::PSIONIC_BLADE);
        assert_eq!(u.mineral_cost, 100);
        assert_eq!(u.build_time, 600);
        assert_eq!(u.supply_required, 4);
    }

    #[test]
    fn gauss_rifle_matches_shipped_table() {
        let g = GameData::bundled().unwrap();
        let w = g.weapon(weapon_ids::GAUSS_RIFLE).unwrap();
        assert_eq!(w.damage, 6);
        assert_eq!(w.damage_bonus, 1);
        assert_eq!(w.damage_type, DamageType::Normal);
        assert_eq!(w.cooldown, 15);
        assert_eq!(w.max_range, 128);
        assert!(w.can_target_air());
        assert!(w.can_target_ground());
    }

    #[test]
    fn explosive_and_concussive_weapons_are_classified() {
        let g = GameData::bundled().unwrap();
        assert_eq!(
            g.weapon(weapon_ids::TWIN_AUTOCANNONS).unwrap().damage_type,
            DamageType::Explosive
        );
        assert_eq!(
            g.weapon(weapon_ids::FLAME_THROWER).unwrap().damage_type,
            DamageType::Concussive
        );
    }
}

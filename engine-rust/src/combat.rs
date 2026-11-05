//! Combat rules reconstructed from StarCraft 1.23.
//!
//! The important pieces, in the order the game applies them:
//!
//! 1. `damage = damage + damage_bonus * upgrade_level`
//! 2. `damage = max(damage, 0.5)`
//! 3. if the target has at least one full point of shields, subtract the
//!    Protoss Plasma Shields upgrade level and give the rest to shields
//! 4. subtract the target's armour from the part that overflowed the shields
//! 5. apply the size multiplier (explosive/concussive) *after* armour
//! 6. if no shield damage was dealt, floor the hit-point damage at 0.5
//!
//! Shields are therefore not reduced by normal armour and are not scaled by
//! damage type; they are only reduced by the shield upgrade.  Hit points are
//! reduced by armour and then scaled by unit size.
//!
//! The retail routine (`0x7a13ce30` in the 1.23.10 runtime image) has three
//! additional branches that are status effects and are outside this slice:
//! hallucination double damage, Defensive Matrix absorbing damage before
//! shields, and acid spores adding one point of damage per spore before the
//! initial floor.  All three are documented in `docs/EVIDENCE.md`; none is
//! modelled here.
//!
//! Hit points and shields are tracked in 1/256 point units, matching the
//! fixed-point representation used by the game.

use crate::data::{DamageType, GameData, UnitSize, UnitType, WeaponType};

pub const FP: i32 = 256;
/// Damage cannot be reduced below half a point.
pub const MIN_DAMAGE: i32 = FP / 2;
/// Protoss shields only start absorbing when at least one full point is left.
pub const SHIELD_ABSORB_THRESHOLD: i32 = FP;

/// Convert whole points to the 1/256 representation used by the game.
#[inline]
pub fn fp(whole: i32) -> i32 {
    whole.saturating_mul(FP)
}

/// Convert 1/256 points to whole points, rounding up (the game displays the
/// ceiling of a unit's remaining hit points).
#[inline]
pub fn ceil_points(value: i32) -> i32 {
    if value <= 0 { 0 } else { (value + FP - 1) / FP }
}

/// Everything the damage routine needs to know about the defender.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Defense {
    pub size: UnitSize,
    /// Base armour plus armour upgrades.
    pub armor_level: u32,
    pub has_shields: bool,
    /// Current shields in 1/256 points.
    pub shield_points: i32,
    /// Protoss Plasma Shields upgrade level.
    pub shield_upgrade: u8,
}

impl Defense {
    pub fn from_unit(unit: &UnitType, shields: i32, armor_upgrade: u8) -> Self {
        Self {
            size: unit.size,
            armor_level: unit.armor as u32 + armor_upgrade as u32,
            has_shields: unit.has_shields,
            shield_points: shields,
            shield_upgrade: 0,
        }
    }
}

/// Damage split between the two pools.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DamageResult {
    /// Damage taken by shields, in 1/256 points.
    pub shield_damage: i32,
    /// Damage taken by hit points, in 1/256 points.
    pub hp_damage: i32,
}

/// Damage-type multiplier against a unit size, as a numerator over 256.
/// These are the exact values stored in the executable's damage table at
/// virtual address `0xf0757c` (file offset `0xb05d7c`). The game looks up
/// `table[1 + type*5 + size]`, i.e. base `0xf07580` (`table + 4`).
pub fn size_multiplier(damage_type: DamageType, size: UnitSize) -> i32 {
    if size == UnitSize::None {
        return 0;
    }
    match damage_type {
        DamageType::None => 0,
        DamageType::Explosive => match size {
            UnitSize::Small => 128,
            UnitSize::Medium => 192,
            UnitSize::Large => 256,
            UnitSize::None => 0,
        },
        DamageType::Concussive => match size {
            UnitSize::Small => 256,
            UnitSize::Medium => 128,
            UnitSize::Large => 64,
            UnitSize::None => 0,
        },
        DamageType::Normal | DamageType::IgnoreArmor => 256,
    }
}

/// Resolve a single weapon hit against a defender.
///
/// `upgrade_level` is the attacker's level in the weapon's upgrade line.
/// Armour and shield upgrades are supplied by the defender.
pub fn resolve_damage(weapon: &WeaponType, upgrade_level: u8, defense: Defense) -> DamageResult {
    resolve_damage_divisor(weapon, upgrade_level, defense, 1)
}

/// Same as [`resolve_damage`], with the splash-damage divisor applied before
/// the 0.5-point floor.  The game uses 1 for a direct hit, 2 for the middle
/// splash ring and 4 for the outer ring.
pub fn resolve_damage_divisor(
    weapon: &WeaponType,
    upgrade_level: u8,
    defense: Defense,
    divisor: u32,
) -> DamageResult {
    let base = weapon.damage_for_upgrade(upgrade_level) as i32 * FP / divisor.max(1) as i32;
    let mut damage = base.max(MIN_DAMAGE);
    let mut shield_damage = 0;

    // The retail routine doubles damage for hallucinations, applies
    // Defensive Matrix (CUnit+0x112) before this block, and adds acid spores
    // (CUnit+0x126) before the initial floor.  All three are status effects
    // and are intentionally not modelled here.

    // Shields are hit first, but only when at least one full point remains.
    // Neither base armour nor the size multiplier applies to this part.
    if defense.has_shields && defense.shield_points >= SHIELD_ABSORB_THRESHOLD {
        if weapon.damage_type != DamageType::IgnoreArmor {
            let shield_armor = defense.shield_upgrade as i32 * FP;
            if damage > shield_armor {
                damage -= shield_armor;
            } else {
                damage = MIN_DAMAGE;
            }
        }
        shield_damage = damage.min(defense.shield_points).max(0);
        damage -= shield_damage;
    }

    // Normal armour applies to the overflow that is about to hit hit points.
    if weapon.damage_type != DamageType::IgnoreArmor {
        let armor = defense.armor_level as i32 * FP;
        if damage > armor {
            damage -= armor;
        } else {
            damage = 0;
        }
    }

    // Size scaling happens after armour.  The multiplier is read from the
    // same 5x5 table that the executable stores at 0xf0757c.
    damage = damage * size_multiplier(weapon.damage_type, defense.size) / FP;

    // The hit-point floor is only applied when the shield did not absorb any
    // part of this hit.
    if shield_damage == 0 && damage < MIN_DAMAGE {
        damage = MIN_DAMAGE;
    }

    DamageResult {
        shield_damage,
        hp_damage: damage.max(0),
    }
}

/// Status effects that participate in the damage pipeline but are not part
/// of the unit's base stats.  Recovered from the same routine as the main
/// pipeline (Defensive Matrix at `CUnit+0x112`, acid spores at `CUnit+0x126`,
/// hallucination status bit 30).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatusDamage {
    /// Remaining Defensive Matrix pool in 1/256 points.
    pub matrix: i32,
    /// Acid spore count; each adds one full point before the initial floor.
    pub acid_spores: u8,
    /// Hallucinations take double damage.
    pub hallucination: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatusDamageResult {
    pub damage: DamageResult,
    /// Matrix pool consumed by this hit.
    pub matrix_absorbed: i32,
}

/// Full hit resolution including the three status branches recovered from
/// the retail routine.  `resolve_damage_divisor` remains the plain pipeline
/// used by the existing tests.
pub fn resolve_damage_status(
    weapon: &WeaponType,
    upgrade_level: u8,
    defense: Defense,
    divisor: u32,
    status: StatusDamage,
) -> StatusDamageResult {
    let base = weapon.damage_for_upgrade(upgrade_level) as i32 * FP / divisor.max(1) as i32;
    let mut damage = base;
    if status.hallucination {
        damage = damage.saturating_mul(2);
    }
    damage += status.acid_spores as i32 * FP;
    damage = damage.max(MIN_DAMAGE);

    let mut matrix_absorbed = 0;
    if status.matrix > 0 {
        matrix_absorbed = damage.min(status.matrix).max(0);
        damage -= matrix_absorbed;
    }

    let mut shield_damage = 0;
    if defense.has_shields && defense.shield_points >= SHIELD_ABSORB_THRESHOLD {
        if weapon.damage_type != DamageType::IgnoreArmor {
            let shield_armor = defense.shield_upgrade as i32 * FP;
            if damage > shield_armor {
                damage -= shield_armor;
            } else {
                damage = MIN_DAMAGE;
            }
        }
        shield_damage = damage.min(defense.shield_points).max(0);
        damage -= shield_damage;
    }

    if weapon.damage_type != DamageType::IgnoreArmor {
        let armor = defense.armor_level as i32 * FP;
        if damage > armor {
            damage -= armor;
        } else {
            damage = 0;
        }
    }

    damage = damage * size_multiplier(weapon.damage_type, defense.size) / FP;
    if shield_damage == 0 && damage < MIN_DAMAGE {
        damage = MIN_DAMAGE;
    }

    StatusDamageResult {
        damage: DamageResult {
            shield_damage,
            hp_damage: damage.max(0),
        },
        matrix_absorbed,
    }
}

/// Per-player upgrade state that participates in combat.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerUpgrades {
    /// Weapon damage upgrade levels indexed by `weapons.dat` upgrade id.
    pub weapon: [u8; 32],
    /// Armour upgrade levels indexed by `units.dat` armour-upgrade id.
    pub armor: [u8; 32],
    /// Protoss Plasma Shields upgrade level.
    pub plasma_shields: u8,
}

impl PlayerUpgrades {
    pub fn weapon_level(&self, upgrade_id: u8) -> u8 {
        self.weapon.get(upgrade_id as usize).copied().unwrap_or(0)
    }

    pub fn armor_level(&self, upgrade_id: u8) -> u8 {
        self.armor.get(upgrade_id as usize).copied().unwrap_or(0)
    }
}

/// A live unit in the deterministic battle simulation.
#[derive(Clone, Debug)]
pub struct Unit {
    pub id: u32,
    pub owner: u8,
    pub type_id: u16,
    /// Current hit points in 1/256 points.  May be negative after overkill.
    pub hp: i32,
    /// Current shields in 1/256 points.
    pub shields: i32,
    pub x: i32,
    pub y: i32,
    /// Ticks until this unit may fire again.
    pub cooldown: u32,
    /// Index into `CombatSim::units`.
    pub target: Option<usize>,
    pub alive: bool,
}

impl Unit {
    pub fn new(id: u32, owner: u8, type_id: u16, x: i32, y: i32, data: &GameData) -> Self {
        let ty = data.unit(type_id);
        Self {
            id,
            owner,
            type_id,
            hp: ty.hit_points_raw as i32,
            shields: if ty.has_shields {
                fp(ty.shield_points as i32)
            } else {
                0
            },
            x,
            y,
            cooldown: 0,
            target: None,
            alive: true,
        }
    }

    pub fn is_flying(&self, data: &GameData) -> bool {
        data.unit(self.type_id).is_flyer()
    }

    pub fn distance_to(&self, other: &Unit) -> i32 {
        let dx = (self.x - other.x) as i64;
        let dy = (self.y - other.y) as i64;
        isqrt(dx * dx + dy * dy) as i32
    }
}

/// Integer square root (floor), used for deterministic range checks.
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

/// Movement speed used by the simulation, in 1/256 pixels per tick.
///
/// `GameData::unit_speed` recovers the mean `move` speed of the Walking
/// animation for iscript-controlled units, so the stock `flingy.dat`
/// sentinel never reaches this helper.  The fallback is only for a unit whose
/// animation has no positive `move` and whose flingy speed is zero; it is a
/// guard value, not a recovered speed.
pub const STATIC_SPEED_FALLBACK: i32 = 2 * FP;

pub fn effective_speed(data: &GameData, unit_type: &UnitType) -> i32 {
    let speed = data.unit_speed(unit_type);
    if speed > 0 {
        speed
    } else {
        STATIC_SPEED_FALLBACK
    }
}

/// A deterministic, fixed-tick battle simulation.
pub struct CombatSim {
    pub data: GameData,
    pub units: Vec<Unit>,
    pub upgrades: [PlayerUpgrades; 8],
    pub tick: u64,
}

/// Whether a weapon may legally target a unit type (air/ground plus the
/// mechanical/organic/building restrictions encoded in `target_flags`).
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

impl CombatSim {
    pub fn new(data: GameData) -> Self {
        Self {
            data,
            units: Vec::new(),
            upgrades: [PlayerUpgrades::default(); 8],
            tick: 0,
        }
    }

    pub fn spawn(&mut self, owner: u8, type_id: u16, x: i32, y: i32) -> usize {
        let id = self.units.len() as u32;
        let unit = Unit::new(id, owner, type_id, x, y, &self.data);
        self.units.push(unit);
        self.units.len() - 1
    }

    pub fn living(&self, owner: u8) -> impl Iterator<Item = &Unit> {
        self.units
            .iter()
            .filter(move |u| u.alive && u.owner == owner)
    }

    /// Run one logical tick.  This is the reconstructed combat loop:
    /// acquire a target, move into range, fire when the cooldown allows it,
    /// regenerate shields, and remove dead units.
    pub fn step(&mut self) {
        let n = self.units.len();
        for i in 0..n {
            if !self.units[i].alive {
                continue;
            }
            if self.units[i].cooldown > 0 {
                self.units[i].cooldown -= 1;
            }
            self.regenerate(i);
            self.acquire_or_validate_target(i);
            self.move_or_attack(i);
        }
        self.tick += 1;
    }

    fn regenerate(&mut self, i: usize) {
        let ty = self.data.unit(self.units[i].type_id).clone();
        if ty.has_shields {
            let max = fp(ty.shield_points as i32);
            if self.units[i].shields < max {
                self.units[i].shields = (self.units[i].shields + 7).min(max);
            }
        }
        if ty.regenerates() {
            let max = ty.hit_points_raw as i32;
            if self.units[i].hp < max && self.units[i].hp > 0 {
                self.units[i].hp = (self.units[i].hp + 4).min(max);
            }
        }
    }

    fn acquire_or_validate_target(&mut self, i: usize) {
        let current = self.units[i].target;
        if let Some(t) = current {
            if self.valid_target(i, t) {
                return;
            }
            self.units[i].target = None;
        }
        let owner = self.units[i].owner;
        let x = self.units[i].x;
        let y = self.units[i].y;
        let ty = self.data.unit(self.units[i].type_id).clone();
        let ground = self.data.ground_weapon(&ty);
        let air = self.data.air_weapon(&ty);
        let max_dist = self.data.target_acquisition_px(&ty) * FP;
        let mut best: Option<(i32, usize)> = None;
        for (j, other) in self.units.iter().enumerate() {
            if !other.alive || other.owner == owner || j == i {
                continue;
            }
            let other_ty = self.data.unit(other.type_id);
            let weapon = if other_ty.is_flyer() { air } else { ground };
            let Some(weapon) = weapon else { continue };
            if !weapon_can_target(weapon, other_ty) {
                continue;
            }
            let d = isqrt(((x - other.x) as i64).pow(2) + ((y - other.y) as i64).pow(2)) as i32;
            if d <= max_dist && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, j));
            }
        }
        self.units[i].target = best.map(|(_, j)| j);
    }

    fn valid_target(&self, i: usize, target: usize) -> bool {
        let u = &self.units[i];
        let t = &self.units[target];
        if !t.alive || t.owner == u.owner {
            return false;
        }
        let ty = self.data.unit(u.type_id);
        let tty = self.data.unit(t.type_id);
        let weapon = if tty.is_flyer() {
            self.data.air_weapon(ty)
        } else {
            self.data.ground_weapon(ty)
        };
        weapon.is_some_and(|w| weapon_can_target(w, tty))
    }

    fn move_or_attack(&mut self, i: usize) {
        let Some(target_index) = self.units[i].target else {
            return;
        };
        let u = &self.units[i];
        let t = &self.units[target_index];
        let dist = u.distance_to(t);
        let u_ty = self.data.unit(u.type_id);
        let t_ty = self.data.unit(t.type_id);
        let weapon = if t_ty.is_flyer() {
            self.data.air_weapon(u_ty)
        } else {
            self.data.ground_weapon(u_ty)
        };
        let Some(weapon) = weapon else { return };
        if !weapon_can_target(weapon, t_ty) {
            return;
        }
        let range = weapon.max_range as i32 * FP;
        let min_range = weapon.min_range as i32 * FP;
        if dist <= range && dist >= min_range {
            if self.units[i].cooldown == 0 {
                self.fire(i, target_index, weapon.clone());
            }
        } else {
            self.move_toward(i, target_index, range);
        }
    }

    fn move_toward(&mut self, i: usize, target: usize, range: i32) {
        let ty = self.data.unit(self.units[i].type_id).clone();
        let speed = effective_speed(&self.data, &ty);
        if speed <= 0 {
            return;
        }
        let dx = self.units[target].x - self.units[i].x;
        let dy = self.units[target].y - self.units[i].y;
        let dist = self.units[i].distance_to(&self.units[target]);
        let desired = (dist - range).max(0);
        if desired == 0 {
            return;
        }
        let step = speed.min(desired);
        let denom = dist.max(1) as i64;
        self.units[i].x += ((dx as i64 * step as i64) / denom) as i32;
        self.units[i].y += ((dy as i64 * step as i64) / denom) as i32;
    }

    fn fire(&mut self, attacker: usize, target: usize, weapon: WeaponType) {
        // A damage factor of 2 means two shots per attack.  Other values only
        // affect the displayed damage.
        let shots = if weapon.bullet_count == 2 { 2 } else { 1 };
        for _ in 0..shots {
            self.apply_hit(attacker, target, &weapon, 1);
        }

        // Splash: radial (2), enemy-only radial (3), and air splash (24).
        let splash = matches!(weapon.explosion, 2 | 3 | 24) && weapon.outer_splash > 0;
        if splash {
            let impact = (self.units[target].x, self.units[target].y);
            let inner = weapon.inner_splash as i64 * FP as i64;
            let middle = weapon.middle_splash as i64 * FP as i64;
            let outer = weapon.outer_splash as i64 * FP as i64;
            let attacker_owner = self.units[attacker].owner;
            for j in 0..self.units.len() {
                if j == target || !self.units[j].alive {
                    continue;
                }
                if weapon.explosion == 3 && self.units[j].owner == attacker_owner {
                    continue;
                }
                let dx = (self.units[j].x - impact.0) as i64;
                let dy = (self.units[j].y - impact.1) as i64;
                let d = isqrt(dx * dx + dy * dy);
                let divisor = if d <= inner {
                    1
                } else if d <= middle {
                    2
                } else if d <= outer {
                    4
                } else {
                    continue;
                };
                // Splash is not applied again to the primary target.  The
                // attacker's own units can be damaged by radial splash.
                self.apply_hit(attacker, j, &weapon, divisor);
            }
        }

        self.units[attacker].cooldown = weapon.cooldown.max(1) as u32;
    }

    fn apply_hit(&mut self, attacker: usize, target: usize, weapon: &WeaponType, divisor: u32) {
        let t_owner = self.units[target].owner;
        let t_ty = self.data.unit(self.units[target].type_id).clone();
        let a_up =
            self.upgrades[self.units[attacker].owner as usize].weapon_level(weapon.damage_upgrade);
        let t_armor_up = self.upgrades[t_owner as usize].armor_level(t_ty.armor_upgrade) as u32;
        let defense = Defense {
            size: t_ty.size,
            armor_level: t_ty.armor as u32 + t_armor_up,
            has_shields: t_ty.has_shields,
            shield_points: self.units[target].shields,
            shield_upgrade: self.upgrades[t_owner as usize].plasma_shields,
        };
        let result = resolve_damage_divisor(weapon, a_up, defense, divisor);
        if result.shield_damage > 0 {
            self.units[target].shields = (self.units[target].shields - result.shield_damage).max(0);
        }
        if result.hp_damage > 0 {
            self.units[target].hp -= result.hp_damage;
            if self.units[target].hp <= 0 {
                self.units[target].alive = false;
            }
        }
    }

    pub fn run_until(&mut self, max_ticks: u64) {
        for _ in 0..max_ticks {
            self.step();
            if self.units.iter().any(|u| u.alive && u.owner == 0)
                && !self.units.iter().any(|u| u.alive && u.owner == 1)
            {
                break;
            }
            if self.units.iter().any(|u| u.alive && u.owner == 1)
                && !self.units.iter().any(|u| u.alive && u.owner == 0)
            {
                break;
            }
        }
    }
}

/// Convenience helper used by tests and examples: spawn two units and run a
/// duel for a bounded number of ticks.
pub fn duel(data: GameData, left: u16, right: u16, max_ticks: u64) -> (CombatSim, Option<u8>) {
    let mut sim = CombatSim::new(data);
    // Spawn in weapon range.  Movement (especially Iscript-driven ground
    // movement) is not part of this slice.
    sim.spawn(0, left, 0, 0);
    sim.spawn(1, right, 8 * FP, 0);
    for _ in 0..max_ticks {
        sim.step();
        let l = sim.living(0).count();
        let r = sim.living(1).count();
        if l == 0 && r == 0 {
            return (sim, None);
        }
        if l == 0 {
            return (sim, Some(1));
        }
        if r == 0 {
            return (sim, Some(0));
        }
    }
    (sim, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{DamageType, GameData, UnitSize, unit_ids, weapon_ids};

    fn data() -> GameData {
        GameData::bundled().unwrap()
    }

    fn defense(size: UnitSize, armor: u32) -> Defense {
        Defense {
            size,
            armor_level: armor,
            has_shields: false,
            shield_points: 0,
            shield_upgrade: 0,
        }
    }

    #[test]
    fn gauss_rifle_does_six_to_unarmoured_small_target() {
        let g = data();
        let w = g.weapon(weapon_ids::GAUSS_RIFLE).unwrap();
        let r = resolve_damage(w, 0, defense(UnitSize::Small, 0));
        assert_eq!(r.hp_damage, fp(6));
        assert_eq!(r.shield_damage, 0);
    }

    #[test]
    fn armour_subtracts_before_size_scaling() {
        let g = data();
        let w = g.weapon(weapon_ids::GAUSS_RIFLE).unwrap();
        // Marine rifle: 6 - 1 armour = 5.
        let r = resolve_damage(w, 0, defense(UnitSize::Medium, 1));
        assert_eq!(r.hp_damage, fp(5));
        // +1 weapons: 7 - 1 = 6.
        let r = resolve_damage(w, 1, defense(UnitSize::Medium, 1));
        assert_eq!(r.hp_damage, fp(6));
    }

    #[test]
    fn explosive_damage_is_scaled_after_armour() {
        let g = data();
        // Siege Tank Arclite Shock Cannon: 70 explosive.
        let w = g.weapon(weapon_ids::SIEGE_MODE_SHOCK_CANNON).unwrap();
        assert_eq!(w.damage_type, DamageType::Explosive);
        let small = resolve_damage(w, 0, defense(UnitSize::Small, 0));
        let medium = resolve_damage(w, 0, defense(UnitSize::Medium, 0));
        let large = resolve_damage(w, 0, defense(UnitSize::Large, 0));
        assert_eq!(small.hp_damage, fp(35));
        assert_eq!(medium.hp_damage, fp(52) + 128);
        assert_eq!(large.hp_damage, fp(70));
        // Armour is subtracted before the multiplier.
        let armoured = resolve_damage(w, 0, defense(UnitSize::Small, 3));
        assert_eq!(armoured.hp_damage, (fp(70) - fp(3)) * 128 / FP);
    }

    #[test]
    fn concussive_damage_is_quartered_against_large_units() {
        let g = data();
        // Firebat Flame Thrower: 8 concussive.
        let w = g.weapon(weapon_ids::FLAME_THROWER).unwrap();
        assert_eq!(w.damage_type, DamageType::Concussive);
        let large = resolve_damage(w, 0, defense(UnitSize::Large, 0));
        assert_eq!(large.hp_damage, fp(2));
    }

    #[test]
    fn damage_floor_is_half_a_point() {
        let g = data();
        let w = g.weapon(weapon_ids::GAUSS_RIFLE).unwrap();
        let r = resolve_damage(w, 0, defense(UnitSize::Small, 99));
        assert_eq!(r.hp_damage, MIN_DAMAGE);
    }

    #[test]
    fn shields_take_unscaled_damage_before_armour() {
        let g = data();
        let w = g.weapon(weapon_ids::GAUSS_RIFLE).unwrap();
        let d = Defense {
            size: UnitSize::Small,
            armor_level: 99,
            has_shields: true,
            shield_points: fp(60),
            shield_upgrade: 0,
        };
        let r = resolve_damage(w, 0, d);
        // Six shield damage; the huge armour never applies.
        assert_eq!(r.shield_damage, fp(6));
        assert_eq!(r.hp_damage, 0);
    }

    #[test]
    fn shield_upgrade_reduces_shield_damage() {
        let g = data();
        let w = g.weapon(weapon_ids::GAUSS_RIFLE).unwrap();
        let d = Defense {
            size: UnitSize::Small,
            armor_level: 0,
            has_shields: true,
            shield_points: fp(60),
            shield_upgrade: 2,
        };
        let r = resolve_damage(w, 0, d);
        assert_eq!(r.shield_damage, fp(4));
    }

    #[test]
    fn ignore_armor_skips_armour_and_shield_upgrade() {
        let g = data();
        // Weapon 34 is the 250-damage ignore-armour weapon.
        let w = g.weapon(34).unwrap();
        assert_eq!(w.damage_type, DamageType::IgnoreArmor);
        let d = Defense {
            size: UnitSize::Small,
            armor_level: 9,
            has_shields: true,
            shield_points: fp(10),
            shield_upgrade: 3,
        };
        let r = resolve_damage(w, 0, d);
        // Shields absorb 10; neither the shield upgrade nor armour applies.
        assert_eq!(r.shield_damage, fp(10));
        assert_eq!(r.hp_damage, fp(240));
    }

    #[test]
    fn shields_below_one_point_do_not_absorb() {
        let g = data();
        let w = g.weapon(weapon_ids::GAUSS_RIFLE).unwrap();
        let d = Defense {
            size: UnitSize::Small,
            armor_level: 0,
            has_shields: true,
            shield_points: 128,
            shield_upgrade: 0,
        };
        let r = resolve_damage(w, 0, d);
        assert_eq!(r.shield_damage, 0);
        assert_eq!(r.hp_damage, fp(6));
    }

    #[test]
    fn shields_at_one_full_point_absorb() {
        let g = data();
        let w = g.weapon(weapon_ids::GAUSS_RIFLE).unwrap();
        let d = Defense {
            size: UnitSize::Small,
            armor_level: 0,
            has_shields: true,
            shield_points: fp(1),
            shield_upgrade: 0,
        };
        let r = resolve_damage(w, 0, d);
        // 6 damage: 1 point to shields, 5 to hit points.
        assert_eq!(r.shield_damage, fp(1));
        assert_eq!(r.hp_damage, fp(5));
    }

    #[test]
    fn overshield_damage_carries_to_hit_points() {
        let g = data();
        let w = g.weapon(weapon_ids::GAUSS_RIFLE).unwrap();
        let d = Defense {
            size: UnitSize::Small,
            armor_level: 0,
            has_shields: true,
            shield_points: fp(4),
            shield_upgrade: 0,
        };
        let r = resolve_damage(w, 0, d);
        assert_eq!(r.shield_damage, fp(4));
        assert_eq!(r.hp_damage, fp(2));
    }

    #[test]
    fn marine_duels_are_deterministic() {
        let (sim, winner) = duel(data(), unit_ids::MARINE, unit_ids::MARINE, 2_000);
        assert!(winner.is_some());
        // Both marines have the same stats; the left one fires first.
        assert_eq!(winner, Some(0));
        assert_eq!(sim.tick, 91);
    }

    #[test]
    fn splash_divisor_is_applied_before_size_scaling() {
        let g = data();
        // Siege Mode Shock Cannon: 70 explosive, splash rings 10/25/40.
        let w = g.weapon(27).unwrap();
        let direct = resolve_damage(w, 0, defense(UnitSize::Small, 0));
        let middle = resolve_damage_divisor(w, 0, defense(UnitSize::Small, 0), 2);
        let outer = resolve_damage_divisor(w, 0, defense(UnitSize::Small, 0), 4);
        assert_eq!(direct.hp_damage, fp(35));
        assert_eq!(middle.hp_damage, fp(70) / 2 * 128 / FP);
        assert_eq!(outer.hp_damage, fp(70) / 4 * 128 / FP);
    }

    #[test]
    fn siege_splash_hits_a_second_marine() {
        let g = data();
        let mut sim = CombatSim::new(g);
        // Unit 31 is the Siege Tank siege-mode turret; it carries weapon 27.
        sim.spawn(0, 31, 0, 0);
        sim.spawn(1, unit_ids::MARINE, 100 * FP, 0);
        sim.spawn(1, unit_ids::MARINE, 120 * FP, 0);
        sim.step();
        // The direct hit does 35 to the primary; the second marine is in the
        // middle splash ring (20 px from impact, ring is 25 px) and takes
        // 17.5.
        assert_eq!(sim.units[1].hp, fp(40) - fp(35));
        assert_eq!(sim.units[2].hp, fp(40) - (fp(70) / 2 * 128 / FP));
    }

    #[test]
    fn size_multipliers_match_the_executables_table() {
        // 100 bytes copied from StarCraft.exe at VA 0xf0757c.
        let raw = embedded!("fixtures/damage_multipliers.bin");
        assert_eq!(raw.len(), 100);
        for damage_type_raw in 0u8..5 {
            let damage_type = DamageType::from_raw(damage_type_raw).unwrap();
            for size_raw in 0u8..4 {
                let size = UnitSize::from_raw(size_raw);
                // The game reads table[1 + type*5 + size] from base 0xf07580;
                // the fixture starts at 0xf0757c, hence the extra +4 bytes.
                let row = damage_type_raw as usize * 5;
                let expected = i32::from_le_bytes([
                    raw[row * 4 + 4 + size_raw as usize * 4],
                    raw[row * 4 + 5 + size_raw as usize * 4],
                    raw[row * 4 + 6 + size_raw as usize * 4],
                    raw[row * 4 + 7 + size_raw as usize * 4],
                ]);
                assert_eq!(
                    size_multiplier(damage_type, size),
                    expected,
                    "type {damage_type:?} size {size:?}"
                );
            }
        }
    }

    #[test]
    fn independent_size_takes_only_the_half_point_floor() {
        let g = data();
        let w = g.weapon(weapon_ids::SIEGE_MODE_SHOCK_CANNON).unwrap();
        let r = resolve_damage(w, 0, defense(UnitSize::None, 0));
        assert_eq!(r.hp_damage, MIN_DAMAGE);
    }

    #[test]
    fn zealot_has_shields_and_small_size() {
        let g = data();
        let u = g.unit(unit_ids::ZEALOT);
        assert!(u.has_shields);
        assert_eq!(u.shield_points, 60);
        assert_eq!(u.size, UnitSize::Small);
    }
}

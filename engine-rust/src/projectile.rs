//! Travelling bullets and missiles.
//!
//! `weapons.dat` has a `bullet_type` column with the ten behaviours used by
//! the retail engine (`fly`, `follow target`, `appear at target unit`,
//! `persist at target pos`, `appear at target pos`, `appear at source unit`,
//! `self destruct`, `bounce`, `attack target pos`, `extend to max range`).
//! It also has a separate `hit_type` column with the effect applied on impact
//! (normal, radial splash, enemy splash, lockdown, nuclear, parasite, …).
//!
//! The previous engine applied every shot at fire time.  That is correct for
//! appearance/instant weapons, but it erased travel time, bullet lifetime,
//! homing, bouncing and the distinction between the two splash flavours.  This
//! module stores the travelling state; `World` owns the damage pipeline and
//! resolves an impact when the bullet arrives.

use crate::data::WeaponType;

/// `weapons.dat` bullet_type values.
pub mod behavior {
    pub const FLY: u8 = 0;
    pub const FOLLOW_TARGET: u8 = 1;
    pub const APPEAR_AT_TARGET_UNIT: u8 = 2;
    pub const PERSIST_AT_TARGET_POS: u8 = 3;
    pub const APPEAR_AT_TARGET_POS: u8 = 4;
    pub const APPEAR_AT_SOURCE_UNIT: u8 = 5;
    pub const SELF_DESTRUCT: u8 = 6;
    pub const BOUNCE: u8 = 7;
    pub const ATTACK_TARGET_POS: u8 = 8;
    pub const EXTEND_TO_MAX_RANGE: u8 = 9;
}

/// `weapons.dat` hit_type values.
pub mod hit {
    pub const NONE: u8 = 0;
    pub const NORMAL: u8 = 1;
    pub const RADIAL_SPLASH: u8 = 2;
    pub const ENEMY_SPLASH: u8 = 3;
    pub const LOCKDOWN: u8 = 4;
    pub const NUCLEAR: u8 = 5;
    pub const PARASITE: u8 = 6;
    pub const BROODLINGS: u8 = 7;
    pub const EMP: u8 = 8;
    pub const IRRADIATE: u8 = 9;
    pub const ENSNARE: u8 = 10;
    pub const PLAGUE: u8 = 11;
    pub const STASIS: u8 = 12;
    pub const DARK_SWARM: u8 = 13;
    pub const CONSUME: u8 = 14;
    pub const YAMATO: u8 = 15;
    pub const RESTORATION: u8 = 16;
    pub const DISRUPTION_WEB: u8 = 17;
    pub const CORROSIVE_ACID: u8 = 18;
    pub const MIND_CONTROL: u8 = 19;
    pub const FEEDBACK: u8 = 20;
    pub const OPTICAL_FLARE: u8 = 21;
    pub const MAELSTROM: u8 = 22;
    pub const UNUSED_23: u8 = 23;
    pub const AIR_SPLASH: u8 = 24;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BulletState {
    /// Straight line toward the last known target position.
    Fly,
    /// Re-aims at the target unit every tick.
    Follow,
    /// Re-aims and jumps to another nearby enemy after each hit.
    Bounce,
    /// Travels to a position near the target (Corrosive Acid and friends).
    HitNearTarget,
    /// Travels to the weapon's maximum range along the firing heading.
    Extend,
    /// Stays at the target position and applies its hit every seven ticks.
    Persist,
}

#[derive(Clone, Debug)]
pub struct Projectile {
    pub id: u32,
    pub owner: u8,
    /// Unit that fired; kept for upgrade lookups after it dies.
    pub source: u32,
    pub weapon: u8,
    pub behavior: u8,
    pub hit_type: u8,
    pub state: BulletState,
    /// Position in 1/256 pixels.
    pub x: i32,
    pub y: i32,
    pub target: Option<u32>,
    /// Target position in 1/256 pixels.
    pub target_pos: (i32, i32),
    /// Speed in 1/256 pixels per tick.
    pub speed: i32,
    /// Remaining lifetime in ticks.
    pub remaining_time: i32,
    /// How many bounces are left (Glave Wurm starts at three).
    pub bounces_left: u8,
    /// Bit 0: the shot missed and must not deal damage.  Bit 1: fired by a
    /// hallucination, so it does no real damage.  Bit 2: splash already
    /// touched this target (used to avoid duplicate DoT ticks).
    pub hit_flags: u8,
    /// Previous bounce target, used to avoid bouncing back immediately.
    pub previous_target: Option<u32>,
    /// Tick when a persistent damage-over-time bullet last applied damage.
    pub last_dot_tick: i64,
}

impl Projectile {
    pub fn weapon_is_travelling(weapon: &WeaponType) -> bool {
        matches!(
            weapon.bullet_type,
            behavior::FLY
                | behavior::FOLLOW_TARGET
                | behavior::PERSIST_AT_TARGET_POS
                | behavior::BOUNCE
                | behavior::ATTACK_TARGET_POS
                | behavior::EXTEND_TO_MAX_RANGE
        )
    }

    /// The divisor applied to a direct hit.  Glave Wurm/Venom bounce damage is
    /// divided by three for each preceding bounce, matching the retail
    /// `switchD_7a107501::caseD_0` loop at `0x7a107501`.
    pub fn bounce_divisor(&self) -> u32 {
        let bounces = 3u32.saturating_sub(self.bounces_left as u32);
        3u32.saturating_pow(bounces)
    }
}

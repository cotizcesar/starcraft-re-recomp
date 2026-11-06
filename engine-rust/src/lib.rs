//! GalaxyCraft — a Rust reconstruction of StarCraft 1.23 gameplay.
//!
//! The rules core is deliberately small so it can be audited against the
//! recovered tables and the executable evidence.  [`retail`] reads the
//! player's own CASC/MPQ installation at runtime, so the retail art, audio,
//! maps and tables never need to be redistributed.
//!
//! * [`data`] parses the shipped column-major DAT tables.
//! * [`combat`] implements damage, shields, cooldowns and a deterministic
//!   fixed-tick battle loop.
//! * [`economy`] implements costs, supply, production and mining.
//!
//! See `docs/EVIDENCE.md` for the REA evidence, addresses and decompiled
//! routines that back the rules.

/// Bytes of a retail-derived data file (crate-relative path), mirrored by
/// `build.rs` into `$OUT_DIR/embed/`; a placeholder when the file is absent
/// (see `fixtures/README.md`).
macro_rules! embedded {
    ($rel:literal) => {
        include_bytes!(concat!(env!("OUT_DIR"), "/embed/", $rel))
    };
}

pub mod ai;
pub mod aiscript;
pub mod audio;
pub mod button_sets;
pub mod buttons;
pub mod calibration;
pub mod campaign;
pub mod campaign_progress;
pub mod cinematic;
pub mod combat;
pub mod contour;
pub mod credits;
pub mod data;
pub mod dds;
pub mod economy;
#[rustfmt::skip]
pub mod handlers;
pub mod hd_anim;
pub mod hd_lights;
pub mod hd_sprites;
pub mod hotkey_data;
pub mod ids;
pub mod iscript;
pub mod map;
pub mod menu;
pub mod menu_masks;
pub mod multiplayer;
pub mod net;
pub mod orders;
pub mod path;
pub mod portrait;
pub mod profile;
pub mod projectile;
pub mod ready_room;
pub mod render;
pub mod replay;
pub mod retail;
pub mod retail_replay;
pub mod scene;
pub mod sfx;
pub mod sim;
pub mod tables;
pub mod tech;
pub mod text;
pub mod transmission;
pub mod trigger;
pub mod ui;
pub mod ui_art;
pub mod video;
pub mod vision;
pub mod voice;

pub use combat::{
    CombatSim, DamageResult, Defense, PlayerUpgrades, Unit, duel, resolve_damage,
    resolve_damage_divisor,
};
pub use data::{GameData, UnitSize, UnitType, WeaponType};
pub use economy::{MiningSim, PlayerEconomy, ProductionOrder, ResourcePatch};
pub use orders::{ORDER_SPECS, OrderClass, OrderLane, OrderSpec, order_class};

/// Load the tables bundled with the crate.  These were extracted from the
/// retail CASC storage of StarCraft 1.23.10.13515.
pub fn bundled_data() -> GameData {
    GameData::bundled().expect("bundled StarCraft tables are valid")
}

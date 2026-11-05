//! Differential tests against an independent oracle built from the original
//! `units.dat` and `weapons.dat` bytes extracted from the retail CASC
//! storage.
//!
//! The oracle (`tools/oracle.py`) is deliberately a separate implementation.
//! It reads the same bytes with its own offsets and its own damage routine;
//! these tests compare every unit and weapon row plus 74,880 damage cases (every weapon, upgrade, size, armour and shield state).

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use galaxycraft::combat::{Defense, resolve_damage};
use galaxycraft::data::{DamageType, GameData, UnitSize};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

fn read_tsv(name: &str) -> Vec<HashMap<String, String>> {
    let text = fs::read_to_string(fixtures().join(name)).expect("golden file exists");
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().expect("header").split('\t').collect();
    lines
        .filter(|l| !l.is_empty())
        .map(|line| {
            let values: Vec<&str> = line.split('\t').collect();
            header
                .iter()
                .zip(values)
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        })
        .collect()
}

fn num<T: std::str::FromStr>(row: &HashMap<String, String>, key: &str) -> T {
    row[key]
        .parse()
        .unwrap_or_else(|_| panic!("bad {key}: {}", row[key]))
}

#[test]
fn every_unit_matches_the_independent_oracle() {
    let data = GameData::bundled().unwrap();
    let rows = read_tsv("oracle_units.tsv");
    assert_eq!(rows.len(), 228);
    for row in rows {
        let id: usize = num(&row, "id");
        let u = &data.units[id];
        assert_eq!(u.flingy, num::<u16>(&row, "flingy"), "unit {id} flingy");
        assert_eq!(u.turret, num::<u16>(&row, "turret"), "unit {id} turret");
        assert_eq!(u.hit_points_raw, num::<u32>(&row, "hp"), "unit {id} hp");
        assert_eq!(
            u.shield_points,
            num::<u16>(&row, "shield"),
            "unit {id} shield"
        );
        assert_eq!(
            u.has_shields,
            num::<u8>(&row, "has_shield") != 0,
            "unit {id} has_shield"
        );
        assert_eq!(
            u.elevation,
            num::<u8>(&row, "elevation"),
            "unit {id} elevation"
        );
        assert_eq!(u.armor, num::<u8>(&row, "armor"), "unit {id} armor");
        assert_eq!(
            u.armor_upgrade,
            num::<u8>(&row, "armor_upgrade"),
            "unit {id} armor upgrade"
        );
        assert_eq!(
            u.size,
            UnitSize::from_raw(num::<u8>(&row, "size")),
            "unit {id} size"
        );
        assert_eq!(
            u.ground_weapon,
            num::<u8>(&row, "ground_weapon"),
            "unit {id} ground weapon"
        );
        assert_eq!(
            u.max_ground_hits,
            num::<u8>(&row, "max_ground_hits"),
            "unit {id} max ground hits"
        );
        assert_eq!(
            u.air_weapon,
            num::<u8>(&row, "air_weapon"),
            "unit {id} air weapon"
        );
        assert_eq!(
            u.max_air_hits,
            num::<u8>(&row, "max_air_hits"),
            "unit {id} max air hits"
        );
        assert_eq!(u.sight_range, num::<u8>(&row, "sight"), "unit {id} sight");
        assert_eq!(
            u.target_acquisition_range,
            num::<u8>(&row, "target_acq"),
            "unit {id} target acq"
        );
        assert_eq!(
            u.portrait,
            num::<u16>(&row, "portrait"),
            "unit {id} portrait"
        );
        assert_eq!(
            u.mineral_cost,
            num::<u16>(&row, "mineral"),
            "unit {id} mineral"
        );
        assert_eq!(u.gas_cost, num::<u16>(&row, "gas"), "unit {id} gas");
        assert_eq!(
            u.build_time,
            num::<u16>(&row, "build_time"),
            "unit {id} build time"
        );
        assert_eq!(
            u.group_flags,
            num::<u8>(&row, "group_flags"),
            "unit {id} group flags"
        );
        assert_eq!(
            u.supply_required,
            num::<u8>(&row, "supply_required"),
            "unit {id} supply"
        );
        assert_eq!(
            u.supply_provided,
            num::<u8>(&row, "supply_provided"),
            "unit {id} supply provided"
        );
        assert_eq!(
            u.space_required,
            num::<u8>(&row, "space_required"),
            "unit {id} space required"
        );
        assert_eq!(
            u.space_provided,
            num::<u8>(&row, "space_provided"),
            "unit {id} space provided"
        );
        assert_eq!(
            u.build_score,
            num::<u16>(&row, "build_score"),
            "unit {id} build score"
        );
        assert_eq!(
            u.destroy_score,
            num::<u16>(&row, "destroy_score"),
            "unit {id} destroy score"
        );
        assert_eq!(u.flags, num::<u32>(&row, "flags"), "unit {id} flags");
    }
}

#[test]
fn every_weapon_matches_the_independent_oracle() {
    let data = GameData::bundled().unwrap();
    let rows = read_tsv("oracle_weapons.tsv");
    assert_eq!(rows.len(), 130);
    for row in rows {
        let id: usize = num(&row, "id");
        let w = &data.weapons[id];
        assert_eq!(w.label, num::<u16>(&row, "label"), "weapon {id} label");
        assert_eq!(w.flingy, num::<u32>(&row, "flingy"), "weapon {id} flingy");
        assert_eq!(w.damage, num::<u16>(&row, "damage"), "weapon {id} damage");
        assert_eq!(
            w.damage_bonus,
            num::<u16>(&row, "damage_bonus"),
            "weapon {id} bonus"
        );
        assert_eq!(
            w.damage_type,
            DamageType::from_raw(num::<u8>(&row, "damage_type")).unwrap(),
            "weapon {id} type"
        );
        assert_eq!(
            w.damage_upgrade,
            num::<u8>(&row, "damage_upgrade"),
            "weapon {id} damage upgrade"
        );
        assert_eq!(
            w.cooldown,
            num::<u8>(&row, "cooldown"),
            "weapon {id} cooldown"
        );
        assert_eq!(
            w.min_range,
            num::<u32>(&row, "min_range"),
            "weapon {id} min"
        );
        assert_eq!(
            w.max_range,
            num::<u32>(&row, "max_range"),
            "weapon {id} max"
        );
        assert_eq!(
            w.target_flags,
            num::<u16>(&row, "target_flags"),
            "weapon {id} flags"
        );
        assert_eq!(
            w.bullet_count,
            num::<u8>(&row, "bullets"),
            "weapon {id} bullets"
        );
        assert_eq!(
            w.bullet_type,
            num::<u8>(&row, "bullet_type"),
            "weapon {id} bullet type"
        );
        assert_eq!(
            w.lifetime,
            num::<u8>(&row, "lifetime"),
            "weapon {id} lifetime"
        );
        assert_eq!(
            w.explosion,
            num::<u8>(&row, "explosion"),
            "weapon {id} explosion"
        );
        assert_eq!(
            w.inner_splash,
            num::<u16>(&row, "inner_splash"),
            "weapon {id} inner splash"
        );
        assert_eq!(
            w.middle_splash,
            num::<u16>(&row, "middle_splash"),
            "weapon {id} middle splash"
        );
        assert_eq!(
            w.outer_splash,
            num::<u16>(&row, "outer_splash"),
            "weapon {id} outer splash"
        );
        assert_eq!(
            w.attack_angle,
            num::<u8>(&row, "attack_angle"),
            "weapon {id} attack angle"
        );
        assert_eq!(
            w.launch_spin as u8,
            num::<u8>(&row, "launch_spin"),
            "weapon {id} launch spin"
        );
        assert_eq!(
            w.forward_offset as u8,
            num::<u8>(&row, "forward_offset"),
            "weapon {id} forward offset"
        );
        assert_eq!(
            w.vertical_offset as u8,
            num::<u8>(&row, "vertical_offset"),
            "weapon {id} vertical offset"
        );
        assert_eq!(
            w.target_error,
            num::<u16>(&row, "target_error"),
            "weapon {id} target error"
        );
        assert_eq!(w.icon, num::<u16>(&row, "icon"), "weapon {id} icon");
    }
}

#[test]
fn every_flingy_matches_the_independent_oracle() {
    let data = GameData::bundled().unwrap();
    let rows = read_tsv("oracle_flingy.tsv");
    assert_eq!(rows.len(), 209);
    for row in rows {
        let id: usize = num(&row, "id");
        let f = &data.flingies[id];
        assert_eq!(f.sprite, num::<u16>(&row, "sprite"), "flingy {id} sprite");
        assert_eq!(
            f.top_speed,
            num::<i32>(&row, "top_speed"),
            "flingy {id} top speed"
        );
        assert_eq!(
            f.acceleration,
            num::<i16>(&row, "acceleration"),
            "flingy {id} acceleration"
        );
        assert_eq!(
            f.halt_distance,
            num::<i32>(&row, "halt_distance"),
            "flingy {id} halt distance"
        );
        assert_eq!(
            f.turn_rate,
            num::<u8>(&row, "turn_rate"),
            "flingy {id} turn rate"
        );
        let movement_raw = match f.movement_type {
            galaxycraft::data::MovementType::Flingy => 0,
            galaxycraft::data::MovementType::FlingyNoDeceleration => 1,
            galaxycraft::data::MovementType::Iscript => 2,
            galaxycraft::data::MovementType::Unknown(v) => v,
        };
        assert_eq!(
            movement_raw,
            num::<u8>(&row, "movement_type"),
            "flingy {id} movement type"
        );
    }
}

#[test]
fn every_order_row_matches_the_independent_oracle() {
    let data = GameData::bundled().unwrap();
    let rows = read_tsv("oracle_orders.tsv");
    assert_eq!(rows.len(), 189);
    for row in rows {
        let id: usize = num(&row, "id");
        let o = &data.orders[id];
        assert_eq!(o.label, num::<u16>(&row, "label"), "order {id} label");
        assert_eq!(
            o.highlight,
            num::<i16>(&row, "highlight"),
            "order {id} highlight"
        );
        assert_eq!(
            o.dep_index,
            num::<u16>(&row, "dependency"),
            "order {id} dependency"
        );
        assert_eq!(
            o.target_order,
            num::<u8>(&row, "target_order"),
            "order {id} target order"
        );
        assert_eq!(
            o.targets_enemies,
            num::<u8>(&row, "flag0") != 0,
            "order {id} flag0"
        );
        assert_eq!(
            o.background,
            num::<u8>(&row, "flag1") != 0,
            "order {id} flag1"
        );
        assert_eq!(
            o.valid_for_turret,
            num::<u8>(&row, "flag3") != 0,
            "order {id} flag3"
        );
        assert_eq!(
            o.can_be_interrupted,
            num::<u8>(&row, "flag5") != 0,
            "order {id} flag5"
        );
        assert_eq!(
            o.can_be_queued,
            num::<u8>(&row, "flag7") != 0,
            "order {id} flag7"
        );
        assert_eq!(
            o.can_be_obstructed,
            num::<u8>(&row, "flag9") != 0,
            "order {id} flag9"
        );
        assert_eq!(o.weapon, num::<u8>(&row, "flag12"), "order {id} weapon");
        assert_eq!(o.tech_type, num::<u8>(&row, "flag13"), "order {id} tech");
        assert_eq!(
            o.animation,
            num::<u8>(&row, "flag14"),
            "order {id} animation"
        );
    }
}

#[test]
fn damage_matrix_matches_the_independent_oracle() {
    let data = GameData::bundled().unwrap();
    let rows = read_tsv("oracle_damage.tsv");
    assert_eq!(rows.len(), 74_880);
    for row in rows {
        let wid: u8 = num(&row, "weapon");
        let weapon = data.weapon(wid).unwrap();
        let defense = Defense {
            size: match num::<u8>(&row, "size") {
                1 => UnitSize::Small,
                2 => UnitSize::Medium,
                3 => UnitSize::Large,
                other => panic!("bad size {other}"),
            },
            armor_level: num::<u32>(&row, "armor"),
            has_shields: true,
            shield_points: num::<i32>(&row, "shield_points"),
            shield_upgrade: num::<u8>(&row, "shield_upgrade"),
        };
        let result = resolve_damage(weapon, num::<u8>(&row, "upgrade"), defense);
        assert_eq!(
            result.shield_damage,
            num::<i32>(&row, "shield_damage"),
            "weapon {wid} upgrade {} size {} armor {} shields {} shield_up {}",
            row["upgrade"],
            row["size"],
            row["armor"],
            row["shield_points"],
            row["shield_upgrade"]
        );
        assert_eq!(
            result.hp_damage,
            num::<i32>(&row, "hp_damage"),
            "weapon {wid} upgrade {} size {} armor {} shields {} shield_up {}",
            row["upgrade"],
            row["size"],
            row["armor"],
            row["shield_points"],
            row["shield_upgrade"]
        );
    }
}

#[test]
fn damage_table_matches_the_original_executable_when_available() {
    // The 25-int damage multiplier table is at file offset 0xb05d7c in the
    // 1.23.10.13515 x86 executable.
    let Ok(exe) = std::env::var("SC_EXE") else {
        return;
    };
    let bytes = fs::read(&exe).expect("original executable");
    let original = &bytes[0xb05d7c..0xb05d7c + 100];
    let fixture = fs::read(fixtures().join("damage_multipliers.bin")).unwrap();
    assert_eq!(original, fixture, "damage table differs from {exe}");
}

#[test]
fn fixtures_are_the_shipped_bytes_when_an_extracted_copy_is_available() {
    // Set SC_DATA_DIR to a directory containing units.dat, weapons.dat and
    // flingy.dat extracted from the installed game to re-verify provenance.
    let Ok(dir) = std::env::var("SC_DATA_DIR") else {
        return;
    };
    let arr = PathBuf::from(&dir);
    let root = arr.parent().unwrap_or(&arr);
    let candidates: [(&str, PathBuf); 12] = [
        ("units.dat", arr.join("units.dat")),
        ("weapons.dat", arr.join("weapons.dat")),
        ("flingy.dat", arr.join("flingy.dat")),
        ("orders.dat", arr.join("orders.dat")),
        ("techdata.dat", arr.join("techdata.dat")),
        ("upgrades.dat", arr.join("upgrades.dat")),
        ("sprites.dat", arr.join("sprites.dat")),
        ("images.dat", arr.join("images.dat")),
        ("iscript.bin", root.join("scripts/iscript.bin")),
        ("aiscript.bin", root.join("scripts/aiscript.bin")),
        ("bwscript.bin", root.join("scripts/bwscript.bin")),
        ("stat_txt.tbl", root.join("rez/stat_txt.tbl")),
    ];
    let mut checked = 0usize;
    for (name, original_path) in candidates {
        if !original_path.exists() {
            continue;
        }
        let original = fs::read(&original_path).expect("original table");
        let fixture = fs::read(fixtures().join(name)).expect("fixture table");
        assert_eq!(
            original,
            fixture,
            "fixture differs from {}",
            original_path.display()
        );
        checked += 1;
    }
    assert!(checked >= 3, "SC_DATA_DIR must provide the shipped tables");
}

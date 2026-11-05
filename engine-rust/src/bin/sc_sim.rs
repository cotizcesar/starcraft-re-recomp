//! Small command-line demonstration of the reconstructed rules.
//!
//! Examples:
//!   cargo run --bin sc_sim
//!   cargo run --bin sc_sim -- duel marine zealot
//!   cargo run --bin sc_sim -- stats marine
//!   cargo run --bin sc_sim -- battle 3 marine 1 zealot

use galaxycraft::combat::{CombatSim, fp};
use galaxycraft::data::{GameData, unit_ids};
use galaxycraft::economy::{build_seconds, supply_of};

fn unit_id(name: &str) -> Option<u16> {
    Some(match name {
        "marine" => unit_ids::MARINE,
        "ghost" => unit_ids::GHOST,
        "vulture" => unit_ids::VULTURE,
        "goliath" => unit_ids::GOLIATH,
        "tank" | "siege_tank" => unit_ids::SIEGE_TANK,
        "scv" => unit_ids::SCV,
        "firebat" => unit_ids::FIREBAT,
        "zergling" => unit_ids::ZERGLING,
        "hydralisk" => unit_ids::HYDRALISK,
        "ultralisk" => unit_ids::ULTRALISK,
        "mutalisk" => unit_ids::MUTALISK,
        "probe" => unit_ids::PROBE,
        "zealot" => unit_ids::ZEALOT,
        "dragoon" => unit_ids::DRAGOON,
        "archon" => unit_ids::ARCHON,
        "scout" => unit_ids::SCOUT,
        "carrier" => unit_ids::CARRIER,
        _ => return None,
    })
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let data = GameData::bundled().expect("bundled tables");
    if args.is_empty() {
        run_duel(&data, unit_ids::MARINE, unit_ids::ZEALOT);
        return;
    }
    match args[0].as_str() {
        "duel" => {
            let a = args
                .get(1)
                .and_then(|s| unit_id(s))
                .unwrap_or(unit_ids::MARINE);
            let b = args
                .get(2)
                .and_then(|s| unit_id(s))
                .unwrap_or(unit_ids::ZEALOT);
            run_duel(&data, a, b);
        }
        "stats" => {
            let id = args
                .get(1)
                .and_then(|s| unit_id(s))
                .unwrap_or(unit_ids::MARINE);
            print_stats(&data, id);
        }
        "battle" => {
            // battle <left-count> <left-unit> <right-count> <right-unit>
            let left_count: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(3);
            let left = args
                .get(2)
                .and_then(|s| unit_id(s))
                .unwrap_or(unit_ids::MARINE);
            let right_count: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
            let right = args
                .get(4)
                .and_then(|s| unit_id(s))
                .unwrap_or(unit_ids::ZEALOT);
            run_battle(&data, left_count, left, right_count, right);
        }
        _ => {
            eprintln!(
                "usage: sc_sim [duel <unit> <unit> | stats <unit> | battle <n> <unit> <n> <unit>]"
            );
            std::process::exit(2);
        }
    }
}

fn run_duel(data: &GameData, a: u16, b: u16) {
    let (sim, winner) = galaxycraft::duel(data.clone(), a, b, 10_000);
    println!(
        "duel: {} vs {} -> winner {:?} after {} ticks",
        unit_label(data, a),
        unit_label(data, b),
        winner,
        sim.tick
    );
    for u in &sim.units {
        let ty = data.unit(u.type_id);
        println!(
            "  unit {} ({}) hp={}/{} shields={} alive={}",
            u.id,
            unit_label(data, u.type_id),
            u.hp.max(0) as f32 / 256.0,
            ty.hit_points(),
            u.shields.max(0) as f32 / 256.0,
            u.alive
        );
    }
}

fn run_battle(data: &GameData, left_count: usize, left: u16, right_count: usize, right: u16) {
    let mut sim = CombatSim::new(data.clone());
    // Spawn two tiles apart.  Pathfinding and Iscript animation are outside
    // this reconstruction; melee units close with the documented fallback.
    for i in 0..left_count {
        sim.spawn(0, left, 0, (i as i32 - left_count as i32 / 2) * 32 * fp(1));
    }
    for i in 0..right_count {
        sim.spawn(
            1,
            right,
            64 * fp(1),
            (i as i32 - right_count as i32 / 2) * 32 * fp(1),
        );
    }
    sim.run_until(20_000);
    let l = sim.living(0).count();
    let r = sim.living(1).count();
    println!(
        "battle: {} x {} vs {} x {} -> {} vs {} survivors after {} ticks",
        left_count,
        unit_label(data, left),
        right_count,
        unit_label(data, right),
        l,
        r,
        sim.tick
    );
    for owner in 0..=1 {
        for u in sim.units.iter().filter(|u| u.alive && u.owner == owner) {
            let ty = data.unit(u.type_id);
            println!(
                "  p{} {} hp={}/{}",
                owner,
                unit_label(data, u.type_id),
                u.hp.max(0) as f32 / 256.0,
                ty.hit_points()
            );
        }
    }
}

fn print_stats(data: &GameData, id: u16) {
    let u = data.unit(id);
    println!("unit {} ({})", id, unit_label(data, id));
    println!(
        "  hit points: {} ({} raw)",
        u.hit_points(),
        u.hit_points_raw
    );
    println!(
        "  shields: {} (has_shields={})",
        u.shield_points, u.has_shields
    );
    println!("  armor: {}", u.armor);
    println!("  size: {:?}", u.size);
    println!("  sight range: {} tiles", u.sight_range);
    println!(
        "  cost: {} minerals / {} gas, build {:.2}s, supply {:.1}",
        u.mineral_cost,
        u.gas_cost,
        build_seconds(u.build_time),
        supply_of(data, id)
    );
    for (label, wid) in [("ground", u.ground_weapon), ("air", u.air_weapon)] {
        if let Some(w) = data.weapon(wid) {
            println!(
                "  {label} weapon: id={} damage={}+{}/level type={:?} cooldown={} range={}",
                wid, w.damage, w.damage_bonus, w.damage_type, w.cooldown, w.max_range
            );
        } else {
            println!("  {label} weapon: none");
        }
    }
}

fn unit_label(data: &GameData, id: u16) -> &str {
    let name = data.unit_name(id);
    if name.is_empty() { "Unit" } else { name }
}

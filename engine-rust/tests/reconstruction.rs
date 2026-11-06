//! Reconstruction invariants checked against the recovered tables and the
//! executable's documented behaviour.

use std::sync::Arc;

use galaxycraft::ai::SimpleAi;
use galaxycraft::data::GameData;
use galaxycraft::ids::{tech, unit as uid, upgrade};
use galaxycraft::map::{Scenario, Terrain};
use galaxycraft::sim::{
    MAX_ORDER_QUEUE, Order, Race, SimError, World, build_ticks, research_ticks, upgrade_ticks,
};
use galaxycraft::tech as tree;
use galaxycraft::trigger::TriggerSystem;

fn data() -> GameData {
    GameData::bundled().expect("bundled tables")
}

#[test]
fn all_recovered_tables_have_the_shipped_counts() {
    let g = data();
    assert!(g.has_full_tables());
    assert_eq!(g.table_counts(), [228, 130, 209, 189, 44, 61, 517, 999]);
    assert_eq!(g.strings.len(), 1539);
    assert_eq!(g.aiscripts.scripts.len(), 293);
    assert_eq!(g.bwscripts.scripts.len(), 156);
    assert!(g.aiscripts.get_str("Ter3").is_some());
    assert!(g.bwscripts.get_str("PB1A").is_some());
}

#[test]
fn weapon_ids_use_the_mechanics_columns_not_the_shifted_labels() {
    let g = data();
    // `weapons.dat` labels are not a reliable id map: row 30 has the Yamato
    // hit type and 260 damage, row 31 the nuclear hit type and 600 damage.
    assert_eq!(g.weapon(30).unwrap().explosion, 15);
    assert_eq!(g.weapon(30).unwrap().damage, 260);
    assert_eq!(g.weapon(31).unwrap().explosion, 5);
    assert_eq!(g.weapon(31).unwrap().damage, 600);
    assert_eq!(g.weapon(33).unwrap().explosion, 8);
    // The unit weapon columns point at the same rows.
    assert_eq!(g.unit(uid::WRAITH).air_weapon, 15);
    assert_eq!(g.unit(uid::BATTLECRUISER).air_weapon, 20);
    assert_eq!(g.unit(uid::LURKER).ground_weapon, 109);
}

#[test]
fn names_come_from_the_shipped_string_table() {
    let g = data();
    assert_eq!(g.unit_name(uid::MARINE), "Terran Marine");
    assert_eq!(g.unit_name(uid::ZEALOT), "Protoss Zealot");
    assert_eq!(g.order_name(15), "Build");
    assert_eq!(g.tech_name(tech::STIM_PACKS), "Stim Packs");
    assert_eq!(
        g.upgrade_name(upgrade::TERRAN_INFANTRY_WEAPONS),
        "Terran Infantry Weapons"
    );
}

#[test]
fn every_order_tech_and_upgrade_label_resolves() {
    let g = data();
    assert_eq!(g.orders.len(), 189);
    for (i, order) in g.orders.iter().enumerate() {
        assert!(
            g.strings.get(order.label).is_some(),
            "order {i} label {} missing",
            order.label
        );
    }
    for (i, tech) in g.techs.iter().enumerate() {
        let label = tech.label.saturating_sub(1);
        assert!(
            g.strings.get(label).is_some(),
            "tech {i} label {label} missing"
        );
        assert!(tech.race <= 3);
    }
    for (i, upgrade) in g.upgrades.iter().enumerate() {
        let label = upgrade.label.saturating_sub(1);
        assert!(
            g.strings.get(label).is_some(),
            "upgrade {i} label {label} missing"
        );
        assert!(upgrade.max_level <= 3);
        assert!(upgrade.race <= 3);
    }
}

#[test]
fn orders_dat_flags_match_known_stock_orders() {
    let g = data();
    // Attack and Attack Fixed Range are queueable/interruptible; Build is
    // deliberately not queueable in the shipped table.
    assert!(g.order(7).unwrap().can_be_queued);
    assert!(g.order(7).unwrap().can_be_interrupted);
    assert!(g.order(10).unwrap().can_be_interrupted);
    assert!(!g.order(15).unwrap().can_be_queued);
    assert_eq!(g.order_name(7), "Attack");
    assert_eq!(g.order_name(15), "Build");
}

#[test]
fn every_order_row_has_a_behaviour_family() {
    // 189 shipped order rows; every one maps to a family, and the families
    // used by the high-level engine round-trip through order_id().
    let mut seen = std::collections::HashSet::new();
    for id in 0..189u8 {
        seen.insert(galaxycraft::sim::order_class(id));
    }
    assert!(seen.len() >= 12, "the complete order set is classified");
    let g = data();
    for id in 0..189u16 {
        assert!(!g.order_name(id as u8).is_empty(), "order {id} has a name");
    }
}

#[test]
fn order_dispatch_cadence_matches_the_recovered_carry_switch() {
    use galaxycraft::sim::order_runs_this_tick;
    // Fast switch and the 36-case every-tick switch.
    for id in [0u8, 4, 48, 98, 106, 129, 151, 174, 179] {
        for carry in 0..8u8 {
            assert!(order_runs_this_tick(id, carry), "row {id} carry {carry}");
        }
    }
    // The 128-case carry switch runs only when the counter reaches zero.
    for id in [1u8, 6, 7, 9, 13, 15, 47, 57, 84, 90, 97, 148, 180] {
        assert!(order_runs_this_tick(id, 0), "row {id} must run on carry 0");
        for carry in 1..8u8 {
            assert!(!order_runs_this_tick(id, carry), "row {id} carry {carry}");
        }
    }
    // Aliased rows inherit the lane of their target.
    assert_eq!(
        order_runs_this_tick(8, 3),
        order_runs_this_tick(9, 3),
        "Attack Unit 8 aliases 9"
    );
}

#[test]
fn order_specs_match_the_recovered_dispatch_table() {
    // `docs/evidence/order_dispatch.tsv` is the per-row table read out of the
    // decompiled `FUN_7a178340` switch.  Every Rust row must carry the same
    // handler address (or the same "no direct case" marker) as the original.
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("docs/evidence/order_dispatch.tsv");
    let text = std::fs::read_to_string(&path).expect("recovered dispatch table");
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("order_id\torder_name\thandler"));
    // A few `stat_txt.tbl` labels contain embedded newlines/control bytes, so
    // the evidence table occasionally wraps a name onto the next line.  Rows
    // are identified by an integer first field; wrapped text is appended to
    // the previous row's label.
    let mut parsed: Vec<(u8, String, String)> = Vec::new();
    for line in lines {
        let mut fields = line.split('\t');
        let first = fields.next().unwrap_or("");
        if let Ok(id) = first.parse::<u8>() {
            let name = fields.next().unwrap_or("").to_string();
            let handler = fields.next().unwrap_or("").to_string();
            parsed.push((id, name, handler));
        } else if let Some((_, name, _)) = parsed.last_mut() {
            name.push(' ');
            name.push_str(first.trim());
        }
    }
    let mut rows = 0usize;
    for (id, name, handler) in parsed {
        let expected = if handler.is_empty() || handler == "guard_check_icall" {
            None
        } else {
            let hex = handler
                .strip_prefix("FUN_")
                .or_else(|| handler.strip_prefix("func_0x"))
                .unwrap_or_else(|| panic!("unexpected handler text {handler:?}"));
            Some(u32::from_str_radix(hex, 16).unwrap())
        };
        let spec = galaxycraft::orders::spec(id).expect("row exists");
        assert_eq!(spec.id, id, "row {id} id");
        // Some `stat_txt.tbl` labels contain control characters used by the
        // colour/formatting renderer; the recovered bytes are still the same
        // label once those are removed.
        let clean = |s: &str| -> String {
            s.chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == ' ')
                .collect()
        };
        let shipped = clean(name.trim_end_matches(" (row label: Carrier Ignore)"));
        assert!(
            clean(spec.name).contains(&shipped),
            "row {id}: Rust name {:?} must carry the shipped label {name:?}",
            spec.name
        );
        assert_eq!(
            spec.handler, expected,
            "row {id} ({name}) handler differs from the recovered switch"
        );
        rows += 1;
    }
    assert_eq!(rows, 189, "the recovered switch table covers all rows");
}

#[test]
fn order_lanes_match_the_recovered_three_layer_switch() {
    // The full decompile of `FUN_7a178340` has two switch groups before the
    // `CUnit+0x85` carry counter (the 4-case fast switch and the 36-case
    // every-tick switch) and one 128-case switch after it.  Parse the case
    // labels back out and require every Rust row to sit in exactly the group
    // it claims.
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("docs/evidence/order_dispatch_vm.txt");
    let text = std::fs::read_to_string(&path).expect("recovered dispatcher");
    let (before_counter, after_counter) = text
        .split_once("cVar1 = *(char *)(in_ECX + 0x85);")
        .expect("recovered carry counter");
    let cases = |s: &str| -> std::collections::BTreeSet<u8> {
        s.lines()
            .filter_map(|line| {
                let rest = line.trim_start().strip_prefix("case ")?;
                let token = rest.split(':').next()?;
                let value = if let Some(hex) = token.strip_prefix("0x") {
                    u32::from_str_radix(hex, 16).ok()?
                } else {
                    token.parse::<u32>().ok()?
                };
                u8::try_from(value).ok()
            })
            .collect()
    };
    let every = cases(before_counter);
    let carry = cases(after_counter);
    assert_eq!(every.len(), 4 + 36, "fast + every-tick switch rows");
    assert_eq!(carry.len(), 128, "carry-gated switch rows");
    for spec in galaxycraft::orders::ORDER_SPECS.iter() {
        use galaxycraft::orders::OrderLane;
        let id = spec.id;
        match spec.lane {
            OrderLane::EveryTick => {
                assert!(
                    every.contains(&id),
                    "row {id} must be in the every-tick group"
                )
            }
            OrderLane::Carry8 => {
                assert!(carry.contains(&id), "row {id} must be in the carry group")
            }
            OrderLane::Alias(_) | OrderLane::Production | OrderLane::Engine => {
                assert!(
                    !every.contains(&id) && !carry.contains(&id),
                    "row {id} is resolved outside the direct switch"
                )
            }
        }
    }
}

#[test]
fn every_recovered_order_handler_has_decompiled_pseudocode() {
    // The summary table is one row per created/decompiled handler function.
    // Every fixed handler address named by `ORDER_SPECS` must appear here;
    // Hover (12) and Attack Move (13) are virtual `guard_check_icall` sites
    // and intentionally have no address.
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("docs/evidence/order_handlers_summary.tsv");
    let text = std::fs::read_to_string(&path).expect("handler summary");
    let mut decompiled = std::collections::BTreeSet::new();
    let mut rows = 0usize;
    for line in text.lines().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let addr = line.split('\t').next().unwrap();
        let addr = addr.strip_prefix("0x").unwrap_or(addr);
        decompiled.insert(u32::from_str_radix(addr, 16).unwrap());
        rows += 1;
    }
    assert_eq!(rows, 137, "all unique fixed handler addresses decompiled");
    for spec in galaxycraft::orders::ORDER_SPECS.iter() {
        if let Some(addr) = spec.handler {
            assert!(
                decompiled.contains(&addr),
                "row {} ({}) handler {addr:#x} has no decompiled body",
                spec.id,
                spec.name
            );
        }
    }
}

#[test]
fn unit_order_carry_cycles_every_eight_ticks() {
    let mut w = World::new(Arc::new(data()), Terrain::open(32, 32), 2);
    let marine = w.spawn(0, uid::MARINE, 10, 10).unwrap();
    // First tick dispatches (carry starts at zero) and resets the counter.
    w.step();
    assert_eq!(w.units[marine].order_carry, 8);
    // The recovered routine stores the pre-decrement value, so the counter
    // walks 8,7,...,1,0 and then reloads 8 as it dispatches.
    for expected in [7u8, 6, 5, 4, 3, 2, 1, 0] {
        w.step();
        assert_eq!(w.units[marine].order_carry, expected);
    }
    w.step();
    assert_eq!(
        w.units[marine].order_carry, 8,
        "carry reloads as it dispatches"
    );
}

#[test]
fn issue_order_id_translates_all_families() {
    use galaxycraft::sim::Order;
    let mut w = World::new(Arc::new(data()), Terrain::open(32, 32), 2);
    let marine = w.spawn(0, uid::MARINE, 10, 10).unwrap();
    let id = w.units[marine].id;
    // 6 = Move, 8 = Attack Unit, 106 = Hold, 151 = Patrol.
    w.issue_order_id(id, 6, None, Some((20, 20))).unwrap();
    assert!(matches!(w.units[marine].order, Order::Move { .. }));
    w.issue_order_id(id, 106, None, None).unwrap();
    assert!(matches!(w.units[marine].order, Order::HoldPosition));
    w.issue_order_id(id, 151, None, Some((22, 22))).unwrap();
    assert!(matches!(w.units[marine].order, Order::Patrol { .. }));
    assert_eq!(
        w.issue_order_id(id, 6, None, None),
        Err(galaxycraft::sim::SimError::Unsupported)
    );
}

#[test]
fn order_queue_cap_is_the_recovered_eight() {
    // The runtime AddOrder/QueueOrder helpers at 0x7a156cd0 and 0x7a157534
    // check the byte at CUnit+0x84 against 8.
    assert_eq!(MAX_ORDER_QUEUE, 8);
}

#[test]
fn target_acquisition_follows_the_recovered_helper() {
    let g = data();
    // Marine: target_acq 0, Gauss Rifle range 128 -> 128 px (4 tiles).
    assert_eq!(g.target_acquisition_px(g.unit(uid::MARINE)), 128);
    // SCV: target_acq 1 tile, Fusion Cutter range 10 -> 32 px.
    assert_eq!(g.target_acquisition_px(g.unit(uid::SCV)), 32);
    // Zealot: target_acq 3 tiles, Psionic Blade range 15 -> 96 px.
    assert_eq!(g.target_acquisition_px(g.unit(uid::ZEALOT)), 96);
}

#[test]
fn iscript_walking_speeds_are_recovered_per_unit() {
    let g = data();
    let marine = g.unit_speed(g.unit(uid::MARINE));
    let zergling = g.unit_speed(g.unit(uid::ZERGLING));
    let scv = g.unit_speed(g.unit(uid::SCV));
    assert_eq!(marine, 4 * 256);
    assert!(zergling > marine);
    assert_eq!(scv, 5 * 256);
    // A building has no walking animation.
    assert_eq!(g.unit_speed(g.unit(uid::COMMAND_CENTER)), 0);
}

#[test]
fn time_units_are_game_frames() {
    // Backwater Station recording: SCV 300 frames = 12.6 s at 42 ms/tick.
    assert_eq!(build_ticks(300), 300);
    assert_eq!(build_ticks(360), 360);
    assert_eq!(research_ticks(1200), 1200);
    assert_eq!(upgrade_ticks(4000), 4000);
}

#[test]
fn upgrade_costs_are_base_plus_factor_per_level() {
    let g = data();
    let up = g.upgrade(upgrade::TERRAN_INFANTRY_WEAPONS).unwrap();
    assert_eq!((up.mineral_cost(1), up.gas_cost(1)), (175, 175));
    assert_eq!((up.mineral_cost(2), up.gas_cost(2)), (250, 250));
    assert_eq!(up.max_level, 3);
}

#[test]
fn build_tree_matches_the_hard_coded_switch() {
    let marines = tree::build_info(uid::MARINE).unwrap();
    assert_eq!(marines.producer, uid::BARRACKS);
    let barracks = tree::build_info(uid::BARRACKS).unwrap();
    assert_eq!(barracks.producer, uid::SCV);
    assert_eq!(barracks.required_units, &[(uid::COMMAND_CENTER, 1)]);
    let lurker = tree::build_info(uid::LURKER).unwrap();
    assert_eq!(lurker.required_tech, Some(tech::LURKER_ASPECT));
    assert_eq!(tree::tech_buildings(tech::STIM_PACKS), &[uid::ACADEMY]);
}

fn tiny_scenario() -> Scenario {
    let mut chk = Vec::new();
    let mut push = |name: &[u8; 4], bytes: &[u8]| {
        chk.extend_from_slice(name);
        chk.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        chk.extend_from_slice(bytes);
    };
    let mut dim = Vec::new();
    dim.extend_from_slice(&8u16.to_le_bytes());
    dim.extend_from_slice(&8u16.to_le_bytes());
    push(b"DIM ", &dim);
    push(b"ERA ", &1u16.to_le_bytes());
    let mut mtxm = Vec::new();
    for _ in 0..64 {
        mtxm.extend_from_slice(&32u16.to_le_bytes());
    }
    push(b"MTXM", &mtxm);
    push(b"MASK", &[1u8; 64]);
    // A start location (214) for player 0.
    let mut units = Vec::new();
    let mut u = vec![0u8; 36];
    u[4..6].copy_from_slice(&128u16.to_le_bytes());
    u[6..8].copy_from_slice(&128u16.to_le_bytes());
    u[8..10].copy_from_slice(&uid::START_LOCATION.to_le_bytes());
    units.extend_from_slice(&u);
    push(b"UNIT", &units);
    let _ = push;
    Scenario::parse(&chk).unwrap()
}

#[test]
fn scenario_parser_and_starting_base() {
    let sc = tiny_scenario();
    assert_eq!((sc.width, sc.height), (8, 8));
    assert_eq!(sc.start_locations(), vec![(128, 128)]);
    let g = Arc::new(data());
    let mut vf4 = vec![0u8; 96];
    for i in 0..16 {
        vf4[64 + i * 2] = 1;
    }
    let w = World::from_scenario(
        g,
        &sc,
        &galaxycraft::map::TileData::synthetic(&vf4),
        &[Race::Terran, Race::Protoss],
    );
    assert!(
        w.units
            .iter()
            .any(|u| u.alive && u.owner == 0 && u.type_id == uid::COMMAND_CENTER)
    );
    assert!(
        w.units
            .iter()
            .any(|u| u.alive && u.owner == 1 && u.type_id == uid::NEXUS)
    );
    assert_eq!(
        w.units
            .iter()
            .filter(|u| u.alive && u.owner == 0 && u.type_id == uid::SCV)
            .count(),
        4
    );
}

#[test]
fn campaign_scenarios_do_not_get_free_bases() {
    // A scenario that already places a force must not receive the melee
    // stock town hall/workers.  Only start-location-and-resource maps do.
    let mut chk = Vec::new();
    let mut push = |name: &[u8; 4], bytes: &[u8]| {
        chk.extend_from_slice(name);
        chk.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        chk.extend_from_slice(bytes);
    };
    let mut dim = Vec::new();
    dim.extend_from_slice(&8u16.to_le_bytes());
    dim.extend_from_slice(&8u16.to_le_bytes());
    push(b"DIM ", &dim);
    push(b"ERA ", &1u16.to_le_bytes());
    let mut mtxm = Vec::new();
    for _ in 0..64 {
        mtxm.extend_from_slice(&32u16.to_le_bytes());
    }
    push(b"MTXM", &mtxm);
    push(b"MASK", &[1u8; 64]);
    let mut units = Vec::new();
    let mut scv = vec![0u8; 36];
    scv[4..6].copy_from_slice(&128u16.to_le_bytes());
    scv[6..8].copy_from_slice(&128u16.to_le_bytes());
    scv[8..10].copy_from_slice(&uid::SCV.to_le_bytes());
    scv[18] = 0;
    units.extend_from_slice(&scv);
    push(b"UNIT", &units);
    let sc = Scenario::parse(&chk).unwrap();
    let vf4 = vec![0u8; 96];
    let w = World::from_scenario(
        Arc::new(data()),
        &sc,
        &galaxycraft::map::TileData::synthetic(&vf4),
        &[Race::Terran, Race::Protoss],
    );
    assert_eq!(
        w.units.iter().filter(|u| u.alive && u.owner == 0).count(),
        1,
        "the placed SCV is the only player-0 unit"
    );
    assert!(
        !w.units
            .iter()
            .any(|u| u.alive && u.type_id == uid::COMMAND_CENTER)
    );
}

/// Load and step every shipped map when the caller provides extracted copies:
///
/// ```text
/// SC_MAP_DIR=/tmp/sc_maps SC_TILE_DIR=/tmp/tiles/TileSet cargo test --release
/// ```
#[test]
fn every_shipped_map_loads_and_steps_when_available() {
    let Ok(root) = std::env::var("SC_MAP_DIR") else {
        return;
    };
    let Ok(tile_dir) = std::env::var("SC_TILE_DIR") else {
        return;
    };
    // ERA -> vf4 file name.
    const TILESETS: [&str; 8] = [
        "badlands.vf4",
        "platform.vf4",
        "install.vf4",
        "ashworld.vf4",
        "jungle.vf4",
        "Desert.vf4",
        "Ice.vf4",
        "Twilight.vf4",
    ];
    fn collect(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("chk") {
                out.push(path);
            }
        }
    }
    let mut maps = Vec::new();
    collect(std::path::Path::new(&root), &mut maps);
    assert!(!maps.is_empty(), "SC_MAP_DIR must contain extracted maps");
    for map in &maps {
        let bytes = std::fs::read(map).expect("map bytes");
        let scenario = Scenario::parse(&bytes).unwrap_or_else(|e| panic!("{}: {e}", map.display()));
        let era = scenario.era as usize;
        let vf4 = std::fs::read(
            std::path::Path::new(&tile_dir)
                .join(TILESETS[era])
                .as_path(),
        )
        .unwrap_or_else(|e| panic!("{}: tileset {era}: {e}", map.display()));
        let cv5 = std::fs::read(
            std::path::Path::new(&tile_dir)
                .join(TILESETS[era])
                .with_extension("cv5"),
        )
        .unwrap_or_else(|e| panic!("{}: tileset {era} cv5: {e}", map.display()));
        let races = vec![Race::Terran, Race::Protoss, Race::Zerg, Race::Zerg];
        let mut world = World::from_scenario(
            Arc::new(data()),
            &scenario,
            &galaxycraft::map::TileData::new(&cv5, &vf4),
            &races,
        );
        for _ in 0..64 {
            if world.winner.is_some() {
                break;
            }
            world.step();
        }
        assert!(world.tick >= 1, "{} did not step", map.display());
    }
    assert!(maps.len() >= 300, "expected the shipped map set");
}

#[test]
fn trigger_victory_action_sets_the_winner() {
    let mut record = vec![0u8; 2400];
    record[15] = 22; // always
    record[8..12].copy_from_slice(&1u32.to_le_bytes());
    record[320 + 26] = 1; // victory
    record[320 + 16..320 + 20].copy_from_slice(&0u32.to_le_bytes());
    record[2372] = 1; // enabled for player 0
    let triggers = TriggerSystem::parse(&record, &[]);
    let mut world = World::new(Arc::new(data()), Terrain::open(16, 16), 2);
    world.spawn(0, uid::MARINE, 16, 16).unwrap();
    world.spawn(1, uid::MARINE, 48, 48).unwrap();
    world.triggers = Some(triggers);
    for _ in 0..60 {
        world.step();
        if world.winner.is_some() {
            break;
        }
    }
    assert_eq!(world.winner, Some(0));
}

#[test]
fn ai_produces_workers_and_buildings_without_scripted_units() {
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    world.players[0].minerals = 1500;
    world.players[1].minerals = 1500;
    let _cc = world.spawn_px(0, uid::COMMAND_CENTER, 512, 512).unwrap();
    let _enemy = world.spawn_px(1, uid::COMMAND_CENTER, 1536, 1536).unwrap();
    let mut ai = SimpleAi::new(0);
    for _ in 0..5000 {
        ai.update(&mut world, Some((60, 60)));
        world.step();
    }
    let workers = world
        .units
        .iter()
        .filter(|u| u.alive && u.owner == 0 && u.type_id == uid::SCV)
        .count();
    assert!(workers > 4, "AI should train workers");
    assert!(
        world
            .units
            .iter()
            .any(|u| u.alive && u.owner == 0 && u.type_id == uid::SUPPLY_DEPOT)
    );
}

#[test]
fn transports_load_and_unload() {
    let mut world = World::new(Arc::new(data()), Terrain::open(32, 32), 2);
    let dropship = world.spawn_px(0, uid::DROPSHIP, 256, 256).unwrap();
    let marine = world.spawn_px(0, uid::MARINE, 260, 260).unwrap();
    let d_id = world.units[dropship].id;
    let m_id = world.units[marine].id;
    world
        .issue(m_id, Order::EnterTransport { transport: d_id })
        .unwrap();
    for _ in 0..30 {
        world.step();
    }
    assert!(world.units[marine].in_transport.is_some());
    assert!(!world.units[marine].alive);
    world.issue(d_id, Order::Unload).unwrap();
    for _ in 0..5 {
        world.step();
    }
    assert!(world.units[marine].alive);
    assert!(world.units[marine].in_transport.is_none());
}

#[test]
fn vulture_mine_order_places_the_three_carried_mines() {
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    world.players[0].techs[tech::SPIDER_MINES as usize] = true;
    let v = world.spawn_px(0, uid::VULTURE, 8 * 32, 8 * 32).unwrap();
    let id = world.units[v].id;
    assert_eq!(world.units[v].hangar, 3);
    for k in 0..3 {
        world
            .issue_order_id(id, 18, None, Some((10 * 32 + k * 16, 10 * 32)))
            .unwrap();
    }
    assert_eq!(world.units[v].hangar, 0);
    assert_eq!(
        world
            .units
            .iter()
            .filter(|u| u.alive && u.type_id == uid::SPIDER_MINE)
            .count(),
        3
    );
    assert_eq!(
        world.issue_order_id(id, 18, None, Some((10 * 32, 12 * 32))),
        Err(SimError::QueueFull)
    );
}

#[test]
fn internal_ai_and_doodad_rows_have_concrete_behaviour() {
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    let critter = world.spawn_px(0, uid::MARINE, 8 * 32, 8 * 32).unwrap();
    let c_id = world.units[critter].id;
    world.issue_order_id(c_id, 154, None, None).unwrap();
    assert!(matches!(world.units[critter].order, Order::Patrol { .. }));
    world
        .issue_order_id(c_id, 157, None, Some((16 * 32, 8 * 32)))
        .unwrap();
    assert!(matches!(
        world.units[critter].order,
        Order::AttackMove { .. }
    ));
    world.issue_order_id(c_id, 156, None, None).unwrap();
    assert!(matches!(world.units[critter].order, Order::HoldPosition));
    // Computer Return heads for the town hall.
    world.issue_order_id(c_id, 162, None, None).unwrap();
    assert!(matches!(world.units[critter].order, Order::Stop));
    // Self-destruct row kills the unit.
    world.issue_order_id(c_id, 174, None, None).unwrap();
    assert!(!world.units[critter].alive);
}

#[test]
fn ability_order_rows_dispatch_to_the_recovered_casts() {
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    world.players[0].techs[tech::PERSONNEL_CLOAKING as usize] = true;
    world.players[0].techs[tech::YAMATO_GUN as usize] = true;
    let ghost = world.spawn_px(0, uid::GHOST, 8 * 32, 8 * 32).unwrap();
    let g_id = world.units[ghost].id;
    world.issue_order_id(g_id, 57, None, None).unwrap();
    assert!(world.units[ghost].status.cloaked, "row 57 cloaks");
    world.issue_order_id(g_id, 109, None, None).unwrap();
    assert!(!world.units[ghost].status.cloaked, "row 109 decloaks");

    let bc = world
        .spawn_px(0, uid::BATTLECRUISER, 8 * 32, 16 * 32)
        .unwrap();
    let bc_id = world.units[bc].id;
    let z = world.spawn_px(1, uid::ZERGLING, 11 * 32, 16 * 32).unwrap();
    let z_id = world.units[z].id;
    let hp0 = world.units[z].hp;
    world.issue_order_id(bc_id, 112, Some(z_id), None).unwrap();
    assert!(
        !world.units[z].alive || world.units[z].hp < hp0,
        "row 112 fires Yamato Gun"
    );
}

#[test]
fn queen_infestation_converts_a_damaged_command_center() {
    use galaxycraft::sim::{Spell, SpellTarget};
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    world.players[1].race = Race::Zerg;
    world.players[1].techs[tech::INFESTATION as usize] = true;
    let cc = world
        .spawn_px(0, uid::COMMAND_CENTER, 16 * 32, 16 * 32)
        .unwrap();
    let queen = world.spawn_px(1, uid::QUEEN, 18 * 32, 16 * 32).unwrap();
    let cc_id = world.units[cc].id;
    let q_id = world.units[queen].id;
    let max = world.data.unit(uid::COMMAND_CENTER).hit_points_raw as i32;
    world.units[cc].hp = max / 4;
    world
        .cast(q_id, Spell::Infestation, SpellTarget::Unit(cc_id))
        .unwrap();
    assert_eq!(world.units[cc].type_id, uid::INFESTED_COMMAND_CENTER);
    assert_eq!(world.units[cc].owner, 1);
    // The infested Command Center trains Infested Terrans.
    world.players[1].minerals = 500;
    world.players[1].gas = 500;
    let _overlord = world.spawn_px(1, uid::OVERLORD, 24 * 32, 24 * 32).unwrap();
    let trained = world.train(cc_id, uid::INFESTED_TERRAN);
    assert!(
        trained.is_ok(),
        "infested Command Center should train Infested Terrans: {trained:?}"
    );
}

#[test]
fn scv_repair_uses_the_retail_rate_and_cost() {
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    world.players[0].minerals = 1000;
    let cc = world
        .spawn_px(0, uid::COMMAND_CENTER, 16 * 32, 16 * 32)
        .unwrap();
    let scv = world.spawn_px(0, uid::SCV, 17 * 32, 17 * 32).unwrap();
    let cc_id = world.units[cc].id;
    let scv_id = world.units[scv].id;
    let max = world.data.unit(uid::COMMAND_CENTER).hit_points_raw as i32;
    world.units[cc].hp = max - (100 << 8);
    let start_hp = world.units[cc].hp;
    let start_min = world.players[0].minerals;
    world
        .issue(scv_id, Order::Repair { target: cc_id })
        .unwrap();
    for _ in 0..100 {
        world.step();
    }
    let healed = world.units[cc].hp - start_hp;
    // 0.9 * 1500 HP / 120 s = 11.25 HP/s -> 0.469 HP/tick -> ~46.9 HP/100.
    assert!(
        (11_000..=13_000).contains(&healed),
        "repair healed {healed} raw HP in 100 ticks"
    );
    let spent = start_min - world.players[0].minerals;
    assert!(
        (1..=20).contains(&spent),
        "repair cost was {spent} minerals"
    );
}

#[test]
fn terran_buildings_burn_below_one_third_hit_points() {
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    let cc = world
        .spawn_px(0, uid::COMMAND_CENTER, 16 * 32, 16 * 32)
        .unwrap();
    let max = world.data.unit(uid::COMMAND_CENTER).hit_points_raw as i32;
    world.units[cc].hp = max / 4;
    let before = world.units[cc].hp;
    for _ in 0..100 {
        world.step();
    }
    assert_eq!(
        world.units[cc].hp,
        before - 2000,
        "burn rate is 20/256 HP per tick"
    );
    world.units[cc].hp = max / 2;
    let above_third = world.units[cc].hp;
    for _ in 0..100 {
        world.step();
    }
    assert_eq!(world.units[cc].hp, above_third, "no burn above one third");
    // Zerg structures do not burn.
    let hatch = world.spawn_px(0, uid::HATCHERY, 30 * 32, 16 * 32).unwrap();
    world.units[hatch].hp = 1;
    for _ in 0..10 {
        world.step();
    }
    assert!(world.units[hatch].alive);
}

#[test]
fn terran_buildings_lift_move_and_land() {
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    let cc = world
        .spawn_px(0, uid::COMMAND_CENTER, 16 * 32, 16 * 32)
        .unwrap();
    let id = world.units[cc].id;
    world.lift_building(id).unwrap();
    assert!(world.units[cc].lifted);
    assert!(world.is_air_unit(cc));
    // A lifted building cannot produce.
    assert_eq!(
        world.train(id, uid::SCV),
        Err(SimError::MissingRequirements)
    );
    world
        .issue(
            id,
            Order::Move {
                pos: (40 * 32, 16 * 32),
            },
        )
        .unwrap();
    for _ in 0..600 {
        world.step();
    }
    assert!(
        world.units[cc].x > (30 * 32) << 8,
        "lifted Command Center should move, x={}",
        world.units[cc].x / 256
    );
    world.land_building(id, (40 * 32, 16 * 32)).unwrap();
    assert!(!world.units[cc].lifted);
    assert_eq!(world.units[cc].x, (40 * 32) << 8);
    // Supply Depots cannot lift.
    let dep = world
        .spawn_px(0, uid::SUPPLY_DEPOT, 8 * 32, 8 * 32)
        .unwrap();
    assert_eq!(
        world.lift_building(world.units[dep].id),
        Err(SimError::InvalidTarget)
    );
}

#[test]
fn terran_addons_attach_to_their_parent_and_die_with_it() {
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    world.players[0].minerals = 2000;
    world.players[0].gas = 2000;
    let cc = world
        .spawn_px(0, uid::COMMAND_CENTER, 16 * 32, 16 * 32)
        .unwrap();
    let cc_id = world.units[cc].id;
    assert_eq!(
        world.build_addon(cc_id, uid::MACHINE_SHOP),
        Err(SimError::InvalidTarget),
        "Machine Shop is a Factory add-on"
    );
    world.build_addon(cc_id, uid::COMSAT_STATION).unwrap();
    let addon_id = world.units[cc].addon.expect("parent records its addon");
    let addon = world.index_of(addon_id).unwrap();
    assert!(!world.units[addon].completed);
    let ticks = world.units[addon].build_total;
    for _ in 0..ticks {
        world.step();
    }
    assert!(world.units[addon].completed, "addon completes");
    // A parent cannot host a second add-on.
    assert_eq!(
        world.build_addon(cc_id, uid::NUCLEAR_SILO),
        Err(SimError::ProducerBusy)
    );
    // Lifting leaves the add-on grounded and inactive; landing re-attaches.
    world.lift_building(cc_id).unwrap();
    assert!(
        world.units[addon].alive,
        "add-on is not destroyed by lift-off"
    );
    assert!(!world.units[addon].addon_attached);
    assert!(world.units[cc].addon.is_none());
    world.land_building(cc_id, (16 * 32, 16 * 32)).unwrap();
    assert_eq!(world.units[cc].addon, Some(addon_id));
    assert!(world.units[addon].addon_attached);
    // Destroying the Command Center destroys the attached add-on.
    let hp = world.units[cc].hp;
    world.damage_unit(cc_id, hp).unwrap();
    assert!(!world.units[addon].alive);
}

#[test]
fn zerg_building_morphs_keep_the_structure_and_consume_drones() {
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    world.players[0].race = Race::Zerg;
    world.players[0].minerals = 3000;
    world.players[0].gas = 3000;
    let hatch = world.spawn_px(0, uid::HATCHERY, 16 * 32, 16 * 32).unwrap();
    let _pool = world
        .spawn_px(0, uid::SPAWNING_POOL, 20 * 32, 16 * 32)
        .unwrap();
    let hatch_id = world.units[hatch].id;
    world.morph_building(hatch_id, uid::LAIR).unwrap();
    assert_eq!(world.units[hatch].type_id, uid::LAIR);
    assert!(!world.units[hatch].completed, "the Lair is morphing");
    let ticks = world.units[hatch].build_total;
    for _ in 0..ticks {
        world.step();
    }
    assert!(world.units[hatch].completed);
    // A Lair cannot become a Hive without a Queen's Nest.
    assert_eq!(
        world.morph_building(hatch_id, uid::HIVE),
        Err(SimError::MissingRequirements)
    );
    // Let the creep spread (it grows over time; the Lair morph now takes
    // its retail frame count, which is shorter than the old scaling).
    for _ in 0..600 {
        world.step();
    }
    // A Drone that starts a structure is consumed by it.
    let drone = world.spawn_px(0, uid::DRONE, 17 * 32, 17 * 32).unwrap();
    let drone_id = world.units[drone].id;
    let spot = world
        .find_build_spot(uid::EVOLUTION_CHAMBER, (16 * 32, 16 * 32))
        .unwrap();
    world
        .issue(
            drone_id,
            Order::Build {
                unit_type: uid::EVOLUTION_CHAMBER,
                pos: spot,
            },
        )
        .unwrap();
    for _ in 0..400 {
        world.step();
    }
    assert!(!world.units[drone].alive, "the Drone is consumed");
    assert!(
        world
            .units
            .iter()
            .any(|u| u.alive && u.type_id == uid::EVOLUTION_CHAMBER && !u.completed)
    );
}

#[test]
fn lurker_aspect_morphs_a_hydralisk_through_an_egg() {
    use galaxycraft::sim::{Spell, SpellTarget};
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    world.players[0].race = Race::Zerg;
    world.players[0].minerals = 1000;
    world.players[0].gas = 1000;
    world.players[0].techs[tech::LURKER_ASPECT as usize] = true;
    let hydra = world.spawn_px(0, uid::HYDRALISK, 16 * 32, 16 * 32).unwrap();
    let id = world.units[hydra].id;
    world
        .cast(id, Spell::LurkerAspect, SpellTarget::Unit(id))
        .unwrap();
    assert_eq!(world.units[hydra].type_id, uid::EGG);
    assert_eq!(world.units[hydra].morph_to, Some(uid::LURKER));
    let ticks = world.units[hydra].build_total;
    for _ in 0..ticks {
        world.step();
    }
    assert_eq!(world.units[hydra].type_id, uid::LURKER);
    assert!(world.units[hydra].completed);
}

#[test]
fn zerg_structures_require_creep_and_hatcheries_make_it() {
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    world.players[0].race = Race::Zerg;
    world.players[0].minerals = 1000;
    let hatch = world.spawn_px(0, uid::HATCHERY, 16 * 32, 16 * 32).unwrap();
    assert!(world.has_creep_tile(16, 16), "hatchery tile is on creep");
    assert!(!world.has_creep_tile(0, 0), "creep does not cover the map");
    // Creep grows one ring every 8 ticks instead of appearing instantly.
    for _ in 0..32 {
        world.step();
    }
    assert!(world.has_creep_tile(18, 18), "creep spreads three tiles");
    let spot = world
        .find_build_spot(uid::SPAWNING_POOL, (16 * 32, 16 * 32))
        .expect("a creep spot exists beside the hatchery");
    assert!(world.has_creep_px(spot.0, spot.1));
    // Removing the hatchery removes the creep and the build spot.
    let hp = world.units[hatch].hp;
    world.damage_unit(world.units[hatch].id, hp).unwrap();
    for _ in 0..40 {
        world.step();
    }
    assert!(
        !world.has_creep_tile(16, 16),
        "creep recedes after the source dies"
    );
    assert!(
        world
            .find_build_spot(uid::SPAWNING_POOL, (16 * 32, 16 * 32))
            .is_none()
    );
    // The Extractor is exempt from the creep requirement.
    let _geyser = world
        .spawn_px(0, uid::VESPENE_GEYSER, 40 * 32, 40 * 32)
        .unwrap();
    assert!(
        world
            .find_build_spot(uid::EXTRACTOR, (40 * 32, 40 * 32))
            .is_some(),
        "Extractor does not need creep"
    );
}

#[test]
fn protoss_production_requires_a_pylon_power_field() {
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    world.players[0].race = Race::Protoss;
    world.players[0].minerals = 2000;
    let _nexus = world.spawn_px(0, uid::NEXUS, 16 * 32, 16 * 32).unwrap();
    let gateway = world.spawn_px(0, uid::GATEWAY, 20 * 32, 16 * 32).unwrap();
    let g_id = world.units[gateway].id;
    assert_eq!(
        world.train(g_id, uid::ZEALOT),
        Err(SimError::Unpowered),
        "a gateway outside power cannot train"
    );
    let _pylon = world.spawn_px(0, uid::PYLON, 21 * 32, 16 * 32).unwrap();
    assert!(
        world.train(g_id, uid::ZEALOT).is_ok(),
        "pylon power enables the gateway"
    );
}

#[test]
fn attack_move_resumes_toward_the_destination_after_a_kill() {
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    let marine = world.spawn_px(0, uid::MARINE, 6 * 32, 8 * 32).unwrap();
    let zergling = world.spawn_px(1, uid::ZERGLING, 9 * 32, 8 * 32).unwrap();
    // Keep player 1 alive so `check_victory` does not freeze the world.
    let _far = world.spawn_px(1, uid::ZERGLING, 60 * 32, 60 * 32).unwrap();
    let m_id = world.units[marine].id;
    world
        .issue(
            m_id,
            Order::AttackMove {
                pos: (24 * 32, 8 * 32),
            },
        )
        .unwrap();
    let mut saw_resume = false;
    for _ in 0..900 {
        world.step();
        if !world.units[zergling].alive
            && matches!(world.units[marine].order, Order::AttackMove { .. })
        {
            saw_resume = true;
        }
    }
    assert!(!world.units[zergling].alive, "the target died");
    assert!(saw_resume, "attack-move resumed after the kill");
    assert!(
        world.units[marine].x > (16 * 32) << 8,
        "the marine continued toward the attack-move destination, x={}",
        world.units[marine].x / 256
    );
}

#[test]
fn supply_is_capped_at_the_retail_two_hundred() {
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    for k in 0..60 {
        world
            .spawn_px(0, uid::SUPPLY_DEPOT, 64 + (k % 10) * 48, 64 + (k / 10) * 48)
            .unwrap();
    }
    // 60 depots provide far more than the cap; supply is in half-points.
    assert_eq!(world.supply_cap(0), 400);
}

#[test]
fn ground_units_do_not_stack_on_one_another() {
    let mut world = World::new(Arc::new(data()), Terrain::open(32, 32), 2);
    let a = world.spawn_px(0, uid::MARINE, 256, 256).unwrap();
    let b = world.spawn_px(0, uid::MARINE, 256, 256).unwrap();
    world.step();
    let dx = (world.units[a].x - world.units[b].x).abs();
    let dy = (world.units[a].y - world.units[b].y).abs();
    assert!(
        dx.max(dy) >= 8 << 8,
        "overlapping ground units were separated (dx={dx}, dy={dy})"
    );
    // Flyers are not pushed by ground bodies.
    let fly = world.spawn_px(0, uid::DROPSHIP, 256, 256).unwrap();
    world.step();
    assert!((world.units[fly].x - (256 << 8)).abs() < (4 << 8));
}

#[test]
fn bunker_holds_four_infantry_and_kills_them_when_destroyed() {
    let mut world = World::new(Arc::new(data()), Terrain::open(32, 32), 2);
    let bunker = world.spawn_px(0, uid::BUNKER, 256, 256).unwrap();
    let b_id = world.units[bunker].id;
    let mut marines = Vec::new();
    for k in 0..5 {
        let m = world.spawn_px(0, uid::MARINE, 260 + k * 8, 264).unwrap();
        marines.push(m);
    }
    for &m in &marines {
        let id = world.units[m].id;
        world
            .issue(id, Order::EnterTransport { transport: b_id })
            .unwrap();
        for _ in 0..12 {
            world.step();
        }
    }
    assert_eq!(
        world.units[bunker].cargo.len(),
        4,
        "bunker holds exactly four infantry"
    );
    assert!(
        world.units[marines[4]].alive,
        "the fifth infantry unit is rejected"
    );
    // Vehicles cannot enter a bunker.
    let vulture = world.spawn_px(0, uid::VULTURE, 270, 270).unwrap();
    let v_id = world.units[vulture].id;
    world
        .issue(v_id, Order::EnterTransport { transport: b_id })
        .unwrap();
    for _ in 0..12 {
        world.step();
    }
    assert!(world.units[vulture].alive);
    // Destroying the bunker kills its occupants.
    let hp = world.units[bunker].hp;
    world.damage_unit(b_id, hp).unwrap();
    assert!(!world.units[bunker].alive);
    for (k, &m) in marines.iter().take(4).enumerate() {
        assert!(
            !world.units[m].alive,
            "bunker occupant {k} dies with the bunker"
        );
    }
}

#[test]
fn bunker_occupants_fire_from_the_bunker() {
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    let bunker = world.spawn_px(0, uid::BUNKER, 16 * 32, 16 * 32).unwrap();
    let b_id = world.units[bunker].id;
    let marine = world.spawn_px(0, uid::MARINE, 16 * 32, 17 * 32).unwrap();
    let m_id = world.units[marine].id;
    let supply_before = world.supply_used(0);
    world
        .issue(m_id, Order::EnterTransport { transport: b_id })
        .unwrap();
    for _ in 0..20 {
        world.step();
    }
    assert!(world.units[marine].in_transport.is_some());
    assert_eq!(
        world.supply_used(0),
        supply_before,
        "loaded units still consume supply"
    );
    let enemy = world
        .spawn_px(1, uid::ZERGLING, 16 * 32 + 80, 16 * 32)
        .unwrap();
    let hp0 = world.units[enemy].hp;
    for _ in 0..200 {
        world.step();
    }
    assert!(
        !world.units[enemy].alive || world.units[enemy].hp < hp0,
        "the marine inside the bunker should shoot (hp {} -> {})",
        hp0,
        world.units[enemy].hp
    );
}

#[test]
fn nydus_canal_teleports_to_the_paired_canal() {
    let mut world = World::new(Arc::new(data()), Terrain::open(32, 32), 2);
    let a = world.spawn_px(0, uid::NYDUS_CANAL, 128, 128).unwrap();
    let b = world.spawn_px(0, uid::NYDUS_CANAL, 700, 700).unwrap();
    let z = world.spawn_px(0, uid::ZERGLING, 140, 140).unwrap();
    let a_id = world.units[a].id;
    let b_id = world.units[b].id;
    let z_id = world.units[z].id;
    world
        .issue(z_id, Order::EnterTransport { transport: a_id })
        .unwrap();
    for _ in 0..30 {
        world.step();
    }
    let (bx, by) = (world.units[b].x, world.units[b].y);
    assert!(world.units[z].alive);
    assert!(
        (world.units[z].x - bx).abs() < (128 << 8) && (world.units[z].y - by).abs() < (128 << 8),
        "unit exits at the paired canal"
    );
    assert_eq!(world.units[z].in_transport, None);
    let _ = b_id;
}

#[test]
fn shield_battery_converts_energy_into_two_shields_per_point() {
    let mut world = World::new(Arc::new(data()), Terrain::open(32, 32), 2);
    let battery = world.spawn_px(0, uid::SHIELD_BATTERY, 128, 128).unwrap();
    let zealot = world.spawn_px(0, uid::ZEALOT, 140, 128).unwrap();
    let b_id = world.units[battery].id;
    let z_id = world.units[zealot].id;
    world.set_unit_shields(z_id, 0).unwrap();
    let max = world.data.unit(uid::ZEALOT).shield_points as i32;
    let restored = world.shield_battery(b_id, z_id).unwrap();
    assert_eq!(restored, max, "one call can fill the shield pool");
    assert_eq!(world.units[zealot].shields, max << 8);
    // Two shield points per energy point, rounded up.
    assert_eq!(world.units[battery].energy, 200 - (max + 1) / 2);
    // Full shields need no energy.
    let before = world.units[battery].energy;
    assert_eq!(world.shield_battery(b_id, z_id).unwrap(), 0);
    assert_eq!(world.units[battery].energy, before);
}

#[test]
fn production_errors_are_explicit() {
    let mut world = World::new(Arc::new(data()), Terrain::open(32, 32), 2);
    let rax = world.spawn_px(0, uid::BARRACKS, 100, 100).unwrap();
    // No supply provider.
    assert_eq!(
        world.train(world.units[rax].id, uid::MARINE),
        Err(SimError::NotEnoughSupply)
    );
    // Wrong producer for SCV.
    let _ = world.spawn_px(0, uid::COMMAND_CENTER, 200, 200);
    assert_eq!(
        world.train(world.units[rax].id, uid::SCV),
        Err(SimError::MissingRequirements)
    );
}

#[test]
fn full_match_on_a_real_map_when_available() {
    // Set SC_MAP_CHK and SC_VF4 to a CHK extracted from the retail Maps/*.scm
    // and the matching tileset vf4 to run a full deterministic match.
    let (Ok(chk_path), Ok(vf4_path)) = (std::env::var("SC_MAP_CHK"), std::env::var("SC_VF4"))
    else {
        return;
    };
    let chk = std::fs::read(chk_path).expect("chk");
    let cv5 = std::fs::read(std::path::Path::new(&vf4_path).with_extension("cv5")).expect("cv5");
    let vf4 = std::fs::read(vf4_path).expect("vf4");
    let scenario = Scenario::parse(&chk).expect("scenario");
    let mut world = World::from_scenario(
        Arc::new(data()),
        &scenario,
        &galaxycraft::map::TileData::new(&cv5, &vf4),
        &[Race::Terran, Race::Protoss],
    );
    let mut ais = vec![SimpleAi::new(0), SimpleAi::new(1)];
    for _ in 0..40_000 {
        for ai in &mut ais {
            let enemy_base = world
                .players
                .iter()
                .enumerate()
                .find(|(i, _)| *i != ai.player as usize)
                .and_then(|(i, _)| {
                    world
                        .units
                        .iter()
                        .find(|u| u.alive && u.owner as usize == i && u.completed)
                        .map(|u| (u.x / 256, u.y / 256))
                });
            ai.update(&mut world, enemy_base);
        }
        world.step();
        if world.winner.is_some() {
            break;
        }
    }
    assert!(
        world.winner.is_some(),
        "a real melee map should produce a winner"
    );
    assert!(world.units.iter().any(|u| !u.alive), "combat should occur");
}

#[test]
fn zerg_workers_mine_like_the_other_races() {
    let mut w = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    w.players[0].race = Race::Zerg;
    w.players[0].minerals = 50;
    w.spawn_px(0, uid::HATCHERY, 16 * 32, 16 * 32).unwrap();
    for k in 0..4 {
        w.spawn_px(0, uid::DRONE, 18 * 32 + k * 16, 18 * 32)
            .unwrap();
    }
    for k in 0..4 {
        let m = w
            .spawn_px(0, uid::MINERAL_FIELD, 22 * 32 + k * 32, 20 * 32)
            .unwrap();
        w.units[m].resource_amount = 1500;
    }
    let mut ai = SimpleAi::new(0);
    for _ in 0..3000 {
        ai.update(&mut w, None);
        w.step();
    }
    assert!(
        w.players[0].minerals_gathered > 0,
        "zerg drones should mine; orders: {:?}",
        w.units
            .iter()
            .filter(|u| u.alive && u.type_id == uid::DRONE)
            .map(|u| w.data.order_name(u.order.order_id()).to_string())
            .collect::<Vec<_>>()
    );
}

#[test]
fn melee_start_composition_matches_retail() {
    // A melee start is a town hall, four workers and, for Zerg, one
    // Overlord.  The Overlord matters mechanically: a Hatchery provides one
    // supply, so without it four Drones already exceed the cap and the Zerg
    // economy cannot grow.
    let sc = tiny_scenario();
    let mut vf4 = vec![0u8; 96];
    for i in 0..16 {
        vf4[64 + i * 2] = 1;
    }
    for (race, hall, worker) in [
        (Race::Terran, uid::COMMAND_CENTER, uid::SCV),
        (Race::Protoss, uid::NEXUS, uid::PROBE),
        (Race::Zerg, uid::HATCHERY, uid::DRONE),
    ] {
        let w = World::from_scenario(
            Arc::new(data()),
            &sc,
            &galaxycraft::map::TileData::synthetic(&vf4),
            &[race, Race::Terran],
        );
        let own = |t: u16| {
            w.units
                .iter()
                .filter(|u| u.alive && u.owner == 0 && u.type_id == t)
                .count()
        };
        assert_eq!(own(hall), 1, "{race:?} town hall");
        assert_eq!(own(worker), 4, "{race:?} workers");
        if race == Race::Zerg {
            assert_eq!(own(uid::OVERLORD), 1, "Zerg start includes an Overlord");
            // Retail create_starting_units: the Overlord sits the Hatchery's
            // extent + 1 toward the map centre (start (128,128) is not left
            // of / above the centre of this 256 px map, so it goes up-left),
            // and two larvae spawn 10 px below the Hatchery's box.
            let d = w.data.unit(uid::HATCHERY).dimensions;
            let ov = w
                .units
                .iter()
                .find(|u| u.alive && u.type_id == uid::OVERLORD)
                .expect("overlord");
            assert_eq!(
                (ov.x / 256, ov.y / 256),
                (
                    128 - (d.left as i32 + d.right as i32 + 1),
                    128 - (d.top as i32 + d.bottom as i32 + 1)
                )
            );
            assert_eq!(own(uid::LARVA), 2, "two starting larvae");
            assert!(
                w.supply_cap(0) >= 8,
                "the starting Overlord supplies the initial army"
            );
        }
    }
}

#[test]
fn refinery_takes_over_a_geyser_and_workers_gather_gas() {
    let mut w = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    w.players[0].race = Race::Protoss;
    w.players[0].minerals = 200;
    let _nexus = w.spawn_px(0, uid::NEXUS, 16 * 32, 16 * 32).unwrap();
    let geyser = w
        .spawn_px(0, uid::VESPENE_GEYSER, 22 * 32, 20 * 32)
        .unwrap();
    w.units[geyser].resource_amount = 1500;
    let mut probes = Vec::new();
    for k in 0..3 {
        probes.push(
            w.spawn_px(0, uid::PROBE, 18 * 32 + k * 16, 18 * 32)
                .unwrap(),
        );
    }

    // The gas building is placed on the geyser, not on a nearby free tile.
    assert_eq!(
        w.find_build_spot(uid::ASSIMILATOR, (16, 16)),
        Some((22 * 32, 20 * 32))
    );

    // Place an (almost finished) assimilator on the geyser.  On completion
    // it must take the gas pool and hide the geyser.
    let assimilator = w.spawn_px(0, uid::ASSIMILATOR, 22 * 32, 20 * 32).unwrap();
    w.units[assimilator].completed = false;
    w.units[assimilator].build_remaining = 1;
    w.step();
    assert!(w.units[assimilator].completed);
    assert_eq!(w.units[assimilator].resource_amount, 1500);
    assert!(!w.units[geyser].alive);

    // Order the probes to harvest the geyser: the order resolves to the
    // completed assimilator and returns gas.
    for p in &probes {
        w.issue(
            w.units[*p].id,
            Order::Gather {
                resource: w.units[assimilator].id,
            },
        )
        .unwrap();
    }
    for _ in 0..3000 {
        w.step();
        if w.players[0].gas_gathered > 0 {
            break;
        }
    }
    assert!(
        w.players[0].gas_gathered > 0,
        "workers should return harvested gas"
    );
    assert!(w.players[0].gas > 0);
}

#[test]
fn ai_assigns_workers_to_a_completed_gas_building() {
    let mut w = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    w.players[0].race = Race::Protoss;
    w.players[0].minerals = 500;
    let _nexus = w.spawn_px(0, uid::NEXUS, 16 * 32, 16 * 32).unwrap();
    for k in 0..12 {
        w.spawn_px(
            0,
            uid::PROBE,
            17 * 32 + (k % 4) * 16,
            17 * 32 + (k / 4) * 16,
        )
        .unwrap();
    }
    let geyser = w
        .spawn_px(0, uid::VESPENE_GEYSER, 22 * 32, 20 * 32)
        .unwrap();
    w.units[geyser].resource_amount = 3000;
    // Minerals too, so the AI has something to do with the rest.
    for k in 0..4 {
        let m = w
            .spawn_px(0, uid::MINERAL_FIELD, 20 * 32 + k * 32, 18 * 32)
            .unwrap();
        w.units[m].resource_amount = 1500;
    }
    let a = w.spawn_px(0, uid::ASSIMILATOR, 22 * 32, 20 * 32).unwrap();
    w.units[a].completed = false;
    w.units[a].build_remaining = 1;
    w.step();
    assert!(w.units[a].completed);
    assert!(w.units[a].resource_amount > 0);

    let mut ai = SimpleAi::new(0);
    for _ in 0..5000 {
        ai.update(&mut w, None);
        w.step();
    }
    assert!(
        w.players[0].gas_gathered >= 200,
        "AI gas workers should deliver; got {} gas, {} probes",
        w.players[0].gas_gathered,
        w.units
            .iter()
            .filter(|u| u.alive && u.type_id == uid::PROBE)
            .count()
    );
}

/// The retail 1.23.10 corpus must not depend on Remastered-only trigger ids:
/// every condition in every shipped map has to be one of the recovered classic
/// ids (conditions 0..=23, actions 1..=59).  `sc_audit` prints the same
/// histogram for inspection.
#[test]
fn every_shipped_map_uses_only_classic_trigger_ids() {
    let Ok(root) = std::env::var("SC_MAP_DIR") else {
        return;
    };
    fn collect(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("chk") {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    collect(std::path::Path::new(&root), &mut files);
    assert!(!files.is_empty(), "SC_MAP_DIR must contain extracted maps");
    let mut conditions = std::collections::BTreeSet::new();
    let mut actions = std::collections::BTreeSet::new();
    for path in &files {
        let bytes = std::fs::read(path).unwrap();
        let scenario = galaxycraft::map::Scenario::parse(&bytes).expect("shipped map parses");
        for record in scenario
            .triggers
            .as_chunks::<{ galaxycraft::trigger::RECORD_SIZE }>()
            .0
        {
            for c in 0..16 {
                let kind = record[c * 20 + 15];
                if kind != 0 {
                    conditions.insert(kind);
                }
            }
            for a in 0..64 {
                let kind = record[320 + a * 32 + 26];
                if kind != 0 {
                    actions.insert(kind);
                }
            }
        }
    }
    let extra_conditions: Vec<u8> = conditions.iter().copied().filter(|k| *k > 23).collect();
    let extra_actions: Vec<u8> = actions.iter().copied().filter(|k| *k > 59).collect();
    assert!(
        extra_conditions.is_empty(),
        "maps use non-classic condition ids {extra_conditions:?}"
    );
    assert!(
        extra_actions.is_empty(),
        "maps use non-classic action ids {extra_actions:?}"
    );
    assert!(!conditions.is_empty() && !actions.is_empty());
}

#[test]
fn issue_order_id_rejects_parameter_orders_instead_of_ignoring_them() {
    // `issue_order_id` has no unit-type/tech argument.  Rows in those
    // families must report Unsupported rather than silently becoming Stop,
    // which is what the evidence ledger promises.
    let mut w = World::new(Arc::new(data()), Terrain::open(32, 32), 2);
    let idx = w.spawn(0, uid::MARINE, 10, 10).unwrap();
    let id = w.units[idx].id;
    for order in [15u8, 37, 74, 75, 41, 113] {
        assert_eq!(
            w.issue_order_id(id, order, None, None),
            Err(SimError::Unsupported),
            "order {order} needs a parameter"
        );
    }
    // Move with a position still works.
    w.issue_order_id(id, 6, None, Some((20, 20))).unwrap();
}

#[test]
fn map_unit_records_use_the_chk_property_offsets() {
    // One UNIT record with the retail field layout: owner at 16, HP/shields/
    // energy at 17/18/19, resources at 20, hangar at 24, states at 26.
    let mut chk = Vec::new();
    let mut push = |name: &[u8; 4], bytes: &[u8]| {
        chk.extend_from_slice(name);
        chk.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        chk.extend_from_slice(bytes);
    };
    let mut dim = Vec::new();
    dim.extend_from_slice(&4u16.to_le_bytes());
    dim.extend_from_slice(&4u16.to_le_bytes());
    push(b"DIM ", &dim);
    push(b"ERA ", &0u16.to_le_bytes());
    let mut mtxm = vec![0u8; 4 * 4 * 2];
    for i in 0..16 {
        mtxm[i * 2..i * 2 + 2].copy_from_slice(&32u16.to_le_bytes());
    }
    push(b"MTXM", &mtxm);
    push(b"MASK", &[1u8; 16]);
    let mut u = vec![0u8; 36];
    u[0..4].copy_from_slice(&17u32.to_le_bytes());
    u[4..6].copy_from_slice(&64u16.to_le_bytes());
    u[6..8].copy_from_slice(&96u16.to_le_bytes());
    u[8..10].copy_from_slice(&uid::MARINE.to_le_bytes());
    u[12..14].copy_from_slice(&0x12u16.to_le_bytes()); // cloak + invincible set-mask
    u[14..16].copy_from_slice(&0x3eu16.to_le_bytes()); // HP/shields/energy/res/hangar/state
    u[16] = 3; // owner
    u[17] = 50;
    u[18] = 25;
    u[19] = 10;
    u[20..24].copy_from_slice(&123u32.to_le_bytes());
    u[24..26].copy_from_slice(&2u16.to_le_bytes());
    u[26..28].copy_from_slice(&0x10u16.to_le_bytes()); // invincible
    u[32..36].copy_from_slice(&99u32.to_le_bytes());
    push(b"UNIT", &u);
    let sc = Scenario::parse(&chk).unwrap();
    let mu = &sc.units[0];
    assert_eq!(mu.unit_id, uid::MARINE);
    assert_eq!((mu.x, mu.y), (64, 96));
    assert_eq!(mu.owner, 3);
    assert_eq!(mu.fields_set, 0x3e);
    assert_eq!(
        (mu.hit_point_percent, mu.shield_percent, mu.energy_percent),
        (50, 25, 10)
    );
    assert_eq!(mu.resource_amount, 123);
    assert_eq!(mu.hangar, 2);
    assert_eq!((mu.states_set, mu.states), (0x12, 0x10));
    assert_eq!(mu.link_serial, 99);
}

#[test]
fn shipped_map_units_keep_their_real_owners() {
    let Ok(root) = std::env::var("SC_MAP_DIR") else {
        return;
    };
    let mut checked = 0usize;
    let mut owners = [0usize; 12];
    let mut stack = vec![std::path::PathBuf::from(root)];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|s| s.to_str()) != Some("chk") {
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let Ok(sc) = Scenario::parse(&bytes) else {
                continue;
            };
            if sc.units.is_empty() {
                continue;
            }
            for u in &sc.units {
                // Protected maps can carry junk records in a scrambled
                // section; the engine drops them, so only assert the valid
                // unit range.
                if u.unit_id >= 228 {
                    continue;
                }
                assert!(
                    u.owner < 12,
                    "{}: unit {} has owner {} outside CHK 0..11",
                    path.display(),
                    u.unit_id,
                    u.owner
                );
                owners[u.owner as usize] += 1;
            }
            checked += 1;
        }
    }
    if checked == 0 {
        return;
    }
    // Before the offset fix every record's "owner" byte was the shield
    // percentage (usually 0), so all map units landed on player 0.
    assert!(
        owners[1] + owners[2] + owners[3] > 0,
        "map corpus has no non-zero unit owners: {owners:?}"
    );
}

#[test]
fn map_serial_links_attach_addons_and_pair_nydus_canals() {
    let mut chk = Vec::new();
    let mut push = |name: &[u8; 4], bytes: &[u8]| {
        chk.extend_from_slice(name);
        chk.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        chk.extend_from_slice(bytes);
    };
    let mut dim = Vec::new();
    dim.extend_from_slice(&8u16.to_le_bytes());
    dim.extend_from_slice(&8u16.to_le_bytes());
    push(b"DIM ", &dim);
    push(b"ERA ", &0u16.to_le_bytes());
    let mut mtxm = vec![0u8; 8 * 8 * 2];
    for i in 0..64 {
        mtxm[i * 2..i * 2 + 2].copy_from_slice(&32u16.to_le_bytes());
    }
    push(b"MTXM", &mtxm);
    push(b"MASK", &[1u8; 64]);

    let record = |serial: u32, type_id: u16, x: u16, link: u32| {
        let mut u = vec![0u8; 36];
        u[0..4].copy_from_slice(&serial.to_le_bytes());
        u[4..6].copy_from_slice(&x.to_le_bytes());
        u[6..8].copy_from_slice(&128u16.to_le_bytes());
        u[8..10].copy_from_slice(&type_id.to_le_bytes());
        u[32..36].copy_from_slice(&link.to_le_bytes());
        u
    };
    let mut units = Vec::new();
    units.extend_from_slice(&record(1, uid::COMMAND_CENTER, 112, 2));
    units.extend_from_slice(&record(2, uid::COMSAT_STATION, 144, 1));
    units.extend_from_slice(&record(3, uid::NYDUS_CANAL, 176, 4));
    units.extend_from_slice(&record(4, uid::NYDUS_CANAL, 208, 3));
    push(b"UNIT", &units);

    let sc = Scenario::parse(&chk).unwrap();
    assert_eq!(sc.units[0].serial, 1);
    assert_eq!(sc.units[0].link_serial, 2);
    let w = World::from_scenario(
        Arc::new(data()),
        &sc,
        &galaxycraft::map::TileData::synthetic(&[0u8; 96]),
        &[Race::Terran],
    );
    let cc = w
        .units
        .iter()
        .find(|u| u.type_id == uid::COMMAND_CENTER)
        .unwrap();
    let addon = w
        .units
        .iter()
        .find(|u| u.type_id == uid::COMSAT_STATION)
        .unwrap();
    assert_eq!(cc.addon, Some(addon.id));
    assert!(addon.addon_attached);
    let canals: Vec<_> = w
        .units
        .iter()
        .filter(|u| u.type_id == uid::NYDUS_CANAL)
        .collect();
    assert_eq!(canals.len(), 2);
    assert_eq!(canals[0].nydus_exit, Some(canals[1].id));
    assert_eq!(canals[1].nydus_exit, Some(canals[0].id));
}

#[test]
fn campaign_player_slots_keep_non_contiguous_owners() {
    use galaxycraft::render::{asset_root, era_name};

    let raw = std::fs::read(asset_root().join("maps/Enslavers 01.chk")).expect("bundled map");
    let scenario = Scenario::parse(&raw).expect("valid CHK");
    assert!(scenario.has_placed_force(), "campaign map places a force");
    let slots = scenario.player_slots();
    assert_eq!(slots.len(), 7, "slots 0..=6 are live in Enslavers 01");
    let active: Vec<usize> = slots.iter().filter(|s| s.active).map(|s| s.slot).collect();
    assert_eq!(active, vec![0, 3, 4, 6]);
    assert_eq!(
        slots
            .iter()
            .find(|s| s.slot == 0)
            .map(|s| s.race(Race::Terran)),
        Some(Race::Terran)
    );
    assert_eq!(
        slots
            .iter()
            .find(|s| s.slot == 3)
            .map(|s| s.race(Race::Terran)),
        Some(Race::Zerg)
    );
    assert_eq!(
        slots
            .iter()
            .find(|s| s.slot == 6)
            .map(|s| s.race(Race::Terran)),
        Some(Race::Terran)
    );

    let vf4 = std::fs::read(
        asset_root()
            .join("TileSet")
            .join(format!("{}.vf4", era_name(scenario.era))),
    )
    .expect("vf4");
    let cv5 = std::fs::read(
        asset_root()
            .join("TileSet")
            .join(format!("{}.cv5", era_name(scenario.era))),
    )
    .expect("cv5");
    let races: Vec<Race> = slots.iter().map(|s| s.race(Race::Terran)).collect();
    let world = World::from_scenario(
        Arc::new(data()),
        &scenario,
        &galaxycraft::map::TileData::new(&cv5, &vf4),
        &races,
    );
    // The old start-location-derived count was 4 and dropped owners 4 and 6.
    for owner in [4u8, 6u8] {
        assert!(
            world
                .units
                .iter()
                .any(|u| u.alive && u.owner == owner && !u.is_resource()),
            "owner {owner} units were dropped"
        );
    }
}

#[test]
fn campaign_briefing_and_objectives_come_from_the_map_strings() {
    use galaxycraft::render::asset_root;

    let raw = std::fs::read(asset_root().join("maps/Enslavers 01.chk")).expect("bundled map");
    let scenario = Scenario::parse(&raw).expect("valid CHK");
    let briefing = scenario.briefing().expect("mission copy");
    assert_eq!(briefing.name, "Mission 1: Schezar's Scavengers");
    assert!(
        briefing.text.contains("Allen Schezar"),
        "briefing body: {}",
        briefing.text
    );
    assert!(
        !briefing.text.contains("(USE MAP SETTINGS)"),
        "StarEdit placeholder leaked into the briefing"
    );
    assert!(
        briefing
            .objectives
            .contains("- Locate and Destroy Schezar's"),
        "objectives: {}",
        briefing.objectives
    );

    // A melee map still has a description but places no force, so the client
    // does not show it as a campaign briefing.  Its seats come from the two
    // start locations, not from spare OWNR bytes.
    let raw = std::fs::read(asset_root().join("maps/Bottleneck.chk")).expect("bundled map");
    let melee = Scenario::parse(&raw).expect("valid CHK");
    assert!(!melee.has_placed_force());
    let slots = melee.player_slots();
    assert_eq!(slots.len(), 2);
    assert_eq!(slots.iter().filter(|s| s.active).count(), 2);
    assert_eq!(slots.iter().filter(|s| s.computer).count(), 1);
}

#[test]
fn next_scenario_action_resolves_to_the_bundled_followup_mission() {
    use galaxycraft::map::scenario_file_key;
    use galaxycraft::render::{asset_root, era_name};

    let raw = std::fs::read(asset_root().join("maps/Enslavers 01.chk")).expect("bundled map");
    let scenario = Scenario::parse(&raw).expect("valid CHK");
    let vf4 = std::fs::read(
        asset_root()
            .join("TileSet")
            .join(format!("{}.vf4", era_name(scenario.era))),
    )
    .expect("vf4");
    let cv5 = std::fs::read(
        asset_root()
            .join("TileSet")
            .join(format!("{}.cv5", era_name(scenario.era))),
    )
    .expect("cv5");
    let slots = scenario.player_slots();
    let races: Vec<Race> = slots.iter().map(|s| s.race(Race::Terran)).collect();
    let world = World::from_scenario(
        Arc::new(data()),
        &scenario,
        &galaxycraft::map::TileData::new(&cv5, &vf4),
        &races,
    );
    let action = world
        .triggers
        .as_ref()
        .expect("triggers")
        .triggers
        .iter()
        .flat_map(|t| t.actions.iter())
        .find(|a| a.kind == 41)
        .expect("Next Scenario action");
    // The CHK stores the STR index plus one.
    let index = (action.text_id - 1) as usize;
    let next = scenario.strings.get(index).expect("next mission string");
    let key = scenario_file_key(next);
    assert_eq!(key, "enslavers02a");

    let bundled = std::fs::read_dir(asset_root().join("maps"))
        .expect("bundled maps")
        .flatten()
        .any(|e| scenario_file_key(&e.file_name().to_string_lossy()) == key);
    assert!(bundled, "next mission {next} is not in the map pack");
}

#[test]
fn minimap_dots_apply_fog_per_unit() {
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    world.spawn_px(0, uid::MARINE, 100, 100).expect("own unit");
    world
        .spawn_px(1, uid::ZERGLING, 1800, 1800)
        .expect("enemy unit");
    world.enable_fog();
    let fogged = world.minimap_units(0, true);
    assert_eq!(fogged.len(), 1, "only the visible own unit is drawn");
    assert_eq!(fogged[0].owner, 0);
    // Without fog the same query returns both; the enemy is not permanently
    // filtered out.
    assert_eq!(world.minimap_units(0, false).len(), 2);
}

/// Retail `spread_creep_completely` (OpenBW): a Hatchery's creep fills the
/// ellipse 25 dx^2 + 64 dy^2 <= 25 * 320^2 around its position, measured from
/// tile centres, and never covers a resource's tiles.
#[test]
fn hatchery_creep_fills_the_retail_ellipse() {
    let mut world = World::new(Arc::new(data()), Terrain::open(128, 128), 2);
    let h = world
        .spawn_px(0, uid::HATCHERY, 2048, 1024)
        .expect("hatchery");
    world.units[h].completed = true;
    // Mineral field footprint (2x1 tiles) at tiles 60..61 x 28.
    world
        .spawn_px(11, uid::MINERAL_FIELD, 61 * 32, 28 * 32 + 16)
        .expect("mineral");
    world.mature_creep();
    // dx = -304 px at the centre row: inside; one tile further: outside.
    assert!(world.has_creep_tile(54, 32));
    assert!(!world.has_creep_tile(53, 32));
    // dy = -176 px: inside; dy = -208 px: outside.
    assert!(world.has_creep_tile(64, 26));
    assert!(!world.has_creep_tile(64, 25));
    // Ellipse corner: dx = 272, dy = 144 -> 1849600 + 1327104 > 2560000.
    assert!(!world.has_creep_tile(72, 36));
    assert!(!world.has_creep_tile(60, 28), "resource tiles stay clear");
    assert!(world.has_creep_tile(60, 29));
}

#[test]
fn a_terran_scv_stays_on_its_building_and_construction_pauses_without_it() {
    // Retail Terran construction: the SCV keeps building (orders.dat 33)
    // until the structure completes; a stopped SCV pauses it, and an SCV
    // ordered back resumes it.  Supply Depot = 600 frames.
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    world.players[0].minerals = 500;
    let scv = world.spawn_px(0, uid::SCV, 10 * 32, 10 * 32).unwrap();
    let scv_id = world.units[scv].id;
    let spot = world
        .find_build_spot(uid::SUPPLY_DEPOT, (14 * 32, 10 * 32))
        .unwrap();
    world
        .issue(
            scv_id,
            Order::Build {
                unit_type: uid::SUPPLY_DEPOT,
                pos: spot,
            },
        )
        .unwrap();
    let mut depot = None;
    for _ in 0..400 {
        world.step();
        depot = world
            .units
            .iter()
            .position(|u| u.alive && u.type_id == uid::SUPPLY_DEPOT);
        if depot.is_some() {
            break;
        }
    }
    let d = depot.expect("the depot is placed");
    let depot_id = world.units[d].id;
    assert!(
        matches!(world.units[scv].order, Order::Construct { building } if building == depot_id)
    );
    let hp0 = world.units[d].hp;
    for _ in 0..100 {
        world.step();
    }
    let left = world.units[d].build_remaining;
    assert!(world.units[d].hp > hp0, "hit points grow with the progress");
    // Stop the SCV: construction pauses.
    world.stop(scv_id);
    for _ in 0..100 {
        world.step();
    }
    assert_eq!(
        world.units[d].build_remaining, left,
        "paused without its SCV"
    );
    // Send it back: construction resumes and completes.
    world
        .issue(scv_id, Order::Construct { building: depot_id })
        .unwrap();
    for _ in 0..(left + 50) {
        world.step();
    }
    assert!(world.units[d].completed);
    assert!(!matches!(world.units[scv].order, Order::Construct { .. }));
    let max = world.data.unit(uid::SUPPLY_DEPOT).hit_points_raw as i32;
    assert_eq!(world.units[d].hp, max);
}

#[test]
fn a_chasing_unit_walks_to_its_target_and_stays_on_the_map() {
    // `attack_target` passed the target's 1/256 px position as pixels, so
    // every chase headed 256x too far (Wasteland: Marines at x 3700-4200 on
    // a 2048 px map).
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    let marine = world.spawn_px(0, uid::MARINE, 200, 200).unwrap();
    let ling = world.spawn_px(1, uid::ZERGLING, 900, 700).unwrap();
    let target = world.units[ling].id;
    world
        .issue(world.units[marine].id, Order::AttackUnit { target })
        .unwrap();
    let start = (world.units[marine].x, world.units[marine].y);
    for _ in 0..120 {
        world.step();
    }
    let (x, y) = (world.units[marine].x / 256, world.units[marine].y / 256);
    assert!(
        (0..2048).contains(&x) && (0..2048).contains(&y),
        "on the map: {x},{y}"
    );
    let before = ((start.0 / 256 - 900).pow(2) + (start.1 / 256 - 700).pow(2)) as f64;
    let after = ((x - 900).pow(2) + (y - 700).pow(2)) as f64;
    assert!(after < before, "closer to the target");
}

#[test]
fn an_idle_unit_that_is_shot_fights_back() {
    // Wasteland: Zerglings (3-tile acquisition) shot by Marines (4-tile
    // range) stayed idle until dead.
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    let marine = world.spawn_px(0, uid::MARINE, 500, 500).unwrap();
    let ling = world.spawn_px(1, uid::ZERGLING, 620, 500).unwrap();
    let marine_id = world.units[marine].id;
    world
        .issue(
            marine_id,
            Order::AttackUnit {
                target: world.units[ling].id,
            },
        )
        .unwrap();
    let mut answered = false;
    for _ in 0..200 {
        world.step();
        if matches!(world.units[ling].order, Order::AttackUnit { target } if target == marine_id) {
            answered = true;
            break;
        }
    }
    assert!(answered, "the Zergling attacks the Marine that shot it");
}

#[test]
fn a_group_move_through_one_corner_arrives_and_goes_idle() {
    // Nine units sharing the same path corners froze when each had to stand
    // exactly on the corner (Wasteland jam); crowded arrivals never ended.
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    let mut ids = Vec::new();
    for k in 0..9 {
        let i = world
            .spawn_px(0, uid::MARINE, 200 + (k % 3) * 20, 200 + (k / 3) * 20)
            .unwrap();
        ids.push(world.units[i].id);
    }
    world.command_attack_move(&ids, (1400, 1400));
    for _ in 0..1500 {
        world.step();
    }
    for id in ids {
        let u = world.unit(id).unwrap();
        assert!(
            matches!(u.order, Order::Idle),
            "unit {id} still {:?} at ({}, {})",
            u.order,
            u.x / 256,
            u.y / 256
        );
        let d = (((u.x / 256 - 1400).pow(2) + (u.y / 256 - 1400).pow(2)) as f64).sqrt();
        assert!(d < 150.0, "unit {id} arrived ({d:.0} px away)");
    }
}

#[test]
fn an_scv_walks_to_its_build_site_at_full_speed() {
    // `Order::Build` only ran on the 8-tick order cadence, so the SCV
    // walked one tick in eight (~0.6 px/tick instead of 5, Wasteland).
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    world.players[0].minerals = 500;
    let scv = world.spawn_px(0, uid::SCV, 200, 200).unwrap();
    let id = world.units[scv].id;
    world
        .issue(
            id,
            Order::Build {
                unit_type: uid::BARRACKS,
                pos: (1600, 208),
            },
        )
        .unwrap();
    let x0 = world.units[scv].x;
    for _ in 0..48 {
        world.step();
    }
    let speed = (world.units[scv].x - x0) as f64 / 256.0 / 48.0;
    assert!(speed > 4.0, "SCV build walk {speed:.2} px/tick");
}

#[test]
fn a_mined_out_field_disappears_and_its_worker_moves_to_the_next() {
    // Backwater Station: SCVs on a depleted field went idle and the empty
    // field stayed on the map, so income stopped for good.
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    let cc = world.spawn_px(0, uid::COMMAND_CENTER, 400, 400).unwrap();
    world.units[cc].completed = true;
    let small = world.spawn_px(0, uid::MINERAL_FIELD, 400, 560).unwrap();
    world.units[small].resource_amount = 8;
    let next = world.spawn_px(0, uid::MINERAL_FIELD, 470, 560).unwrap();
    world.units[next].resource_amount = 500;
    let (small_id, next_id) = (world.units[small].id, world.units[next].id);
    let scv = world.spawn_px(0, uid::SCV, 400, 480).unwrap();
    let scv_id = world.units[scv].id;
    world
        .issue(scv_id, Order::Gather { resource: small_id })
        .unwrap();
    for _ in 0..1500 {
        world.step();
    }
    assert!(
        world.unit(small_id).is_none_or(|u| !u.alive),
        "the empty field is gone"
    );
    assert!(
        world.unit(next_id).unwrap().resource_amount < 500,
        "the SCV mines the next field"
    );
    assert!(world.players[0].minerals_gathered > 8);
}

#[test]
fn cancelling_a_building_under_construction_refunds_75_percent_and_frees_the_scv() {
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    world.players[0].minerals = 500;
    world.recorder = Some(galaxycraft::replay::Recorder::new());
    let scv = world.spawn_px(0, uid::SCV, 10 * 32, 10 * 32).unwrap();
    let scv_id = world.units[scv].id;
    let spot = world
        .find_build_spot(uid::SUPPLY_DEPOT, (14 * 32, 10 * 32))
        .unwrap();
    world
        .issue(
            scv_id,
            Order::Build {
                unit_type: uid::SUPPLY_DEPOT,
                pos: spot,
            },
        )
        .unwrap();
    let mut depot = None;
    for _ in 0..400 {
        world.step();
        depot = world
            .units
            .iter()
            .position(|u| u.alive && u.type_id == uid::SUPPLY_DEPOT);
        if depot.is_some() {
            break;
        }
    }
    let d = depot.expect("placed");
    let depot_id = world.units[d].id;
    assert_eq!(world.players[0].minerals, 400);
    let deaths = world.players[0].deaths;
    world.cancel_construction(depot_id).unwrap();
    assert!(!world.units[d].alive, "the structure is gone");
    assert_eq!(world.players[0].minerals, 475, "75 % of 100 back");
    assert_eq!(world.players[0].deaths, deaths, "not counted as a loss");
    world.step();
    assert!(
        matches!(world.units[scv].order, Order::Idle),
        "the SCV is free"
    );
    // A finished building cannot be cancelled; the act is in the log.
    assert!(world.cancel_construction(depot_id).is_err());
    let log = &world.recorder.as_ref().unwrap().cmds;
    assert!(log.iter().any(|c| matches!(c.act, galaxycraft::replay::RecAct::CancelBuild { building } if building == depot_id)));
}

#[test]
fn a_spider_mine_order_walks_the_vulture_there_and_lays_one_mine() {
    // Card flow: `Order::Cast { SpiderMines, Point }`; the Vulture moves in
    // range, lays one of its three mines; the hero Vulture needs no research.
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    let v = world.spawn_px(0, 19, 200, 200).unwrap();
    let id = world.units[v].id;
    assert_eq!(world.units[v].hangar, 3);
    world
        .issue(
            id,
            Order::Cast {
                spell: galaxycraft::sim::Spell::SpiderMines,
                target: galaxycraft::sim::SpellTarget::Point(700, 200),
            },
        )
        .unwrap();
    for _ in 0..200 {
        world.step();
    }
    let mines: Vec<_> = world
        .units
        .iter()
        .filter(|u| u.alive && u.type_id == uid::SPIDER_MINE)
        .collect();
    assert_eq!(mines.len(), 1, "one mine");
    assert!((mines[0].x / 256 - 700).abs() <= 40, "at the clicked spot");
    assert_eq!(world.units[v].hangar, 2);
    assert!(world.units[v].x / 256 > 600, "the Vulture went there");
    // A plain Vulture without the research cannot.
    let p = world.spawn_px(0, uid::VULTURE, 300, 300).unwrap();
    assert_eq!(
        world.cast(
            world.units[p].id,
            galaxycraft::sim::Spell::SpiderMines,
            galaxycraft::sim::SpellTarget::Point(400, 300)
        ),
        Err(SimError::MissingTech)
    );
}

#[test]
fn one_marine_leaves_the_bunker_and_the_act_is_logged() {
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    world.recorder = Some(galaxycraft::replay::Recorder::new());
    let b = world.spawn_px(0, uid::BUNKER, 500, 500).unwrap();
    world.units[b].completed = true;
    let bunker = world.units[b].id;
    let mut marines = Vec::new();
    for k in 0..2 {
        let m = world.spawn_px(0, uid::MARINE, 440 + k * 20, 440).unwrap();
        let id = world.units[m].id;
        world
            .issue(id, Order::EnterTransport { transport: bunker })
            .unwrap();
        marines.push(id);
    }
    for _ in 0..200 {
        world.step();
    }
    assert_eq!(world.units[b].cargo.len(), 2, "both Marines are inside");
    world.unload_unit(bunker, marines[1]).unwrap();
    assert_eq!(world.units[b].cargo, vec![marines[0]]);
    assert!(
        world
            .unit(marines[1])
            .is_some_and(|u| u.alive && u.in_transport.is_none())
    );
    assert_eq!(
        world.unload_unit(bunker, marines[1]),
        Err(SimError::InvalidTarget)
    );
    let log = &world.recorder.as_ref().unwrap().cmds;
    assert!(log.iter().any(|c| matches!(c.act, galaxycraft::replay::RecAct::UnloadOne { unit, .. } if unit == marines[1])));
}

#[test]
fn flingy_units_accelerate_from_rest_and_walkers_do_not() {
    // flingy.dat: SCV top speed 1280, acceleration 67 (1/256 px per frame);
    // retail SCVs ramp up over ~19 frames.  Marines move by iscript.
    let mut world = World::new(Arc::new(data()), Terrain::open(64, 64), 2);
    let scv = world.spawn_px(0, uid::SCV, 200, 200).unwrap();
    let marine = world.spawn_px(0, uid::MARINE, 200, 600).unwrap();
    let (sid, mid) = (world.units[scv].id, world.units[marine].id);
    world.issue(sid, Order::Move { pos: (1800, 200) }).unwrap();
    world.issue(mid, Order::Move { pos: (1800, 600) }).unwrap();
    let x0 = world.units[scv].x;
    let mut speeds = Vec::new();
    for _ in 0..30 {
        let before = world.units[scv].x;
        world.step();
        speeds.push(world.units[scv].x - before);
    }
    assert!(speeds[0] <= 2 * 67, "starts slow: {}", speeds[0]);
    assert!(
        speeds[5] > speeds[0] && speeds[10] > speeds[5],
        "speeds up: {speeds:?}"
    );
    assert!(
        (speeds[25] - 1280).abs() <= 1,
        "top speed after the ramp: {}",
        speeds[25]
    );
    assert!(
        world.units[scv].x - x0 < 30 * 1280,
        "the ramp costs distance"
    );
    let m0 = world.units[marine].x;
    world.step();
    assert!(
        world.units[marine].x > m0,
        "the Marine walks by its animation"
    );
}

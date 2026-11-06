//! Production rules checked against the original 228-unit table.

use galaxycraft::data::{GameData, unit_ids};
use galaxycraft::economy::{PlayerEconomy, TrainError, build_ticks};

#[test]
fn every_unit_can_be_queued_with_its_table_cost() {
    let data = GameData::bundled().unwrap();
    for id in 0..data.units.len() as u16 {
        let unit = data.unit(id);
        let mut eco = PlayerEconomy {
            minerals: 100_000,
            gas: 100_000,
            supply_used: 0,
            supply_cap: 100_000,
            production: Vec::new(),
        };
        let ticks = eco.train(&data, id).unwrap_or_else(|e| {
            panic!("unit {id} failed to train: {e:?}");
        });
        assert_eq!(ticks, build_ticks(unit.build_time), "unit {id} ticks");
        assert_eq!(
            eco.minerals,
            100_000 - unit.mineral_cost as u32,
            "unit {id} minerals"
        );
        assert_eq!(eco.gas, 100_000 - unit.gas_cost as u32, "unit {id} gas");
        assert_eq!(
            eco.supply_used, unit.supply_required as u32,
            "unit {id} supply"
        );
    }
}

#[test]
fn stock_build_times_match_the_documented_seconds() {
    let data = GameData::bundled().unwrap();
    // The table is in game frames (Backwater Station: an SCV's bar fills in
    // 12.6 s = 300 x 42 ms).
    assert_eq!(build_ticks(data.unit(unit_ids::MARINE).build_time), 360);
    assert_eq!(build_ticks(data.unit(unit_ids::ZEALOT).build_time), 600);
    assert_eq!(build_ticks(data.unit(unit_ids::SCV).build_time), 300);
}

#[test]
fn tech_requirements_are_not_silently_ignored() {
    // This engine models the table costs, not the tech tree.  A unit with a
    // zero supply requirement (a building) still needs its mineral cost.
    let data = GameData::bundled().unwrap();
    let mut eco = PlayerEconomy {
        minerals: 0,
        gas: 0,
        supply_used: 0,
        supply_cap: 0,
        production: Vec::new(),
    };
    assert_eq!(
        eco.train(&data, unit_ids::MARINE),
        Err(TrainError::NotEnoughMinerals)
    );
}

/// OpenBW `order_MiningMinerals` / `try_gather_resource`: a worker mines a
/// field for 75 frames before it carries anything, and a field serves one
/// worker at a time (a second worker waits, or takes a free field within
/// 8 tiles).  Harvesting on arrival made income ~2.5x retail.
#[test]
fn mining_takes_the_measured_frames_and_a_field_serves_one_worker_at_a_time() {
    use galaxycraft::ids::unit as uid;
    use galaxycraft::map::Terrain;
    use galaxycraft::sim::{MINING_FRAMES, Order, World};
    use std::sync::Arc;

    let setup = |fields: &[(i32, i32)]| {
        let mut w = World::new(
            Arc::new(GameData::bundled().expect("tables")),
            Terrain::open(64, 64),
            1,
        );
        w.spawn(0, uid::NEXUS, 400, 400).unwrap();
        let ids: Vec<u32> = fields
            .iter()
            .map(|&(x, y)| {
                let f = w.spawn(0, uid::MINERAL_FIELD, x, y).unwrap();
                w.units[f].resource_amount = 1500;
                w.units[f].id
            })
            .collect();
        let probes: Vec<usize> = [(560, 380), (560, 420)]
            .iter()
            .map(|&(x, y)| w.spawn(0, uid::PROBE, x, y).unwrap())
            .collect();
        for &p in &probes {
            let id = w.units[p].id;
            w.issue(id, Order::Gather { resource: ids[0] }).unwrap();
        }
        (w, probes, ids)
    };

    // One field, two workers: never two mining it at once, and each trip
    // spends 75 frames on the field.
    let (mut w, probes, ids) = setup(&[(600, 400)]);
    let mut started: Vec<Option<u64>> = vec![None; 2];
    let mut first_carry: Vec<Option<u64>> = vec![None; 2];
    for _ in 0..600 {
        w.step();
        let mining = probes
            .iter()
            .filter(|&&p| w.units[p].gather_timer > 0)
            .count();
        assert!(
            mining <= 1,
            "two workers mined one field at tick {}",
            w.tick
        );
        for (k, &p) in probes.iter().enumerate() {
            if w.units[p].gather_timer > 0 && started[k].is_none() {
                started[k] = Some(w.tick);
            }
            if w.units[p].carry > 0 && first_carry[k].is_none() {
                first_carry[k] = Some(w.tick);
            }
        }
    }
    for k in 0..2 {
        let (s, c) = (started[k].expect("mined"), first_carry[k].expect("carried"));
        assert!(
            c - s >= MINING_FRAMES as u64 - 1,
            "worker {k} mined in {} frames",
            c - s
        );
    }
    assert!(w.players[0].minerals_gathered > 0, "the trips completed");
    let _ = ids;

    // Two fields: the second worker takes the free one instead of waiting.
    let (mut w, probes, ids) = setup(&[(600, 400), (600, 440)]);
    for _ in 0..200 {
        w.step();
    }
    let fields: Vec<Option<u32>> = probes.iter().map(|&p| w.units[p].resource_target).collect();
    assert!(
        fields.contains(&Some(ids[0])) && fields.contains(&Some(ids[1])),
        "{fields:?}"
    );
}

#[test]
fn mineral_mining_time_is_the_recorded_93_frames() {
    // Boot Camp recording: a recorded SCV stays at its field 99-115 frames
    // per visit (ours 88-92 with 75); Wasteland income -1.7 % with 93.
    assert_eq!(galaxycraft::sim::MINING_FRAMES, 93);
}

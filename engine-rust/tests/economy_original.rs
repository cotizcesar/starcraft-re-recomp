//! Production rules checked against the original 228-unit table.

use galaxycraft::data::{unit_ids, GameData};
use galaxycraft::economy::{build_ticks, PlayerEconomy, TrainError};

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
    // 360 table units / 15 = 24 s; 600 / 15 = 40 s; 300 / 15 = 20 s.
    assert_eq!(build_ticks(data.unit(unit_ids::MARINE).build_time), 576);
    assert_eq!(build_ticks(data.unit(unit_ids::ZEALOT).build_time), 960);
    assert_eq!(build_ticks(data.unit(unit_ids::SCV).build_time), 480);
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

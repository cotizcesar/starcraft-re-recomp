//! Economy and production rules.
//!
//! Costs, build times and supply values come directly from `units.dat`.
//! Mining yields and trip timing are the parts that are not stored in the
//! data tables; they are exposed as a small configuration type so tests and
//! callers can see exactly which values are reconstructed assumptions.

use crate::data::{GameData, UnitType};

/// The game's logical update rate, used for weapon cooldowns and simulation.
pub const TICKS_PER_SECOND: u32 = 24;
/// `units.dat` build times are expressed in 1/15 second units.
pub const TABLE_TIME_UNITS_PER_SECOND: u32 = 15;

/// Stock mining values.  A worker carries 8 minerals or 8 gas per trip.
pub const MINERALS_PER_TRIP: u32 = 8;
pub const GAS_PER_TRIP: u32 = 8;
pub const MINERAL_PATCH_CAPACITY: u32 = 1_500;
pub const GAS_GEYSER_CAPACITY: u32 = 5_000;

/// Convert a `units.dat` build time into simulation ticks: the table is in
/// game frames (`sim::build_ticks`).
pub fn build_ticks(build_time: u16) -> u32 {
    build_time as u32
}

/// Convert a `units.dat` build time into seconds.
pub fn build_seconds(build_time: u16) -> f32 {
    build_time as f32 / TABLE_TIME_UNITS_PER_SECOND as f32
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceKind {
    Minerals,
    Gas,
}

#[derive(Clone, Debug)]
pub struct ResourcePatch {
    pub kind: ResourceKind,
    pub remaining: u32,
}

impl ResourcePatch {
    pub fn minerals() -> Self {
        Self {
            kind: ResourceKind::Minerals,
            remaining: MINERAL_PATCH_CAPACITY,
        }
    }

    pub fn gas() -> Self {
        Self {
            kind: ResourceKind::Gas,
            remaining: GAS_GEYSER_CAPACITY,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Worker {
    pub patch: usize,
    /// Ticks left before the worker deposits its current load.
    pub trip_remaining: u32,
    pub carrying: u32,
}

/// A minimal, deterministic mining simulator.  Each worker repeatedly
/// completes a round trip and deposits its carried load.
#[derive(Clone, Debug)]
pub struct MiningSim {
    pub patches: Vec<ResourcePatch>,
    pub workers: Vec<Worker>,
    pub minerals: u32,
    pub gas: u32,
    /// Round-trip duration in ticks (out, harvest, back).
    pub trip_ticks: u32,
}

impl MiningSim {
    pub fn new(trip_ticks: u32) -> Self {
        Self {
            patches: Vec::new(),
            workers: Vec::new(),
            minerals: 0,
            gas: 0,
            trip_ticks: trip_ticks.max(1),
        }
    }

    pub fn add_patch(&mut self, patch: ResourcePatch) -> usize {
        self.patches.push(patch);
        self.patches.len() - 1
    }

    pub fn add_worker(&mut self, patch: usize) {
        self.workers.push(Worker {
            patch,
            trip_remaining: self.trip_ticks,
            carrying: 0,
        });
    }

    pub fn step(&mut self) {
        let trip = self.trip_ticks;
        for worker in &mut self.workers {
            if worker.trip_remaining > 0 {
                worker.trip_remaining -= 1;
                if worker.trip_remaining > 0 {
                    continue;
                }
            }
            // Deposit the previous load and start a new trip.
            if worker.carrying > 0 {
                let patch = &mut self.patches[worker.patch];
                match patch.kind {
                    ResourceKind::Minerals => self.minerals += worker.carrying,
                    ResourceKind::Gas => self.gas += worker.carrying,
                }
                worker.carrying = 0;
            }
            let patch = &mut self.patches[worker.patch];
            if patch.remaining > 0 {
                let per_trip = match patch.kind {
                    ResourceKind::Minerals => MINERALS_PER_TRIP,
                    ResourceKind::Gas => GAS_PER_TRIP,
                };
                let load = per_trip.min(patch.remaining);
                patch.remaining -= load;
                worker.carrying = load;
                worker.trip_remaining = trip;
            }
        }
    }

    pub fn run(&mut self, ticks: u32) {
        for _ in 0..ticks {
            self.step();
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductionOrder {
    pub unit_id: u16,
    pub ticks_remaining: u32,
}

#[derive(Clone, Debug)]
pub struct PlayerEconomy {
    pub minerals: u32,
    pub gas: u32,
    /// Supply used in half-points.
    pub supply_used: u32,
    /// Supply cap in half-points (200 supply -> 400).
    pub supply_cap: u32,
    pub production: Vec<ProductionOrder>,
}

impl Default for PlayerEconomy {
    fn default() -> Self {
        Self {
            minerals: 50,
            gas: 0,
            supply_used: 0,
            supply_cap: 20, // 10 supply, the stock starting value
            production: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrainError {
    NotEnoughMinerals,
    NotEnoughGas,
    NotEnoughSupply,
    NoProducer,
}

impl PlayerEconomy {
    pub fn can_afford(&self, unit: &UnitType) -> bool {
        self.minerals >= unit.mineral_cost as u32 && self.gas >= unit.gas_cost as u32
    }

    pub fn has_supply(&self, unit: &UnitType) -> bool {
        self.supply_used + unit.supply_required as u32 <= self.supply_cap
    }

    /// Queue a unit.  The caller is responsible for checking that a producer
    /// exists; that is a map/tech-tree concern outside this table model.
    pub fn train(&mut self, data: &GameData, unit_id: u16) -> Result<u32, TrainError> {
        let unit = data.unit(unit_id);
        if !self.can_afford(unit) {
            if self.minerals < unit.mineral_cost as u32 {
                return Err(TrainError::NotEnoughMinerals);
            }
            return Err(TrainError::NotEnoughGas);
        }
        if !self.has_supply(unit) {
            return Err(TrainError::NotEnoughSupply);
        }
        self.minerals -= unit.mineral_cost as u32;
        self.gas -= unit.gas_cost as u32;
        self.supply_used += unit.supply_required as u32;
        let ticks = build_ticks(unit.build_time);
        self.production.push(ProductionOrder {
            unit_id,
            ticks_remaining: ticks,
        });
        Ok(ticks)
    }

    /// Advance production.  Returns the unit ids that completed this tick.
    pub fn step(&mut self) -> Vec<u16> {
        let mut completed = Vec::new();
        for order in &mut self.production {
            if order.ticks_remaining > 0 {
                order.ticks_remaining -= 1;
            }
            if order.ticks_remaining == 0 {
                completed.push(order.unit_id);
            }
        }
        self.production.retain(|o| o.ticks_remaining > 0);
        completed
    }
}

/// Supply cost of a unit in whole points.
pub fn supply_of(data: &GameData, unit_id: u16) -> f32 {
    data.unit(unit_id).supply()
}

/// A stock Terran opening: one Command Center worth of supply and an SCV.
pub fn default_terran_economy() -> PlayerEconomy {
    PlayerEconomy {
        minerals: 50,
        gas: 0,
        supply_used: 0,
        supply_cap: 20,
        production: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{GameData, unit_ids};

    #[test]
    fn marine_cost_and_build_time_come_from_units_dat() {
        let g = GameData::bundled().unwrap();
        let mut eco = PlayerEconomy {
            minerals: 100,
            ..Default::default()
        };
        let ticks = eco.train(&g, unit_ids::MARINE).unwrap();
        assert_eq!(eco.minerals, 50);
        assert_eq!(ticks, 360); // units.dat frames (`sim::build_ticks`)
        assert_eq!(eco.supply_used, 2);
    }

    #[test]
    fn cannot_train_without_minerals() {
        let g = GameData::bundled().unwrap();
        let mut eco = PlayerEconomy {
            minerals: 49,
            ..Default::default()
        };
        assert_eq!(
            eco.train(&g, unit_ids::MARINE),
            Err(TrainError::NotEnoughMinerals)
        );
    }

    #[test]
    fn supply_is_counted_in_half_points() {
        let g = GameData::bundled().unwrap();
        let mut eco = PlayerEconomy {
            minerals: 1_000,
            gas: 1_000,
            supply_used: 0,
            supply_cap: 2,
            production: Vec::new(),
        };
        assert!(eco.train(&g, unit_ids::MARINE).is_ok());
        assert_eq!(
            eco.train(&g, unit_ids::MARINE),
            Err(TrainError::NotEnoughSupply)
        );
    }

    #[test]
    fn worker_mines_and_deposits() {
        let mut sim = MiningSim::new(4);
        let p = sim.add_patch(ResourcePatch::minerals());
        sim.add_worker(p);
        // 4 ticks to load + 4 ticks to return, then deposit.
        sim.run(8);
        assert_eq!(sim.minerals, 8);
        // The worker has deposited the first load and is carrying the second.
        assert_eq!(sim.patches[p].remaining, MINERAL_PATCH_CAPACITY - 16);
        assert_eq!(sim.workers[0].carrying, 8);
    }

    #[test]
    fn exhausted_patch_stops_giving_resources() {
        let mut sim = MiningSim::new(1);
        let p = sim.add_patch(ResourcePatch {
            kind: ResourceKind::Minerals,
            remaining: 3,
        });
        sim.add_worker(p);
        sim.run(20);
        assert_eq!(sim.minerals, 3);
        assert_eq!(sim.patches[p].remaining, 0);
    }
}

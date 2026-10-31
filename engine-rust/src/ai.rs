//! A small deterministic computer opponent.
//!
//! This is not the retail campaign AI (which is driven by `aiscript.bin` and
//! `bwscript.bin`).  It is a rules-level opponent for exercising the engine:
//! it keeps workers mining, maintains supply, follows a short build order and
//! sends its army at the enemy base.  Every decision is deterministic.

use crate::ids::{tech, unit as uid, upgrade};
use crate::sim::{Order, Race, SimError, World};

#[derive(Clone, Debug)]
pub struct SimpleAi {
    pub player: u8,
    pub base: Option<(i32, i32)>,
    pub next_think: u64,
    pub next_attack: u64,
    pub last_build: u64,
}

impl SimpleAi {
    pub fn new(player: u8) -> Self {
        Self {
            player,
            base: None,
            next_think: 0,
            next_attack: 1200,
            last_build: 0,
        }
    }

    pub fn update(&mut self, world: &mut World, enemy_base: Option<(i32, i32)>) {
        if world.tick < self.next_think {
            return;
        }
        self.next_think = world.tick + 12;
        let owner = self.player;
        if !world.players[owner as usize].defeated && world.unit_ids(owner).is_empty() {
            world.players[owner as usize].defeated = true;
            return;
        }
        self.find_base(world);
        self.keep_workers_mining(world);
        self.manage_supply(world);
        self.build_order(world);
        self.train_army(world);
        if world.tick >= self.next_attack {
            self.next_attack = world.tick + 1200;
            self.attack(world, enemy_base);
        }
    }

    fn find_base(&mut self, world: &World) {
        if self.base.is_some() {
            return;
        }
        self.base = world
            .units
            .iter()
            .filter(|u| {
                u.alive
                    && u.owner == self.player
                    && u.completed
                    && matches!(
                        u.type_id,
                        uid::COMMAND_CENTER | uid::NEXUS | uid::HATCHERY | uid::LAIR | uid::HIVE
                    )
            })
            .map(|u| (u.x / 256, u.y / 256))
            .next();
    }

    fn keep_workers_mining(&self, world: &mut World) {
        let gas_building = world
            .units
            .iter()
            .find(|u| {
                u.alive
                    && u.owner == self.player
                    && u.completed
                    && matches!(u.type_id, uid::REFINERY | uid::ASSIMILATOR | uid::EXTRACTOR)
                    && u.resource_amount > 0
            })
            .map(|u| u.id);
        let gas_workers = gas_building
            .map(|g| {
                world
                    .units
                    .iter()
                    .filter(|u| {
                        u.alive
                            && u.owner == self.player
                            && is_worker(u.type_id)
                            && (u.carry_kind == crate::sim::CarryKind::Gas
                                || u.resource_target == Some(g)
                                || matches!(u.order, Order::Gather { resource } if resource == g))
                    })
                    .count()
            })
            .unwrap_or(0);
        // Candidates ordered by how little they would disrupt: idle first,
        // then workers that are not carrying anything, then loaded workers.
        let mut candidates: Vec<(bool, i32, u32)> = world
            .units
            .iter()
            .filter(|u| u.alive && u.owner == self.player && is_worker(u.type_id))
            .filter(|u| {
                // Never steal a worker already assigned to gas.
                let ordered_to_gas = gas_building
                    .map(|g| matches!(u.order, Order::Gather { resource } if resource == g))
                    .unwrap_or(false);
                !ordered_to_gas
                    && gas_building.is_none_or(|g| u.resource_target != Some(g))
                    && u.carry_kind != crate::sim::CarryKind::Gas
            })
            .map(|u| (u.order == Order::Idle, u.carry, u.id))
            .collect();
        candidates.sort_by_key(|&(idle, carry, id)| (!idle, carry, id));
        if gas_workers < 3 {
            if let Some(g) = gas_building {
                for (_, _, worker) in candidates.iter().take(3 - gas_workers) {
                    let _ = world.issue(*worker, Order::Gather { resource: g });
                }
            }
        }
        for (idle, _, worker) in candidates {
            if !idle {
                break;
            }
            if world.units[world.index_of(worker).unwrap()].order != Order::Idle {
                continue;
            }
            let Some(widx) = world.index_of(worker) else {
                continue;
            };
            let (wx, wy) = (world.units[widx].x, world.units[widx].y);
            let resource = world
                .units
                .iter()
                .filter(|u| u.alive && is_resource(u.type_id) && u.resource_amount > 0)
                .min_by_key(|u| {
                    let dx = (u.x - wx) as i64;
                    let dy = (u.y - wy) as i64;
                    dx * dx + dy * dy
                })
                .map(|u| u.id);
            if let Some(r) = resource {
                let _ = world.issue(worker, Order::Gather { resource: r });
            }
        }
    }

    fn manage_supply(&mut self, world: &mut World) {
        let owner = self.player;
        let cap = world.supply_cap(owner);
        let used = world.supply_used(owner);
        if cap - used > 6 {
            return;
        }
        let race = world.players[owner as usize].race;
        let supply_type = match race {
            Race::Terran => Some(uid::SUPPLY_DEPOT),
            Race::Protoss => Some(uid::PYLON),
            Race::Zerg => Some(uid::OVERLORD),
            Race::Neutral => None,
        };
        if let Some(supply_type) = supply_type {
            let pending = world
                .units
                .iter()
                .any(|u| u.alive && u.owner == owner && u.type_id == supply_type && !u.completed);
            if pending {
                return;
            }
        }
        match race {
            Race::Terran => {
                self.try_build(world, uid::SUPPLY_DEPOT);
            }
            Race::Protoss => {
                self.try_build(world, uid::PYLON);
            }
            Race::Zerg => {
                let _ = self.try_train(world, uid::OVERLORD);
            }
            Race::Neutral => {}
        }
    }

    fn build_order(&mut self, world: &mut World) {
        let owner = self.player;
        let race = world.players[owner as usize].race;
        let workers = world
            .units
            .iter()
            .filter(|u| u.alive && u.owner == owner && is_worker(u.type_id))
            .count();
        let want_workers = if world.tick < 2400 { 12 } else { 18 };
        if workers < want_workers {
            let worker_type = match race {
                Race::Terran => uid::SCV,
                Race::Protoss => uid::PROBE,
                Race::Zerg => uid::DRONE,
                Race::Neutral => return,
            };
            if race == Race::Zerg {
                let _ = self.try_morph(world, worker_type);
            } else {
                let _ = self.try_train(world, worker_type);
            }
            return;
        }

        match race {
            Race::Terran => {
                if !self.has(world, uid::REFINERY) {
                    self.try_build(world, uid::REFINERY);
                } else if !self.has(world, uid::BARRACKS) {
                    self.try_build(world, uid::BARRACKS);
                } else if !self.has(world, uid::ACADEMY) {
                    self.try_build(world, uid::ACADEMY);
                } else if !self.has(world, uid::FACTORY) {
                    self.try_build(world, uid::FACTORY);
                } else if !self.has(world, uid::ENGINEERING_BAY) {
                    self.try_build(world, uid::ENGINEERING_BAY);
                } else if !self.has(world, uid::STARPORT) {
                    self.try_build(world, uid::STARPORT);
                }
            }
            Race::Protoss => {
                if self.count(world, uid::PYLON) < 2 {
                    self.try_build(world, uid::PYLON);
                } else if !self.has(world, uid::ASSIMILATOR) {
                    self.try_build(world, uid::ASSIMILATOR);
                } else if !self.has(world, uid::GATEWAY) {
                    self.try_build(world, uid::GATEWAY);
                } else if !self.has(world, uid::CYBERNETICS_CORE) {
                    self.try_build(world, uid::CYBERNETICS_CORE);
                } else if !self.has(world, uid::FORGE) {
                    self.try_build(world, uid::FORGE);
                } else if !self.has(world, uid::STARGATE) {
                    self.try_build(world, uid::STARGATE);
                }
            }
            Race::Zerg => {
                // The Spawning Pool comes first: the Extractor needs creep
                // at the geyser, which may only arrive after the creep
                // maturity grid has spread.
                if !self.has(world, uid::SPAWNING_POOL) {
                    self.try_build(world, uid::SPAWNING_POOL);
                } else if !self.has(world, uid::EXTRACTOR) {
                    self.try_build(world, uid::EXTRACTOR);
                } else if !self.has(world, uid::HYDRALISK_DEN) {
                    self.try_build(world, uid::HYDRALISK_DEN);
                } else if !self.has(world, uid::LAIR) {
                    self.try_build(world, uid::LAIR);
                }
            }
            Race::Neutral => {}
        }
    }

    fn train_army(&mut self, world: &mut World) {
        let owner = self.player;
        let race = world.players[owner as usize].race;
        let army = world
            .units
            .iter()
            .filter(|u| u.alive && u.owner == owner && is_combat(u.type_id))
            .count();
        if army >= 24 {
            return;
        }
        // Keep a little bank for buildings.
        if world.players[owner as usize].minerals < 250 {
            return;
        }
        match race {
            Race::Terran => {
                if self.has(world, uid::FACTORY) {
                    let _ = self.try_train(world, uid::SIEGE_TANK);
                } else if self.has(world, uid::BARRACKS) {
                    let _ = self.try_train(world, uid::MARINE);
                }
            }
            Race::Protoss => {
                if self.has(world, uid::CYBERNETICS_CORE) {
                    let _ = self.try_train(world, uid::DRAGOON);
                } else if self.has(world, uid::GATEWAY) {
                    let _ = self.try_train(world, uid::ZEALOT);
                }
            }
            Race::Zerg => {
                if self.has(world, uid::HYDRALISK_DEN) {
                    if self.try_morph(world, uid::HYDRALISK).is_err() {
                        let _ = self.try_morph(world, uid::ZERGLING);
                    }
                } else if self.has(world, uid::SPAWNING_POOL) {
                    let _ = self.try_morph(world, uid::ZERGLING);
                }
            }
            Race::Neutral => {}
        }
        let _ = tech::STIM_PACKS;
        let _ = upgrade::TERRAN_INFANTRY_WEAPONS;
    }

    fn attack(&self, world: &mut World, enemy_base: Option<(i32, i32)>) {
        let owner = self.player;
        let army: Vec<u32> = world
            .units
            .iter()
            .filter(|u| u.alive && u.owner == owner && is_combat(u.type_id))
            .map(|u| u.id)
            .collect();
        if army.is_empty() {
            return;
        }
        let target = enemy_base
            .or_else(|| {
                world
                    .units
                    .iter()
                    .filter(|u| u.alive && u.owner != owner && u.completed)
                    .map(|u| (u.x / 256, u.y / 256))
                    .next()
            })
            .unwrap_or((64, 64));
        world.command_attack_move(&army, target);
    }

    fn has(&self, world: &World, type_id: u16) -> bool {
        self.count(world, type_id) > 0
    }

    fn count(&self, world: &World, type_id: u16) -> usize {
        world
            .units
            .iter()
            .filter(|u| u.alive && u.owner == self.player && u.type_id == type_id)
            .count()
    }

    fn try_train(&mut self, world: &mut World, unit_type: u16) -> Result<(), SimError> {
        let owner = self.player;
        let producer = crate::tech::build_info(unit_type)
            .map(|i| i.producer)
            .ok_or(SimError::MissingRequirements)?;
        let building = world
            .units
            .iter()
            .filter(|u| u.alive && u.owner == owner && u.completed && u.type_id == producer)
            .map(|u| u.id)
            .next()
            .ok_or(SimError::MissingRequirements)?;
        world.train(building, unit_type)
    }

    fn try_morph(&mut self, world: &mut World, unit_type: u16) -> Result<(), SimError> {
        let owner = self.player;
        let larva = world
            .units
            .iter()
            .filter(|u| u.alive && u.owner == owner && u.type_id == uid::LARVA)
            .map(|u| u.id)
            .next()
            .ok_or(SimError::MissingRequirements)?;
        world.morph(larva, unit_type)
    }

    fn try_build(&mut self, world: &mut World, type_id: u16) {
        if world.tick < self.last_build + 24 {
            return;
        }
        let owner = self.player;
        let Some(base) = self.base else { return };
        // Only build what the tree allows.
        if !world.has_requirements(owner, type_id) {
            return;
        }
        if !world.can_afford(owner, type_id) {
            return;
        }
        // Zerg building upgrades are morphs of an existing structure, not a
        // new worker-built structure.
        if let Some(source) = world.morph_source_unit(owner, type_id) {
            if world.morph_building(source, type_id).is_ok() {
                self.last_build = world.tick;
            }
            return;
        }
        let Some(spot) = world.find_build_spot(type_id, base) else {
            return;
        };
        // Prefer an idle worker, but pull one off the mineral line when the
        // base has no free worker.  This mirrors the retail "worker leaves
        // the patch to build" behaviour.
        let mut candidates: Vec<(bool, i32, u32)> = world
            .units
            .iter()
            .filter(|u| u.alive && u.owner == owner && is_worker(u.type_id))
            .map(|u| (u.order == Order::Idle, u.carry, u.id))
            .collect();
        candidates.sort_by_key(|&(idle, carry, _)| (!idle, carry));
        let worker = candidates.first().map(|&(_, _, id)| id);
        let Some(worker) = worker else { return };
        if world
            .issue(
                worker,
                Order::Build {
                    unit_type: type_id,
                    pos: spot,
                },
            )
            .is_ok()
        {
            self.last_build = world.tick;
        }
    }
}

fn is_worker(type_id: u16) -> bool {
    matches!(type_id, uid::SCV | uid::PROBE | uid::DRONE)
}

fn is_resource(type_id: u16) -> bool {
    matches!(
        type_id,
        uid::MINERAL_FIELD | uid::MINERAL_FIELD_2 | uid::MINERAL_FIELD_3
    )
}

fn is_combat(type_id: u16) -> bool {
    matches!(
        type_id,
        uid::MARINE
            | uid::FIREBAT
            | uid::GHOST
            | uid::VULTURE
            | uid::SIEGE_TANK
            | uid::SIEGE_MODE
            | uid::GOLIATH
            | uid::WRAITH
            | uid::BATTLECRUISER
            | uid::ZEALOT
            | uid::DRAGOON
            | uid::HIGH_TEMPLAR
            | uid::DARK_TEMPLAR
            | uid::ARCHON
            | uid::DARK_ARCHON
            | uid::SCOUT
            | uid::CARRIER
            | uid::REAVER
            | uid::ZERGLING
            | uid::HYDRALISK
            | uid::LURKER
            | uid::ULTRALISK
            | uid::MUTALISK
            | uid::GUARDIAN
            | uid::DEVOURER
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::map::Terrain;
    use crate::sim::World;
    use std::sync::Arc;

    #[test]
    fn ai_builds_and_trains_over_time() {
        let mut world = World::new(
            Arc::new(GameData::bundled().unwrap()),
            Terrain::open(64, 64),
            2,
        );
        world.players[0].minerals = 2000;
        world.players[0].gas = 1000;
        world.players[1].minerals = 2000;
        let cc = world
            .spawn(0, uid::COMMAND_CENTER, 16 * 32, 16 * 32)
            .unwrap();
        let enemy = world
            .spawn(1, uid::COMMAND_CENTER, 48 * 32, 48 * 32)
            .unwrap();
        let mut ai = SimpleAi::new(0);
        for _ in 0..6000 {
            ai.update(&mut world, Some((48, 48)));
            world.step();
        }
        assert!(world.players[0].minerals < 2000);
        assert!(world
            .units
            .iter()
            .any(|u| u.alive && u.owner == 0 && is_worker(u.type_id) && u.type_id == uid::SCV));
        let _ = (cc, enemy);
    }
}

//! `scripts\aiscript.bin` / `scripts\bwscript.bin` reader.
//!
//! Both files start with a little-endian offset to a script table.
//!
//! * `aiscript.bin` entries are 16 bytes: 4-byte id, 32-bit code offset,
//!   32-bit `stat_txt.tbl` label, 32-bit flags.
//! * `bwscript.bin` entries are 8 bytes: 4-byte id and 32-bit code offset.
//!
//! The table ends with a null id.  The bytes after the table are the script
//! programs.  This module decodes both table layouts and the opcode stream;
//! [`CampaignAi`] executes the common town/build/train/attack subset.
//!
//! The opcode names and operand layouts are the ones used by the retail
//! interpreter at `0x7a1e0b20` and by the independent PyMS `AIBIN` reader.

use std::collections::{HashMap, HashSet};

use crate::path::{Obstacles, PathRequest, find_path};
use crate::sim::World;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operand {
    Byte(u8),
    Word(u16),
    Dword(u32),
    /// Null-terminated display string.
    String(u16),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instruction {
    pub offset: usize,
    pub opcode: u8,
    pub args: Vec<Operand>,
}

impl Instruction {
    pub fn name(&self) -> &'static str {
        OPCODE_NAMES
            .get(self.opcode as usize)
            .copied()
            .unwrap_or("unknown")
    }

    pub fn byte(&self, n: usize) -> Option<u8> {
        match self.args.get(n) {
            Some(Operand::Byte(v)) => Some(*v),
            Some(Operand::Word(v)) => Some(*v as u8),
            Some(Operand::Dword(v)) => Some(*v as u8),
            Some(Operand::String(v)) => Some(*v as u8),
            None => None,
        }
    }

    pub fn word(&self, n: usize) -> Option<u16> {
        match self.args.get(n) {
            Some(Operand::Byte(v)) => Some(*v as u16),
            Some(Operand::Word(v)) => Some(*v),
            Some(Operand::Dword(v)) => Some(*v as u16),
            Some(Operand::String(v)) => Some(*v),
            None => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AiScript {
    pub id: [u8; 4],
    pub label: u32,
    pub flags: u32,
    /// Empty for an `aiscript.bin` entry whose code lives in `bwscript.bin`.
    pub code: Vec<u8>,
    pub external: bool,
}

impl AiScript {
    pub fn id_str(&self) -> String {
        String::from_utf8_lossy(&self.id).into_owned()
    }

    /// Decode the instruction stream.  Malformed/unknown opcodes are returned
    /// as a single `unknown` instruction and stop decoding, matching the
    /// conservative behaviour of the rest of this crate.
    pub fn instructions(&self) -> Vec<Instruction> {
        let mut out = Vec::new();
        let mut p = 0usize;
        while p < self.code.len() {
            let offset = p;
            let opcode = self.code[p];
            p += 1;
            let Some(spec) = OPCODE_ARGS.get(opcode as usize) else {
                out.push(Instruction {
                    offset,
                    opcode,
                    args: Vec::new(),
                });
                break;
            };
            let mut args = Vec::new();
            let mut ok = true;
            for kind in *spec {
                match *kind {
                    1 => {
                        if p >= self.code.len() {
                            ok = false;
                            break;
                        }
                        args.push(Operand::Byte(self.code[p]));
                        p += 1;
                    }
                    2 => {
                        if p + 2 > self.code.len() {
                            ok = false;
                            break;
                        }
                        args.push(Operand::Word(u16::from_le_bytes([
                            self.code[p],
                            self.code[p + 1],
                        ])));
                        p += 2;
                    }
                    3 => {
                        if p + 4 > self.code.len() {
                            ok = false;
                            break;
                        }
                        args.push(Operand::Dword(u32::from_le_bytes([
                            self.code[p],
                            self.code[p + 1],
                            self.code[p + 2],
                            self.code[p + 3],
                        ])));
                        p += 4;
                    }
                    4 => {
                        let start = p;
                        while p < self.code.len() && self.code[p] != 0 {
                            p += 1;
                        }
                        let text = String::from_utf8_lossy(&self.code[start..p]).into_owned();
                        args.push(Operand::String(text.len() as u16));
                        if p < self.code.len() {
                            p += 1;
                        }
                    }
                    _ => {}
                }
            }
            if !ok {
                break;
            }
            out.push(Instruction {
                offset,
                opcode,
                args,
            });
        }
        out
    }
}

#[derive(Clone, Debug, Default)]
pub struct AiScriptSet {
    pub scripts: Vec<AiScript>,
    pub by_id: HashMap<[u8; 4], usize>,
}

impl AiScriptSet {
    pub fn get(&self, id: &[u8; 4]) -> Option<&AiScript> {
        self.by_id.get(id).map(|i| &self.scripts[*i])
    }

    pub fn get_str(&self, id: &str) -> Option<&AiScript> {
        let mut key = [0u8; 4];
        for (i, b) in id.as_bytes().iter().take(4).enumerate() {
            key[i] = *b;
        }
        self.get(&key)
    }
}

/// Parse `aiscript.bin` (16-byte table entries).
pub fn parse_aiscript(data: &[u8]) -> AiScriptSet {
    parse_table(data, 16)
}

/// Parse `bwscript.bin` (8-byte table entries).
pub fn parse_bwscript(data: &[u8]) -> AiScriptSet {
    parse_table(data, 8)
}

fn parse_table(data: &[u8], entry_size: usize) -> AiScriptSet {
    let mut set = AiScriptSet::default();
    if data.len() < 4 || entry_size < 8 {
        return set;
    }
    let table = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    if table >= data.len() {
        return set;
    }
    // First pass: collect ids and code offsets.
    let mut raw: Vec<([u8; 4], u32, u32, u32)> = Vec::new();
    let mut p = table;
    while p + entry_size <= data.len() {
        let id = [data[p], data[p + 1], data[p + 2], data[p + 3]];
        if id == [0, 0, 0, 0] {
            break;
        }
        let offset = u32::from_le_bytes([data[p + 4], data[p + 5], data[p + 6], data[p + 7]]);
        let (label, flags) = if entry_size >= 16 {
            (
                u32::from_le_bytes([data[p + 8], data[p + 9], data[p + 10], data[p + 11]]),
                u32::from_le_bytes([data[p + 12], data[p + 13], data[p + 14], data[p + 15]]),
            )
        } else {
            (0, 0)
        };
        raw.push((id, offset, label, flags));
        p += entry_size;
    }
    let mut offsets: Vec<u32> = raw
        .iter()
        .map(|(_, o, _, _)| *o)
        .filter(|o| *o > 0)
        .collect();
    offsets.sort_unstable();
    offsets.dedup();
    for (id, offset, label, flags) in raw {
        let external = offset == 0;
        let code = if external || offset as usize >= data.len() || offset as usize >= table {
            Vec::new()
        } else {
            let end = offsets
                .iter()
                .copied()
                .find(|o| *o > offset)
                .map(|o| o as usize)
                .unwrap_or(data.len())
                .min(table);
            let start = offset as usize;
            if end <= start {
                Vec::new()
            } else {
                data[start..end].to_vec()
            }
        };
        let script = AiScript {
            id,
            label,
            flags,
            code,
            external,
        };
        let idx = set.scripts.len();
        set.by_id.insert(id, idx);
        set.scripts.push(script);
    }
    set
}

/// A small deterministic interpreter for the shipped campaign AI scripts.
///
/// It executes the town-management commands that this headless engine can
/// express: `build`, `train`, `upgrade`, `tech`, the attack-party commands,
/// `wait`, `goto`, `time_jump` and `stop`.  Commands that are purely about
/// the retail tactical layer (bunkers, transports, nukes, harassment regions)
/// are skipped rather than guessed; the script PC still advances so a later
/// instruction can run.
#[derive(Clone, Debug)]
pub struct CampaignAi {
    pub player: u8,
    pub id: [u8; 4],
    pub instructions: Vec<Instruction>,
    pub pc: usize,
    pub wait_until: u64,
    pub max_units: HashMap<u16, u8>,
    pub attack_group: Vec<u16>,
    pub town_started: bool,
    pub finished: bool,
    call_stack: Vec<usize>,
    rng: u64,
    /// `multirun` / `expand` child scripts.  The retail engine runs up to 100
    /// threads; a deterministic bound keeps a malformed script finite.
    pub subthreads: Vec<Box<CampaignAi>>,
    spawned_blocks: HashSet<usize>,
    /// Campaign scripts get the longer attack-preparation window.
    pub campaign: bool,
    pub default_build: bool,
    pub farms_timing: bool,
    pub farms_notiming: bool,
    pub max_force: i32,
    pub nuke_rate: u32,
    pub nukes: u32,
    pub harass_factor: u8,
    pub region_size: u16,
    pub default_min: u16,
    pub transports_off: bool,
    pub shared_vision: Vec<u8>,
    /// Current trigger location centre, when a scenario selected one.
    pub location: Option<(i32, i32)>,
    /// `set_gencmd` target used by `make_patrol`.
    pub gencmd: Option<(i32, i32)>,
    /// defensebuild/defenseuse requests for (gg, ag, ga, aa).
    pub defense_requests: [Option<(u8, u16)>; 4],
    pub defense_units: [Option<(u8, u16)>; 4],
    pub attack_prepared: bool,
    pub killable: bool,
    pub scout_unit: Option<u16>,
    pub bunkers: u8,
    pub turrets: u8,
    pub random_seed_set: bool,
    /// Players explicitly marked by `player_enemy` / `player_ally`.
    pub enemies: Vec<u8>,
    pub allies: Vec<u8>,
    /// Original program, used to start `expand`/`multirun` children at a
    /// code offset.
    pub script: AiScript,
}

impl CampaignAi {
    pub fn new(player: u8, script: &AiScript) -> Self {
        Self {
            player,
            id: script.id,
            instructions: script.instructions(),
            pc: 0,
            wait_until: 0,
            max_units: HashMap::new(),
            attack_group: Vec::new(),
            town_started: false,
            finished: false,
            call_stack: Vec::new(),
            rng: 0x9e37_79b9_7f4a_7c15,
            subthreads: Vec::new(),
            spawned_blocks: HashSet::new(),
            campaign: false,
            default_build: true,
            farms_timing: false,
            farms_notiming: false,
            max_force: 1500,
            nuke_rate: 0,
            nukes: 0,
            harass_factor: 0,
            region_size: 0,
            default_min: 0,
            transports_off: false,
            shared_vision: Vec::new(),
            location: None,
            gencmd: None,
            defense_requests: [None; 4],
            defense_units: [None; 4],
            attack_prepared: false,
            killable: false,
            scout_unit: None,
            bunkers: 0,
            turrets: 0,
            random_seed_set: false,
            enemies: Vec::new(),
            allies: Vec::new(),
            script: script.clone(),
        }
    }

    /// Start a child thread at a code offset.
    fn at_offset(player: u8, script: &AiScript, offset: usize) -> Self {
        let mut ai = Self::new(player, script);
        ai.pc = ai
            .instructions
            .iter()
            .position(|i| i.offset >= offset)
            .unwrap_or(ai.instructions.len());
        ai
    }

    pub fn update(&mut self, world: &mut World, enemy_base: Option<(i32, i32)>) {
        self.maintain_town(world);
        self.maintain_army(world, enemy_base);
        self.update_main(world, enemy_base);
        self.update_subthreads(world, enemy_base);
    }

    /// `default_build` trains a race's standard army when the town has spare
    /// resources.
    fn maintain_army(&self, world: &mut World, enemy_base: Option<(i32, i32)>) {
        if !self.town_started {
            return;
        }
        let race = world.players[self.player as usize].race;
        let roster: &[u16] = match race {
            crate::sim::Race::Terran => &[
                crate::ids::unit::MARINE,
                crate::ids::unit::FIREBAT,
                crate::ids::unit::SIEGE_TANK,
                crate::ids::unit::GOLIATH,
                crate::ids::unit::WRAITH,
            ],
            crate::sim::Race::Protoss => &[
                crate::ids::unit::ZEALOT,
                crate::ids::unit::DRAGOON,
                crate::ids::unit::SCOUT,
                crate::ids::unit::CARRIER,
            ],
            crate::sim::Race::Zerg => &[
                crate::ids::unit::ZERGLING,
                crate::ids::unit::HYDRALISK,
                crate::ids::unit::MUTALISK,
                crate::ids::unit::ULTRALISK,
            ],
            crate::sim::Race::Neutral => return,
        };
        let p = &world.players[self.player as usize];
        if self.default_build && p.minerals >= 600 && p.gas >= 150 {
            for &unit in roster {
                if let Some(limit) = self.max_units.get(&unit)
                    && *limit != 255
                    && self.count(world, unit) >= *limit as i32
                {
                    continue;
                }
                if self.try_train_or_morph(world, unit) {
                    break;
                }
            }
        }
        // No periodic "send everything" here: the attacks are the script's
        // own `attack_add` / `attack_do` waves.  FACT (Desperate Alliance,
        // 2026-11-06): an every-480-ticks commit of all idle fighters sent
        // 96 Zerg units at the player at tick 480, while the retail recording
        // is calm for the first 2.5 minutes.
        let _ = enemy_base;
    }

    /// Retail `start_town` starts the engine-side town manager.  The manager
    /// keeps the worker count near the requested maximum and reacts to the
    /// `farms_*` supply policy.  Build/train requests from the script remain
    /// separate; this only supplies the always-on worker and supply logic.
    fn maintain_town(&self, world: &mut World) {
        if !self.town_started {
            return;
        }
        let worker = match world.players[self.player as usize].race {
            crate::sim::Race::Terran => crate::ids::unit::SCV,
            crate::sim::Race::Protoss => crate::ids::unit::PROBE,
            crate::sim::Race::Zerg => crate::ids::unit::DRONE,
            crate::sim::Race::Neutral => return,
        };
        let desired = self.max_units.get(&worker).copied().unwrap_or(24).min(30) as i32;
        if self.count(world, worker) + self.training_count(world, worker) < desired {
            let _ = self.try_train_or_morph(world, worker);
        }
        if self.farms_timing || self.farms_notiming {
            self.maintain_supply(world);
        }
    }

    fn update_main(&mut self, world: &mut World, enemy_base: Option<(i32, i32)>) {
        if self.pc >= self.instructions.len() {
            self.finished = true;
        }
        if self.finished || world.tick < self.wait_until {
            return;
        }
        let mut steps = 0;
        while self.pc < self.instructions.len() && steps < 128 {
            steps += 1;
            let ins = self.instructions[self.pc].clone();
            if !self.execute(world, enemy_base, &ins) {
                break;
            }
        }
    }

    fn update_subthreads(&mut self, world: &mut World, enemy_base: Option<(i32, i32)>) {
        let mut live = Vec::new();
        for mut th in std::mem::take(&mut self.subthreads) {
            th.update(world, enemy_base);
            if !th.finished {
                live.push(th);
            }
        }
        self.subthreads = live;
    }

    fn execute(
        &mut self,
        world: &mut World,
        enemy_base: Option<(i32, i32)>,
        ins: &Instruction,
    ) -> bool {
        match ins.opcode {
            0 => {
                // goto <code offset>
                if let Some(target) = ins.word(0) {
                    self.pc = self
                        .instructions
                        .iter()
                        .position(|i| i.offset >= target as usize)
                        .unwrap_or(self.instructions.len());
                } else {
                    self.pc += 1;
                }
            }
            2 => {
                self.pc += 1;
                self.wait_until = world.tick + ins.word(0).unwrap_or(0) as u64;
                return false;
            }
            3 | 4 | 56 => {
                if ins.opcode == 56 {
                    self.campaign = true;
                } else {
                    self.town_started = true;
                }
                self.pc += 1;
            }
            6 => {
                let count = ins.byte(0).unwrap_or(1);
                let unit_type = ins.word(1).unwrap_or(0);
                let already = self.count(world, unit_type) + self.training_count(world, unit_type);
                if already < count as i32 {
                    let building = world.data.unit(unit_type).is_building();
                    if building {
                        let _ = self.try_build(world, unit_type);
                    } else {
                        let _ = self.try_train_or_morph(world, unit_type);
                    }
                }
                // `build` enqueues a town request in the retail engine and
                // continues; it does not block the script thread.
                self.pc += 1;
            }
            7 => {
                let level = ins.byte(0).unwrap_or(1);
                let upgrade = ins.word(1).unwrap_or(0) as u8;
                if world.players[self.player as usize].upgrade_level(upgrade) >= level
                    || self.try_upgrade(world, upgrade)
                {
                    self.pc += 1;
                } else {
                    self.wait_until = world.tick + 24;
                    return false;
                }
            }
            8 => {
                let tech = ins.word(0).unwrap_or(0) as u8;
                if world.players[self.player as usize].has_tech(tech) || self.try_tech(world, tech)
                {
                    self.pc += 1;
                } else {
                    self.wait_until = world.tick + 24;
                    return false;
                }
            }
            1 => {
                // notowns_jump <unit> <address>
                let unit = ins.word(0).unwrap_or(0);
                if self.count(world, unit) == 0 {
                    self.jump_to(ins.word(1));
                } else {
                    self.pc += 1;
                }
            }
            12 => {
                let qty = ins.byte(0).unwrap_or(1) as usize;
                if let Some(unit) = ins.word(1) {
                    for _ in 0..qty.min(64) {
                        self.attack_group.push(unit);
                    }
                }
                self.pc += 1;
            }
            14 => {
                self.launch_attack(world, enemy_base, false);
                self.pc += 1;
                self.wait_until = world.tick + 48;
                return false;
            }
            36 => {
                self.finished = true;
                return false;
            }
            48 => {
                // random_jump <chance-out-of-256> <code offset>
                self.rng = self
                    .rng
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let chance = ins.byte(0).unwrap_or(0) as u64;
                if (self.rng >> 33) % 256 < chance {
                    self.jump_to(ins.word(1));
                } else {
                    self.pc += 1;
                }
            }
            57 => {
                // race_jump is relative to the nearest enemy player.
                let race = self
                    .enemy_player(world)
                    .map(|p| world.players[p].race)
                    .unwrap_or(crate::sim::Race::Neutral);
                let target = match race {
                    crate::sim::Race::Terran => ins.word(0),
                    crate::sim::Race::Zerg => ins.word(1),
                    crate::sim::Race::Protoss => ins.word(2),
                    crate::sim::Race::Neutral => None,
                };
                self.jump_to(target);
            }
            60 => {
                // groundmap_jump <offset>: jump when the enemy base is
                // reachable by a ground path (no transport required).
                let reachable = self
                    .base(world)
                    .zip(enemy_base)
                    .is_some_and(|(a, b)| self.ground_route(world, a, b));
                if reachable {
                    self.jump_to(ins.word(0));
                } else {
                    self.pc += 1;
                }
            }
            64 => {
                self.call_stack.push(self.pc + 1);
                self.jump_to(ins.word(0));
            }
            65 => {
                if let Some(target) = self.call_stack.pop() {
                    self.pc = target;
                } else {
                    self.pc += 1;
                }
            }
            49 => {
                // time_jump <minutes> <code offset>
                let minutes = ins.byte(0).unwrap_or(0) as u64;
                if world.tick / (crate::sim::TICKS_PER_SECOND as u64 * 60) < minutes {
                    self.pc += 1;
                } else if let Some(target) = ins.word(1) {
                    self.pc = self
                        .instructions
                        .iter()
                        .position(|i| i.offset >= target as usize)
                        .unwrap_or(self.instructions.len());
                }
            }
            75 => {
                if let (Some(unit), Some(max)) = (ins.word(1), ins.byte(0)) {
                    self.max_units.insert(unit, max);
                }
                self.pc += 1;
            }
            76 => {
                let qty = ins.byte(0).unwrap_or(1) as i32;
                let unit_type = ins.word(1).unwrap_or(0);
                if self.count(world, unit_type) + self.training_count(world, unit_type) >= qty {
                    self.pc += 1;
                } else if self.try_train(world, unit_type) {
                    self.wait_until = world.tick + 16;
                    return false;
                } else {
                    self.wait_until = world.tick + 24;
                    return false;
                }
            }
            78 => {
                let qty = ins.byte(0).unwrap_or(1) as i32;
                let unit_type = ins.word(1).unwrap_or(0);
                if self.count(world, unit_type) + self.training_count(world, unit_type) >= qty {
                    self.pc += 1;
                } else {
                    self.wait_until = world.tick + 24;
                    return false;
                }
            }
            93 => {
                // enemyowns_jump <unit> <address>
                let unit = ins.word(0).unwrap_or(0);
                let enemy_has = world
                    .units
                    .iter()
                    .any(|u| u.alive && u.owner != self.player && u.type_id == unit);
                if enemy_has {
                    self.jump_to(ins.word(1));
                } else {
                    self.pc += 1;
                }
            }
            94 => {
                // enemyresources_jump <minerals> <gas> <address>
                let enemy = world
                    .players
                    .iter()
                    .find(|p| p.team != world.players[self.player as usize].team);
                let enough = enemy.is_some_and(|p| {
                    p.minerals >= ins.word(0).unwrap_or(0) as i32
                        && p.gas >= ins.word(1).unwrap_or(0) as i32
                });
                if enough {
                    self.jump_to(ins.word(2));
                } else {
                    self.pc += 1;
                }
            }
            95 => {
                // if_dif <compare> <difficulty> <address>: no difficulty model.
                self.pc += 1;
            }
            106 => {
                // if_owned <unit> <address>
                let unit = ins.word(0).unwrap_or(0);
                if self.count(world, unit) > 0 {
                    self.jump_to(ins.word(1));
                } else {
                    self.pc += 1;
                }
            }
            108 => {
                // create_unit <unit> <x> <y>
                let unit = ins.word(0).unwrap_or(0);
                let x = ins.word(1).unwrap_or(0) as i32;
                let y = ins.word(2).unwrap_or(0) as i32;
                let _ = world.spawn_px(self.player as usize, unit, x, y);
                self.pc += 1;
            }
            109 => {
                // nuke_pos <x> <y>
                let x = ins.word(0).unwrap_or(0) as i32;
                let y = ins.word(1).unwrap_or(0) as i32;
                if let Some(ghost) = world.units.iter().find(|u| {
                    u.alive && u.owner == self.player && u.type_id == crate::ids::unit::GHOST
                }) {
                    let id = ghost.id;
                    let _ = world.launch_nuke(id, (x, y));
                }
                self.pc += 1;
            }
            113 => {
                // if_towns has no operands in the shipped interpreter.
                self.pc += 1;
            }
            84 => {
                // resources_jump <minerals> <gas> <code offset>
                let minerals = ins.word(0).unwrap_or(0) as i32;
                let gas = ins.word(1).unwrap_or(0) as i32;
                let p = &world.players[self.player as usize];
                if p.minerals >= minerals && p.gas >= gas {
                    self.pc += 1;
                } else if let Some(target) = ins.word(2) {
                    self.pc = self
                        .instructions
                        .iter()
                        .position(|i| i.offset >= target as usize)
                        .unwrap_or(self.instructions.len());
                }
            }
            5 => {
                // expand <expansion> <block>
                if let Some(block) = ins.word(1) {
                    self.spawn_thread(block);
                }
                self.pc += 1;
            }
            9 => {
                let qty = ins.byte(0).unwrap_or(1) as i32;
                let unit_type = ins.word(1).unwrap_or(0);
                if self.completed_count(world, unit_type) >= qty {
                    self.pc += 1;
                } else {
                    self.wait_until = world.tick + 16;
                    return false;
                }
            }
            10 => {
                let qty = ins.byte(0).unwrap_or(1) as i32;
                let unit_type = ins.word(1).unwrap_or(0);
                if self.count(world, unit_type) >= qty {
                    self.pc += 1;
                } else {
                    self.wait_until = world.tick + 16;
                    return false;
                }
            }
            11 => {
                self.attack_group.clear();
                self.attack_prepared = false;
                self.pc += 1;
            }
            13 => {
                self.attack_prepared = true;
                self.pc += 1;
            }
            15 => {
                let trained =
                    self.defense_units
                        .iter()
                        .flatten()
                        .any(|(_, u)| self.count(world, *u) > 0)
                        || self.defense_requests.iter().flatten().any(|(_, u)| {
                            self.count(world, *u) + self.training_count(world, *u) > 0
                        });
                if trained {
                    self.pc += 1;
                } else {
                    self.wait_until = world.tick + 24;
                    return false;
                }
            }
            16 => {
                // capt_expand: acquire a nearby expansion town.
                let hall = self.town_hall_type(world);
                let _ = self.try_build(world, hall);
                self.pc += 1;
            }
            17 => {
                if self.bunkers < 3 && self.try_build(world, crate::ids::unit::BUNKER) {
                    self.bunkers += 1;
                }
                self.pc += 1;
            }
            18 => {
                let built = self.count(world, crate::ids::unit::BUNKER);
                if built >= self.bunkers as i32 || self.bunkers >= 3 {
                    self.pc += 1;
                } else {
                    self.wait_until = world.tick + 16;
                    return false;
                }
            }
            19..=22 => {
                let idx = (ins.opcode - 19) as usize;
                let qty = ins.byte(0).unwrap_or(1);
                let unit = ins.word(1).unwrap_or(0);
                self.defense_requests[idx] = Some((qty, unit));
                if self.count(world, unit) >= qty as i32 || self.try_train(world, unit) {
                    self.pc += 1;
                } else {
                    self.wait_until = world.tick + 24;
                    return false;
                }
            }
            23..=26 => {
                let idx = (ins.opcode - 23) as usize;
                let qty = ins.byte(0).unwrap_or(1);
                let unit = ins.word(1).unwrap_or(0);
                self.defense_units[idx] = Some((qty, unit));
                self.pc += 1;
            }
            27..=30 => {
                let idx = (ins.opcode - 27) as usize;
                self.defense_requests[idx] = None;
                self.defense_units[idx] = None;
                self.pc += 1;
            }
            31 => {
                // send_suicide: 0 = strategic, 1 = random.
                if ins.byte(0).unwrap_or(0) == 1 {
                    if let Some(target) = self.nearest_enemy_unit(world) {
                        let ids: Vec<u32> = world
                            .units
                            .iter()
                            .filter(|u| u.alive && u.owner == self.player)
                            .map(|u| u.id)
                            .collect();
                        world.command_attack_move(&ids, (target.0 / 256, target.1 / 256));
                    }
                } else {
                    self.launch_attack(world, enemy_base, false);
                }
                self.pc += 1;
            }
            32 => {
                let target = self.location_center(world);
                for (i, u) in world.units.iter().enumerate() {
                    if u.alive
                        && (u.x / 256 - target.0).abs() < 12
                        && (u.y / 256 - target.1).abs() < 12
                        && i as u8 != self.player
                        && !self.enemies.contains(&(i as u8))
                    {
                        self.enemies.push(i as u8);
                    }
                }
                self.pc += 1;
            }
            33 => {
                let target = self.location_center(world);
                for (i, u) in world.units.iter().enumerate() {
                    if u.alive
                        && (u.x / 256 - target.0).abs() < 12
                        && (u.y / 256 - target.1).abs() < 12
                        && i as u8 != self.player
                        && !self.allies.contains(&(i as u8))
                    {
                        self.allies.push(i as u8);
                    }
                }
                self.pc += 1;
            }
            34 => {
                self.default_min = ins.word(0).unwrap_or(0);
                self.pc += 1;
            }
            35 => {
                self.default_build = false;
                self.pc += 1;
            }
            37 => {
                // switch_rescue: a rescuable player has no scripted economy.
                self.finished = true;
                return false;
            }
            38 | 39 => {
                // move_dt / debug: location-targeted command.
                self.pc += 1;
            }
            40 => {
                // fatal_error
                self.finished = true;
                return false;
            }
            41 => {
                // enter_bunker: load eligible infantry into the nearest bunker.
                if let Some(bunker) = world.units.iter().find(|u| {
                    u.alive && u.owner == self.player && u.type_id == crate::ids::unit::BUNKER
                }) {
                    let bid = bunker.id;
                    let (bx, by) = (bunker.x, bunker.y);
                    let ids: Vec<u32> = world
                        .units
                        .iter()
                        .filter(|u| {
                            u.alive
                                && u.owner == self.player
                                && !world.data.unit(u.type_id).is_building()
                                && u.type_id != crate::ids::unit::SCV
                                && u.type_id != crate::ids::unit::PROBE
                                && u.type_id != crate::ids::unit::DRONE
                                && (u.x - bx).abs() < 8 * 256
                                && (u.y - by).abs() < 8 * 256
                        })
                        .map(|u| u.id)
                        .collect();
                    for id in ids {
                        let _ =
                            world.issue(id, crate::sim::Order::EnterTransport { transport: bid });
                    }
                }
                self.pc += 1;
            }
            42 => {
                self.pc += 1;
            }
            43 => {
                self.transports_off = true;
                self.pc += 1;
            }
            44 => {
                let t = match world.players[self.player as usize].race {
                    crate::sim::Race::Terran => crate::ids::unit::DROPSHIP,
                    crate::sim::Race::Protoss => crate::ids::unit::SHUTTLE,
                    crate::sim::Race::Zerg => crate::ids::unit::OVERLORD,
                    crate::sim::Race::Neutral => crate::ids::unit::DROPSHIP,
                };
                let _ = self.try_train(world, t);
                self.pc += 1;
            }
            45 => {
                self.nuke_rate = ins.byte(0).unwrap_or(0) as u32;
                self.pc += 1;
            }
            46 => {
                self.max_force = ins.word(0).unwrap_or(0) as i32;
                self.pc += 1;
            }
            47 => {
                self.attack_group.clear();
                self.pc += 1;
            }
            50 => {
                self.farms_notiming = true;
                self.farms_timing = false;
                self.maintain_supply(world);
                self.pc += 1;
            }
            51 => {
                self.farms_timing = true;
                self.farms_notiming = false;
                self.maintain_supply(world);
                self.pc += 1;
            }
            52 => {
                if self.turrets < 6 && self.try_build(world, crate::ids::unit::MISSILE_TURRET) {
                    self.turrets += 1;
                }
                self.pc += 1;
            }
            53 => {
                let built = self.count(world, crate::ids::unit::MISSILE_TURRET);
                if built >= self.turrets as i32 || self.turrets >= 6 {
                    self.pc += 1;
                } else {
                    self.wait_until = world.tick + 16;
                    return false;
                }
            }
            54 => {
                self.default_build = true;
                self.pc += 1;
            }
            55 => {
                self.harass_factor = ins.byte(0).unwrap_or(0);
                self.pc += 1;
            }
            58 => {
                let limit = ins.byte(0).unwrap_or(0) as i32 * 32;
                if self.region_count(world) < limit {
                    self.jump_to(ins.word(1));
                } else {
                    self.pc += 1;
                }
            }
            59 => {
                self.pc += 1;
            }
            61 => {
                let unit = ins.word(0).unwrap_or(0);
                let _ = self.try_train(world, unit);
                self.pc += 1;
            }
            62 => {
                let qty = ins.byte(0).unwrap_or(1) as i32;
                let unit = ins.word(1).unwrap_or(0);
                if self.count(world, unit) + self.training_count(world, unit) >= qty {
                    self.pc += 1;
                } else {
                    let _ = self.try_train(world, unit);
                    self.wait_until = world.tick + 16;
                    return false;
                }
            }
            63 => {
                let unit = ins.word(0).unwrap_or(0);
                let _ = self.try_train(world, unit);
                self.pc += 1;
            }
            66 => {
                let ours = self.military_ids(world).len() as i32;
                let theirs = world
                    .units
                    .iter()
                    .filter(|u| {
                        u.alive
                            && u.owner != self.player
                            && !world.data.unit(u.type_id).is_building()
                    })
                    .count() as i32;
                if ours > theirs {
                    self.launch_attack(world, enemy_base, false);
                    self.jump_to(ins.word(0));
                } else {
                    self.pc += 1;
                }
            }
            67 => {
                let unit = match world.players[self.player as usize].race {
                    crate::sim::Race::Protoss => crate::ids::unit::PHOTON_CANNON,
                    _ => crate::ids::unit::CREEP_COLONY,
                };
                let _ = self.try_build(world, unit);
                self.pc += 1;
            }
            68 => {
                let low = world
                    .units
                    .iter()
                    .filter(|u| u.alive && u.is_resource())
                    .map(|u| u.resource_amount)
                    .sum::<i32>()
                    < 7500;
                if low {
                    self.jump_to(ins.word(0));
                } else {
                    self.pc += 1;
                }
            }
            69 => {
                let qty = ins.byte(0).unwrap_or(1) as i32;
                let unit = ins.word(1).unwrap_or(0);
                if self.count(world, unit) < qty {
                    let _ = self.try_build(world, unit);
                }
                self.pc += 1;
            }
            70 => {
                let qty = ins.byte(0).unwrap_or(1) as i32;
                let unit = ins.word(1).unwrap_or(0);
                if self.count(world, unit) + self.training_count(world, unit) < qty {
                    let _ = self.try_train_or_morph(world, unit);
                }
                self.pc += 1;
            }
            71 => {
                self.pc += 1;
            }
            72 => {
                if let Some(block) = ins.word(0) {
                    self.spawn_thread(block);
                }
                self.pc += 1;
            }
            73 => {
                let seconds = ins.word(0).unwrap_or(0) as u64;
                if world.tick / crate::sim::TICKS_PER_SECOND as u64 >= seconds {
                    self.jump_to(ins.word(1));
                } else {
                    self.pc += 1;
                }
            }
            74 => {
                self.scout_unit = ins.word(0);
                self.pc += 1;
            }
            77 => {
                self.launch_attack(world, enemy_base, true);
                self.pc += 1;
            }
            79 => {
                self.pc += 1;
            }
            80 => {
                self.gencmd = Some(self.location_center(world));
                self.pc += 1;
            }
            81 => {
                if let Some(target) = self.gencmd {
                    let ids = self.military_ids(world);
                    for id in ids {
                        let _ = world.issue(
                            id,
                            crate::sim::Order::Patrol {
                                a: target,
                                b: target,
                                forward: true,
                            },
                        );
                    }
                }
                self.pc += 1;
            }
            82 => {
                if world.players[self.player as usize].minerals < 500 {
                    world.players[self.player as usize].minerals += 2000;
                }
                if world.players[self.player as usize].gas < 500 {
                    world.players[self.player as usize].gas += 2000;
                }
                self.pc += 1;
            }
            83 => {
                let keep = ins.byte(0).unwrap_or(0) as usize;
                let min = ins.byte(1).unwrap_or(0) as usize;
                let unit = ins.word(2).unwrap_or(0);
                let mut ids: Vec<u32> = world
                    .units
                    .iter()
                    .filter(|u| {
                        u.alive
                            && u.owner == self.player
                            && u.type_id == unit
                            && !world.data.unit(u.type_id).is_building()
                    })
                    .map(|u| u.id)
                    .collect();
                let take = ids.len().saturating_sub(keep).max(min);
                ids.truncate(take);
                if let Some(base) = enemy_base {
                    world.command_attack_move(&ids, base);
                }
                self.pc += 1;
            }
            85 => {
                if let Some(transport) = self.nearest_transport(world) {
                    let ids: Vec<u32> = world
                        .units
                        .iter()
                        .filter(|u| {
                            u.alive
                                && u.owner == self.player
                                && !world.data.unit(u.type_id).is_building()
                                && u.id != transport
                        })
                        .map(|u| u.id)
                        .collect();
                    for id in ids.into_iter().take(8) {
                        let _ = world.issue(id, crate::sim::Order::EnterTransport { transport });
                    }
                }
                self.pc += 1;
            }
            86 => {
                let transports: Vec<u32> = world
                    .units
                    .iter()
                    .filter(|u| {
                        u.alive
                            && u.owner == self.player
                            && world.data.unit(u.type_id).space_provided > 0
                    })
                    .map(|u| u.id)
                    .collect();
                for id in transports {
                    let _ = world.issue(id, crate::sim::Order::Unload);
                }
                self.pc += 1;
            }
            87 => {
                let p = ins.byte(0).unwrap_or(0);
                if !self.shared_vision.contains(&p) {
                    self.shared_vision.push(p);
                }
                self.pc += 1;
            }
            88 => {
                let p = ins.byte(0).unwrap_or(0);
                self.shared_vision.retain(|x| *x != p);
                self.pc += 1;
            }
            89 => {
                let (x, y) = self.location_center(world);
                if let Some(ghost) = world.units.iter().find(|u| {
                    u.alive && u.owner == self.player && u.type_id == crate::ids::unit::GHOST
                }) {
                    let id = ghost.id;
                    let _ = world.launch_nuke(id, (x, y));
                }
                self.pc += 1;
            }
            90 => {
                self.harass_factor = self.harass_factor.saturating_add(1);
                self.pc += 1;
            }
            91 | 92 => {
                self.pc += 1;
            }
            96 => {
                let qty = ins.byte(0).unwrap_or(1) as usize;
                if let Some(unit) = ins.word(1) {
                    for _ in 0..qty.min(64) {
                        self.attack_group.push(unit);
                    }
                }
                self.pc += 1;
            }
            97 => {
                if self.killable {
                    self.finished = true;
                    return false;
                }
                self.pc += 1;
            }
            98 => {
                self.killable = true;
                self.pc += 1;
            }
            99 | 100 => {
                self.pc += 1;
            }
            101 => {
                let ids: Vec<u32> = world
                    .units
                    .iter()
                    .filter(|u| {
                        u.alive && u.owner == self.player && u.order == crate::sim::Order::Idle
                    })
                    .map(|u| u.id)
                    .collect();
                for id in ids {
                    let x = (world.world_rand() % (world.terrain.width.max(1) as u32)) as i32;
                    let y = (world.world_rand() % (world.terrain.height.max(1) as u32)) as i32;
                    let _ = world.issue(id, crate::sim::Order::Move { pos: (x, y) });
                }
                self.pc += 1;
            }
            102 => {
                self.pc += 1;
            }
            103 => {
                if let Some(corsair) = world.units.iter().find(|u| {
                    u.alive && u.owner == self.player && u.type_id == crate::ids::unit::CORSAIR
                }) {
                    let id = corsair.id;
                    let (x, y) = self.location_center(world);
                    let _ = world.cast(
                        id,
                        crate::sim::Spell::DisruptionWeb,
                        crate::sim::SpellTarget::Point(x, y),
                    );
                }
                self.pc += 1;
            }
            104 => {
                if let Some(arbiter) = world.units.iter().find(|u| {
                    u.alive && u.owner == self.player && u.type_id == crate::ids::unit::ARBITER
                }) {
                    let id = arbiter.id;
                    let (x, y) = self.location_center(world);
                    let _ = world.cast(
                        id,
                        crate::sim::Spell::Recall,
                        crate::sim::SpellTarget::Point(x, y),
                    );
                }
                self.pc += 1;
            }
            105 => {
                if let Some(crate::aiscript::Operand::Dword(v)) = ins.args.first() {
                    self.rng = *v as u64;
                    self.random_seed_set = true;
                }
                self.pc += 1;
            }
            107 => {
                self.nukes = self.nukes.saturating_add(1);
                self.pc += 1;
            }
            110 => {
                self.pc += 1;
            }
            111 => {
                if let Some(block) = ins.word(1) {
                    self.spawn_thread(block);
                }
                self.pc += 1;
            }
            112 => {
                let qty = ins.byte(0).unwrap_or(0) as i32;
                if self.town_halls(world) < qty {
                    self.jump_to(ins.word(1));
                } else {
                    self.pc += 1;
                }
            }
            _ => {
                self.pc += 1;
            }
        }
        true
    }

    fn jump_to(&mut self, target: Option<u16>) {
        if let Some(target) = target {
            self.pc = self
                .instructions
                .iter()
                .position(|i| i.offset >= target as usize)
                .unwrap_or(self.instructions.len());
        } else {
            self.pc += 1;
        }
    }

    fn count(&self, world: &World, unit_type: u16) -> i32 {
        world
            .units
            .iter()
            .filter(|u| u.alive && u.owner == self.player && u.type_id == unit_type)
            .count() as i32
    }

    fn base(&self, world: &World) -> Option<(i32, i32)> {
        world
            .units
            .iter()
            .find(|u| {
                u.alive
                    && u.owner == self.player
                    && matches!(
                        u.type_id,
                        crate::ids::unit::COMMAND_CENTER
                            | crate::ids::unit::NEXUS
                            | crate::ids::unit::HATCHERY
                            | crate::ids::unit::LAIR
                            | crate::ids::unit::HIVE
                    )
            })
            .map(|u| (u.x / 256, u.y / 256))
    }

    fn try_build(&self, world: &mut World, unit_type: u16) -> bool {
        let Some(info) = crate::tech::build_info(unit_type) else {
            return true;
        };
        if !world.has_requirements(self.player, unit_type)
            || !world.can_afford(self.player, unit_type)
        {
            return false;
        }
        if world
            .units
            .iter()
            .any(|u| u.alive && u.owner == self.player && u.type_id == unit_type && !u.completed)
        {
            return true;
        }
        // Building morphs and add-ons do not use a worker construction order.
        if let Some(source) = world.morph_source_unit(self.player, unit_type) {
            return world.morph_building(source, unit_type).is_ok();
        }
        if let Some(parent) = world.addon_parent_unit(self.player, unit_type) {
            return world.build_addon(parent, unit_type).is_ok();
        }
        // Do not issue the same construction request to every idle worker:
        // the building unit is not spawned until the worker reaches the site.
        if world.units.iter().any(|u| {
            u.alive
                && u.owner == self.player
                && matches!(u.order, crate::sim::Order::Build { unit_type: t, .. } if t == unit_type)
        }) {
            return true;
        }
        let Some(base) = self.base(world) else {
            return false;
        };
        let Some(spot) = world.find_build_spot(unit_type, base) else {
            return false;
        };
        let worker_type = info.producer;
        let worker = world.units.iter().find(|u| {
            u.alive
                && u.owner == self.player
                && u.type_id == worker_type
                && matches!(u.order, crate::sim::Order::Idle)
                && u.carry_kind == crate::sim::CarryKind::None
        });
        let Some(worker) = worker else {
            return false;
        };
        let id = worker.id;
        world
            .issue(
                id,
                crate::sim::Order::Build {
                    unit_type,
                    pos: spot,
                },
            )
            .is_ok()
    }

    fn try_train(&self, world: &mut World, unit_type: u16) -> bool {
        let Some(info) = crate::tech::build_info(unit_type) else {
            return true;
        };
        if !world.has_requirements(self.player, unit_type)
            || !world.can_afford(self.player, unit_type)
        {
            return false;
        }
        if let Some(max) = self.max_units.get(&unit_type)
            && self.count(world, unit_type) >= *max as i32
        {
            return true;
        }
        let producer = world.units.iter().find(|u| {
            u.alive && u.completed && u.owner == self.player && u.type_id == info.producer
        });
        let Some(producer) = producer else {
            return false;
        };
        let id = producer.id;
        world.train(id, unit_type).is_ok()
    }

    fn try_tech(&self, world: &mut World, tech: u8) -> bool {
        let Some(building) = world.units.iter().find(|u| {
            u.alive
                && u.completed
                && u.owner == self.player
                && crate::tech::tech_buildings(tech).contains(&u.type_id)
        }) else {
            return false;
        };
        let id = building.id;
        world.research(id, tech).is_ok()
    }

    fn try_upgrade(&self, world: &mut World, upgrade: u8) -> bool {
        let level = world.players[self.player as usize].upgrade_level(upgrade);
        let Some(building) = world.units.iter().find(|u| {
            u.alive
                && u.completed
                && u.owner == self.player
                && crate::tech::upgrade_buildings(upgrade).contains(&u.type_id)
        }) else {
            return false;
        };
        let id = building.id;
        let next = level.saturating_add(1);
        if !crate::tech::upgrade_level_requirement(upgrade, level)
            .iter()
            .all(|required| self.count(world, *required) > 0)
        {
            return false;
        }
        world.upgrade(id, upgrade).is_ok() && next > level
    }

    fn completed_count(&self, world: &World, unit_type: u16) -> i32 {
        world
            .units
            .iter()
            .filter(|u| u.alive && u.completed && u.owner == self.player && u.type_id == unit_type)
            .count() as i32
    }

    fn training_count(&self, world: &World, unit_type: u16) -> i32 {
        world
            .units
            .iter()
            .filter(|u| u.alive && u.owner == self.player)
            .map(|u| {
                u.production
                    .iter()
                    .filter(|o| matches!(o, crate::sim::Order::Train { unit_type: t } if *t == unit_type))
                    .count() as i32
            })
            .sum()
    }

    fn town_hall_type(&self, world: &World) -> u16 {
        match world.players[self.player as usize].race {
            crate::sim::Race::Terran => crate::ids::unit::COMMAND_CENTER,
            crate::sim::Race::Protoss => crate::ids::unit::NEXUS,
            crate::sim::Race::Zerg => crate::ids::unit::HATCHERY,
            crate::sim::Race::Neutral => crate::ids::unit::COMMAND_CENTER,
        }
    }

    fn town_halls(&self, world: &World) -> i32 {
        world
            .units
            .iter()
            .filter(|u| {
                u.alive
                    && u.completed
                    && u.owner == self.player
                    && matches!(
                        u.type_id,
                        crate::ids::unit::COMMAND_CENTER
                            | crate::ids::unit::NEXUS
                            | crate::ids::unit::HATCHERY
                            | crate::ids::unit::LAIR
                            | crate::ids::unit::HIVE
                    )
            })
            .count() as i32
    }

    fn enemy_player(&self, world: &World) -> Option<usize> {
        if let Some(&p) = self
            .enemies
            .iter()
            .find(|p| (**p as usize) < world.players.len())
        {
            return Some(p as usize);
        }
        let team = world.players[self.player as usize].team;
        world
            .players
            .iter()
            .enumerate()
            .find(|(i, p)| *i != self.player as usize && p.team != team)
            .map(|(i, _)| i)
    }

    /// The attack force of `attack_do`: `attack_add N unit` asks for N units
    /// of that type, so exactly N are taken per type, nearest the AI's town
    /// first.  FACT (Desperate Alliance, 2026-11-06): taking every unit of a
    /// listed type sent all 75 Zerglings at tick ~600 for `attack_add 2
    /// Zergling`, while the retail recording is calm for 2.5 minutes.  An
    /// empty group (a bare `attack_do`) takes the fighting units only: never
    /// workers, Overlords or Larvae (they had joined the attack too).
    fn military_ids(&self, world: &World) -> Vec<u32> {
        let (bx, by) = self.base(world).unwrap_or((0, 0));
        let fighter = |u: &crate::sim::SimUnit| {
            let ty = world.data.unit(u.type_id);
            u.alive
                && u.completed
                && u.owner == self.player
                && !ty.is_building()
                && !ty.is_worker()
                && !matches!(
                    u.type_id,
                    crate::ids::unit::OVERLORD | crate::ids::unit::LARVA | crate::ids::unit::EGG
                )
        };
        let mut by_dist: Vec<(i64, u16, u32)> = world
            .units
            .iter()
            .filter(|u| fighter(u))
            .map(|u| {
                let (dx, dy) = ((u.x - bx) as i64 / 256, (u.y - by) as i64 / 256);
                (dx * dx + dy * dy, u.type_id, u.id)
            })
            .collect();
        by_dist.sort();
        if self.attack_group.is_empty() {
            return by_dist.into_iter().map(|(_, _, id)| id).collect();
        }
        let mut want: HashMap<u16, usize> = HashMap::new();
        for t in &self.attack_group {
            *want.entry(*t).or_default() += 1;
        }
        by_dist
            .into_iter()
            .filter(|(_, t, _)| match want.get_mut(t) {
                Some(n) if *n > 0 => {
                    *n -= 1;
                    true
                }
                _ => false,
            })
            .map(|(_, _, id)| id)
            .collect()
    }

    fn launch_attack(&self, world: &mut World, enemy_base: Option<(i32, i32)>, _group_only: bool) {
        let ids = self.military_ids(world);
        if ids.is_empty() {
            return;
        }
        let target = enemy_base
            .or_else(|| {
                self.nearest_enemy_unit(world)
                    .map(|(x, y)| (x / 256, y / 256))
            })
            .unwrap_or((0, 0));
        world.command_attack_move(&ids, target);
    }

    fn nearest_enemy_unit(&self, world: &World) -> Option<(i32, i32)> {
        let (bx, by) = self.base(world).unwrap_or((0, 0));
        world
            .units
            .iter()
            .filter(|u| {
                u.alive && u.owner != self.player && !world.data.unit(u.type_id).is_building()
            })
            .min_by_key(|u| {
                let dx = u.x / 256 - bx;
                let dy = u.y / 256 - by;
                dx * dx + dy * dy
            })
            .map(|u| (u.x, u.y))
    }

    fn nearest_transport(&self, world: &World) -> Option<u32> {
        world
            .units
            .iter()
            .filter(|u| {
                u.alive
                    && u.owner == self.player
                    && world.data.unit(u.type_id).space_provided > 0
                    && u.cargo.len() < world.data.unit(u.type_id).space_provided as usize
            })
            .map(|u| u.id)
            .next()
    }

    fn location_center(&self, world: &World) -> (i32, i32) {
        self.location.or_else(|| self.base(world)).unwrap_or((0, 0))
    }

    fn spawn_thread(&mut self, target: u16) {
        if self.subthreads.len() >= 16 {
            return;
        }
        if !self.spawned_blocks.insert(target as usize) {
            return;
        }
        let mut child = CampaignAi::at_offset(self.player, &self.script, target as usize);
        child.campaign = self.campaign;
        child.default_build = self.default_build;
        child.location = self.location;
        self.subthreads.push(Box::new(child));
    }

    fn region_count(&self, world: &World) -> i32 {
        world.regions.count() as i32
    }

    fn ground_route(&self, world: &World, a: (i32, i32), b: (i32, i32)) -> bool {
        let obstacles = Obstacles::new();
        find_path(
            &world.terrain,
            &obstacles,
            &PathRequest {
                start: a,
                goal: b,
                half_width: 4,
                half_height: 4,
                flying: false,
            },
        )
        .is_some()
    }

    fn maintain_supply(&self, world: &mut World) {
        let used = world.supply_used(self.player);
        let cap = world.supply_cap(self.player);
        if cap - used > 4 {
            return;
        }
        match world.players[self.player as usize].race {
            crate::sim::Race::Terran => {
                let _ = self.try_build(world, crate::ids::unit::SUPPLY_DEPOT);
            }
            crate::sim::Race::Protoss => {
                let _ = self.try_build(world, crate::ids::unit::PYLON);
            }
            crate::sim::Race::Zerg => {
                let _ = self.try_train_or_morph(world, crate::ids::unit::OVERLORD);
            }
            crate::sim::Race::Neutral => {}
        }
    }

    fn try_train_or_morph(&self, world: &mut World, unit_type: u16) -> bool {
        let info = crate::tech::build_info(unit_type);
        if (info.is_some_and(|i| i.producer == crate::ids::unit::LARVA)
            || world.data.unit(unit_type).is_building())
            && let Some(larva) = world
                .units
                .iter()
                .find(|u| u.alive && u.owner == self.player && u.type_id == crate::ids::unit::LARVA)
        {
            let id = larva.id;
            return world.morph(id, unit_type).is_ok();
        }
        self.try_train(world, unit_type)
    }
}

/// Opcode names, indexed by opcode byte.  From the retail interpreter and the
/// independent PyMS `AIBIN` command table.
pub const OPCODE_NAMES: &[&str] = &[
    "goto",
    "notowns_jump",
    "wait",
    "start_town",
    "start_areatown",
    "expand",
    "build",
    "upgrade",
    "tech",
    "wait_build",
    "wait_buildstart",
    "attack_clear",
    "attack_add",
    "attack_prepare",
    "attack_do",
    "wait_secure",
    "capt_expand",
    "build_bunkers",
    "wait_bunkers",
    "defensebuild_gg",
    "defensebuild_ag",
    "defensebuild_ga",
    "defensebuild_aa",
    "defenseuse_gg",
    "defenseuse_ag",
    "defenseuse_ga",
    "defenseuse_aa",
    "defenseclear_gg",
    "defenseclear_ag",
    "defenseclear_ga",
    "defenseclear_aa",
    "send_suicide",
    "player_enemy",
    "player_ally",
    "default_min",
    "defaultbuild_off",
    "stop",
    "switch_rescue",
    "move_dt",
    "debug",
    "fatal_error",
    "enter_bunker",
    "value_area",
    "transports_off",
    "check_transports",
    "nuke_rate",
    "max_force",
    "clear_combatdata",
    "random_jump",
    "time_jump",
    "farms_notiming",
    "farms_timing",
    "build_turrets",
    "wait_turrets",
    "default_build",
    "harass_factor",
    "start_campaign",
    "race_jump",
    "region_size",
    "get_oldpeons",
    "groundmap_jump",
    "place_guard",
    "wait_force",
    "guard_resources",
    "call",
    "return",
    "eval_harass",
    "creep",
    "panic",
    "player_need",
    "do_morph",
    "wait_upgrades",
    "multirun",
    "rush",
    "scout_with",
    "define_max",
    "train",
    "target_expansion",
    "wait_train",
    "set_attacks",
    "set_gencmd",
    "make_patrol",
    "give_money",
    "prep_down",
    "resources_jump",
    "enter_transport",
    "exit_transport",
    "sharedvision_on",
    "sharedvision_off",
    "nuke_location",
    "harass_location",
    "implode",
    "guard_all",
    "enemyowns_jump",
    "enemyresources_jump",
    "if_dif",
    "easy_attack",
    "kill_thread",
    "killable",
    "wait_finishattack",
    "quick_attack",
    "junkyard_dog",
    "fake_nuke",
    "disruption_web",
    "recall_location",
    "set_randomseed",
    "if_owned",
    "create_nuke",
    "create_unit",
    "nuke_pos",
    "help_iftrouble",
    "allies_watch",
    "try_townpoint",
    "if_towns",
];

/// Operand kinds per opcode: 1 byte, 2 word, 3 dword, 4 string.
pub const OPCODE_ARGS: &[&[u8]] = &[
    &[2],
    &[2, 2],
    &[2],
    &[],
    &[],
    &[1, 2],
    &[1, 2, 1],
    &[1, 2, 1],
    &[2, 1],
    &[1, 2],
    &[1, 2],
    &[],
    &[1, 2],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[1, 2],
    &[1, 2],
    &[1, 2],
    &[1, 2],
    &[1, 2],
    &[1, 2],
    &[1, 2],
    &[1, 2],
    &[],
    &[],
    &[],
    &[],
    &[1],
    &[],
    &[],
    &[1],
    &[],
    &[],
    &[],
    &[],
    &[2, 4],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[1],
    &[2],
    &[],
    &[1, 2],
    &[1, 2],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[2],
    &[],
    &[2, 2, 2],
    &[1, 2],
    &[1],
    &[2],
    &[2, 1],
    &[1, 2],
    &[2],
    &[2],
    &[],
    &[2],
    &[1],
    &[2],
    &[1, 2],
    &[1, 2],
    &[],
    &[2],
    &[1, 2],
    &[2],
    &[1, 2],
    &[1, 2],
    &[],
    &[1, 2],
    &[1],
    &[],
    &[],
    &[],
    &[1, 1, 2],
    &[2, 2, 2],
    &[],
    &[],
    &[1],
    &[1],
    &[],
    &[],
    &[],
    &[],
    &[2, 2],
    &[2, 2, 2],
    &[1, 1, 2],
    &[1, 2],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[3],
    &[2, 2],
    &[],
    &[2, 2, 2],
    &[2, 2],
    &[],
    &[1, 2],
    &[1, 2],
    &[],
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_opcode_usage() {
        let ai = parse_aiscript(embedded!("fixtures/aiscript.bin"));
        let bw = parse_bwscript(embedded!("fixtures/bwscript.bin"));
        let mut counts = std::collections::BTreeMap::new();
        for set in [&ai, &bw] {
            for s in &set.scripts {
                for ins in s.instructions() {
                    *counts.entry(ins.opcode).or_insert(0usize) += 1;
                    if ins.opcode as usize >= OPCODE_NAMES.len() {
                        println!(
                            "unknown opcode {} in {} at {}",
                            ins.opcode,
                            s.id_str(),
                            ins.offset
                        );
                        let all = s.instructions();
                        if let Some(pos) = all.iter().position(|i| i.offset == ins.offset) {
                            for i in all[pos.saturating_sub(4)..=pos].iter() {
                                println!("   {} @{} {:?}", i.name(), i.offset, i.args);
                            }
                        }
                    }
                }
            }
        }
        for (op, n) in &counts {
            println!(
                "{op:3} {:24} {n}",
                OPCODE_NAMES.get(*op as usize).copied().unwrap_or("unknown")
            );
        }
        assert!(counts.keys().all(|op| (*op as usize) < OPCODE_NAMES.len()));
    }

    #[test]
    fn every_shipped_script_decodes_with_known_opcodes() {
        let ai = parse_aiscript(embedded!("fixtures/aiscript.bin"));
        let bw = parse_bwscript(embedded!("fixtures/bwscript.bin"));
        let mut total = 0usize;
        for set in [&ai, &bw] {
            for s in &set.scripts {
                for ins in s.instructions() {
                    assert!(
                        (ins.opcode as usize) < OPCODE_NAMES.len(),
                        "unknown opcode {} in {} at {}",
                        ins.opcode,
                        s.id_str(),
                        ins.offset
                    );
                    total += 1;
                }
            }
        }
        assert!(
            total > 10_000,
            "the shipped scripts contain a large program"
        );
    }

    #[test]
    fn melee_ai_runs_a_shipped_script_on_a_real_world() {
        use crate::data::GameData;
        use crate::ids::unit as uid;
        use crate::map::Terrain;
        use crate::sim::World;
        use std::sync::Arc;

        let ai_set = parse_aiscript(embedded!("fixtures/aiscript.bin"));
        let script = ai_set.get_str("TMCu").expect("Terran melee script");
        let mut ai = CampaignAi::new(0, script);
        let mut world = World::new(
            Arc::new(GameData::bundled().unwrap()),
            Terrain::open(64, 64),
            2,
        );
        world.players[0].minerals = 2000;
        world.spawn_px(0, uid::COMMAND_CENTER, 512, 512).unwrap();
        for k in 0..4 {
            world.spawn_px(0, uid::SCV, 560 + k * 24, 560).unwrap();
        }
        world.spawn_px(1, uid::COMMAND_CENTER, 1600, 1600).unwrap();
        for _ in 0..2400 {
            ai.update(&mut world, Some((50, 50)));
            world.step();
        }
        assert!(
            world
                .units
                .iter()
                .filter(|u| u.alive && u.owner == 0)
                .count()
                > 5,
            "the shipped melee AI builds or trains units"
        );
    }

    #[test]
    fn parses_both_shipped_tables() {
        let ai = parse_aiscript(embedded!("fixtures/aiscript.bin"));
        let bw = parse_bwscript(embedded!("fixtures/bwscript.bin"));
        assert_eq!(ai.scripts.len(), 293);
        assert_eq!(bw.scripts.len(), 156);
        assert_eq!(ai.scripts.iter().filter(|s| s.external).count(), 156);
        assert!(ai.get_str("Ter3").is_some());
        assert!(bw.get_str("PB1A").is_some());
        assert!(ai.scripts.iter().all(|s| s.label > 0));
    }

    #[test]
    fn campaign_interpreter_runs_build_and_train_commands() {
        use crate::data::GameData;
        use crate::ids::unit as uid;
        use crate::map::Terrain;
        use crate::sim::World;
        use std::sync::Arc;

        // start_town; train 1 SCV; stop
        let code = vec![3, 76, 1, 7, 0, 36];
        let script = AiScript {
            id: *b"Test",
            label: 1,
            flags: 0,
            code,
            external: false,
        };
        let mut ai = CampaignAi::new(0, &script);
        let mut world = World::new(
            Arc::new(GameData::bundled().unwrap()),
            Terrain::open(32, 32),
            2,
        );
        world.players[0].minerals = 500;
        world.spawn(0, uid::COMMAND_CENTER, 320, 320).unwrap();
        world.spawn(0, uid::SUPPLY_DEPOT, 420, 320).unwrap();
        for _ in 0..1200 {
            ai.update(&mut world, None);
            world.step();
        }
        assert!(
            world
                .units
                .iter()
                .any(|u| u.alive && u.owner == 0 && u.type_id == uid::SCV),
            "the AI script should train an SCV"
        );
    }

    #[test]
    fn attack_do_sends_the_requested_count_and_no_periodic_attack() {
        // Desperate Alliance: `attack_add 2 Zergling` sent all 75 Zerglings
        // (plus Drones and Overlords), and an invented every-480-ticks
        // commit sent the whole army at tick 480.
        use crate::data::GameData;
        use crate::ids::unit as uid;
        use crate::map::Terrain;
        use crate::sim::{Order, Race, World};
        use std::sync::Arc;

        let mut world = World::new(
            Arc::new(GameData::bundled().unwrap()),
            Terrain::open(64, 64),
            2,
        );
        world.players[0].race = Race::Zerg;
        world.spawn_px(0, uid::HATCHERY, 320, 320).unwrap();
        for k in 0..10 {
            world.spawn_px(0, uid::ZERGLING, 420 + k * 20, 420).unwrap();
        }
        world.spawn_px(0, uid::DRONE, 300, 420).unwrap();
        world.spawn_px(0, uid::OVERLORD, 300, 460).unwrap();
        let attacking = |w: &World| {
            w.units
                .iter()
                .filter(|u| u.alive && u.owner == 0 && matches!(u.order, Order::AttackMove { .. }))
                .map(|u| u.type_id)
                .collect::<Vec<_>>()
        };
        // start_town; wait 600; stop: nothing may attack on its own.
        let idle = AiScript {
            id: *b"Idle",
            label: 1,
            flags: 0,
            code: vec![3, 2, 0x58, 0x02, 36],
            external: false,
        };
        let mut ai = CampaignAi::new(0, &idle);
        for _ in 0..600 {
            ai.update(&mut world, Some((1800, 1800)));
            world.step();
        }
        assert!(attacking(&world).is_empty(), "no attack without attack_do");
        // attack_add 2 Zergling; attack_do; stop.
        let wave = AiScript {
            id: *b"Wave",
            label: 1,
            flags: 0,
            code: vec![12, 2, 37, 0, 14, 36],
            external: false,
        };
        let mut ai = CampaignAi::new(0, &wave);
        ai.update(&mut world, Some((1800, 1800)));
        assert_eq!(attacking(&world), vec![uid::ZERGLING; 2]);
    }

    #[test]
    fn decodes_known_instructions() {
        let ai = parse_aiscript(embedded!("fixtures/aiscript.bin"));
        let script = ai.get_str("Ter3").unwrap();
        let ins = script.instructions();
        assert!(!ins.is_empty());
        assert!(
            ins.iter()
                .any(|i| i.name() == "build" || i.name() == "train")
        );
        for i in &ins {
            assert!(
                i.opcode < OPCODE_NAMES.len() as u8,
                "bad opcode {} at {} in {}",
                i.opcode,
                i.offset,
                script.id_str()
            );
        }
    }
}

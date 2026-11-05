//! CHK trigger parsing and a deterministic subset of the trigger VM.
//!
//! A `TRIG` record is 2,400 bytes: 16 conditions (20 bytes each), 64 actions
//! (32 bytes each), a 32-bit execution-flags field and 28 per-player/force
//! enabled bytes.  The layout is the one used by the shipped `triggers\*.trg`
//! files and by the campaign maps.
//!
//! The classic condition/action ids are implemented.  StarCraft Remastered
//! adds a few extended ids; unknown ids are ignored rather than guessed, and
//! that limitation is recorded in `docs/EVIDENCE.md`.

use crate::sim::{Order, World};

pub const RECORD_SIZE: usize = 2400;
pub const TRIGGER_INTERVAL: u64 = 30;

/// `(player, four-byte script id, optional centre)` queued by trigger action
/// 15/16.
pub type PendingAiScript = (usize, [u8; 4], Option<(i32, i32)>);

#[derive(Clone, Debug, Default)]
pub struct TriggerCondition {
    /// 1-based index into the scenario's `MRGN` locations (0 = anywhere).
    pub location: u32,
    /// Player/force selector.
    pub group: u32,
    pub quantity: u32,
    pub unit_id: u16,
    /// 0 = at least, 1 = at most, 2 = exactly.
    pub comparison: u8,
    pub kind: u8,
    pub amount_modifier: u8,
}

/// How long a Transmission talks (ms).  INFERENCE from the public CHK
/// trigger spec (staredit.net `TRIG`): `time` holds the WAV length, the
/// second group field the modifier amount and `num` the modifier (7 set to,
/// 8 add, 9 subtract).  Falsifier: a retail transmission whose portrait
/// stops talking at a different moment.
pub fn transmission_ms(action: &TriggerAction) -> u32 {
    match action.num {
        7 => action.group2,
        8 => action.time.saturating_add(action.group2),
        9 => action.time.saturating_sub(action.group2),
        _ => action.time,
    }
}

/// Action flag "Always Display": the text shows even with subtitles off.
/// INFERENCE from the public CHK spec (staredit.net `TRIG`, action flags
/// bit 2); falsifier: a retail map whose flagged Display Text hides with
/// subtitles off.
pub const ALWAYS_DISPLAY: u8 = 0x04;

/// One thing a trigger shows or plays (`string` = 1-based `STR` id, 0 =
/// none).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriggerCue {
    Text {
        player: usize,
        string: u32,
        always: bool,
        /// Transmission length (0 for Display Text Message).
        ms: u32,
        /// A trigger's Pause Game was in force when the text was shown.
        paused: bool,
    },
    Wav {
        player: usize,
        string: u32,
    },
}

impl TriggerCue {
    pub fn player(&self) -> usize {
        match *self {
            TriggerCue::Text { player, .. } | TriggerCue::Wav { player, .. } => player,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TriggerAction {
    pub location: u32,
    pub text_id: u32,
    pub sound_id: u32,
    pub time: u32,
    pub group: u32,
    pub group2: u32,
    pub extra: u16,
    pub kind: u8,
    pub num: u8,
    pub flags: u8,
}

#[derive(Clone, Debug, Default)]
pub struct Trigger {
    pub conditions: Vec<TriggerCondition>,
    pub actions: Vec<TriggerAction>,
    pub execution_flags: u32,
    pub enabled: [bool; 28],
}

#[derive(Clone, Debug, Default)]
pub struct TriggerState {
    /// One entry per player, per trigger: has it fired and is it preserved?
    fired: Vec<Vec<bool>>,
    wait_until: Vec<u64>,
    /// Action index to resume at once a `Wait` action expires: `(trigger,
    /// next action)`.  StarCraft resumes the same trigger after a wait rather
    /// than restarting it, so this must survive the 30-tick trigger cadence.
    resume: Vec<Option<(usize, usize)>>,
}

/// One leaderboard request (actions 17..=21 and 33..=37).  The headless
/// engine has no UI, but the score/unit/resource query the retail game makes
/// is deterministic and is recorded here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Leaderboard {
    pub player: usize,
    pub text_id: u32,
    pub kind: u8,
    pub unit_id: u16,
    pub score_kind: u16,
    pub goal: Option<u32>,
    pub location: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct TriggerSystem {
    pub triggers: Vec<Trigger>,
    pub locations: Vec<(i32, i32, i32, i32)>,
    /// The scenario's `UPRP` slots, copied here so actions 11/44 can apply
    /// them without threading the whole scenario through the VM.
    pub unit_properties: Vec<crate::map::UnitProperties>,
    pub state: TriggerState,
    pub switches: [bool; 256],
    pub countdown_timer: u32,
    /// Trigger actions that start or relocate an AI script.  The match runner
    /// consumes these after the trigger VM runs.
    pub pending_ai_scripts: Vec<PendingAiScript>,
    pub paused: bool,
    /// True while action 54 (Pause Timer) is in force.
    pub timer_paused: bool,
    /// Per-player, per-trigger preserve flag set by action 3.
    pub preserved: Vec<Vec<bool>>,
    /// Presentation/state side effects that a renderer or campaign shell would
    /// consume.  They are recorded so the headless VM still executes the
    /// action instead of silently dropping it.
    pub mission_objectives: Vec<Option<u32>>,
    /// `(player, unit type, text id, duration ms)` of each Transmission
    /// (see [`transmission_ms`]).
    pub transmissions: Vec<(usize, u16, u32, u32)>,
    /// What players read and hear, in execution order: Transmission (7),
    /// Play WAV (8) and Display Text Message (9).  Presentation only (not
    /// part of `state_hash`); the client shows / plays the new entries.
    pub cues: Vec<TriggerCue>,
    pub center_views: Vec<(usize, u32)>,
    pub minimap_pings: Vec<(usize, u32)>,
    pub talking_portraits: Vec<(usize, u16, u32)>,
    pub muted_speech: bool,
    pub next_scenario: Option<u32>,
    pub leaderboard: Option<Leaderboard>,
    pub leaderboard_computer_players: bool,
    /// Force (0..=3) of players 1..8, from the scenario's `FORC`.
    pub forces: [u8; 8],
    /// Players 1..8 in the game (`OWNR` not 0): only they run the
    /// triggers enabled for a whole force.
    pub present: [bool; 8],
}

impl Default for TriggerSystem {
    fn default() -> Self {
        Self {
            triggers: Vec::new(),
            locations: Vec::new(),
            unit_properties: Vec::new(),
            state: TriggerState::default(),
            switches: [false; 256],
            countdown_timer: 0,
            pending_ai_scripts: Vec::new(),
            paused: false,
            timer_paused: false,
            preserved: Vec::new(),
            mission_objectives: Vec::new(),
            transmissions: Vec::new(),
            cues: Vec::new(),
            center_views: Vec::new(),
            minimap_pings: Vec::new(),
            talking_portraits: Vec::new(),
            muted_speech: false,
            next_scenario: None,
            leaderboard: None,
            leaderboard_computer_players: true,
            forces: [0; 8],
            present: [true; 8],
        }
    }
}

impl TriggerSystem {
    pub fn parse(chunk: &[u8], locations: &[(i32, i32, i32, i32)]) -> Self {
        let mut triggers = Vec::new();
        for record in chunk.as_chunks::<RECORD_SIZE>().0 {
            let mut conditions = Vec::new();
            for c in 0..16 {
                let o = c * 20;
                let kind = record[o + 15];
                if kind == 0 {
                    continue;
                }
                conditions.push(TriggerCondition {
                    location: u32::from_le_bytes([
                        record[o],
                        record[o + 1],
                        record[o + 2],
                        record[o + 3],
                    ]),
                    group: u32::from_le_bytes([
                        record[o + 4],
                        record[o + 5],
                        record[o + 6],
                        record[o + 7],
                    ]),
                    quantity: u32::from_le_bytes([
                        record[o + 8],
                        record[o + 9],
                        record[o + 10],
                        record[o + 11],
                    ]),
                    unit_id: u16::from_le_bytes([record[o + 12], record[o + 13]]),
                    comparison: record[o + 14],
                    kind,
                    amount_modifier: record[o + 16],
                });
            }
            let mut actions = Vec::new();
            for a in 0..64 {
                let o = 320 + a * 32;
                let kind = record[o + 26];
                if kind == 0 {
                    continue;
                }
                actions.push(TriggerAction {
                    location: u32::from_le_bytes([
                        record[o],
                        record[o + 1],
                        record[o + 2],
                        record[o + 3],
                    ]),
                    text_id: u32::from_le_bytes([
                        record[o + 4],
                        record[o + 5],
                        record[o + 6],
                        record[o + 7],
                    ]),
                    sound_id: u32::from_le_bytes([
                        record[o + 8],
                        record[o + 9],
                        record[o + 10],
                        record[o + 11],
                    ]),
                    time: u32::from_le_bytes([
                        record[o + 12],
                        record[o + 13],
                        record[o + 14],
                        record[o + 15],
                    ]),
                    group: u32::from_le_bytes([
                        record[o + 16],
                        record[o + 17],
                        record[o + 18],
                        record[o + 19],
                    ]),
                    group2: u32::from_le_bytes([
                        record[o + 20],
                        record[o + 21],
                        record[o + 22],
                        record[o + 23],
                    ]),
                    extra: u16::from_le_bytes([record[o + 24], record[o + 25]]),
                    kind,
                    num: record[o + 27],
                    flags: record[o + 28],
                });
            }
            let mut enabled = [false; 28];
            for (i, slot) in enabled.iter_mut().enumerate() {
                *slot = record[2372 + i] != 0;
            }
            triggers.push(Trigger {
                conditions,
                actions,
                execution_flags: u32::from_le_bytes([
                    record[2368],
                    record[2369],
                    record[2370],
                    record[2371],
                ]),
                enabled,
            });
        }
        Self {
            triggers,
            locations: locations.to_vec(),
            unit_properties: Vec::new(),
            state: TriggerState::default(),
            switches: [false; 256],
            countdown_timer: 0,
            pending_ai_scripts: Vec::new(),
            paused: false,
            timer_paused: false,
            preserved: Vec::new(),
            mission_objectives: Vec::new(),
            transmissions: Vec::new(),
            cues: Vec::new(),
            center_views: Vec::new(),
            minimap_pings: Vec::new(),
            talking_portraits: Vec::new(),
            muted_speech: false,
            next_scenario: None,
            leaderboard: None,
            leaderboard_computer_players: true,
            forces: [0; 8],
            present: [true; 8],
        }
    }

    pub fn process(&mut self, world: &mut World) {
        // Triggers first run at game start (elapsed time 0: `terran04`
        // sets its objectives on "Elapsed Time at most 0"), then on the
        // 30-tick cadence.
        // Conditions are evaluated on the 30-tick cadence, but a running
        // Wait / Transmission resumes on the tick it ends (retail
        // Wasteland: the 3956 ms "Howdy" line plus a 2000 ms Wait put
        // "NEW MISSION OBJECTIVE" exactly 6.0 s after it).
        let cycle = world.tick == 1 || world.tick.is_multiple_of(TRIGGER_INTERVAL);
        if !cycle {
            if self.state.fired.len() == world.players.len() {
                for player in 0..world.players.len() {
                    if world.tick < self.state.wait_until[player] {
                        continue;
                    }
                    if let Some((ti, start)) = self.state.resume[player].take() {
                        if let Some(trigger) = self.triggers.get(ti).cloned() {
                            self.run_actions(world, player, ti, &trigger, start);
                        }
                    }
                }
            }
            return;
        }
        if self.state.fired.len() != world.players.len() {
            self.state.fired = vec![vec![false; self.triggers.len()]; world.players.len()];
            self.state.wait_until = vec![0; world.players.len()];
            self.state.resume = vec![None; world.players.len()];
            self.preserved = vec![vec![false; self.triggers.len()]; world.players.len()];
            self.mission_objectives = vec![None; world.players.len()];
        }
        // The countdown timer advances once per trigger evaluation (30 ticks)
        // unless action 54 (Pause Timer) is in force.
        if !self.timer_paused {
            self.countdown_timer = self.countdown_timer.saturating_sub(1);
        }
        for player in 0..world.players.len() {
            if world.tick < self.state.wait_until[player] {
                continue;
            }
            // A previous Wait action resumes the rest of that trigger first.
            if let Some((ti, start)) = self.state.resume[player].take() {
                if let Some(trigger) = self.triggers.get(ti).cloned() {
                    self.run_actions(world, player, ti, &trigger, start);
                }
                continue;
            }
            for ti in 0..self.triggers.len() {
                let trigger = self.triggers[ti].clone();
                if !self.runs_for(&trigger, player) {
                    continue;
                }
                let preserved = self
                    .preserved
                    .get(player)
                    .and_then(|p| p.get(ti))
                    .copied()
                    .unwrap_or(false);
                if self.state.fired[player][ti] && !preserved && trigger.execution_flags & 4 == 0 {
                    continue;
                }
                if !self.conditions_hold(world, player, &trigger) {
                    continue;
                }
                self.state.fired[player][ti] = true;
                self.run_actions(world, player, ti, &trigger, 0);
            }
        }
    }

    /// Execute actions from `start`, stopping at a Wait (action 4) so the same
    /// trigger can resume at the following action once the player's wait slot
    /// expires.  Preserve Trigger (action 3) is persistent state, not a
    /// one-shot action.
    fn run_actions(
        &mut self,
        world: &mut World,
        player: usize,
        trigger_index: usize,
        trigger: &Trigger,
        start: usize,
    ) {
        for (ai, action) in trigger.actions.iter().enumerate().skip(start) {
            match action.kind {
                3 => {
                    if let Some(p) = self.preserved.get_mut(player) {
                        p[trigger_index] = true;
                    }
                    continue;
                }
                4 => {
                    // Trigger Wait times are milliseconds.
                    let ticks =
                        ((action.time as u64) * crate::sim::TICKS_PER_SECOND as u64 / 1000).max(1);
                    self.state.wait_until[player] = world.tick + ticks;
                    self.state.resume[player] = Some((trigger_index, ai + 1));
                    return;
                }
                7 => {
                    // A Transmission holds its trigger for the line's
                    // length, so a conversation plays line after line
                    // (`The Trump Card` opens with three).  INFERENCE from
                    // the public CHK trigger spec; falsifier: a retail
                    // recording where the lines overlap.
                    self.execute(world, player, trigger, action);
                    let ms = transmission_ms(action) as u64;
                    let ticks = (ms * crate::sim::TICKS_PER_SECOND as u64 / 1000).max(1);
                    self.state.wait_until[player] = world.tick + ticks;
                    self.state.resume[player] = Some((trigger_index, ai + 1));
                    return;
                }
                _ => self.execute(world, player, trigger, action),
            }
        }
    }

    fn conditions_hold(&self, world: &World, player: usize, trigger: &Trigger) -> bool {
        trigger
            .conditions
            .iter()
            .all(|c| self.condition_holds(world, player, c))
    }

    fn condition_holds(&self, world: &World, player: usize, c: &TriggerCondition) -> bool {
        // Condition ids are the classic CHK list (0 no condition, 1 countdown,
        // 2 command, 3 bring, ... 22 always, 23 never).
        if c.kind == 11 {
            // Switch condition: switch number is in the modifier byte, and
            // comparison 2 means "set", 3 means "cleared".
            let on = self
                .switches
                .get(c.amount_modifier as usize)
                .copied()
                .unwrap_or(false);
            return match c.comparison {
                2 => on,
                3 => !on,
                _ => false,
            };
        }
        let actual = match c.kind {
            0 | 22 => return true,
            23 => return false,
            1 => self.countdown_timer as i32,
            2 => self.command_count(world, player, c),
            3 => self.bring_count(world, player, c),
            4 => {
                let p = &world.players[player.min(world.players.len().saturating_sub(1))];
                match c.amount_modifier {
                    0 => p.minerals_gathered,
                    1 => p.gas_gathered,
                    _ => p.minerals_gathered + p.gas_gathered,
                }
            }
            5 => self.kill_count(world, player, c.unit_id),
            6 => {
                let mine = self.command_count(world, player, c);
                let most = (0..world.players.len())
                    .map(|p| self.command_count(world, p, c))
                    .max()
                    .unwrap_or(0);
                if mine == most {
                    1
                } else {
                    0
                }
            }
            7 => {
                let mine = self.bring_count(world, player, c);
                let most = (0..world.players.len())
                    .map(|p| self.bring_count(world, p, c))
                    .max()
                    .unwrap_or(0);
                if mine == most {
                    1
                } else {
                    0
                }
            }
            8 => {
                let mine = self.kill_count(world, player, c.unit_id);
                let most = (0..world.players.len())
                    .map(|p| self.kill_count(world, p, c.unit_id))
                    .max()
                    .unwrap_or(0);
                if mine == most {
                    1
                } else {
                    0
                }
            }
            9 => {
                let kind = c.amount_modifier as u16;
                let mine = self.score(world, player, kind);
                let most = (0..world.players.len())
                    .map(|p| self.score(world, p, kind))
                    .max()
                    .unwrap_or(0);
                if mine == most {
                    1
                } else {
                    0
                }
            }
            10 => {
                let mine = self.resources(world, player, c.amount_modifier);
                let most = (0..world.players.len())
                    .map(|p| self.resources(world, p, c.amount_modifier))
                    .max()
                    .unwrap_or(0);
                if mine == most {
                    1
                } else {
                    0
                }
            }
            11 => self
                .switches
                .get(c.quantity as usize)
                .copied()
                .unwrap_or(false) as i32,
            12 => (world.tick / crate::sim::TICKS_PER_SECOND as u64) as i32,
            13 => return false,
            14 => {
                let alive = world
                    .players
                    .iter()
                    .enumerate()
                    .filter(|(i, p)| *i != player && !p.defeated)
                    .count();
                alive as i32
            }
            15 => {
                let p = &world.players[player.min(world.players.len() - 1)];
                if c.unit_id <= 227 {
                    p.death_counts.get(c.unit_id as usize).copied().unwrap_or(0) as i32
                } else if c.unit_id == 230 || c.unit_id == 231 || c.unit_id == 232 {
                    p.death_counts
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| {
                            let building = world.data.unit(*i as u16).is_building();
                            if c.unit_id == 230 {
                                !building
                            } else {
                                building
                            }
                        })
                        .map(|(_, n)| *n)
                        .sum::<u32>() as i32
                } else {
                    p.deaths as i32
                }
            }
            16 => {
                let mine = self.command_count(world, player, c);
                let least = (0..world.players.len())
                    .map(|p| self.command_count(world, p, c))
                    .min()
                    .unwrap_or(0);
                if mine == least {
                    1
                } else {
                    0
                }
            }
            17 => {
                let mine = self.bring_count(world, player, c);
                let least = (0..world.players.len())
                    .map(|p| self.bring_count(world, p, c))
                    .min()
                    .unwrap_or(0);
                if mine == least {
                    1
                } else {
                    0
                }
            }
            18 => {
                let mine = self.kill_count(world, player, c.unit_id);
                let least = (0..world.players.len())
                    .map(|p| self.kill_count(world, p, c.unit_id))
                    .min()
                    .unwrap_or(0);
                if mine == least {
                    1
                } else {
                    0
                }
            }
            19 => {
                let kind = c.amount_modifier as u16;
                let mine = self.score(world, player, kind);
                let least = (0..world.players.len())
                    .map(|p| self.score(world, p, kind))
                    .min()
                    .unwrap_or(0);
                if mine == least {
                    1
                } else {
                    0
                }
            }
            20 => {
                let mine = self.resources(world, player, c.amount_modifier);
                let least = (0..world.players.len())
                    .map(|p| self.resources(world, p, c.amount_modifier))
                    .min()
                    .unwrap_or(0);
                if mine == least {
                    1
                } else {
                    0
                }
            }
            21 => self.score(world, player, c.amount_modifier as u16),
            _ => return false,
        };
        let expected = c.quantity as i32;
        // CHK comparison bytes: 0 at least, 1 at most, 10 exactly.
        match c.comparison {
            0 => actual >= expected,
            1 => actual <= expected,
            10 => actual == expected,
            _ => false,
        }
    }

    /// The `units.dat` unit classes used by trigger arguments.
    fn unit_matches(world: &World, type_id: u16, wanted: u16) -> bool {
        match wanted {
            229 => true,
            230 => !world.data.unit(type_id).is_building(),
            231 => world.data.unit(type_id).is_building(),
            232 => world.data.unit(type_id).is_building(),
            _ => type_id == wanted,
        }
    }

    /// Score category lookup (CHK score types 0..=7).
    fn score(&self, world: &World, player: usize, kind: u16) -> i32 {
        let p = &world.players[player.min(world.players.len() - 1)];
        match kind {
            0 => p.scores[1] + p.scores[2] + p.scores[4] + p.scores[5],
            3 => p.scores[1] + p.scores[2],
            6 => p.scores[4] + p.scores[5],
            1..=5 | 7 => p.scores[kind as usize],
            _ => 0,
        }
    }

    fn resources(&self, world: &World, player: usize, kind: u8) -> i32 {
        let p = &world.players[player.min(world.players.len() - 1)];
        match kind {
            0 => p.minerals,
            1 => p.gas,
            _ => p.minerals + p.gas,
        }
    }

    fn kill_count(&self, world: &World, player: usize, unit: u16) -> i32 {
        let p = &world.players[player.min(world.players.len() - 1)];
        if unit <= 227 {
            p.kill_counts.get(unit as usize).copied().unwrap_or(0) as i32
        } else if unit == 230 || unit == 231 || unit == 232 {
            p.kill_counts
                .iter()
                .enumerate()
                .filter(|(i, _)| {
                    let building = world.data.unit(*i as u16).is_building();
                    if unit == 230 {
                        !building
                    } else {
                        building
                    }
                })
                .map(|(_, n)| *n)
                .sum::<u32>() as i32
        } else {
            p.kills as i32
        }
    }

    /// Owners a Command/Bring condition counts: one player, a force's
    /// members, or `None` for any owner.
    fn condition_owners(&self, group: u32, player: usize) -> Option<Vec<usize>> {
        match group {
            18..=21 => Some(self.force_members(group as usize - 18)),
            _ => self.select_owner(group, player).map(|o| vec![o]),
        }
    }

    fn command_count(&self, world: &World, player: usize, c: &TriggerCondition) -> i32 {
        let owners = self.condition_owners(c.group, player);
        world
            .units
            .iter()
            .filter(|u| {
                u.alive
                    && Self::unit_matches(world, u.type_id, c.unit_id)
                    && owners
                        .as_ref()
                        .is_none_or(|o| o.contains(&(u.owner as usize)))
            })
            .count() as i32
    }

    fn bring_count(&self, world: &World, player: usize, c: &TriggerCondition) -> i32 {
        let owners = self.condition_owners(c.group, player);
        let loc = c.location as usize;
        world
            .units
            .iter()
            .filter(|u| {
                if !u.alive {
                    return false;
                }
                if owners
                    .as_ref()
                    .is_some_and(|o| !o.contains(&(u.owner as usize)))
                {
                    return false;
                }
                if !Self::unit_matches(world, u.type_id, c.unit_id) {
                    return false;
                }
                if loc == 0 {
                    return true;
                }
                let Some(l) = self.locations.get(loc - 1) else {
                    return false;
                };
                let x = u.x / 256;
                let y = u.y / 256;
                x >= l.0 && x < l.2 && y >= l.1 && y < l.3
            })
            .count() as i32
    }

    /// Whether `player` executes `trigger`: enabled for the player, for All
    /// Players (17) or for the player's force (18..=21, `FORC`).
    fn runs_for(&self, trigger: &Trigger, player: usize) -> bool {
        if trigger.enabled[player.min(27)] || trigger.enabled[17] {
            return true;
        }
        player < 8 && self.present[player] && trigger.enabled[18 + self.forces[player] as usize]
    }

    /// Players of force `force` (0..=3) that are in the game.
    fn force_members(&self, force: usize) -> Vec<usize> {
        (0..8)
            .filter(|&p| self.present[p] && self.forces[p] as usize == force)
            .collect()
    }

    fn select_owner(&self, group: u32, current: usize) -> Option<usize> {
        match group {
            0..=11 => Some(group as usize),
            13 => Some(current),
            _ => None,
        }
    }

    /// All players a trigger argument selects.  Group 12 is "All players";
    /// group 13 is the current player.  Force groups are resolved to their
    /// member players when the force bytes are present, otherwise they are
    /// treated like "all".
    fn select_owners(&self, group: u32, current: usize) -> Vec<usize> {
        match group {
            0..=11 => vec![group as usize],
            12 => (0..12).collect(),
            13 => vec![current],
            14..=17 => (0..12).collect(),
            18..=21 => self.force_members(group as usize - 18),
            _ => vec![current],
        }
    }

    fn loc_center_px(&self, location: u32) -> Option<(i32, i32)> {
        if location == 0 || location == 64 {
            return None;
        }
        self.locations
            .get(location as usize - 1)
            .map(|l| ((l.0 + l.2) / 2, (l.1 + l.3) / 2))
    }

    fn unit_in_location(&self, x_px: i32, y_px: i32, location: u32) -> bool {
        if location == 0 || location == 64 {
            return true;
        }
        let Some(l) = self.locations.get(location as usize - 1) else {
            return false;
        };
        let x = x_px / 256;
        let y = y_px / 256;
        x >= l.0 && x < l.2 && y >= l.1 && y < l.3
    }

    /// Apply an enable/disable/toggle state byte (4/5/6).
    fn next_state(current: bool, state: u8) -> bool {
        match state {
            4 => true,
            5 => false,
            6 => !current,
            _ => current,
        }
    }

    /// Pick up to `count` matching units (0 means all) in `location`.
    fn collect_targets(
        world: &World,
        owners: &[usize],
        unit: u16,
        count: u32,
        location: u32,
        locations: &[(i32, i32, i32, i32)],
    ) -> Vec<usize> {
        let limit = if count == 0 {
            usize::MAX
        } else {
            count as usize
        };
        let mut out = Vec::new();
        for (i, u) in world.units.iter().enumerate() {
            if !u.alive || !owners.contains(&(u.owner as usize)) {
                continue;
            }
            if !Self::unit_matches(world, u.type_id, unit) {
                continue;
            }
            if location != 0 && location != 64 {
                let Some(l) = locations.get(location as usize - 1) else {
                    continue;
                };
                let x = u.x / 256;
                let y = u.y / 256;
                if !(x >= l.0 && x < l.2 && y >= l.1 && y < l.3) {
                    continue;
                }
            }
            out.push(i);
            if out.len() >= limit {
                break;
            }
        }
        out
    }

    fn execute(
        &mut self,
        world: &mut World,
        player: usize,
        _trigger: &Trigger,
        action: &TriggerAction,
    ) {
        let owner = self.select_owner(action.group, player);
        match action.kind {
            // Victory / Defeat end the game for the player running the
            // trigger; their `group` operand is unused (0 in the shipped
            // maps, which made player 1 win Wasteland "for" player 0).
            1 => {
                if let Some(p) = world.players.get_mut(player) {
                    p.defeated = false;
                }
                world.winner = Some(player as u8);
            }
            2 => {
                if player < world.players.len() {
                    let o = player;
                    world.players[o].defeated = true;
                    for u in &mut world.units {
                        if u.owner as usize == o {
                            u.alive = false;
                            u.order = Order::Die;
                        }
                    }
                }
            }
            22 | 24 => {
                let owners = self.select_owners(action.group, player);
                for i in 0..world.units.len() {
                    let type_id = world.units[i].type_id;
                    if world.units[i].alive
                        && owners.contains(&(world.units[i].owner as usize))
                        && Self::unit_matches(world, type_id, action.extra)
                    {
                        world.units[i].alive = false;
                        world.units[i].order = Order::Die;
                    }
                }
            }
            23 | 25 => {
                let owners = self.select_owners(action.group, player);
                let targets = Self::collect_targets(
                    world,
                    &owners,
                    action.extra,
                    action.num as u32,
                    action.location,
                    &self.locations,
                );
                for i in targets {
                    world.units[i].alive = false;
                    world.units[i].order = Order::Die;
                }
            }
            13 => {
                // Set Switch: `group2` is the switch number, `num` the state
                // (4 set, 5 clear, 6 toggle, 11 randomize).
                let sw = action.group2 as usize;
                if sw < self.switches.len() {
                    self.switches[sw] = match action.num {
                        4 => true,
                        5 => false,
                        6 => !self.switches[sw],
                        11 => {
                            let mut x = self.switches[sw] as u64;
                            x = x.wrapping_mul(6364136223846793005).wrapping_add(1);
                            (x >> 33) & 1 != 0
                        }
                        _ => self.switches[sw],
                    };
                }
            }
            14 => {
                // Set Countdown Timer: time, modifier (7 set, 8 add, 9 sub).
                let t = action.time as i64;
                self.countdown_timer = match action.num {
                    7 => t.max(0) as u32,
                    8 => (self.countdown_timer as i64 + t).max(0) as u32,
                    9 => (self.countdown_timer as i64 - t).max(0) as u32,
                    _ => self.countdown_timer,
                };
            }
            26 => {
                // `num` selects set (7), add (8) or subtract (9); `extra`
                // selects minerals (0), gas (1) or both (2).
                let amount = action.group2 as i32;
                let owners = self.select_owners(action.group, player);
                for o in owners {
                    let Some(p) = world.players.get_mut(o) else {
                        continue;
                    };
                    let apply = |value: &mut i32| match action.num {
                        7 => *value = amount,
                        8 => *value = value.saturating_add(amount),
                        9 => *value = value.saturating_sub(amount),
                        _ => {}
                    };
                    if action.extra == 0 || action.extra == 2 {
                        apply(&mut p.minerals);
                    }
                    if action.extra == 1 || action.extra == 2 {
                        apply(&mut p.gas);
                    }
                }
            }
            38 => {
                // Move Location: shift a named location to a unit/tile.
                let src = action.location.saturating_sub(1) as usize;
                let dst = action.group2.saturating_sub(1) as usize;
                if src < self.locations.len() && dst < self.locations.len() {
                    let old = self.locations[src];
                    let (cx, cy) = ((old.0 + old.2) / 2, (old.1 + old.3) / 2);
                    let (nx, ny) = if action.extra != 0 {
                        world
                            .units
                            .iter()
                            .find(|u| u.alive && u.type_id == action.extra)
                            .map(|u| (u.x / 256, u.y / 256))
                            .unwrap_or((cx, cy))
                    } else {
                        (cx, cy)
                    };
                    let dx = nx - cx;
                    let dy = ny - cy;
                    self.locations[dst].0 += dx;
                    self.locations[dst].2 += dx;
                    self.locations[dst].1 += dy;
                    self.locations[dst].3 += dy;
                }
            }
            11 | 44 => {
                // Unit classes and out-of-range ids create nothing.
                if action.extra > 227 {
                    return;
                }
                let slot = action.group2 as usize;
                let props = if (1..=64).contains(&slot) {
                    self.unit_properties.get(slot - 1).cloned()
                } else {
                    None
                };
                let (x, y) = self.loc_center_px(action.location).unwrap_or((
                    world.terrain.width as i32 * 16,
                    world.terrain.height as i32 * 16,
                ));
                let count = if action.num == 0 {
                    1
                } else {
                    action.num as usize
                };
                for o in self.select_owners(action.group, player) {
                    if o >= world.players.len() {
                        continue;
                    }
                    for _ in 0..count {
                        let spawned = if let Some(props) = props.as_ref() {
                            world.spawn_with_properties(o, action.extra, x, y, props)
                        } else {
                            world.spawn_px(o, action.extra, x, y)
                        };
                        // A single failed placement does not stop the count;
                        // retail tries the nearest free spot for each unit.
                        let _ = spawned;
                    }
                }
            }
            46 => {
                // Order: group=player, extra=unit, from=location,
                // group2=destination, num=0 move / 1 patrol / 2 attack.
                let owners = self.select_owners(action.group, player);
                let targets = Self::collect_targets(
                    world,
                    &owners,
                    action.extra,
                    0,
                    action.location,
                    &self.locations,
                );
                let Some(dst) = self.loc_center_px(action.group2) else {
                    return;
                };
                let ids: Vec<u32> = targets.iter().map(|&i| world.units[i].id).collect();
                match action.num {
                    1 => {
                        for &i in &targets {
                            let from = (world.units[i].x / 256, world.units[i].y / 256);
                            world.units[i].order = Order::Patrol {
                                a: from,
                                b: dst,
                                forward: true,
                            };
                        }
                    }
                    2 => world.command_attack_move(&ids, dst),
                    _ => world.command_move(&ids, dst),
                }
            }
            5 | 6 | 54 | 55 => match action.kind {
                5 => self.paused = true,
                6 => self.paused = false,
                54 => self.timer_paused = true,
                55 => self.timer_paused = false,
                _ => {}
            },
            15 | 16 => {
                // Run AI Script / Run AI Script At Location.  The four-byte
                // script id is stored in `group2` by StarEdit.
                let mut id = [0u8; 4];
                id.copy_from_slice(&action.group2.to_le_bytes());
                let at = if action.kind == 16 {
                    self.locations
                        .get(action.location.saturating_sub(1) as usize)
                        .map(|l| ((l.0 + l.2) / 2, (l.1 + l.3) / 2))
                } else {
                    None
                };
                self.pending_ai_scripts.push((player, id, at));
            }
            39 => {
                // Move Unit: group=player, extra=unit, num=count,
                // location=from, group2=to.
                let owners = self.select_owners(action.group, player);
                let targets = Self::collect_targets(
                    world,
                    &owners,
                    action.extra,
                    action.num as u32,
                    action.location,
                    &self.locations,
                );
                if let Some((x, y)) = self.loc_center_px(action.group2) {
                    for i in targets {
                        world.units[i].x = x.saturating_mul(256);
                        world.units[i].y = y.saturating_mul(256);
                        world.units[i].path.clear();
                    }
                }
            }
            45 => {
                // Set Deaths: group=player, extra=unit, group2=amount,
                // num=modifier (7 set, 8 add, 9 subtract).  Unit classes
                // 230..=232 write the real loss counters; Any unit writes
                // nothing (retail behaviour).
                let owners = self.select_owners(action.group, player);
                let n = action.group2;
                for o in owners {
                    let Some(p) = world.players.get_mut(o) else {
                        continue;
                    };
                    if action.extra <= 227 {
                        let idx = action.extra as usize;
                        if idx < p.death_counts.len() {
                            let v = &mut p.death_counts[idx];
                            match action.num {
                                7 => *v = n,
                                8 => *v = v.saturating_add(n),
                                9 => *v = v.saturating_sub(n),
                                _ => {}
                            }
                        }
                    } else if action.extra == 230 || action.extra == 231 || action.extra == 232 {
                        let v = &mut p.deaths;
                        match action.num {
                            7 => *v = n,
                            8 => *v = v.saturating_add(n),
                            9 => *v = v.saturating_sub(n),
                            _ => {}
                        }
                    }
                }
            }
            48 => {
                // Give Units to Player: group=from, group2=to, extra=unit,
                // num=count, location.  A group "from" applies per player.
                let sources = self.select_owners(action.group, player);
                let Some(dst) = self.select_owners(action.group2, player).first().copied() else {
                    return;
                };
                let targets = Self::collect_targets(
                    world,
                    &sources,
                    action.extra,
                    action.num as u32,
                    action.location,
                    &self.locations,
                );
                for i in targets {
                    world.units[i].owner = dst as u8;
                }
            }
            49 | 50 | 51 | 53 => {
                let owners = self.select_owners(action.group, player);
                let targets = Self::collect_targets(
                    world,
                    &owners,
                    action.extra,
                    action.num as u32,
                    action.location,
                    &self.locations,
                );
                let percent = action.group2.min(100) as i32;
                for i in targets {
                    match action.kind {
                        49 => {
                            let max = world.data.unit(world.units[i].type_id).hit_points_raw as i32;
                            world.units[i].hp = (max * percent / 100).max(1);
                        }
                        50 => {
                            world.units[i]
                                .set_energy_fixed(crate::sim::MAX_ENERGY * 256 * percent / 100);
                        }
                        51 => {
                            let ty = world.data.unit(world.units[i].type_id);
                            if ty.has_shields {
                                world.units[i].shields = (ty.shield_points as i32 * percent).max(0);
                            }
                        }
                        53 => {
                            let amount = action.group2.min(255) as u8;
                            let cap = world.units[i].hangar_capacity;
                            world.units[i].hangar =
                                world.units[i].hangar.saturating_add(amount).min(cap);
                        }
                        _ => {}
                    }
                }
            }
            52 => {
                let owners = self.select_owners(action.group, player);
                let amount = (action.group2 as u16) as i32;
                for u in &mut world.units {
                    if u.alive
                        && owners.contains(&(u.owner as usize))
                        && u.is_resource()
                        && self.unit_in_location(u.x, u.y, action.location)
                    {
                        u.resource_amount = amount;
                    }
                }
            }
            7 => {
                let ms = transmission_ms(action);
                self.transmissions
                    .push((player, action.extra, action.text_id, ms));
                self.cues.push(TriggerCue::Wav {
                    player,
                    string: action.sound_id,
                });
                self.cues.push(TriggerCue::Text {
                    player,
                    string: action.text_id,
                    always: action.flags & ALWAYS_DISPLAY != 0,
                    ms,
                    paused: self.paused,
                });
            }
            8 => {
                self.cues.push(TriggerCue::Wav {
                    player,
                    string: action.sound_id,
                });
            }
            9 => {
                self.cues.push(TriggerCue::Text {
                    player,
                    string: action.text_id,
                    always: action.flags & ALWAYS_DISPLAY != 0,
                    ms: 0,
                    paused: self.paused,
                });
            }
            10 => {
                self.center_views.push((player, action.location));
            }
            12 => {
                if self.mission_objectives.len() <= player {
                    self.mission_objectives.resize(player + 1, None);
                }
                self.mission_objectives[player] = Some(action.text_id);
            }
            17..=21 | 33..=37 => {
                self.leaderboard = Some(Leaderboard {
                    player,
                    text_id: action.text_id,
                    kind: action.kind,
                    unit_id: action.extra,
                    score_kind: action.extra,
                    goal: (33..=37).contains(&action.kind).then_some(action.group2),
                    location: matches!(action.kind, 18 | 34).then_some(action.location),
                });
            }
            27 => {
                // Set Score: group=player, group2=amount, extra=score kind,
                // num=modifier.
                let targets = self.select_owners(action.group, player);
                let amount = action.group2 as i32;
                let kind = action.extra as usize;
                let apply = |slot: &mut i32, share: i32| match action.num {
                    7 => *slot = share,
                    8 => *slot = slot.saturating_add(share),
                    9 => *slot = slot.saturating_sub(share),
                    _ => {}
                };
                for o in targets {
                    let Some(p) = world.players.get_mut(o) else {
                        continue;
                    };
                    match kind {
                        0 => {
                            let share = amount / 4;
                            apply(&mut p.scores[1], share);
                            apply(&mut p.scores[2], share);
                            apply(&mut p.scores[4], share);
                            apply(&mut p.scores[5], share);
                        }
                        3 => {
                            let share = amount / 2;
                            apply(&mut p.scores[1], share);
                            apply(&mut p.scores[2], share);
                        }
                        6 => {
                            let share = amount / 2;
                            apply(&mut p.scores[4], share);
                            apply(&mut p.scores[5], share);
                        }
                        1..=5 | 7 => apply(&mut p.scores[kind], amount),
                        _ => {}
                    }
                }
            }
            28 => {
                self.minimap_pings.push((player, action.location));
            }
            29 => {
                self.talking_portraits
                    .push((player, action.extra, action.time));
            }
            30 => self.muted_speech = true,
            31 => self.muted_speech = false,
            32 => {
                self.leaderboard_computer_players =
                    Self::next_state(self.leaderboard_computer_players, action.num);
            }
            40 => {
                self.leaderboard = Some(Leaderboard {
                    player,
                    text_id: action.text_id,
                    kind: 40,
                    unit_id: 0,
                    score_kind: 0,
                    goal: Some(action.group2),
                    location: None,
                });
            }
            41 => self.next_scenario = Some(action.text_id.saturating_sub(1)),
            42 | 43 => {
                let owners = self.select_owners(action.group, player);
                let targets = Self::collect_targets(
                    world,
                    &owners,
                    action.extra,
                    if action.kind == 42 { 1 } else { 0 },
                    action.location,
                    &self.locations,
                );
                for i in targets {
                    if action.kind == 42 {
                        let current = world.units[i].doodad_enabled.unwrap_or(false);
                        world.units[i].doodad_enabled = Some(Self::next_state(current, action.num));
                    } else {
                        world.units[i].invincible =
                            Self::next_state(world.units[i].invincible, action.num);
                    }
                }
            }
            58 | 59 => {
                // Enable/Disable Debug Mode (Remastered): controls whether Esc
                // can skip a trigger.  There is no renderer or Esc key here.
            }
            56 => {
                // Draw: no winner.
                world.winner = None;
            }
            57 => {
                // Set Alliance Status: group=player the trigger runs for,
                // extra=0 enemy / 1 ally / 2 allied victory.
                let Some(o) = owner else {
                    return;
                };
                let targets = self.select_owners(action.group2, player);
                let status = action.extra;
                // Ally/allied-victory also converts every player already
                // allied with the trigger's player (retail rule).
                let old_team = world.players[o].team;
                for target in targets {
                    if target >= world.players.len() || target == o {
                        continue;
                    }
                    if status == 0 {
                        world.players[target].team = old_team.wrapping_add(1);
                    } else {
                        world.players[target].team = old_team;
                        for p in 0..world.players.len() {
                            if p != o && world.players[p].team == old_team {
                                world.players[p].team = old_team;
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn transmission_duration_applies_the_modifier() {
        let a = |num, time, group2| TriggerAction {
            num,
            time,
            group2,
            ..Default::default()
        };
        assert_eq!(super::transmission_ms(&a(7, 2000, 500)), 500);
        assert_eq!(super::transmission_ms(&a(8, 2000, 500)), 2500);
        assert_eq!(super::transmission_ms(&a(9, 2000, 500)), 1500);
        assert_eq!(super::transmission_ms(&a(9, 200, 500)), 0);
        assert_eq!(super::transmission_ms(&a(0, 2000, 500)), 2000);
    }

    use super::*;

    fn record(kind: u8, action: u8, enabled: usize) -> Vec<u8> {
        let mut r = vec![0u8; RECORD_SIZE];
        r[15] = kind;
        // The zero quantity is the neutral default for always/countdown tests.
        r[8..12].copy_from_slice(&0u32.to_le_bytes());
        // action 0
        r[320 + 26] = action;
        r[320 + 27] = 0;
        r[2372 + enabled] = 1;
        let _ = kind;
        r
    }

    #[test]
    fn transmission_play_wav_and_display_text_log_ordered_cues() {
        // One Always trigger: Transmission (text 5, wav 6, 2 s, flag
        // "always display"), Play WAV (wav 7), Display Text (text 8).
        let mut r = record(22, 7, 0);
        let act = |r: &mut Vec<u8>, i: usize, kind: u8, text: u32, wav: u32, flags: u8| {
            let o = 320 + 32 * i;
            r[o + 4..o + 8].copy_from_slice(&text.to_le_bytes());
            r[o + 8..o + 12].copy_from_slice(&wav.to_le_bytes());
            r[o + 12..o + 16].copy_from_slice(&2000u32.to_le_bytes());
            r[o + 24..o + 26].copy_from_slice(&1u16.to_le_bytes()); // Marine
            r[o + 26] = kind;
            r[o + 28] = flags;
        };
        act(&mut r, 0, 7, 5, 6, ALWAYS_DISPLAY);
        act(&mut r, 1, 8, 0, 7, 0);
        act(&mut r, 2, 9, 8, 0, 0);
        let mut t = TriggerSystem::parse(&r, &[]);
        let mut w = world();
        t.process(&mut w);
        // The transmission holds the trigger for its 2 s line.
        assert_eq!(t.cues.len(), 2, "only the transmission so far");
        w.tick += 60;
        t.process(&mut w);
        assert_eq!(
            t.cues,
            vec![
                TriggerCue::Wav {
                    player: 0,
                    string: 6
                },
                TriggerCue::Text {
                    player: 0,
                    string: 5,
                    always: true,
                    ms: 2000,
                    paused: false
                },
                TriggerCue::Wav {
                    player: 0,
                    string: 7
                },
                TriggerCue::Text {
                    player: 0,
                    string: 8,
                    always: false,
                    ms: 0,
                    paused: false
                },
            ]
        );
        assert_eq!(
            t.transmissions,
            vec![(0, 1, 5, 2000)],
            "portrait record kept"
        );
        assert!(t.cues.iter().all(|c| c.player() == 0));
    }

    #[test]
    fn every_classic_action_id_is_accepted() {
        // The CHK action list runs 0..=59.  Feed each id a fully populated
        // record so missing operands cannot mask a panic.
        for action in 1..=59u8 {
            let mut r = record(22, action, 0);
            r[320..320 + 4].copy_from_slice(&1u32.to_le_bytes()); // location
            r[320 + 4..320 + 8].copy_from_slice(&1u32.to_le_bytes()); // text
            r[320 + 8..320 + 12].copy_from_slice(&1u32.to_le_bytes()); // wav
            r[320 + 12..320 + 16].copy_from_slice(&500u32.to_le_bytes()); // time
            r[320 + 16..320 + 20].copy_from_slice(&0u32.to_le_bytes()); // player
            r[320 + 20..320 + 24].copy_from_slice(&1u32.to_le_bytes()); // group2
            r[320 + 24..320 + 26].copy_from_slice(&1u16.to_le_bytes()); // extra
            r[320 + 27] = 8;
            let mut t = TriggerSystem::parse(&r, &[]);
            let mut w = world();
            t.process(&mut w);
        }
    }

    #[test]
    fn every_classic_condition_id_is_accepted() {
        for kind in 0..=23u8 {
            let mut r = record(kind, 26, 0);
            r[8..12].copy_from_slice(&0u32.to_le_bytes());
            r[14] = 0;
            r[16] = 0;
            r[320 + 20..320 + 24].copy_from_slice(&1u32.to_le_bytes());
            r[320 + 27] = 8;
            let mut t = TriggerSystem::parse(&r, &[]);
            let mut w = world();
            t.process(&mut w);
        }
    }

    #[test]
    fn shipped_melee_trigger_uses_command_and_opponents() {
        let t = TriggerSystem::parse(embedded!("fixtures/melee.trg"), &[]);
        assert_eq!(t.triggers.len(), 2);
        assert_eq!(t.triggers[0].conditions[0].kind, 2); // command
        assert_eq!(t.triggers[0].actions[0].kind, 2); // defeat
        assert_eq!(t.triggers[1].conditions[0].kind, 14); // opponents
        assert_eq!(t.triggers[1].actions[0].kind, 1); // victory
        assert!(t.triggers[0].enabled[17], "melee enables all players");
    }

    #[test]
    fn parses_a_single_trigger_record() {
        let t = TriggerSystem::parse(&record(15, 2, 0), &[]);
        assert_eq!(t.triggers.len(), 1);
        assert_eq!(t.triggers[0].conditions.len(), 1);
        assert_eq!(t.triggers[0].actions.len(), 1);
        assert!(t.triggers[0].enabled[0]);
    }

    fn world() -> World {
        use crate::data::GameData;
        use crate::map::Terrain;
        use std::sync::Arc;
        let mut w = World::new(
            Arc::new(GameData::bundled().unwrap()),
            Terrain::open(16, 16),
            2,
        );
        w.tick = 30;
        w
    }

    #[test]
    fn set_resources_action_honours_the_operation_and_resource_mask() {
        let mut r = record(22, 26, 0);
        // group2 = 500, extra = 2 (both), num = 7 (set)
        r[320 + 20..320 + 24].copy_from_slice(&500u32.to_le_bytes());
        r[320 + 24] = 2;
        r[320 + 27] = 7;
        let mut t = TriggerSystem::parse(&r, &[]);
        let mut w = world();
        t.process(&mut w);
        assert_eq!(w.players[0].minerals, 500);
        assert_eq!(w.players[0].gas, 500);

        // Add 25 gas, leave minerals untouched.
        let mut r = record(22, 26, 0);
        r[320 + 20..320 + 24].copy_from_slice(&25u32.to_le_bytes());
        r[320 + 24] = 1;
        r[320 + 27] = 8;
        let mut t = TriggerSystem::parse(&r, &[]);
        let mut w = world();
        w.players[0].gas = 10;
        t.process(&mut w);
        assert_eq!(w.players[0].minerals, 50);
        assert_eq!(w.players[0].gas, 35);
    }

    #[test]
    fn set_switch_action_feeds_the_switch_condition() {
        // First trigger: always -> set switch #3.
        let mut a = record(22, 13, 0);
        a[320 + 20..320 + 24].copy_from_slice(&3u32.to_le_bytes());
        a[320 + 27] = 4; // set
                         // Second trigger: switch #3 is set -> set minerals to 123.
        let mut b = record(22, 26, 0);
        b[15] = 11;
        b[14] = 2; // switch is set
        b[16] = 3; // switch number
        b[320 + 20..320 + 24].copy_from_slice(&123u32.to_le_bytes());
        b[320 + 24] = 0;
        b[320 + 27] = 7;
        let mut bytes = a;
        bytes.extend_from_slice(&b);
        let mut t = TriggerSystem::parse(&bytes, &[]);
        let mut w = world();
        t.process(&mut w);
        assert!(t.switches[3]);
        assert_eq!(w.players[0].minerals, 123);
    }

    #[test]
    fn next_scenario_text_id_is_stored_one_greater_than_the_str_index() {
        // Every shipped action-41 use points one past the `.scm` string:
        // Enslavers 01 stores 21 for string 20, 02a stores 16 for string 15.
        let mut r = record(22, 41, 0);
        r[320 + 4..320 + 8].copy_from_slice(&17u32.to_le_bytes());
        let mut t = TriggerSystem::parse(&r, &[]);
        let mut w = world();
        t.process(&mut w);
        assert_eq!(t.next_scenario, Some(16));
    }

    #[test]
    fn elapsed_time_condition_is_in_seconds() {
        let mut r = record(12, 26, 0);
        r[15] = 12;
        r[8..12].copy_from_slice(&30u32.to_le_bytes());
        r[320 + 20..320 + 24].copy_from_slice(&5u32.to_le_bytes());
        r[320 + 24] = 0;
        r[320 + 27] = 7;
        let mut t = TriggerSystem::parse(&r, &[]);
        let mut w = world(); // tick 30 = 1 second
        t.process(&mut w);
        assert_eq!(w.players[0].minerals, 50, "1 second is not 30 seconds");
        w.tick = 30 * 30;
        t.process(&mut w);
        assert_eq!(w.players[0].minerals, 5);
    }

    fn add_action(rec: &mut [u8], slot: usize, kind: u8) -> usize {
        let o = 320 + slot * 32;
        rec[o + 26] = kind;
        o
    }

    #[test]
    fn exactly_comparison_is_the_chk_value_ten() {
        // First trigger: always -> Set Score Custom 5.
        let mut a = record(22, 27, 0);
        a[320 + 20..320 + 24].copy_from_slice(&5u32.to_le_bytes());
        a[320 + 24..320 + 26].copy_from_slice(&7u16.to_le_bytes()); // custom
        a[320 + 27] = 7; // set
                         // Second trigger: Score(Custom) Exactly 5 -> minerals 77.
        let mut b = record(21, 26, 0);
        b[15] = 21;
        b[8..12].copy_from_slice(&5u32.to_le_bytes());
        b[14] = 10; // Exactly
        b[16] = 7; // Custom
        b[320 + 20..320 + 24].copy_from_slice(&77u32.to_le_bytes());
        b[320 + 24] = 0;
        b[320 + 27] = 7;
        let mut bytes = a;
        bytes.extend_from_slice(&b);
        let mut t = TriggerSystem::parse(&bytes, &[]);
        let mut w = world();
        t.process(&mut w);
        assert_eq!(w.players[0].scores[7], 5);
        assert_eq!(w.players[0].minerals, 77);
    }

    #[test]
    fn create_unit_with_properties_applies_uprp_slot_to_every_unit() {
        let mut props = vec![0u8; 64 * 20];
        props[2..4].copy_from_slice(&0x12u16.to_le_bytes()); // HP + resources
        props[5] = 50;
        props[8..12].copy_from_slice(&123u32.to_le_bytes());
        props[0..2].copy_from_slice(&0x10u16.to_le_bytes()); // invincible state mask
        props[14..16].copy_from_slice(&0x10u16.to_le_bytes()); // invincible
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
        push(b"UPRP", &props);
        let mut uprp = crate::map::Scenario::parse(&chk).unwrap().unit_properties;

        let mut r = record(22, 11, 0);
        r[320 + 16..320 + 20].copy_from_slice(&0u32.to_le_bytes()); // player
        r[320 + 20..320 + 24].copy_from_slice(&1u32.to_le_bytes()); // slot 1
        r[320 + 24..320 + 26].copy_from_slice(&crate::ids::unit::MARINE.to_le_bytes());
        r[320 + 27] = 2; // two units
        let mut t = TriggerSystem::parse(&r, &[]);
        t.unit_properties = std::mem::take(&mut uprp);
        let mut w = world();
        t.process(&mut w);
        let marines: Vec<_> = w
            .units
            .iter()
            .filter(|u| u.alive && u.type_id == crate::ids::unit::MARINE)
            .collect();
        assert_eq!(marines.len(), 2);
        let max = w.data.unit(crate::ids::unit::MARINE).hit_points_raw as i32;
        for u in marines {
            assert_eq!(u.hp, (max * 50 / 100).max(1));
            assert_eq!(u.resource_amount, 123);
            assert!(u.invincible);
        }
    }

    #[test]
    fn create_unit_count_and_kill_unit_at_location_count_are_honoured() {
        let locations = vec![(0, 0, 64, 64)];
        let mut r = record(22, 44, 0);
        r[320..320 + 4].copy_from_slice(&1u32.to_le_bytes());
        r[320 + 16..320 + 20].copy_from_slice(&0u32.to_le_bytes());
        r[320 + 20..320 + 24].copy_from_slice(&0u32.to_le_bytes());
        r[320 + 24..320 + 26].copy_from_slice(&crate::ids::unit::MARINE.to_le_bytes());
        r[320 + 27] = 3;
        let mut t = TriggerSystem::parse(&r, &locations);
        let mut w = world();
        t.process(&mut w);
        assert_eq!(
            w.units
                .iter()
                .filter(|u| u.alive && u.type_id == crate::ids::unit::MARINE)
                .count(),
            3
        );

        let mut k = record(22, 23, 0);
        k[320..320 + 4].copy_from_slice(&1u32.to_le_bytes());
        k[320 + 16..320 + 20].copy_from_slice(&0u32.to_le_bytes());
        k[320 + 24..320 + 26].copy_from_slice(&crate::ids::unit::MARINE.to_le_bytes());
        k[320 + 27] = 2;
        let mut t2 = TriggerSystem::parse(&k, &locations);
        t2.process(&mut w);
        assert_eq!(
            w.units
                .iter()
                .filter(|u| u.alive && u.type_id == crate::ids::unit::MARINE)
                .count(),
            1
        );
    }

    #[test]
    fn modify_hit_points_uses_a_percentage_not_a_delta() {
        let mut r = record(22, 49, 0);
        r[320 + 16..320 + 20].copy_from_slice(&0u32.to_le_bytes());
        r[320 + 20..320 + 24].copy_from_slice(&50u32.to_le_bytes()); // 50%
        r[320 + 24..320 + 26].copy_from_slice(&crate::ids::unit::MARINE.to_le_bytes());
        let mut t = TriggerSystem::parse(&r, &[]);
        let mut w = world();
        let id = w.spawn_px(0, crate::ids::unit::MARINE, 64, 64).unwrap();
        let max = w.data.unit(crate::ids::unit::MARINE).hit_points_raw as i32;
        w.units[id].hp = max;
        t.process(&mut w);
        assert_eq!(w.units[id].hp, max * 50 / 100);
    }

    #[test]
    fn set_invincibility_blocks_damage_until_disabled() {
        let mut r = record(22, 43, 0);
        r[320 + 16..320 + 20].copy_from_slice(&0u32.to_le_bytes());
        r[320 + 24..320 + 26].copy_from_slice(&crate::ids::unit::MARINE.to_le_bytes());
        r[320 + 27] = 4; // enable
        let mut t = TriggerSystem::parse(&r, &[]);
        let mut w = world();
        let id = w.spawn_px(0, crate::ids::unit::MARINE, 64, 64).unwrap();
        let before = w.units[id].hp;
        t.process(&mut w);
        assert!(w.units[id].invincible);
        w.damage_unit(w.units[id].id, 1_000).unwrap();
        assert_eq!(w.units[id].hp, before);

        // Toggle off, then damage lands.
        let mut r2 = record(22, 43, 0);
        r2[320 + 24..320 + 26].copy_from_slice(&crate::ids::unit::MARINE.to_le_bytes());
        r2[320 + 27] = 5; // disable
        let mut t2 = TriggerSystem::parse(&r2, &[]);
        t2.process(&mut w);
        assert!(!w.units[id].invincible);
        w.damage_unit(w.units[id].id, 100).unwrap();
        assert!(w.units[id].hp < before);
    }

    #[test]
    fn force_triggers_run_for_the_force_members_in_the_game() {
        // A trigger enabled only for Force 1 (index 18) that sets minerals.
        let mut r = vec![0u8; RECORD_SIZE];
        r[15] = 22; // Always
        r[320 + 20..320 + 24].copy_from_slice(&777u32.to_le_bytes());
        r[320 + 16..320 + 20].copy_from_slice(&13u32.to_le_bytes()); // current player
        r[320 + 26] = 26; // Set Resources
        r[320 + 27] = 7; // set to
        r[2372 + 18] = 1;
        let mut t = TriggerSystem::parse(&r, &[]);
        // Player 0 in force 1 (index 0), player 1 in force 2.
        t.forces = [0, 1, 0, 0, 0, 0, 0, 0];
        t.present = [true, true, false, false, false, false, false, false];
        let mut w = world();
        w.tick = 1;
        t.process(&mut w);
        assert_eq!(w.players[0].minerals, 777, "force 1 member runs it");
        assert_ne!(w.players[1].minerals, 777, "force 2 does not");
        // Before the fix only per-player (0..8) and All Players (17)
        // triggers ran, so Force 1 campaign triggers never executed.
    }

    #[test]
    fn triggers_first_run_at_elapsed_time_zero() {
        // `terran04`: Elapsed Time at most 0 -> set objectives.
        let mut r = record(12, 26, 0);
        r[13] = 1; // at most
        r[8..12].copy_from_slice(&0u32.to_le_bytes());
        r[320 + 20..320 + 24].copy_from_slice(&5u32.to_le_bytes());
        r[320 + 27] = 7;
        let mut t = TriggerSystem::parse(&r, &[]);
        let mut w = world();
        w.tick = 1;
        t.process(&mut w);
        assert_eq!(w.players[0].minerals, 5);
    }

    #[test]
    fn a_wait_resumes_on_the_tick_it_ends() {
        // Wait 2000 ms (48 ticks) then set minerals: retail resumes at
        // the end of the wait, not at the next 30-tick trigger cycle.
        let mut r = record(22, 4, 0);
        r[320 + 12..320 + 16].copy_from_slice(&2000u32.to_le_bytes());
        let a1 = add_action(&mut r, 1, 26);
        r[a1 + 20..a1 + 24].copy_from_slice(&777u32.to_le_bytes());
        r[a1 + 27] = 7;
        let mut t = TriggerSystem::parse(&r, &[]);
        let mut w = world();
        w.tick = 1;
        t.process(&mut w);
        for tick in 2..=48 {
            w.tick = tick;
            t.process(&mut w);
            assert_ne!(w.players[0].minerals, 777, "still waiting at {tick}");
        }
        w.tick = 49;
        t.process(&mut w);
        assert_eq!(w.players[0].minerals, 777, "resumed at tick 1 + 48");
    }

    #[test]
    fn victory_and_defeat_apply_to_the_player_running_the_trigger() {
        // Always -> Victory, enabled for player 2 (index 1), group 0.
        let mut t = TriggerSystem::parse(&record(22, 1, 1), &[]);
        let mut w = world();
        w.tick = 1;
        t.process(&mut w);
        assert_eq!(
            w.winner,
            Some(1),
            "the trigger's player wins, not player 1 (group 0)"
        );
        let mut t = TriggerSystem::parse(&record(22, 2, 1), &[]);
        let mut w = world();
        w.tick = 1;
        t.process(&mut w);
        assert!(w.players[1].defeated && !w.players[0].defeated);
    }

    #[test]
    fn transmissions_play_one_after_another() {
        // Two 3 s transmissions in one trigger: the second waits for the
        // first line to finish instead of starting in the same tick.
        let mut r = record(22, 7, 0);
        r[320 + 12..320 + 16].copy_from_slice(&3000u32.to_le_bytes());
        let a1 = add_action(&mut r, 1, 7);
        r[a1 + 12..a1 + 16].copy_from_slice(&3000u32.to_le_bytes());
        let mut t = TriggerSystem::parse(&r, &[]);
        let mut w = world();
        t.process(&mut w);
        assert_eq!(t.transmissions.len(), 1);
        w.tick += 30;
        t.process(&mut w);
        assert_eq!(t.transmissions.len(), 1, "still talking at 1.25 s");
        w.tick += 60;
        t.process(&mut w);
        assert_eq!(t.transmissions.len(), 2, "next line after 3 s");
    }

    #[test]
    fn wait_action_resumes_the_remaining_actions() {
        // Actions: set minerals 10, wait 0 ms, set gas 20.
        let mut r = record(22, 26, 0);
        let a0 = 320;
        r[a0 + 20..a0 + 24].copy_from_slice(&10u32.to_le_bytes());
        r[a0 + 24] = 0;
        r[a0 + 27] = 7;
        let a1 = add_action(&mut r, 1, 4);
        r[a1 + 12..a1 + 16].copy_from_slice(&0u32.to_le_bytes());
        let a2 = add_action(&mut r, 2, 26);
        r[a2 + 20..a2 + 24].copy_from_slice(&20u32.to_le_bytes());
        r[a2 + 24] = 1;
        r[a2 + 27] = 7;
        let mut t = TriggerSystem::parse(&r, &[]);
        let mut w = world();
        t.process(&mut w);
        assert_eq!(w.players[0].minerals, 10);
        assert_eq!(w.players[0].gas, 0);
        w.tick += 30;
        t.process(&mut w);
        assert_eq!(w.players[0].gas, 20);
        assert_eq!(w.players[0].minerals, 10, "wait resumed, not restarted");
    }

    #[test]
    fn set_doodad_state_and_order_action_apply_the_chosen_mode() {
        let locations = vec![(0, 0, 64, 64), (64, 64, 128, 128)];
        let mut r = record(22, 42, 0);
        r[320..320 + 4].copy_from_slice(&1u32.to_le_bytes());
        r[320 + 24..320 + 26].copy_from_slice(&crate::ids::unit::MARINE.to_le_bytes());
        r[320 + 27] = 5; // disable
        let mut t = TriggerSystem::parse(&r, &locations);
        let mut w = world();
        let id = w.spawn_px(0, crate::ids::unit::MARINE, 32, 32).unwrap();
        t.process(&mut w);
        assert_eq!(w.units[id].doodad_enabled, Some(false));

        let mut o = record(22, 46, 0);
        o[320..320 + 4].copy_from_slice(&1u32.to_le_bytes());
        o[320 + 16..320 + 20].copy_from_slice(&0u32.to_le_bytes());
        o[320 + 20..320 + 24].copy_from_slice(&2u32.to_le_bytes());
        o[320 + 24..320 + 26].copy_from_slice(&crate::ids::unit::MARINE.to_le_bytes());
        o[320 + 27] = 1; // patrol
        let mut t2 = TriggerSystem::parse(&o, &locations);
        t2.process(&mut w);
        assert!(matches!(w.units[id].order, Order::Patrol { .. }));

        o[320 + 27] = 2; // attack
        let mut t3 = TriggerSystem::parse(&o, &locations);
        t3.process(&mut w);
        assert!(matches!(w.units[id].order, Order::AttackMove { .. }));
    }

    #[test]
    fn every_classic_action_id_dispatches_without_silently_dropping_gameplay_ids() {
        // The semantic ids covered by this pass must reach their handler even
        // when the operands are the neutral defaults.
        for kind in [
            7u8, 8, 9, 10, 11, 12, 17, 18, 19, 20, 21, 23, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36,
            37, 40, 41, 42, 43, 44, 46, 49, 50, 51, 52, 53, 54, 55, 57,
        ] {
            let mut r = record(22, kind, 0);
            r[320 + 16..320 + 20].copy_from_slice(&0u32.to_le_bytes());
            r[320 + 20..320 + 24].copy_from_slice(&1u32.to_le_bytes());
            r[320 + 24..320 + 26].copy_from_slice(&crate::ids::unit::MARINE.to_le_bytes());
            r[320 + 27] = 4;
            let mut t = TriggerSystem::parse(&r, &[]);
            let mut w = world();
            let _ = w.spawn_px(0, crate::ids::unit::MARINE, 64, 64);
            t.process(&mut w);
        }
    }
}

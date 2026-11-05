//! Deterministic command log: save games, replays and LAN sync share it.
//!
//! Saving = initial setup + every [`Order`](crate::sim::Order) issued with
//! its tick. Loading = rebuild the identical [`World`](crate::sim::World)
//! and fast-forward it by re-applying the log. The simulation is
//! deterministic (LCG in `World::rng`), so the restored state hashes
//! exactly like the original at the same tick.
//!
//! File layout (all integers little-endian):
//! `MAGIC u32` + `VERSION u16` + `seed u64` + `map_hash [u8;32]` +
//! `cmd_count u32` + cmds + `sha256(payload) [u8;32]`.
//! Each command: `tick u64` + `unit u32` + order bytes.
//! Each order: `tag u8` + fields. Tags follow declaration order.

use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;

use sha2::{Digest, Sha256};

use crate::projectile::BulletState;
use crate::sim::{CarryKind, Order, Race, SimUnit, Spell, SpellTarget, World};

pub const MAGIC: u32 = 0x4743_5331; // "GCS1"
pub const VERSION: u16 = 2; // v2: commands carry Issue or economy acts.

/// One recorded action: a player/AI order or a direct economy call
/// (`World::train` / `morph` / `morph_building`, used by the AI and the
/// client outside `World::issue`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecCmd {
    pub tick: u64,
    pub act: RecAct,
}

/// What to re-apply on load. Order of variants is the encoding tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecAct {
    Issue { unit: u32, order: Order },
    Train { building: u32, unit: u16 },
    Morph { larva: u32, unit: u16 },
    MorphBuilding { source: u32, unit: u16 },
}

/// A save game / replay: setup plus the command log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recording {
    /// RNG state the fresh world starts from (`World::new` default).
    pub seed: u64,
    /// SHA-256 of the source CHK (all zeros when built ad hoc in tests).
    pub map_hash: [u8; 32],
    pub cmds: Vec<RecCmd>,
}

/// Live recorder attached to [`World::recorder`]; `World::issue` pushes here.
#[derive(Clone, Debug, Default)]
pub struct Recorder {
    pub cmds: Vec<RecCmd>,
}

impl Recorder {
    pub fn new() -> Self {
        Self { cmds: Vec::new() }
    }

    pub fn record(&mut self, tick: u64, unit: u32, order: Order) {
        self.cmds.push(RecCmd {
            tick,
            act: RecAct::Issue { unit, order },
        });
    }

    pub fn record_econ(&mut self, tick: u64, act: RecAct) {
        debug_assert!(!matches!(act, RecAct::Issue { .. }));
        self.cmds.push(RecCmd { tick, act });
    }
}

// ---------------------------------------------------------------------------
// Manual little-endian encoding (stable across versions, no serde needed).
// ---------------------------------------------------------------------------

pub(crate) fn w_u8(out: &mut Vec<u8>, v: u8) {
    out.push(v);
}
pub(crate) fn w_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}
pub(crate) fn w_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}
pub(crate) fn w_i32(out: &mut Vec<u8>, v: i32) {
    out.extend_from_slice(&v.to_le_bytes());
}
pub(crate) fn w_u64(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_le_bytes());
}
pub(crate) fn w_bool(out: &mut Vec<u8>, v: bool) {
    out.push(v as u8);
}

/// Byte cursor for manual decoding (public so `decode_order` can stay public).
pub struct Cursor<'a> {
    b: &'a [u8],
    o: usize,
}

impl<'a> Cursor<'a> {
    /// Fresh cursor over `b`.
    pub fn new(b: &'a [u8]) -> Self {
        Self { b, o: 0 }
    }

    /// Bytes consumed so far.
    pub fn offset(&self) -> usize {
        self.o
    }

    /// True when the whole buffer was consumed.
    pub fn at_end(&self) -> bool {
        self.o == self.b.len()
    }

    /// 32 raw bytes (map hashes, keys).
    pub fn take32(&mut self) -> Option<[u8; 32]> {
        let s = self.take(32)?;
        let mut out = [0u8; 32];
        out.copy_from_slice(s);
        Some(out)
    }

    /// `n` raw bytes.
    pub fn take_bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        self.take(n)
    }

    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.b.get(self.o..self.o + n)?;
        self.o += n;
        Some(s)
    }
    pub fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|s| s[0])
    }
    pub fn u16(&mut self) -> Option<u16> {
        self.take(2).map(|s| u16::from_le_bytes([s[0], s[1]]))
    }
    pub fn u32(&mut self) -> Option<u32> {
        self.take(4)
            .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    pub fn i32(&mut self) -> Option<i32> {
        self.take(4)
            .map(|s| i32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    pub fn u64(&mut self) -> Option<u64> {
        self.take(8)
            .map(|s| u64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]))
    }
    pub fn bool(&mut self) -> Option<bool> {
        self.u8().map(|v| v != 0)
    }
}

fn spell_tag(s: Spell) -> u8 {
    match s {
        Spell::StimPacks => 0,
        Spell::Lockdown => 1,
        Spell::EmpShockwave => 2,
        Spell::SpiderMines => 3,
        Spell::ScannerSweep => 4,
        Spell::TankSiegeMode => 5,
        Spell::DefensiveMatrix => 6,
        Spell::Irradiate => 7,
        Spell::YamatoGun => 8,
        Spell::CloakingField => 9,
        Spell::PersonnelCloaking => 10,
        Spell::Burrowing => 11,
        Spell::SpawnBroodlings => 12,
        Spell::DarkSwarm => 13,
        Spell::Plague => 14,
        Spell::Consume => 15,
        Spell::Ensnare => 16,
        Spell::Parasite => 17,
        Spell::Infestation => 18,
        Spell::PsionicStorm => 19,
        Spell::Hallucination => 20,
        Spell::Recall => 21,
        Spell::StasisField => 22,
        Spell::ArchonWarp => 23,
        Spell::Restoration => 24,
        Spell::DisruptionWeb => 25,
        Spell::MindControl => 26,
        Spell::DarkArchonMeld => 27,
        Spell::Feedback => 28,
        Spell::OpticalFlare => 29,
        Spell::Maelstrom => 30,
        Spell::LurkerAspect => 31,
        Spell::Healing => 32,
    }
}

fn spell_from_tag(t: u8) -> Option<Spell> {
    Some(match t {
        0 => Spell::StimPacks,
        1 => Spell::Lockdown,
        2 => Spell::EmpShockwave,
        3 => Spell::SpiderMines,
        4 => Spell::ScannerSweep,
        5 => Spell::TankSiegeMode,
        6 => Spell::DefensiveMatrix,
        7 => Spell::Irradiate,
        8 => Spell::YamatoGun,
        9 => Spell::CloakingField,
        10 => Spell::PersonnelCloaking,
        11 => Spell::Burrowing,
        12 => Spell::SpawnBroodlings,
        13 => Spell::DarkSwarm,
        14 => Spell::Plague,
        15 => Spell::Consume,
        16 => Spell::Ensnare,
        17 => Spell::Parasite,
        18 => Spell::Infestation,
        19 => Spell::PsionicStorm,
        20 => Spell::Hallucination,
        21 => Spell::Recall,
        22 => Spell::StasisField,
        23 => Spell::ArchonWarp,
        24 => Spell::Restoration,
        25 => Spell::DisruptionWeb,
        26 => Spell::MindControl,
        27 => Spell::DarkArchonMeld,
        28 => Spell::Feedback,
        29 => Spell::OpticalFlare,
        30 => Spell::Maelstrom,
        31 => Spell::LurkerAspect,
        32 => Spell::Healing,
        _ => return None,
    })
}

fn encode_target(out: &mut Vec<u8>, t: &SpellTarget) {
    match t {
        SpellTarget::None => w_u8(out, 0),
        SpellTarget::Unit(u) => {
            w_u8(out, 1);
            w_u32(out, *u);
        }
        SpellTarget::Point(x, y) => {
            w_u8(out, 2);
            w_i32(out, *x);
            w_i32(out, *y);
        }
    }
}

fn decode_target(c: &mut Cursor) -> Option<SpellTarget> {
    match c.u8()? {
        0 => Some(SpellTarget::None),
        1 => Some(SpellTarget::Unit(c.u32()?)),
        2 => {
            let x = c.i32()?;
            let y = c.i32()?;
            Some(SpellTarget::Point(x, y))
        }
        _ => None,
    }
}

/// Order tags follow declaration order in `sim.rs` (Idle = 0 .. Die = 23).
pub fn encode_order(out: &mut Vec<u8>, o: &Order) {
    match o {
        Order::Idle => w_u8(out, 0),
        Order::Stop => w_u8(out, 1),
        Order::Move { pos } => {
            w_u8(out, 2);
            w_i32(out, pos.0);
            w_i32(out, pos.1);
        }
        Order::AttackMove { pos } => {
            w_u8(out, 3);
            w_i32(out, pos.0);
            w_i32(out, pos.1);
        }
        Order::AttackUnit { target } => {
            w_u8(out, 4);
            w_u32(out, *target);
        }
        Order::HoldPosition => w_u8(out, 5),
        Order::Patrol { a, b, forward } => {
            w_u8(out, 6);
            w_i32(out, a.0);
            w_i32(out, a.1);
            w_i32(out, b.0);
            w_i32(out, b.1);
            w_bool(out, *forward);
        }
        Order::Gather { resource } => {
            w_u8(out, 7);
            w_u32(out, *resource);
        }
        Order::ReturnCargo { depot } => {
            w_u8(out, 8);
            w_u32(out, *depot);
        }
        Order::Build { unit_type, pos } => {
            w_u8(out, 9);
            w_u16(out, *unit_type);
            w_i32(out, pos.0);
            w_i32(out, pos.1);
        }
        Order::Repair { target } => {
            w_u8(out, 10);
            w_u32(out, *target);
        }
        Order::Train { unit_type } => {
            w_u8(out, 11);
            w_u16(out, *unit_type);
        }
        Order::Research { tech_id } => {
            w_u8(out, 12);
            w_u8(out, *tech_id);
        }
        Order::Upgrade { upgrade_id, level } => {
            w_u8(out, 13);
            w_u8(out, *upgrade_id);
            w_u8(out, *level);
        }
        Order::Cast { spell, target } => {
            w_u8(out, 14);
            w_u8(out, spell_tag(*spell));
            encode_target(out, target);
        }
        Order::EnterTransport { transport } => {
            w_u8(out, 15);
            w_u32(out, *transport);
        }
        Order::Unload => w_u8(out, 16),
        Order::Burrow => w_u8(out, 17),
        Order::Unburrow => w_u8(out, 18),
        Order::Siege => w_u8(out, 19),
        Order::Unsiege => w_u8(out, 20),
        Order::Lift => w_u8(out, 21),
        Order::Land { pos } => {
            w_u8(out, 22);
            w_i32(out, pos.0);
            w_i32(out, pos.1);
        }
        Order::Die => w_u8(out, 23),
    }
}

pub fn decode_order(c: &mut Cursor<'_>) -> Option<Order> {
    Some(match c.u8()? {
        0 => Order::Idle,
        1 => Order::Stop,
        2 => {
            let x = c.i32()?;
            let y = c.i32()?;
            Order::Move { pos: (x, y) }
        }
        3 => {
            let x = c.i32()?;
            let y = c.i32()?;
            Order::AttackMove { pos: (x, y) }
        }
        4 => Order::AttackUnit { target: c.u32()? },
        5 => Order::HoldPosition,
        6 => {
            let ax = c.i32()?;
            let ay = c.i32()?;
            let bx = c.i32()?;
            let by = c.i32()?;
            let forward = c.bool()?;
            Order::Patrol {
                a: (ax, ay),
                b: (bx, by),
                forward,
            }
        }
        7 => Order::Gather { resource: c.u32()? },
        8 => Order::ReturnCargo { depot: c.u32()? },
        9 => {
            let unit_type = c.u16()?;
            let x = c.i32()?;
            let y = c.i32()?;
            Order::Build {
                unit_type,
                pos: (x, y),
            }
        }
        10 => Order::Repair { target: c.u32()? },
        11 => Order::Train {
            unit_type: c.u16()?,
        },
        12 => Order::Research { tech_id: c.u8()? },
        13 => Order::Upgrade {
            upgrade_id: c.u8()?,
            level: c.u8()?,
        },
        14 => {
            let spell = spell_from_tag(c.u8()?)?;
            let target = decode_target(c)?;
            Order::Cast { spell, target }
        }
        15 => Order::EnterTransport {
            transport: c.u32()?,
        },
        16 => Order::Unload,
        17 => Order::Burrow,
        18 => Order::Unburrow,
        19 => Order::Siege,
        20 => Order::Unsiege,
        21 => Order::Lift,
        22 => {
            let x = c.i32()?;
            let y = c.i32()?;
            Order::Land { pos: (x, y) }
        }
        23 => Order::Die,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Whole-file save / load.
// ---------------------------------------------------------------------------

fn encode_recording(rec: &Recording) -> Vec<u8> {
    let mut payload = Vec::new();
    w_u64(&mut payload, rec.seed);
    payload.extend_from_slice(&rec.map_hash);
    w_u32(&mut payload, rec.cmds.len() as u32);
    for cmd in &rec.cmds {
        w_u64(&mut payload, cmd.tick);
        encode_act(&mut payload, &cmd.act);
    }
    let digest = Sha256::digest(&payload);
    let mut out = Vec::with_capacity(4 + 2 + payload.len() + 32);
    out.extend_from_slice(&MAGIC.to_le_bytes());
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&payload);
    out.extend_from_slice(&digest);
    out
}

fn decode_recording(bytes: &[u8]) -> Option<Recording> {
    if bytes.len() < 4 + 2 + 8 + 32 + 4 + 32 {
        return None;
    }
    if u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) != MAGIC {
        return None;
    }
    if u16::from_le_bytes([bytes[4], bytes[5]]) != VERSION {
        return None;
    }
    decode_recording_inner(bytes)
}

impl Recording {
    pub fn new(seed: u64, map_hash: [u8; 32]) -> Self {
        Self {
            seed,
            map_hash,
            cmds: Vec::new(),
        }
    }

    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        std::fs::write(path, encode_recording(self))
    }

    pub fn load(path: &std::path::Path) -> Option<Self> {
        decode_recording(&std::fs::read(path).ok()?)
    }

    /// In-memory form, for embedding in a network message or a save file.
    pub fn to_bytes(&self) -> Vec<u8> {
        encode_recording(self)
    }

    /// Parse the in-memory form (same bytes as a `.gcrep` file).
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        decode_recording(bytes)
    }
}

/// A self-contained save game / replay: map bytes plus setup plus log.
///
/// `map_chk` is the raw `staredit\scenario.chk` (or whole `.chk` file) so a
/// save loads even if the original map file moved. `tick` is the world tick
/// at save time; loading replays `rec` up to `tick`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveFile {
    pub map_name: String,
    pub map_chk: Vec<u8>,
    pub races: Vec<Race>,
    pub local_player: usize,
    pub tick: u64,
    pub rec: Recording,
}

const SAVE_MAGIC: u32 = 0x4743_5356; // "GCSF"
const SAVE_VERSION: u16 = 1;

fn encode_str(out: &mut Vec<u8>, s: &str) {
    let b = s.as_bytes();
    w_u32(out, b.len().min(1024) as u32);
    out.extend_from_slice(&b[..b.len().min(1024)]);
}

fn decode_str(c: &mut Cursor) -> Option<String> {
    let n = c.u32()? as usize;
    if n > 1024 {
        return None;
    }
    let b = c.take(n)?;
    String::from_utf8(b.to_vec()).ok()
}

fn encode_races(out: &mut Vec<u8>, races: &[Race]) {
    w_u32(out, races.len().min(12) as u32);
    for r in races.iter().take(12) {
        w_u8(out, race_tag(*r));
    }
}

fn decode_races(c: &mut Cursor) -> Option<Vec<Race>> {
    let n = c.u32()? as usize;
    if n > 12 {
        return None;
    }
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(match c.u8()? {
            0 => Race::Zerg,
            1 => Race::Terran,
            2 => Race::Protoss,
            _ => Race::Neutral,
        });
    }
    Some(out)
}

impl SaveFile {
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        let mut payload = Vec::new();
        encode_str(&mut payload, &self.map_name);
        w_u32(&mut payload, self.map_chk.len() as u32);
        payload.extend_from_slice(&self.map_chk);
        encode_races(&mut payload, &self.races);
        w_u32(&mut payload, self.local_player as u32);
        w_u64(&mut payload, self.tick);
        let rec_bytes = encode_recording(&self.rec);
        w_u32(&mut payload, rec_bytes.len() as u32);
        payload.extend_from_slice(&rec_bytes);
        let digest = Sha256::digest(&payload);
        let mut out = Vec::with_capacity(4 + 2 + payload.len() + 32);
        out.extend_from_slice(&SAVE_MAGIC.to_le_bytes());
        out.extend_from_slice(&SAVE_VERSION.to_le_bytes());
        out.extend_from_slice(&payload);
        out.extend_from_slice(&digest);
        std::fs::write(path, out)
    }

    pub fn load(path: &std::path::Path) -> Option<Self> {
        let bytes = std::fs::read(path).ok()?;
        if bytes.len() < 4 + 2 + 32 {
            return None;
        }
        if u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) != SAVE_MAGIC {
            return None;
        }
        if u16::from_le_bytes([bytes[4], bytes[5]]) != SAVE_VERSION {
            return None;
        }
        let payload = &bytes[6..bytes.len() - 32];
        if Sha256::digest(payload).as_slice() != &bytes[bytes.len() - 32..] {
            return None;
        }
        let mut c = Cursor { b: payload, o: 0 };
        let map_name = decode_str(&mut c)?;
        let chk_len = c.u32()? as usize;
        if chk_len > 8 << 20 {
            return None;
        }
        let map_chk = c.take(chk_len)?.to_vec();
        let races = decode_races(&mut c)?;
        let local_player = c.u32()? as usize;
        let tick = c.u64()?;
        let rec_len = c.u32()? as usize;
        let rec_bytes = c.take(rec_len)?;
        let rec = decode_recording_from(rec_bytes)?;
        if c.o != payload.len() {
            return None;
        }
        if local_player > races.len() {
            return None;
        }
        Some(Self {
            map_name,
            map_chk,
            races,
            local_player,
            tick,
            rec,
        })
    }
}

fn decode_recording_from(bytes: &[u8]) -> Option<Recording> {
    decode_recording(bytes)
}

fn decode_recording_inner(bytes: &[u8]) -> Option<Recording> {
    let payload = &bytes[6..bytes.len() - 32];
    if Sha256::digest(payload).as_slice() != &bytes[bytes.len() - 32..] {
        return None;
    }
    let mut c = Cursor { b: payload, o: 0 };
    let seed = c.u64()?;
    let mut map_hash = [0u8; 32];
    map_hash.copy_from_slice(c.take(32)?);
    let n = c.u32()? as usize;
    if n > 10_000_000 {
        return None;
    }
    let mut cmds = Vec::with_capacity(n.min(1 << 20));
    for _ in 0..n {
        let tick = c.u64()?;
        let act = decode_act(&mut c)?;
        cmds.push(RecCmd { tick, act });
    }
    if c.o != payload.len() {
        return None;
    }
    Some(Recording {
        seed,
        map_hash,
        cmds,
    })
}

// ---------------------------------------------------------------------------
// Deterministic state hash (save/load + desync checksum).
// ---------------------------------------------------------------------------

fn race_tag(r: Race) -> u8 {
    match r {
        Race::Zerg => 0,
        Race::Terran => 1,
        Race::Protoss => 2,
        Race::Neutral => 3,
    }
}

fn carry_tag(c: CarryKind) -> u8 {
    match c {
        CarryKind::None => 0,
        CarryKind::Minerals => 1,
        CarryKind::Gas => 2,
    }
}

fn bullet_tag(s: BulletState) -> u8 {
    match s {
        BulletState::Fly => 0,
        BulletState::Follow => 1,
        BulletState::Bounce => 2,
        BulletState::HitNearTarget => 3,
        BulletState::Extend => 4,
        BulletState::Persist => 5,
    }
}

fn hash_order(h: &mut DefaultHasher, o: &Order) {
    use std::hash::Hash;
    let mut buf = Vec::new();
    encode_order(&mut buf, o);
    buf.hash(h);
}

fn hash_opt_u32(h: &mut DefaultHasher, v: Option<u32>) {
    use std::hash::Hash;
    match v {
        None => 0u8.hash(h),
        Some(x) => {
            1u8.hash(h);
            x.hash(h);
        }
    }
}

fn hash_unit(h: &mut DefaultHasher, u: &SimUnit) {
    use std::hash::Hash;
    u.id.hash(h);
    u.owner.hash(h);
    u.type_id.hash(h);
    u.hp.hash(h);
    u.shields.hash(h);
    u.energy.hash(h);
    u.x.hash(h);
    u.y.hash(h);
    u.facing.hash(h);
    u.alive.hash(h);
    u.completed.hash(h);
    u.cooldown.hash(h);
    hash_order(h, &u.order);
    h.write_usize(u.queue.len());
    for o in &u.queue {
        hash_order(h, o);
    }
    hash_opt_u32(h, u.target);
    u.carry.hash(h);
    carry_tag(u.carry_kind).hash(h);
    u.build_remaining.hash(h);
    u.hangar.hash(h);
    // Status effect timers decide future damage/healing: hash them all.
    let s = &u.status;
    s.matrix.hash(h);
    s.acid_spores.hash(h);
    s.hallucination.hash(h);
    s.stim_ticks.hash(h);
    s.lockdown_ticks.hash(h);
    s.stasis_ticks.hash(h);
    s.irradiate_ticks.hash(h);
    s.plague_ticks.hash(h);
    s.ensnare_ticks.hash(h);
    s.maelstrom_ticks.hash(h);
    s.disruption_ticks.hash(h);
    s.parasite.hash(h);
    s.optical_flare_ticks.hash(h);
    s.cloaked.hash(h);
    s.burrowed.hash(h);
    s.sieged.hash(h);
    s.revealed_ticks.hash(h);
}

/// Hash of everything that decides the future: tick, RNG, players, units,
/// projectiles. Two runs that reach the same hash from the same setup are
/// observationally identical from here on.
pub fn state_hash(w: &World) -> u64 {
    use std::hash::Hash;
    let mut h = DefaultHasher::new();
    w.tick.hash(&mut h);
    w.rng.hash(&mut h);
    match w.winner {
        None => 0u8.hash(&mut h),
        Some(p) => {
            1u8.hash(&mut h);
            p.hash(&mut h);
        }
    }
    w.next_id.hash(&mut h);
    for p in &w.players {
        race_tag(p.race).hash(&mut h);
        p.team.hash(&mut h);
        p.minerals.hash(&mut h);
        p.gas.hash(&mut h);
        p.techs.hash(&mut h);
        p.upgrades.hash(&mut h);
        p.defeated.hash(&mut h);
        p.minerals_gathered.hash(&mut h);
        (
            p.units_produced,
            p.buildings_built,
            p.units_lost,
            p.buildings_lost,
        )
            .hash(&mut h);
        (p.minerals_spent, p.gas_spent, p.scores).hash(&mut h);
        p.gas_gathered.hash(&mut h);
    }
    h.write_usize(w.units.len());
    for u in &w.units {
        hash_unit(&mut h, u);
    }
    h.write_usize(w.projectiles.len());
    for b in &w.projectiles {
        b.id.hash(&mut h);
        b.owner.hash(&mut h);
        b.weapon.hash(&mut h);
        b.behavior.hash(&mut h);
        b.x.hash(&mut h);
        b.y.hash(&mut h);
        hash_opt_u32(&mut h, b.target);
        b.target_pos.0.hash(&mut h);
        b.target_pos.1.hash(&mut h);
        b.speed.hash(&mut h);
        b.remaining_time.hash(&mut h);
        b.bounces_left.hash(&mut h);
        bullet_tag(b.state).hash(&mut h);
    }
    h.finish()
}

// ---------------------------------------------------------------------------
// Fast-forward restore: rebuild, then re-apply the log up to `upto_tick.
// ---------------------------------------------------------------------------

/// Re-apply `rec` onto a freshly built world, stepping to `upto_tick`.
/// Commands are applied at the start of their tick, exactly like the live
/// loop (`apply cmds[t]`, then `step`), so the result hashes like the
/// original run at the same tick. Re-running `issue`/`train`/`morph` calls
/// is exact because the sim is deterministic and the log captures every
/// external mutation in order.
pub fn replay_to(world: &mut World, rec: &Recording, upto_tick: u64) {
    let mut i = 0;
    while world.tick < upto_tick {
        while i < rec.cmds.len() && rec.cmds[i].tick == world.tick {
            apply_act(world, &rec.cmds[i].act);
            i += 1;
        }
        world.step();
    }
    // Commands stamped exactly at the final tick apply without a step.
    while i < rec.cmds.len() && rec.cmds[i].tick == upto_tick {
        apply_act(world, &rec.cmds[i].act);
        i += 1;
    }
}

/// Apply one logged action (shared by `replay_to` and the sc_view player).
pub fn apply_act(world: &mut World, act: &RecAct) {
    match act {
        RecAct::Issue { unit, order } => {
            let _ = world.issue(*unit, order.clone());
        }
        RecAct::Train { building, unit } => {
            let _ = world.train(*building, *unit);
        }
        RecAct::Morph { larva, unit } => {
            let _ = world.morph(*larva, *unit);
        }
        RecAct::MorphBuilding { source, unit } => {
            let _ = world.morph_building(*source, *unit);
        }
    }
}

pub fn encode_act(out: &mut Vec<u8>, act: &RecAct) {
    match act {
        RecAct::Issue { unit, order } => {
            w_u8(out, 0);
            w_u32(out, *unit);
            encode_order(out, order);
        }
        RecAct::Train { building, unit } => {
            w_u8(out, 1);
            w_u32(out, *building);
            w_u16(out, *unit);
        }
        RecAct::Morph { larva, unit } => {
            w_u8(out, 2);
            w_u32(out, *larva);
            w_u16(out, *unit);
        }
        RecAct::MorphBuilding { source, unit } => {
            w_u8(out, 3);
            w_u32(out, *source);
            w_u16(out, *unit);
        }
    }
}

pub fn decode_act(c: &mut Cursor<'_>) -> Option<RecAct> {
    Some(match c.u8()? {
        0 => RecAct::Issue {
            unit: c.u32()?,
            order: decode_order(c)?,
        },
        1 => RecAct::Train {
            building: c.u32()?,
            unit: c.u16()?,
        },
        2 => RecAct::Morph {
            larva: c.u32()?,
            unit: c.u16()?,
        },
        3 => RecAct::MorphBuilding {
            source: c.u32()?,
            unit: c.u16()?,
        },
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_roundtrip_all_tags() {
        let samples = vec![
            Order::Idle,
            Order::Stop,
            Order::Move { pos: (1, -2) },
            Order::AttackMove { pos: (3, 4) },
            Order::AttackUnit { target: 99 },
            Order::HoldPosition,
            Order::Patrol {
                a: (1, 2),
                b: (3, 4),
                forward: true,
            },
            Order::Gather { resource: 7 },
            Order::ReturnCargo { depot: 8 },
            Order::Build {
                unit_type: 106,
                pos: (-5, 6),
            },
            Order::Repair { target: 9 },
            Order::Train { unit_type: 10 },
            Order::Research { tech_id: 3 },
            Order::Upgrade {
                upgrade_id: 4,
                level: 2,
            },
            Order::Cast {
                spell: Spell::PsionicStorm,
                target: SpellTarget::Point(100, -200),
            },
            Order::Cast {
                spell: Spell::MindControl,
                target: SpellTarget::Unit(11),
            },
            Order::Cast {
                spell: Spell::Healing,
                target: SpellTarget::None,
            },
            Order::EnterTransport { transport: 12 },
            Order::Unload,
            Order::Burrow,
            Order::Unburrow,
            Order::Siege,
            Order::Unsiege,
            Order::Lift,
            Order::Land { pos: (7, 7) },
            Order::Die,
        ];
        assert_eq!(samples.len(), 26); // 24 variantes + 2 formas Cast extra
        let mut tags = std::collections::BTreeSet::new();
        for o in &samples {
            let mut buf = Vec::new();
            encode_order(&mut buf, o);
            tags.insert(buf[0]);
            let mut c = Cursor { b: &buf, o: 0 };
            assert_eq!(decode_order(&mut c), Some(o.clone()), "tag {:?}", o);
            assert_eq!(c.o, buf.len());
        }
        // All 24 variants (tags 0..=23) are covered.
        assert_eq!(tags.len(), 24);
        assert!(tags.contains(&0) && tags.contains(&23));
        assert!(decode_order(&mut Cursor { b: &[99], o: 0 }).is_none());
    }
}

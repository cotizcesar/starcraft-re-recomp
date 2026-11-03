//! `scripts\iscript.bin` reader and animation virtual machine.
//!
//! The image/unit animation system stores one script per iscript id.  The
//! Walking animation is where StarCraft's ground-unit speeds actually live:
//! `units.dat`'s flingy for a walking unit carries the sentinel speed `1`, and
//! the `move <speed>` opcode in the iscript `Walking` animation supplies the
//! real pixels-per-tick value.
//!
//! This module recovers both the data table and the opcode dispatcher.  The
//! dispatcher layout was recovered from the retail executor at runtime
//! `0x7a1134f0` (see `docs/EVIDENCE.md`); the opcode names and operand widths
//! are the shipped 0x00..0x44 instruction set.  [`AnimState`] and
//! [`Iscript::tick_anim`] implement the parts of the VM that affect gameplay
//! (frames, waits, jumps, calls, movement, attack/cast events, flips, turns
//! and animation end) and record the purely visual overlay/sound opcodes as
//! events so a renderer can consume them without the rules engine having to
//! know about graphics.

use std::collections::HashMap;

use crate::data::DataError;

// The animation slot order is fixed by the engine.  `FUN_7a1148f0` indexes
// the script header with the animation id, so these are data-layout constants.
pub const ANIM_INIT: usize = 0;
pub const ANIM_DEATH: usize = 1;
pub const ANIM_GND_ATTACK_INIT: usize = 2;
pub const ANIM_AIR_ATTACK_INIT: usize = 3;
pub const ANIM_UNUSED_1: usize = 4;
pub const ANIM_GND_ATTACK_REPEAT: usize = 5;
pub const ANIM_AIR_ATTACK_REPEAT: usize = 6;
pub const ANIM_CAST_SPELL: usize = 7;
pub const ANIM_GND_ATTACK_TO_IDLE: usize = 8;
pub const ANIM_AIR_ATTACK_TO_IDLE: usize = 9;
pub const ANIM_UNUSED_2: usize = 10;
pub const ANIM_WALKING: usize = 11;
pub const ANIM_WALKING_TO_IDLE: usize = 12;
pub const ANIM_SPECIAL_STATE_1: usize = 13;
pub const ANIM_SPECIAL_STATE_2: usize = 14;
pub const ANIM_ALMOST_BUILT: usize = 15;
pub const ANIM_BUILT: usize = 16;
pub const ANIM_LANDING: usize = 17;
pub const ANIM_LIFT_OFF: usize = 18;
pub const ANIM_IS_WORKING: usize = 19;
pub const ANIM_WORKING_TO_IDLE: usize = 20;
pub const ANIM_WARP_IN: usize = 21;
pub const ANIM_UNUSED_3: usize = 22;
pub const ANIM_BURROW: usize = 23;
pub const ANIM_UNBURROW: usize = 24;
pub const ANIM_SHIELD_OVER: usize = 25;
pub const ANIM_SHIELD_ON: usize = 26;
pub const ANIM_SHIELD_OFF: usize = 27;

/// Number of animation slots in the shipped header.  The table itself is
/// variable length; this is the count the engine recognises.
pub const ANIM_COUNT: usize = 28;

/// One animation event.  The rules engine consumes the gameplay events; the
/// visual events are retained so overlays and sounds are not silently lost.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnimEvent {
    /// Show a frame.  `raw` is the script operand; the engine adds the
    /// current direction to it (`FUN_7a114890`).
    Frame { raw: u16, direction: u8 },
    /// Play a sound id from `sfxdata.dat`.
    Sound(u16),
    /// A weapon event.  `weapon == None` means "choose ground/air from the
    /// current target" (opcode `attack`); `melee` is `attackmelee`.
    Attack {
        weapon: Option<u8>,
        melee: bool,
        ground: bool,
    },
    /// `castspell`: spend the current order's spell at this frame.
    CastSpell,
    /// `move <n>`: pixels per tick in 1/256 units.
    Move(i32),
    /// `setflspeed <raw>`.
    SetFlingSpeed(i32),
    /// `setfldirect <direction>` (already multiplied by 8).
    SetDirection(i32),
    /// `setflipstate`.
    Flip(bool),
    /// `turnccwise`/`turncwise`/`turn1cwise`/`turnrand`.
    Turn(i32),
    /// `sigorder <flag>`.
    Signal(u8),
    /// `orderdone <flag>`.
    OrderDone(u8),
    /// `imgol`, `imgul`, `sprol`, `sprul`, `grdsprol`, ...  Kept as a typed
    /// event rather than modelled by the rules engine.
    Overlay { id: u16, x: i8, y: i8, below: bool },
    /// `playsndbtwn` / `playsndrand` resolved to one id.
    RandomSound(u16),
    /// `end`: remove the image.
    End,
    /// `ignorerest`: hold until the next animation is selected.
    IgnoreRest,
}

/// Mutable execution state for one image/unit animation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnimState {
    pub iscript: u16,
    pub anim: u8,
    /// Absolute file offset of the next opcode.
    pub pc: u16,
    /// Remaining `wait` ticks (the retail engine stores `n - 1`).
    pub wait: u8,
    /// Last `playfram` operand (before the direction offset).
    pub frame: u16,
    pub direction: u8,
    pub flip: bool,
    /// Return stack used by `call`/`return`.
    pub call_stack: Vec<u16>,
    /// Current `move` speed in 1/256 pixels per tick.
    pub move_speed: i32,
    /// Current `setflspeed` override, if any.
    pub fling_speed: i32,
    /// The script executed `end`.
    pub ended: bool,
    /// The script executed `ignorerest` and is waiting for a new animation.
    pub ignore_rest: bool,
    /// `nobrkcodestart` has blocked order interruption.
    pub orders_blocked: bool,
    /// `gotorepeatattk` was executed.
    pub repeat_attack: bool,
    /// Last signal emitted by `sigorder`.
    pub signal: u8,
    /// Number of opcodes executed, useful for diagnostics and loop guards.
    pub steps: u32,
}

/// Per-tick context supplied by the simulation.
#[derive(Clone, Debug, Default)]
pub struct AnimContext {
    pub direction: u8,
    pub tileset: u8,
    /// Deterministic random state.  The retail interpreter uses `ulRandID`;
    /// a simple LCG is sufficient for the animation-only random opcodes.
    pub rng: u32,
    /// Distance to the current target in pixels, if any.
    pub target_distance: Option<i32>,
    /// Angle to the current target in 1/256 turns, if any.
    pub target_angle: Option<i32>,
    /// Building is lifted off.
    pub lifted: bool,
    /// Powerup has been picked up.
    pub powerup_picked: bool,
    /// The unit is a hallucination (some overlay frames are suppressed).
    pub hallucination: bool,
}

impl AnimContext {
    fn next_random(&mut self) -> u32 {
        self.rng = self.rng.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        self.rng
    }
}

#[derive(Clone, Debug, Default)]
pub struct Iscript {
    /// iscript id -> animation offset table.
    scripts: HashMap<u16, Vec<u16>>,
    /// Raw file, retained so per-animation decoding does not need a second
    /// copy of the table.
    bytes: Vec<u8>,
    /// Cached walking speed in 1/256 pixels per tick.
    walk_speeds: HashMap<u16, i32>,
    /// Full per-tick speed sequence for the Walking animation.  A walking
    /// script alternates several `move` values separated by `wait` opcodes;
    /// replaying the cycle preserves the minor speed ripple instead of using
    /// only the mean.
    walk_cycles: HashMap<u16, Vec<i32>>,
}

impl Iscript {
    pub fn parse(d: &[u8]) -> Result<Self, DataError> {
        if d.len() < 8 {
            return Err(DataError::ShortFile {
                which: "iscript.bin",
                expected: 8,
                actual: d.len(),
            });
        }
        let id_list = u32::from_le_bytes([d[0], d[1], d[2], d[3]]) as usize;
        if id_list + 4 > d.len() {
            return Err(DataError::ShortFile {
                which: "iscript.bin",
                expected: id_list + 4,
                actual: d.len(),
            });
        }
        let mut scripts = HashMap::new();
        let mut p = id_list;
        while p + 4 <= d.len() {
            let id = i16::from_le_bytes([d[p], d[p + 1]]);
            if id == -1 {
                break;
            }
            let addr = u16::from_le_bytes([d[p + 2], d[p + 3]]) as usize;
            p += 4;
            if addr + 8 > d.len() {
                continue;
            }
            let highest =
                u32::from_le_bytes([d[addr + 4], d[addr + 5], d[addr + 6], d[addr + 7]]) as usize;
            let count = (highest + 2) & !1;
            let mut offsets = Vec::with_capacity(count);
            let base = addr + 8;
            for k in 0..count {
                let o = base + k * 2;
                if o + 2 > d.len() {
                    break;
                }
                offsets.push(u16::from_le_bytes([d[o], d[o + 1]]));
            }
            scripts.insert(id as u16, offsets);
        }
        let mut this = Self {
            scripts,
            bytes: d.to_vec(),
            walk_speeds: HashMap::new(),
            walk_cycles: HashMap::new(),
        };
        // Precompute speeds for every script that has a walking animation.
        let ids: Vec<u16> = this.scripts.keys().copied().collect();
        for id in ids {
            let mut cycle = this.decode_walk_cycle(id);
            // Scripts whose Walking animation only waits (for example the SCV,
            // whose flingy carries the real speed) must fall back to the
            // flingy top speed rather than freezing at zero.
            if !cycle.iter().any(|s| *s > 0) {
                cycle.clear();
            }
            let mean = if cycle.is_empty() {
                0
            } else {
                cycle.iter().sum::<i32>() / cycle.len() as i32
            };
            this.walk_speeds.insert(id, mean.max(0));
            this.walk_cycles.insert(id, cycle);
        }
        Ok(this)
    }

    pub fn script_count(&self) -> usize {
        self.scripts.len()
    }

    pub fn animations(&self, id: u16) -> Option<&[u16]> {
        self.scripts.get(&id).map(Vec::as_slice)
    }

    /// Absolute file offset of an animation, or `None` when the animation is
    /// empty (offset zero).
    pub fn animation_offset(&self, id: u16, anim: usize) -> Option<u16> {
        let offsets = self.scripts.get(&id)?;
        let &offset = offsets.get(anim)?;
        if offset == 0 {
            None
        } else {
            Some(offset)
        }
    }

    /// Walking speed in 1/256 pixels per logical tick.  Zero means the script
    /// has no walking animation (buildings and static units).
    pub fn walk_speed(&self, id: u16) -> i32 {
        self.walk_speeds.get(&id).copied().unwrap_or(0)
    }

    /// Per-tick Walking speed sequence in 1/256 pixels per tick.  The retail
    /// iscript VM executes one opcode per frame until `wait`; `move n` sets
    /// the speed to n pixels/tick and the following wait repeats it.  Replaying
    /// this cycle preserves the alternating values (for example the Zergling's
    /// 2–9 px/tick animation) rather than flattening them to a mean.
    pub fn walk_cycle(&self, id: u16) -> &[i32] {
        self.walk_cycles.get(&id).map(Vec::as_slice).unwrap_or(&[])
    }

    /// True when the animation reaches a weapon-release opcode.  Buildings
    /// such as the Missile Turret carry the weapon on the parent unit while
    /// the visible attack animation lives on the turret subunit; the parent's
    /// own animation is a plain wait loop with no event, so the rules engine
    /// must fall back to firing on the order tick.
    pub fn animation_has_attack(&self, id: u16, anim: usize) -> bool {
        self.animation_has_opcode(id, anim, &[0x1b, 0x1c, 0x25, 0x26, 0x44])
    }

    /// True when the animation reaches `castspell`.
    pub fn animation_has_cast(&self, id: u16, anim: usize) -> bool {
        self.animation_has_opcode(id, anim, &[0x27])
    }

    fn animation_has_opcode(&self, id: u16, anim: usize, wanted: &[u8]) -> bool {
        let Some(offset) = self.animation_offset(id, anim) else {
            return false;
        };
        let mut p = offset as usize;
        let mut guard = 0;
        while p < self.bytes.len() && guard < 512 {
            guard += 1;
            let op = self.bytes[p];
            if wanted.contains(&op) {
                return true;
            }
            p += 1;
            if op == 0x19 || op == 0x1c {
                let n = self.bytes.get(p).copied().unwrap_or(0) as usize;
                p += 1 + n * 2;
            } else {
                p += opcode_operand_len(op);
            }
            if matches!(op, 0x07 | 0x16 | 0x30 | 0x36) {
                break;
            }
        }
        false
    }

    /// Start an animation.  Returns `None` when the script has no such
    /// animation or the unit's iscript id is unknown.
    pub fn start_animation(&self, id: u16, anim: usize, direction: u8) -> Option<AnimState> {
        let pc = self.animation_offset(id, anim)?;
        Some(AnimState {
            iscript: id,
            anim: anim as u8,
            pc,
            wait: 0,
            frame: 0,
            direction,
            flip: false,
            call_stack: Vec::new(),
            move_speed: 0,
            fling_speed: 0,
            ended: false,
            ignore_rest: false,
            orders_blocked: false,
            repeat_attack: false,
            signal: 0,
            steps: 0,
        })
    }

    /// Execute one logical tick of an animation.  The returned events are in
    /// the order the opcodes executed them.  The VM stops at `wait`, `end`,
    /// `ignorerest`, or after a bounded number of opcodes so a malformed
    /// script cannot hang the simulation.
    pub fn tick_anim(&self, st: &mut AnimState, ctx: &mut AnimContext) -> Vec<AnimEvent> {
        let mut out = Vec::new();
        if st.ended || st.ignore_rest {
            return out;
        }
        if st.wait > 0 {
            st.wait -= 1;
            return out;
        }
        let mut guard = 0usize;
        while guard < 512 {
            guard += 1;
            let Some(opcode) = self.bytes.get(st.pc as usize).copied() else {
                st.ended = true;
                out.push(AnimEvent::End);
                break;
            };
            st.pc = st.pc.wrapping_add(1);
            st.steps = st.steps.saturating_add(1);
            let mut stop = false;
            match opcode {
                0x00 => {
                    // playfram <frame>
                    let frame = self.read_u16(st).unwrap_or(0);
                    st.frame = frame;
                    out.push(AnimEvent::Frame {
                        raw: frame,
                        direction: st.direction,
                    });
                }
                0x01 => {
                    // playframtile <frame>: the engine adds the tileset base.
                    let frame = self.read_u16(st).unwrap_or(0);
                    let tile_offset = (ctx.tileset as u16).saturating_mul(7);
                    st.frame = frame.wrapping_add(tile_offset);
                    out.push(AnimEvent::Frame {
                        raw: st.frame,
                        direction: st.direction,
                    });
                }
                0x02 => {
                    // sethorpos <x>
                    let _ = self.read_i8(st);
                }
                0x03 => {
                    // setvertpos <y>
                    let _ = self.read_i8(st);
                }
                0x04 => {
                    // setpos <x> <y>
                    let x = self.read_i8(st).unwrap_or(0);
                    let y = self.read_i8(st).unwrap_or(0);
                    out.push(AnimEvent::Overlay {
                        id: 0,
                        x,
                        y,
                        below: false,
                    });
                }
                0x05 => {
                    // wait <ticks>: the retail VM stores n-1 and returns.
                    let n = self.read_u8(st).unwrap_or(1);
                    st.wait = n.saturating_sub(1);
                    stop = true;
                }
                0x06 => {
                    // waitrand <min> <max>
                    let a = self.read_u8(st).unwrap_or(0);
                    let b = self.read_u8(st).unwrap_or(a);
                    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                    let span = (hi as i32 - lo as i32 + 1).max(1) as u32;
                    let n = lo + (ctx.next_random() % span) as u8;
                    st.wait = n.saturating_sub(1);
                    stop = true;
                }
                0x07 => {
                    // goto <offset>
                    if let Some(target) = self.read_u16(st) {
                        st.pc = target;
                    }
                }
                0x08..=0x0b | 0x0d..=0x15 | 0x38 | 0x3d | 0x42 => {
                    // Overlay/sprite/gas opcodes.  The operand layout differs
                    // per opcode; skip it and record a visual event.
                    let (id, x, y, below) = self.read_overlay_operands(opcode, st, ctx);
                    if opcode == 0x16 {
                        // (handled below; unreachable)
                    }
                    out.push(AnimEvent::Overlay { id, x, y, below });
                    if let Some((sound, len)) = self.overlay_sound(opcode) {
                        let _ = sound;
                        let _ = len;
                    }
                }
                0x0c | 0x2d | 0x3e | 0x43 => {
                    // Unused/unknown opcodes have no operands.
                }
                0x16 => {
                    // end: remove the image and finish the animation.
                    st.ended = true;
                    out.push(AnimEvent::End);
                    stop = true;
                }
                0x17 => {
                    // setflipstate <bool>
                    let v = self.read_u8(st).unwrap_or(0) != 0;
                    st.flip = v;
                    out.push(AnimEvent::Flip(v));
                }
                0x18 => {
                    // playsnd <id>
                    if let Some(id) = self.read_u16(st) {
                        out.push(AnimEvent::Sound(id));
                    }
                }
                0x19 => {
                    // playsndrand <count> <id>...
                    if let Some(count) = self.read_u8(st) {
                        let mut ids = Vec::with_capacity(count as usize);
                        for _ in 0..count {
                            if let Some(id) = self.read_u16(st) {
                                ids.push(id);
                            }
                        }
                        if !ids.is_empty() {
                            let pick = (ctx.next_random() as usize) % ids.len();
                            out.push(AnimEvent::RandomSound(ids[pick]));
                        }
                    }
                }
                0x1a => {
                    // playsndbtwn <first> <last>
                    let a = self.read_u16(st).unwrap_or(0);
                    let b = self.read_u16(st).unwrap_or(a);
                    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                    let span = (hi as u32 - lo as u32 + 1).max(1);
                    out.push(AnimEvent::RandomSound(
                        (lo as u32 + ctx.next_random() % span) as u16,
                    ));
                }
                0x1b => {
                    // domissiledmg: the current bullet applies its weapon.
                    out.push(AnimEvent::Attack {
                        weapon: None,
                        melee: false,
                        ground: false,
                    });
                }
                0x1c => {
                    // attackmelee <count> <sound>...
                    let count = self.read_u8(st).unwrap_or(0);
                    for _ in 0..count {
                        let _ = self.read_u16(st);
                    }
                    out.push(AnimEvent::Attack {
                        weapon: None,
                        melee: true,
                        ground: true,
                    });
                }
                0x1d => {
                    // followmaingraphic: copy the parent's frame/flip.  The
                    // simulation has no parent/overlay object here.
                }
                0x1e => {
                    // randcondjmp <chance> <offset>
                    let chance = self.read_u8(st).unwrap_or(0) as u32;
                    if let Some(target) = self.read_u16(st) {
                        if (ctx.next_random() & 0xff) < chance {
                            st.pc = target;
                        }
                    }
                }
                0x1f => {
                    // turnccwise <amount>
                    let amount = self.read_u8(st).unwrap_or(0) as i32;
                    st.direction = st.direction.wrapping_sub(amount as u8);
                    out.push(AnimEvent::Turn(-amount));
                }
                0x20 => {
                    // turncwise <amount>
                    let amount = self.read_u8(st).unwrap_or(0) as i32;
                    st.direction = st.direction.wrapping_add(amount as u8);
                    out.push(AnimEvent::Turn(amount));
                }
                0x21 => {
                    // turn1cwise
                    st.direction = st.direction.wrapping_add(1);
                    out.push(AnimEvent::Turn(1));
                }
                0x22 => {
                    // turnrand <amount>
                    let amount = self.read_u8(st).unwrap_or(0);
                    let turn = if ctx.next_random() & 1 != 0 {
                        -((amount as i32) / 2)
                    } else {
                        amount as i32
                    };
                    st.direction = st.direction.wrapping_add(turn as u8);
                    out.push(AnimEvent::Turn(turn));
                }
                0x23 => {
                    // setspawnframe <direction>
                    let dir = self.read_u8(st).unwrap_or(0);
                    st.direction = dir;
                }
                0x24 => {
                    // sigorder <flag>
                    let flag = self.read_u8(st).unwrap_or(0);
                    st.signal = flag;
                    out.push(AnimEvent::Signal(flag));
                }
                0x25 => {
                    // attackwith <is_ground>
                    let ground = self.read_u8(st).unwrap_or(0) != 0;
                    out.push(AnimEvent::Attack {
                        weapon: None,
                        melee: false,
                        ground,
                    });
                }
                0x26 => {
                    // attack
                    out.push(AnimEvent::Attack {
                        weapon: None,
                        melee: false,
                        ground: false,
                    });
                }
                0x27 => {
                    // castspell
                    out.push(AnimEvent::CastSpell);
                }
                0x28 => {
                    // useweapon <weapon>
                    let weapon = self.read_u8(st);
                    out.push(AnimEvent::Attack {
                        weapon,
                        melee: false,
                        ground: false,
                    });
                }
                0x29 => {
                    // move <px/tick>
                    let n = self.read_u8(st).unwrap_or(0) as i32;
                    st.move_speed = n * 256;
                    out.push(AnimEvent::Move(st.move_speed));
                }
                0x2a => {
                    // gotorepeatattk
                    st.repeat_attack = true;
                }
                0x2b | 0x2c => {
                    // engframe / engset
                    let _ = self.read_u8(st);
                }
                0x2e => {
                    st.orders_blocked = true;
                }
                0x2f => {
                    st.orders_blocked = false;
                }
                0x30 => {
                    // ignorerest: stop until the next animation is selected.
                    st.ignore_rest = true;
                    out.push(AnimEvent::IgnoreRest);
                    stop = true;
                }
                0x31 => {
                    // attkshiftproj <distance>
                    let _ = self.read_u8(st);
                }
                0x32 => {
                    // tmprmgraphicstart: hide the image.
                }
                0x33 => {
                    // tmprmgraphicend: show the image.
                }
                0x34 => {
                    // setfldirect <direction>
                    let dir = self.read_u8(st).unwrap_or(0) as i32 * 8;
                    st.direction = (dir / 8) as u8;
                    out.push(AnimEvent::SetDirection(dir));
                }
                0x35 => {
                    // call <offset>
                    let target = self.read_u16(st).unwrap_or(0);
                    st.call_stack.push(st.pc);
                    st.pc = target;
                }
                0x36 => {
                    // return
                    if let Some(target) = st.call_stack.pop() {
                        st.pc = target;
                    } else {
                        st.ended = true;
                        out.push(AnimEvent::End);
                        stop = true;
                    }
                }
                0x37 => {
                    // setflspeed <raw>
                    let raw = self.read_u16(st).unwrap_or(0) as i32;
                    st.fling_speed = raw;
                    out.push(AnimEvent::SetFlingSpeed(raw));
                }
                0x39 => {
                    // pwrupcondjmp <offset>
                    let target = self.read_u16(st).unwrap_or(0);
                    if ctx.powerup_picked {
                        st.pc = target;
                    }
                }
                0x3a => {
                    // trgtrangecondjmp <range> <offset>
                    let range = self.read_u16(st).unwrap_or(0) as i32;
                    let target = self.read_u16(st).unwrap_or(0);
                    if ctx.target_distance.is_some_and(|d| d <= range) {
                        st.pc = target;
                    }
                }
                0x3b => {
                    // trgtarccondjmp <min-angle> <max-angle> <offset>?  The
                    // shipped operand width is 6 bytes; the first two words
                    // bound the target angle and the third is the destination.
                    let lo = self.read_u16(st).unwrap_or(0) as i32;
                    let hi = self.read_u16(st).unwrap_or(0) as i32;
                    let target = self.read_u16(st).unwrap_or(0);
                    if let Some(a) = ctx.target_angle {
                        let inside = if lo <= hi {
                            a >= lo && a <= hi
                        } else {
                            a >= lo || a <= hi
                        };
                        if inside {
                            st.pc = target;
                        }
                    }
                }
                0x3c => {
                    // curdirectcondjmp <direction> <offset> (6-byte operand
                    // form in the shipped table).
                    let dir = self.read_u16(st).unwrap_or(0);
                    let target = self.read_u16(st).unwrap_or(0);
                    let _extra = self.read_u16(st);
                    if st.direction as u16 == dir {
                        st.pc = target;
                    }
                }
                0x3f => {
                    // liftoffcondjmp <offset>
                    let target = self.read_u16(st).unwrap_or(0);
                    if ctx.lifted {
                        st.pc = target;
                    }
                }
                0x40 => {
                    // warpoverlay <frame>
                    let frame = self.read_u16(st).unwrap_or(0);
                    st.frame = frame;
                    out.push(AnimEvent::Frame {
                        raw: frame,
                        direction: st.direction,
                    });
                }
                0x41 => {
                    // orderdone <flag>
                    let flag = self.read_u8(st).unwrap_or(0);
                    st.signal = flag;
                    out.push(AnimEvent::OrderDone(flag));
                }
                0x44 => {
                    // dogrddamage: like domissiledmg, but only on ground.
                    out.push(AnimEvent::Attack {
                        weapon: None,
                        melee: false,
                        ground: true,
                    });
                }
                _ => {
                    // Unknown opcode: skip its known operand length.
                    let len = opcode_operand_len(opcode);
                    for _ in 0..len {
                        let _ = self.read_u8(st);
                    }
                }
            }
            if stop {
                break;
            }
        }
        out
    }

    fn read_u8(&self, st: &mut AnimState) -> Option<u8> {
        let b = *self.bytes.get(st.pc as usize)?;
        st.pc = st.pc.wrapping_add(1);
        Some(b)
    }

    fn read_i8(&self, st: &mut AnimState) -> Option<i8> {
        self.read_u8(st).map(|b| b as i8)
    }

    fn read_u16(&self, st: &mut AnimState) -> Option<u16> {
        let lo = self.read_u8(st)? as u16;
        let hi = self.read_u8(st)? as u16;
        Some(lo | (hi << 8))
    }

    /// Skip the operands of an overlay/sprite opcode and return a visual
    /// description.
    fn read_overlay_operands(
        &self,
        opcode: u8,
        st: &mut AnimState,
        _ctx: &mut AnimContext,
    ) -> (u16, i8, i8, bool) {
        match opcode {
            0x08 | 0x09 | 0x0f | 0x10 | 0x11 | 0x13 | 0x14 | 0x42 => {
                let id = self.read_u16(st).unwrap_or(0);
                let x = self.read_i8(st).unwrap_or(0);
                let y = self.read_i8(st).unwrap_or(0);
                let below = matches!(opcode, 0x09 | 0x11 | 0x14);
                (id, x, y, below)
            }
            0x0a | 0x0b => {
                let id = self.read_u16(st).unwrap_or(0);
                (id, 0, 0, opcode == 0x0b)
            }
            0x0d | 0x0e => {
                let id = self.read_u16(st).unwrap_or(0);
                let _lo = self.read_u8(st);
                let _dir = self.read_u8(st);
                (id, 0, 0, opcode == 0x0e)
            }
            0x15 => {
                let id = self.read_u16(st).unwrap_or(0);
                let _lo = self.read_u8(st);
                (id, 0, 0, false)
            }
            0x38 => {
                let id = self.read_u8(st).unwrap_or(0) as u16;
                (id, 0, 0, false)
            }
            0x12 => {
                let id = self.read_u16(st).unwrap_or(0);
                (id, 0, 0, false)
            }
            0x3d => {
                let id = self.read_u16(st).unwrap_or(0);
                (id, 0, 0, false)
            }
            _ => (0, 0, 0, false),
        }
    }

    fn overlay_sound(&self, _opcode: u8) -> Option<(u16, usize)> {
        None
    }

    fn decode_walk_cycle(&self, id: u16) -> Vec<i32> {
        let Some(&addr) = self
            .scripts
            .get(&id)
            .and_then(|offsets| offsets.get(ANIM_WALKING))
        else {
            return Vec::new();
        };
        if addr == 0 {
            return Vec::new();
        }
        let mut p = addr as usize;
        let end = self.bytes.len();
        let mut speed = 0i32;
        let mut forced_speed: Option<i32> = None;
        let mut cycle = Vec::new();
        let mut steps = 0;
        // Upper bound: enough for every stock Walking animation plus a safety
        // margin.  A malformed custom script that never waits is truncated.
        while p + 1 < end && steps < 8192 && cycle.len() < 1024 {
            steps += 1;
            let opcode = self.bytes[p];
            p += 1;
            if opcode == 0x19 || opcode == 0x1c {
                if p < end {
                    let n = self.bytes[p] as usize;
                    p += 1 + n * 2;
                }
                continue;
            }
            let len = opcode_operand_len(opcode);
            match opcode {
                0x05 => {
                    // wait <frames>: the current speed is held.
                    if p < end {
                        let frames = self.bytes[p] as usize;
                        for _ in 0..frames {
                            cycle.push(speed);
                        }
                    }
                }
                0x29 => {
                    // move <px/tick>
                    if p < end {
                        speed = self.bytes[p] as i32 * 256;
                    }
                }
                0x37 => {
                    // setflspeed <raw>
                    if p + 2 <= end {
                        let raw = u16::from_le_bytes([self.bytes[p], self.bytes[p + 1]]) as i32;
                        if raw != 0 {
                            forced_speed = Some(raw);
                        }
                    }
                }
                0x07 => {
                    // goto: a backward jump closes the cycle.
                    if p + 2 <= end {
                        let target =
                            u16::from_le_bytes([self.bytes[p], self.bytes[p + 1]]) as usize;
                        if target <= p && !cycle.is_empty() {
                            break;
                        }
                        if target < end {
                            p = target;
                            continue;
                        }
                    }
                }
                0x16 | 0x30 => break,
                _ => {}
            }
            p += len;
        }
        if let Some(raw) = forced_speed {
            if !cycle.is_empty() {
                for s in &mut cycle {
                    if *s == 0 {
                        *s = raw;
                    }
                }
            }
        }
        cycle
    }
}

/// Operand byte length for an iscript opcode.  This is the shipped 0x00..0x44
/// instruction table; the `playsndrand` (0x19) and `attackmelee` (0x1c)
/// opcodes are variable-length and decoded separately.
pub fn opcode_operand_len(opcode: u8) -> usize {
    const LEN: [u8; 69] = [
        2, 2, 1, 1, 2, 1, 2, 2, 4, 4, 2, 2, 0, 4, 4, 4, 4, 4, 2, 4, 4, 3, 0, 1, 2, 0, 4, 0, 0, 0,
        3, 1, 1, 0, 1, 1, 1, 1, 0, 0, 1, 1, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 1, 2, 0, 2, 1, 2, 4, 6,
        6, 2, 0, 2, 2, 1, 4, 0, 0,
    ];
    LEN.get(opcode as usize).copied().unwrap_or(0) as usize
}

/// Human-readable opcode name, used by the disassembler and diagnostics.
pub fn opcode_name(opcode: u8) -> &'static str {
    const NAMES: [&str; 69] = [
        "playfram",
        "playframtile",
        "sethorpos",
        "setvertpos",
        "setpos",
        "wait",
        "waitrand",
        "goto",
        "imgol",
        "imgul",
        "imgolorig",
        "switchul",
        "__0c",
        "imgoluselo",
        "imguluselo",
        "sprol",
        "highsprol",
        "lowsprul",
        "uflunstable",
        "spruluselo",
        "sprul",
        "sproluselo",
        "end",
        "setflipstate",
        "playsnd",
        "playsndrand",
        "playsndbtwn",
        "domissiledmg",
        "attackmelee",
        "followmaingraphic",
        "randcondjmp",
        "turnccwise",
        "turncwise",
        "turn1cwise",
        "turnrand",
        "setspawnframe",
        "sigorder",
        "attackwith",
        "attack",
        "castspell",
        "useweapon",
        "move",
        "gotorepeatattk",
        "engframe",
        "engset",
        "__2d",
        "nobrkcodestart",
        "nobrkcodeend",
        "ignorerest",
        "attkshiftproj",
        "tmprmgraphicstart",
        "tmprmgraphicend",
        "setfldirect",
        "call",
        "return",
        "setflspeed",
        "creategasoverlays",
        "pwrupcondjmp",
        "trgtrangecondjmp",
        "trgtarccondjmp",
        "curdirectcondjmp",
        "imgulnextid",
        "__3e",
        "liftoffcondjmp",
        "warpoverlay",
        "orderdone",
        "grdsprol",
        "__43",
        "dogrddamage",
    ];
    NAMES.get(opcode as usize).copied().unwrap_or("unknown")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script() -> Iscript {
        Iscript::parse(embedded!("fixtures/iscript.bin")).unwrap()
    }

    #[test]
    fn parses_the_shipped_script_table() {
        let s = script();
        assert!(s.script_count() > 200);
        // Marine is image 239 -> iscript 78; Zealot is iscript 163.
        assert!(s.walk_speed(78) > 0);
        assert!(s.walk_speed(163) > 0);
        // A building (Command Center, iscript 102) has no walking animation.
        assert_eq!(s.walk_speed(102), 0);
    }

    #[test]
    fn faster_units_have_higher_walk_speeds() {
        let s = script();
        // Zergling (iscript 31) is faster than a Marine (78).
        assert!(s.walk_speed(31) > s.walk_speed(78));
    }

    #[test]
    fn walking_cycles_preserve_the_per_frame_speed_changes() {
        let s = script();
        let zergling = s.walk_cycle(31);
        assert!(!zergling.is_empty());
        let min = *zergling.iter().min().unwrap();
        let max = *zergling.iter().max().unwrap();
        assert!(max > min, "the Zergling animation alternates speeds");
        // Every value is a whole number of pixels per tick in 1/256 units.
        assert!(zergling.iter().all(|s| s % 256 == 0));
    }

    #[test]
    fn every_shipped_animation_runs_in_the_vm() {
        let s = script();
        let mut ctx = AnimContext {
            rng: 0x1234_5678,
            ..Default::default()
        };
        for id in 0..400u16 {
            let Some(offsets) = s.animations(id) else {
                continue;
            };
            for (anim, &offset) in offsets.iter().enumerate() {
                if offset == 0 {
                    continue;
                }
                let mut st = s.start_animation(id, anim, 0).unwrap();
                for _ in 0..48 {
                    let _ = s.tick_anim(&mut st, &mut ctx);
                }
                assert!(
                    st.steps <= 48 * 512,
                    "iscript {id} animation {anim} executed without a loop guard"
                );
            }
        }
    }

    #[test]
    fn vm_walk_sequence_matches_the_precomputed_cycle() {
        let s = script();
        for id in 0..400u16 {
            let cycle = s.walk_cycle(id);
            if cycle.is_empty() {
                continue;
            }
            let Some(mut st) = s.start_animation(id, ANIM_WALKING, 0) else {
                continue;
            };
            let mut ctx = AnimContext::default();
            let mut got = Vec::new();
            for _ in 0..cycle.len().saturating_mul(2).max(8) {
                for ev in s.tick_anim(&mut st, &mut ctx) {
                    if let AnimEvent::Move(v) = ev {
                        got.push(v);
                    }
                }
                if got.len() >= cycle.len() {
                    break;
                }
            }
            assert_eq!(
                &got[..cycle.len()],
                cycle,
                "iscript {id} VM walking cycle disagrees with the table"
            );
        }
    }

    #[test]
    fn every_shipped_animation_decodes_to_known_opcodes() {
        let s = script();
        for id in 0..400u16 {
            let Some(offsets) = s.animations(id) else {
                continue;
            };
            for &offset in offsets {
                if offset == 0 {
                    continue;
                }
                let mut p = offset as usize;
                let mut guard = 0;
                while p < s.bytes.len() && guard < 4096 {
                    guard += 1;
                    let op = s.bytes[p];
                    assert!(
                        (op as usize) < 69,
                        "iscript {id} has unknown opcode {op:#x} at {p:#x}"
                    );
                    p += 1;
                    if op == 0x19 || op == 0x1c {
                        let n = s.bytes.get(p).copied().unwrap_or(0) as usize;
                        p += 1 + n * 2;
                    } else {
                        p += opcode_operand_len(op);
                    }
                    // Decode one linear code block.  Jumps, returns and
                    // terminal opcodes end the block; following bytes are
                    // another block or data.
                    if matches!(op, 0x07 | 0x16 | 0x30 | 0x36) {
                        break;
                    }
                }
            }
        }
    }

    #[test]
    fn marine_attack_animation_emits_the_attack_event() {
        let s = script();
        let mut st = s
            .start_animation(78, ANIM_GND_ATTACK_INIT, 0)
            .expect("marine attack animation");
        let mut ctx = AnimContext {
            rng: 1,
            ..Default::default()
        };
        let mut saw_attack = false;
        for _ in 0..64 {
            for ev in s.tick_anim(&mut st, &mut ctx) {
                if matches!(ev, AnimEvent::Attack { .. }) {
                    saw_attack = true;
                }
            }
            if saw_attack {
                break;
            }
        }
        assert!(saw_attack, "the Gauss Rifle attack event must fire");
    }

    #[test]
    fn walking_animation_yields_the_expected_speed_sequence() {
        let s = script();
        let mut st = s.start_animation(31, ANIM_WALKING, 0).unwrap();
        let mut ctx = AnimContext::default();
        let mut speeds = Vec::new();
        for _ in 0..8 {
            for ev in s.tick_anim(&mut st, &mut ctx) {
                if let AnimEvent::Move(v) = ev {
                    speeds.push(v);
                }
            }
        }
        assert_eq!(
            speeds,
            vec![512, 2048, 2304, 1280, 1536, 1792, 512, 512],
            "the recovered Zergling walk opcodes, including the loop back to move 2"
        );
    }
}

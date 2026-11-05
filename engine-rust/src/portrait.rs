//! In-game unit portraits: which `portrait/<stem><n>.webm` clip plays.
//!
//! The HUD portrait is full-motion video.  Every portrait id has up to four
//! idle clips (`...fid00..03`) and three talking clips (`...tlk00..02`),
//! 10 fps VP9 of 1-1.6 s.  Selection rules, FACT from the 64-bit 1.23.10
//! in-game dump `sc_mem_ingame64_20261028_0814` (file offsets):
//!
//! * `arr/portdata.dat` (1320 bytes) = 110 idle and 110 talking
//!   `portdata.tbl` indices (u32, 1-based), then four u8 arrays of 110: the
//!   per-clip idle weights (every row sums to 100, e.g. 85 / 5 / 5 / 5).
//! * Per-frame update `0x32cb20`: when the playing clip stops, a talking
//!   portrait that is still talking picks the next talking clip
//!   (`0x32d180`: `(rand() >> 8) % 3`, `(c + 2) % 3` if it repeats the last
//!   one); otherwise the next idle clip (`0x32cfa0`: `r = (rand() >> 8) %
//!   101`, the first clip whose cumulative weight reaches `r`, clip 0 if
//!   none).  `rand` is the CRT stream ([`crate::render::CrtRand`]).
//! * Setting a portrait (`0x3fd300`) picks the first clip with a separate
//!   additive generator ([`PortraitRng`]) when the `UnitPortraits` option is
//!   animated, and clip 0 otherwise.
//! * A missing clip falls back to the next lower number (`portrait\%s%d.webm`
//!   tried from `c` down to 0).

use crate::render::CrtRand;

/// Portrait ids in `portdata.dat` (idle and talking each).
pub const PORTRAITS: usize = 110;
/// Playback rate of every shipped portrait clip (`r_frame_rate` 10/1).
pub const PORTRAIT_FPS: f32 = 10.0;

/// `arr/portdata.dat`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortData {
    idle: Vec<u32>,
    talk: Vec<u32>,
    /// `weights[clip][portrait]`, percent.
    weights: [Vec<u8>; 4],
}

impl PortData {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let n = PORTRAITS;
        if bytes.len() != n * 8 + n * 4 {
            return None;
        }
        let word =
            |i: usize| u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
        let idle = (0..n).map(|i| word(i * 4)).collect();
        let talk = (0..n).map(|i| word((n + i) * 4)).collect();
        let at = n * 8;
        let weights = std::array::from_fn(|c| bytes[at + c * n..at + (c + 1) * n].to_vec());
        Some(Self {
            idle,
            talk,
            weights,
        })
    }

    /// 1-based `portdata.tbl` index of the stem of a portrait's idle or
    /// talking clips.
    pub fn stem_index(&self, portrait: u16, talking: bool) -> Option<u32> {
        let v = if talking { &self.talk } else { &self.idle };
        v.get(portrait as usize).copied().filter(|&i| i > 0)
    }

    /// Idle clip weights of a portrait (percent per clip 0..=3).
    pub fn idle_weights(&self, portrait: u16) -> [u8; 4] {
        std::array::from_fn(|c| self.weights[c].get(portrait as usize).copied().unwrap_or(0))
    }
}

/// CASC path of clip `c` (`portrait/<stem><n>.webm`, lower case).
pub fn clip_path(
    data: &PortData,
    tbl: &crate::tables::StringTable,
    c: PortraitClip,
    n: u8,
) -> Option<String> {
    let idx = data.stem_index(c.portrait, c.talking)?;
    let stem = tbl.get(u16::try_from(idx.checked_sub(1)?).ok()?)?;
    Some(format!(
        "portrait/{}{n}.webm",
        stem.replace('\\', "/").to_ascii_lowercase()
    ))
}

/// Idle clip for a roll `r` in `0..=100`: the first clip whose cumulative
/// weight reaches `r` (`r <= sum`), clip 0 when the weights never do.
pub fn idle_clip_for_roll(weights: [u8; 4], r: u32) -> u8 {
    let mut sum = 0u32;
    for (c, &w) in weights.iter().enumerate() {
        sum += w as u32;
        if r <= sum {
            return c as u8;
        }
    }
    0
}

/// Talking clip for a roll in `0..3`, never the previous one.
pub fn talk_clip_for_roll(roll: u32, last: u8) -> u8 {
    let c = (roll % 3) as u8;
    if c == last { (c + 2) % 3 } else { c }
}

/// Next idle clip when one ends (`0x32cfa0`).  `rand() >> 8` only spans
/// 0..=127, so rolls 0..=26 are twice as likely as the rest (retail quirk,
/// kept).
pub fn next_idle_clip(rand: &mut CrtRand, weights: [u8; 4]) -> u8 {
    idle_clip_for_roll(weights, (rand.next() >> 8) % 101)
}

/// Next talking clip when one ends mid-speech (`0x32d180`).
pub fn next_talk_clip(rand: &mut CrtRand, last: u8) -> u8 {
    talk_clip_for_roll((rand.next() >> 8) % 3, last)
}

/// The additive generator `0x3fd300` uses for the first clip of a newly set
/// portrait: `a += b; b += 0x71823402; c += a; a >>= 1; b ^= c`, output
/// `a ^ b`, rejection-sampled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortraitRng {
    a: u32,
    b: u32,
    c: u32,
}

impl PortraitRng {
    pub fn new(seed: u32) -> Self {
        Self {
            a: seed,
            b: seed.rotate_left(16),
            c: !seed,
        }
    }

    fn step(&mut self) -> u32 {
        self.a = self.a.wrapping_add(self.b);
        self.b = self.b.wrapping_add(0x7182_3402);
        self.c = self.c.wrapping_add(self.a);
        self.a >>= 1;
        self.b ^= self.c;
        self.a ^ self.b
    }

    /// Uniform roll in `0..=100` (`& 0x7f`, rejected until `< 0x65`).
    pub fn idle_roll(&mut self) -> u32 {
        loop {
            let v = self.step() & 0x7f;
            if v < 0x65 {
                return v;
            }
        }
    }

    /// Uniform roll in `0..3` (`& 3`, rejected until `< 3`).
    pub fn talk_roll(&mut self) -> u32 {
        loop {
            let v = self.step() & 3;
            if v < 3 {
                return v;
            }
        }
    }
}

/// What the HUD portrait is playing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PortraitClip {
    pub portrait: u16,
    pub talking: bool,
    pub clip: u8,
}

/// Portrait playback state: the clip, when it started and the last talking
/// clip (so a new talking clip never repeats it).
#[derive(Clone, Debug)]
pub struct PortraitPlayer {
    pub current: Option<PortraitClip>,
    /// Seconds (caller's clock) at which the current clip started.
    pub started: f64,
    last_talk: u8,
    rng: PortraitRng,
}

impl PortraitPlayer {
    pub fn new(seed: u32) -> Self {
        Self {
            current: None,
            started: 0.0,
            last_talk: 0,
            rng: PortraitRng::new(seed),
        }
    }

    /// Show `portrait` (`None` hides it).  `animated` = the `UnitPortraits`
    /// option allows random clips; otherwise clip 0.  `talking` starts a
    /// talking clip.  Re-setting the same portrait in the same mode keeps
    /// the playing clip, as retail does while a clip is loaded.
    pub fn set(
        &mut self,
        data: &PortData,
        portrait: Option<u16>,
        talking: bool,
        animated: bool,
        now: f64,
    ) {
        let Some(portrait) = portrait else {
            self.current = None;
            return;
        };
        if let Some(cur) = self.current
            && cur.portrait == portrait
            && cur.talking == talking
        {
            return;
        }
        let clip = match (animated, talking) {
            (false, _) => 0,
            (true, false) => idle_clip_for_roll(data.idle_weights(portrait), self.rng.idle_roll()),
            (true, true) => talk_clip_for_roll(self.rng.talk_roll(), self.last_talk),
        };
        if talking {
            self.last_talk = clip;
        }
        self.current = Some(PortraitClip {
            portrait,
            talking,
            clip,
        });
        self.started = now;
    }

    /// The current clip ended at `now`: continue talking while `still_talking`,
    /// else return to (or stay on) idle, picking with the CRT stream.
    pub fn clip_ended(
        &mut self,
        data: &PortData,
        rand: &mut CrtRand,
        still_talking: bool,
        now: f64,
    ) {
        let Some(cur) = self.current else {
            return;
        };
        let (talking, clip) = if cur.talking && still_talking {
            let c = next_talk_clip(rand, self.last_talk);
            self.last_talk = c;
            (true, c)
        } else {
            (false, next_idle_clip(rand, data.idle_weights(cur.portrait)))
        };
        self.current = Some(PortraitClip {
            portrait: cur.portrait,
            talking,
            clip,
        });
        self.started = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn portdata() -> Vec<u8> {
        let mut b = Vec::new();
        for i in 0..PORTRAITS as u32 {
            b.extend((i + 1).to_le_bytes());
        }
        for i in 0..PORTRAITS as u32 {
            b.extend((i + 200).to_le_bytes());
        }
        for w in [85u8, 5, 5, 5] {
            b.extend(std::iter::repeat_n(w, PORTRAITS));
        }
        b
    }

    #[test]
    fn portdata_layout_is_two_index_tables_then_four_weight_rows() {
        let d = PortData::parse(&portdata()).expect("1320 bytes");
        assert_eq!(d.stem_index(0, false), Some(1));
        assert_eq!(d.stem_index(0, true), Some(200));
        assert_eq!(d.idle_weights(7), [85, 5, 5, 5]);
        assert!(PortData::parse(&portdata()[..1319]).is_none());
    }

    #[test]
    fn idle_roll_walks_the_cumulative_weights() {
        let w = [85, 5, 5, 5];
        assert_eq!(idle_clip_for_roll(w, 0), 0);
        assert_eq!(idle_clip_for_roll(w, 85), 0);
        assert_eq!(idle_clip_for_roll(w, 86), 1);
        assert_eq!(idle_clip_for_roll(w, 95), 2);
        assert_eq!(idle_clip_for_roll(w, 100), 3);
        // Weights that never reach the roll fall back to clip 0.
        assert_eq!(idle_clip_for_roll([10, 0, 0, 0], 50), 0);
    }

    #[test]
    fn talking_never_repeats_the_previous_clip() {
        for last in 0..3 {
            for roll in 0..3 {
                assert_ne!(talk_clip_for_roll(roll, last), last);
            }
        }
        assert_eq!(talk_clip_for_roll(1, 1), 0);
        assert_eq!(talk_clip_for_roll(2, 0), 2);
    }

    #[test]
    fn ended_idle_clips_mostly_replay_clip_zero() {
        let d = PortData::parse(&portdata()).unwrap();
        let mut rand = CrtRand::new(7);
        let mut p = PortraitPlayer::new(1);
        p.set(&d, Some(3), false, true, 0.0);
        let mut zero = 0;
        for i in 0..10_000 {
            p.clip_ended(&d, &mut rand, false, i as f64);
            let c = p.current.unwrap();
            assert!(!c.talking && c.clip < 4);
            zero += (c.clip == 0) as usize;
        }
        // `rand() >> 8` is 0..=127, so `% 101` favours rolls 0..=26: clip 0
        // (r <= 85) gets 113 of the 128 values, 88.3 %, not 85 %.
        assert!((8_600..9_050).contains(&zero), "{zero}");
    }

    #[test]
    fn talking_continues_until_the_voice_ends_then_idles() {
        let d = PortData::parse(&portdata()).unwrap();
        let mut rand = CrtRand::new(1);
        let mut p = PortraitPlayer::new(1);
        p.set(&d, Some(3), true, true, 0.0);
        let first = p.current.unwrap();
        assert!(first.talking);
        p.clip_ended(&d, &mut rand, true, 1.0);
        let second = p.current.unwrap();
        assert!(second.talking && second.clip != first.clip);
        p.clip_ended(&d, &mut rand, false, 2.0);
        assert!(!p.current.unwrap().talking);
        // Still portraits always show clip 0.
        let mut still = PortraitPlayer::new(9);
        still.set(&d, Some(3), true, false, 0.0);
        assert_eq!(still.current.unwrap().clip, 0);
        still.set(&d, None, false, false, 0.0);
        assert!(still.current.is_none());
    }

    #[test]
    fn portrait_rng_rolls_stay_in_range() {
        let mut r = PortraitRng::new(0x1234);
        for _ in 0..1000 {
            assert!(r.idle_roll() <= 100);
            assert!(r.talk_roll() < 3);
        }
    }
}

//! The mission briefing "ready room" script (`MBRF` section of a campaign
//! CHK) and its player.
//!
//! `MBRF` uses the `TRIG` record layout (2400 bytes per trigger) with its own
//! action list.  FACT from the shipped Episode I maps (2026-11-05, session
//! log 89): `terran01` runs Mission Objectives (4, `text`), Show Portrait
//! (5, unit in `extra`, slot in `group`), Wait (1, `time` ms), Transmission
//! (8, `text` + `sound`, slot in `group`, `time` with the `num`/`group2`
//! modifier), Hide Portrait (6, slot), Play WAV (2) and Text Message (3,
//! "End of Briefing." for 5000 ms).  The retail Wasteland recording
//! (`Screencast_20261105_073713`) places the second portrait at 35.5 s and
//! the closing line at 63.8 s, which is this list with Waits and
//! Transmissions holding the script for their duration.

use crate::map::Scenario;
use crate::trigger::{TriggerAction, TriggerSystem, transmission_ms};

/// Briefing action ids (`MBRF`).
pub mod action {
    pub const WAIT: u8 = 1;
    pub const PLAY_WAV: u8 = 2;
    pub const TEXT_MESSAGE: u8 = 3;
    pub const MISSION_OBJECTIVES: u8 = 4;
    pub const SHOW_PORTRAIT: u8 = 5;
    pub const HIDE_PORTRAIT: u8 = 6;
    pub const SPEAKING_PORTRAIT: u8 = 7;
    pub const TRANSMISSION: u8 = 8;
    pub const SKIP_TUTORIAL: u8 = 9;
}

/// Something the client plays when the script reaches it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadyEvent {
    /// A map WAV (`STR` id of its path).
    Wav { string: u32 },
}

/// A line on screen: `STR` id, when it started and for how long (ms).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadyLine {
    pub string: u32,
    pub start_ms: u64,
    pub ms: u32,
}

/// Ready-room state at the current script time.
#[derive(Clone, Debug, Default)]
pub struct ReadyRoom {
    actions: Vec<TriggerAction>,
    cursor: usize,
    resume_ms: u64,
    /// Unit type shown in each of the four portrait slots.
    pub portraits: [Option<u16>; 4],
    /// Slot talking and until when (script ms).
    pub speaking: Option<(usize, u64)>,
    /// Transmission text in the dialogue box.
    pub transmission: Option<ReadyLine>,
    /// Text Message ("End of Briefing.").
    pub message: Option<ReadyLine>,
    /// Mission objectives text (`STR` id).
    pub objectives: Option<u32>,
    /// Skip Tutorial button enabled (Boot Camp).
    pub skip_tutorial: bool,
}

impl ReadyRoom {
    /// The script `player` runs: the actions of every `MBRF` trigger
    /// enabled for the player, All Players (17) or the player's force
    /// (18..=21).  `None` when the map has no briefing.
    pub fn from_scenario(scenario: &Scenario, player: usize) -> Option<Self> {
        let system = TriggerSystem::parse(&scenario.briefing_triggers, &[]);
        let force = scenario.forces.get(player).copied().unwrap_or(0) as usize;
        let actions: Vec<TriggerAction> = system
            .triggers
            .iter()
            .filter(|t| {
                t.enabled.get(player).copied().unwrap_or(false)
                    || t.enabled[17]
                    || t.enabled[18 + force.min(3)]
            })
            .flat_map(|t| t.actions.iter().filter(|a| a.kind != 0).cloned())
            .collect();
        (!actions.is_empty()).then(|| Self {
            actions,
            ..Self::default()
        })
    }

    /// Whether every action has run.
    pub fn finished(&self) -> bool {
        self.cursor >= self.actions.len()
    }

    /// Run the script up to `now_ms` and return what to play.  Waits and
    /// Transmissions hold it; the other actions take no time.
    pub fn advance(&mut self, now_ms: u64) -> Vec<ReadyEvent> {
        let mut events = Vec::new();
        while self.cursor < self.actions.len() && now_ms >= self.resume_ms {
            let a = self.actions[self.cursor].clone();
            let at = self.resume_ms;
            self.cursor += 1;
            let slot = (a.group as usize).min(3);
            match a.kind {
                action::WAIT => self.resume_ms = at + a.time as u64,
                action::PLAY_WAV => events.push(ReadyEvent::Wav { string: a.sound_id }),
                action::TEXT_MESSAGE => {
                    self.message = Some(ReadyLine {
                        string: a.text_id,
                        start_ms: at,
                        ms: a.time,
                    })
                }
                action::MISSION_OBJECTIVES => self.objectives = Some(a.text_id),
                action::SHOW_PORTRAIT => self.portraits[slot] = Some(a.extra),
                action::HIDE_PORTRAIT => {
                    self.portraits[slot] = None;
                    if self.speaking.is_some_and(|(s, _)| s == slot) {
                        self.speaking = None;
                    }
                }
                action::SPEAKING_PORTRAIT => self.speaking = Some((slot, at + a.time as u64)),
                action::TRANSMISSION => {
                    let ms = transmission_ms(&a);
                    if a.sound_id != 0 {
                        events.push(ReadyEvent::Wav { string: a.sound_id });
                    }
                    self.transmission = Some(ReadyLine {
                        string: a.text_id,
                        start_ms: at,
                        ms,
                    });
                    self.speaking = Some((slot, at + ms as u64));
                    self.resume_ms = at + ms as u64;
                }
                action::SKIP_TUTORIAL => self.skip_tutorial = true,
                _ => {}
            }
        }
        if self.speaking.is_some_and(|(_, until)| now_ms >= until) {
            self.speaking = None;
        }
        // The box empties when the line ends (retail: Duke's text clears at
        // 64.90 s, his line having started at 37.2 s for 27.79 s).
        if self
            .transmission
            .is_some_and(|l| now_ms >= l.start_ms + l.ms as u64)
        {
            self.transmission = None;
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One `MBRF` record for Force 1 with `actions` as
    /// `(kind, text, wav, time, group, extra)`.
    fn script(actions: &[(u8, u32, u32, u32, u32, u16)]) -> Scenario {
        let mut r = vec![0u8; 2400];
        r[15] = 13; // the briefing condition the shipped maps use
        for (i, &(kind, text, wav, time, group, extra)) in actions.iter().enumerate() {
            let o = 320 + 32 * i;
            r[o + 4..o + 8].copy_from_slice(&text.to_le_bytes());
            r[o + 8..o + 12].copy_from_slice(&wav.to_le_bytes());
            r[o + 12..o + 16].copy_from_slice(&time.to_le_bytes());
            r[o + 16..o + 20].copy_from_slice(&group.to_le_bytes());
            r[o + 24..o + 26].copy_from_slice(&extra.to_le_bytes());
            r[o + 26] = kind;
        }
        r[2372 + 18] = 1;
        Scenario {
            briefing_triggers: r,
            ..Scenario::default()
        }
    }

    #[test]
    fn wasteland_briefing_timeline_matches_the_retail_recording() {
        // The head of `terran01`'s MBRF: objectives, Adjutant (CC portrait)
        // in slot 3, 1 s wait, 26 469 ms line, Duke in slot 0, 0.5 s wait,
        // 27 794 ms line.
        let sc = script(&[
            (4, 7, 0, 0, 0, 0),
            (5, 0, 0, 0, 3, 106),
            (1, 0, 0, 1000, 0, 0),
            (8, 8, 9, 26_469, 3, 0),
            (5, 0, 0, 0, 0, 23),
            (1, 0, 0, 500, 0, 0),
            (8, 10, 11, 27_794, 0, 0),
            (6, 0, 0, 0, 0, 0),
        ]);
        let mut room = ReadyRoom::from_scenario(&sc, 1).expect("script for force 1");
        assert!(room.advance(0).is_empty());
        assert_eq!(room.objectives, Some(7));
        assert_eq!(room.portraits[3], Some(106));
        assert_eq!(room.advance(1000), vec![ReadyEvent::Wav { string: 9 }]);
        assert_eq!(room.speaking.map(|s| s.0), Some(3), "the Adjutant talks");
        room.advance(27_000);
        assert_eq!(room.portraits[0], None, "Duke not yet");
        room.advance(27_469);
        assert_eq!(room.portraits[0], Some(23), "Duke at 27.5 s of script");
        assert_eq!(room.advance(27_969), vec![ReadyEvent::Wav { string: 11 }]);
        assert_eq!(room.transmission.map(|l| l.string), Some(10));
        assert!(!room.finished());
        room.advance(27_969 + 27_793);
        assert!(room.transmission.is_some());
        room.advance(27_969 + 27_794);
        assert_eq!(room.transmission, None, "the box empties at the line's end");
        room.advance(27_969 + 27_794);
        assert_eq!(room.portraits[0], None, "Duke leaves after his line");
        assert!(room.finished());
    }

    #[test]
    fn maps_without_a_briefing_have_no_ready_room() {
        assert!(ReadyRoom::from_scenario(&Scenario::default(), 0).is_none());
    }
}

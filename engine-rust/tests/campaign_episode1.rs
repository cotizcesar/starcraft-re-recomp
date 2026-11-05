//! Episode I (Rebel Yell) against the retail missions: the heroes the maps
//! hand to the player are rescued instead of shot, nothing ends the
//! mission early, and the opening conversation plays line after line.
//! Needs `SC_INSTALL`; skips cleanly without it.

use galaxycraft::campaign_progress::mission_key;
use galaxycraft::map::Scenario;
use galaxycraft::render::era_name;
use galaxycraft::retail::RetailInstall;
use galaxycraft::sim::{Race, World};
use galaxycraft::trigger::TriggerCue;
use std::sync::Arc;

/// Build a mission world the way the client does (slots applied).
fn mission(install: &RetailInstall, mission: u16) -> (Scenario, World, usize) {
    let (dir, key) = mission_key(mission).expect("mapdata mission");
    let raw = install
        .read(&format!("sd/campaign/{dir}/{key}/staredit/scenario.chk"))
        .expect("mission chk");
    let sc = Scenario::parse(&raw).expect("parse");
    let era = era_name(sc.era);
    let vf4 = install.read(&format!("TileSet/{era}.vf4")).expect("vf4");
    let cv5 = install.read(&format!("TileSet/{era}.cv5")).expect("cv5");
    let slots = sc.player_slots();
    let races: Vec<Race> = (0..8)
        .map(|i| {
            slots
                .get(i)
                .map(|s| s.race(Race::Terran))
                .unwrap_or(Race::Terran)
        })
        .collect();
    let data = Arc::new(install.game_data().expect("game data"));
    let mut w = World::from_scenario(
        data,
        &sc,
        &galaxycraft::map::TileData::new(&cv5, &vf4),
        &races,
    );
    for s in &slots {
        if let Some(p) = w.players.get_mut(s.slot) {
            p.defeated = !s.active;
            p.is_computer = s.computer;
        }
    }
    let human = slots.iter().position(|s| s.active && s.human).unwrap_or(0);
    (sc, w, human)
}

#[test]
fn episode_one_heroes_are_rescued_and_the_missions_keep_running() {
    let Some(install) = RetailInstall::from_env() else {
        eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
        return;
    };
    // Mapdata missions: 3 Backwater Station (Raynor's Vulture, units.dat
    // 19), 5 The Jacobs Installation (Raynor, 20).  Both maps defeat the
    // player as soon as no player commands Raynor.
    for (m, hero) in [(3u16, 19u16), (5, 20)] {
        let (_, mut w, human) = mission(&install, m);
        assert!(w.trigger_victory, "mission {m} ends through its triggers");
        let hero_id = w
            .units
            .iter()
            .find(|u| u.alive && u.type_id == hero)
            .map(|u| u.id)
            .expect("hero placed");
        for _ in 0..24 * 30 {
            w.step();
        }
        let raynor = w.units.iter().find(|u| u.id == hero_id).expect("hero");
        assert!(raynor.alive, "mission {m}: Raynor survives the opening");
        assert_eq!(raynor.owner as usize, human, "mission {m}: rescued");
        assert!(!w.players[human].defeated, "mission {m}: no early defeat");
        assert!(w
            .rescues
            .iter()
            .any(|&(id, o)| id == hero_id && o as usize == human));
    }
    // 2 Wasteland: Raynor waits to be found ("Find Raynor"); the Zerg next
    // to him do not take him.
    let (_, mut w, _) = mission(&install, 2);
    for _ in 0..24 * 30 {
        w.step();
    }
    assert!(
        w.rescues.is_empty(),
        "nobody rescues Raynor before the player"
    );
}

#[test]
fn the_trump_card_opens_with_three_lines_in_sequence() {
    let Some(install) = RetailInstall::from_env() else {
        eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
        return;
    };
    let (_, mut w, human) = mission(&install, 8);
    let mut at = Vec::new();
    let mut seen = 0;
    for tick in 0..24 * 30u32 {
        w.step();
        let t = w.triggers.as_ref().expect("triggers");
        for c in &t.cues[seen..] {
            if let TriggerCue::Text { player, ms, .. } = *c {
                if player == human && ms > 0 {
                    at.push(tick);
                }
            }
        }
        seen = t.cues.len();
    }
    assert_eq!(at.len(), 3, "Kerrigan, Mengsk, Kerrigan: {at:?}");
    assert!(
        at.windows(2).all(|p| p[1] > p[0] + 24),
        "one after another: {at:?}"
    );
}

#[test]
fn every_episode_one_mission_sets_its_objectives_at_the_start() {
    let Some(install) = RetailInstall::from_env() else {
        eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
        return;
    };
    // Boot Camp, Desperate Alliance, New Gettysburg and The Hammer Falls
    // set them from Force 1 triggers; The Jacobs Installation on "Elapsed
    // Time at most 0".  Retail shows every list from the first second.
    for m in 1..=11u16 {
        let (sc, mut w, human) = mission(&install, m);
        for _ in 0..2 {
            w.step();
        }
        let t = w.triggers.as_ref().expect("triggers");
        let text = t
            .mission_objectives
            .get(human)
            .copied()
            .flatten()
            .and_then(|s| sc.string(s))
            .unwrap_or("");
        // A bullet list ("- Find Raynor.", or "-Eliminate ..." in The Big Push).
        assert!(text.contains("\n-"), "mission {m} objectives: {text:?}");
    }
}

#[test]
fn wasteland_ready_room_follows_the_retail_recording() {
    let Some(install) = RetailInstall::from_env() else {
        eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
        return;
    };
    // Retail recording `Screencast_20261105_073713` (script time 0 at
    // 9.10 s): the Adjutant (CC portrait, slot 3) first, Duke in slot 0 at
    // ~36.5 s (27.5 s of script), the Adjutant's closing line at ~63.8 s,
    // "End of Briefing." last.
    let (sc, _, human) = mission(&install, 2);
    let mut room = galaxycraft::ready_room::ReadyRoom::from_scenario(&sc, human)
        .expect("terran01 has an MBRF briefing");
    room.advance(0);
    assert_eq!(
        room.portraits[3],
        Some(106),
        "Adjutant (Command Center portrait)"
    );
    assert!(room.objectives.is_some());
    room.advance(27_468);
    assert_eq!(room.portraits[0], None);
    room.advance(27_469);
    assert_eq!(room.portraits[0], Some(23), "Duke at 27.5 s of script");
    // His line runs 27 969..55 763 ms (retail: text gone at 64.90 s =
    // 9.10 + 55.8) and he leaves after the following 500 ms Wait (65.6 s).
    room.advance(55_763);
    assert_eq!(room.transmission, None, "text cleared at the line's end");
    assert_eq!(room.portraits[0], Some(23));
    room.advance(56_263);
    assert_eq!(room.portraits[0], None, "Duke leaves after the Wait");
    assert_eq!(room.speaking.map(|s| s.0), Some(3), "the Adjutant closes");
    room.advance(68_797);
    let end = room.message.expect("End of Briefing");
    assert!(sc
        .string(end.string)
        .unwrap_or("")
        .contains("End of Briefing"));
    assert!(room.finished());
}

#[test]
fn wasteland_rescue_conversation_follows_the_retail_timing() {
    let Some(install) = RetailInstall::from_env() else {
        eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
        return;
    };
    // Retail `Screencast_20261105_074011`: Raynor rescued at 15.7 s, his
    // line at 18.1 s (under Pause Game, text gone at 22.1 s = 4.0 s), "NEW
    // MISSION OBJECTIVE" at 24.1 s (line 3956 ms + Wait 2000 ms later).
    let (sc, mut w, human) = mission(&install, 2);
    let raynor = w
        .units
        .iter()
        .position(|u| u.alive && u.type_id == 19)
        .expect("Raynor placed");
    let (x, y) = (w.units[raynor].x / 256, w.units[raynor].y / 256);
    w.spawn_px(human, galaxycraft::ids::unit::MARINE, x + 40, y)
        .expect("marine");
    let (mut rescued, mut line, mut objective) = (None, None, None);
    let mut seen = 0;
    for tick in 0..24 * 20u64 {
        w.step();
        if rescued.is_none() && w.units[raynor].owner as usize == human {
            rescued = Some(tick);
        }
        let t = w.triggers.as_ref().expect("triggers");
        for c in &t.cues[seen..] {
            if let TriggerCue::Text {
                string, ms, paused, ..
            } = *c
            {
                // The map pads its lines with spaces.
                let text = sc.string(string).unwrap_or("").trim_start();
                if text.starts_with("RAYNOR") {
                    line = Some((tick, ms, paused));
                } else if text.starts_with("NEW MISSION OBJECTIVE") {
                    objective = Some(tick);
                }
            }
        }
        seen = t.cues.len();
    }
    let rescued = rescued.expect("rescued");
    let (at, ms, paused) = line.expect("Raynor's line");
    assert!(paused, "spoken under Pause Game");
    assert_eq!(
        galaxycraft::transmission::text_life_ms(ms, 3, paused),
        3_956,
        "4.0 s on screen"
    );
    let secs = |t: u64| t as f32 / galaxycraft::sim::TICKS_PER_SECOND as f32;
    assert!(
        (1.0..=3.4).contains(&secs(at - rescued)),
        "line {:.2} s after the rescue",
        secs(at - rescued)
    );
    let gap = secs(objective.expect("objective text") - at);
    assert!(
        (gap - 6.0).abs() <= 0.2,
        "objective {gap:.2} s after the line"
    );
}

#[test]
fn wasteland_victory_fires_within_one_trigger_cycle() {
    let Some(install) = RetailInstall::from_env() else {
        eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
        return;
    };
    // Retail `Screencast_20261105_074531`: "10 of 10 Marines" at 3.0 s, the
    // victory dialog at 3.57 s (the next 30-tick trigger cycle).  The
    // Force 1 trigger needs 10 Marines, a Barracks and Raynor.
    let (_, mut w, human) = mission(&install, 2);
    let raynor = w
        .units
        .iter()
        .position(|u| u.alive && u.type_id == 19)
        .expect("Raynor placed");
    let (x, y) = (w.units[raynor].x / 256, w.units[raynor].y / 256);
    w.spawn_px(human, galaxycraft::ids::unit::BARRACKS, x + 200, y + 200)
        .expect("barracks");
    let have = w
        .units
        .iter()
        .filter(|u| {
            u.alive && u.owner as usize == human && u.type_id == galaxycraft::ids::unit::MARINE
        })
        .count() as i32;
    for i in 0..(9 - have).max(0) {
        w.spawn_px(human, galaxycraft::ids::unit::MARINE, x + 40 + 20 * i, y)
            .expect("marine");
    }
    if have == 0 {
        // Something must be next to Raynor to rescue him.
        w.spawn_px(human, galaxycraft::ids::unit::SCV, x + 40, y)
            .expect("scv");
    }
    // Let the opening conversations (Barracks, rescue, SCV at 8 s) finish:
    // a Wait or Transmission holds the player's other triggers, as in BW.
    for _ in 0..24 * 30 {
        w.step();
    }
    assert_eq!(w.winner, None, "nine Marines are not enough");
    w.spawn_px(human, galaxycraft::ids::unit::MARINE, x + 40, y + 30)
        .expect("tenth marine");
    let tenth = w.tick;
    let mut won = None;
    for _ in 0..24 * 3 {
        w.step();
        if w.winner == Some(human as u8) {
            won = Some(w.tick);
            break;
        }
    }
    let delay = won.expect("victory") - tenth;
    assert!(
        delay <= galaxycraft::trigger::TRIGGER_INTERVAL,
        "victory {delay} ticks after the tenth Marine"
    );
}

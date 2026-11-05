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
        assert!(
            w.rescues
                .iter()
                .any(|&(id, o)| id == hero_id && o as usize == human)
        );
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
            if let TriggerCue::Text { player, ms, .. } = *c
                && player == human
                && ms > 0
            {
                at.push(tick);
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
fn every_episode_one_mission_sets_its_objectives() {
    let Some(install) = RetailInstall::from_env() else {
        eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
        return;
    };
    // Boot Camp, Desperate Alliance, New Gettysburg and The Hammer Falls
    // set them from Force 1 triggers; The Jacobs Installation on "Elapsed
    // Time at most 0".  An opening conversation holds the player's later
    // triggers (BW wait-blocking): The Hammer Falls sets its objectives
    // after its 17.2 s + 3.4 s transmissions.
    for m in 1..=11u16 {
        let (sc, mut w, human) = mission(&install, m);
        for _ in 0..24 * 25 {
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
    assert!(
        sc.string(end.string)
            .unwrap_or("")
            .contains("End of Briefing")
    );
    assert!(room.finished());
}

#[test]
fn wasteland_rescue_conversation_follows_the_retail_timing() {
    let Some(install) = RetailInstall::from_env() else {
        eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
        return;
    };
    // Retail `Screencast_20261105_074011`: Raynor rescued at 15.7 s, his
    // line at 18.1 s (text gone at 22.1 s = 4.0 s), "NEW
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
            if let TriggerCue::Text { string, ms, .. } = *c {
                // The map pads its lines with spaces.
                let text = sc.string(string).unwrap_or("").trim_start();
                if text.starts_with("RAYNOR") {
                    line = Some((tick, ms, galaxycraft::transmission::text_chars(text)));
                } else if text.starts_with("NEW MISSION OBJECTIVE") {
                    objective = Some(tick);
                }
            }
        }
        seen = t.cues.len();
    }
    let rescued = rescued.expect("rescued");
    let (at, ms, chars) = line.expect("Raynor's line");
    assert_eq!(
        galaxycraft::transmission::text_life_ms(ms, chars),
        4_000,
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

#[test]
fn every_episode_one_ready_room_runs_to_the_end_with_real_portraits() {
    let Some(install) = RetailInstall::from_env() else {
        eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
        return;
    };
    use galaxycraft::portrait::{PortData, PortraitClip, clip_path};
    let data = install.game_data().expect("game data");
    let dat = PortData::parse(&install.read("arr/portdata.dat").expect("portdata")).expect("parse");
    let tbl =
        galaxycraft::tables::StringTable::parse(&install.read("arr/portdata.tbl").expect("tbl"))
            .expect("tbl parse");
    for m in 1..=11u16 {
        let (sc, _, human) = mission(&install, m);
        let mut room = galaxycraft::ready_room::ReadyRoom::from_scenario(&sc, human)
            .unwrap_or_else(|| panic!("mission {m} has an MBRF briefing"));
        let mut units = std::collections::BTreeSet::new();
        let mut t = 0;
        while !room.finished() && t < 300_000 {
            room.advance(t);
            units.extend(room.portraits.iter().flatten().copied());
            t += 100;
        }
        assert!(room.finished(), "mission {m}: briefing ends (at {t} ms)");
        assert!(
            room.objectives.is_some(),
            "mission {m}: briefing objectives"
        );
        assert_eq!(
            room.skip_tutorial,
            m == 1,
            "mission {m}: Skip Tutorial only in Boot Camp"
        );
        for unit in units {
            for talking in [false, true] {
                let portrait = data.unit(unit).portrait;
                let path = clip_path(
                    &dat,
                    &tbl,
                    PortraitClip {
                        portrait,
                        talking,
                        clip: 0,
                    },
                    0,
                )
                .unwrap_or_else(|| panic!("mission {m}: portrait of unit {unit}"));
                assert!(install.read_by_path(&path).is_ok(), "mission {m}: {path}");
            }
        }
    }
}

#[test]
fn boot_camp_opening_follows_the_retail_recording() {
    let Some(install) = RetailInstall::from_env() else {
        eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
        return;
    };
    // Retail `Screencast_20261105_111714` (game time from the fade-in):
    // T-280 1.6 s, Roger that 9.1 s, In order 10.3 s, You can build 14.9 s,
    // "- Select the Command Center" 18.1 s.  Trigger 1's Wait holds the
    // player's other triggers, so trigger 10 ("MARINE ...") does not run.
    let (sc, mut w, human) = mission(&install, 1);
    let mut cues: Vec<(f32, String)> = Vec::new();
    let mut seen = 0;
    for tick in 0..24 * 25u64 {
        w.step();
        let t = w.triggers.as_ref().expect("triggers");
        for c in &t.cues[seen..] {
            if let TriggerCue::Text { player, string, .. } = *c
                && player == human
            {
                let text = sc.string(string).unwrap_or("").trim_start().to_string();
                cues.push((
                    tick as f32 / galaxycraft::sim::TICKS_PER_SECOND as f32,
                    text,
                ));
            }
        }
        seen = t.cues.len();
    }
    let expect = [
        ("ADVISOR", "The T-280", 1.6),
        ("SCV", "Roger that", 9.1),
        ("ADVISOR", "In order", 10.3),
        ("ADVISOR", "You can build", 14.9),
        ("- Select the Command Center", "", 18.1),
    ];
    assert!(cues.len() >= expect.len(), "{cues:?}");
    let first = cues[0].0;
    for ((at, text), (head, body, retail)) in cues.iter().zip(expect) {
        assert!(text.starts_with(head) && text.contains(body), "{text:?}");
        let gap = at - first;
        assert!(
            (gap - (retail - 1.6)).abs() <= 0.6,
            "{head} {body}: {gap:.2} s after the first vs {:.2}",
            retail - 1.6
        );
    }
    assert!(
        !cues.iter().any(|(_, t)| t.starts_with("MARINE")),
        "{cues:?}"
    );
}

#[test]
fn trigger_locations_are_the_map_pixel_rectangles() {
    let Some(install) = RetailInstall::from_env() else {
        eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
        return;
    };
    // Boot Camp's location 1 is MRGN (0,0)-(1280,2048) px; the trigger VM
    // multiplied it by 32, so "Bring 1 Marine to location 1" held at once
    // and trigger 10 ("MARINE ...") ran 18.7 s in (retail: not in the
    // first 100 s).
    let (sc, w, _) = mission(&install, 1);
    let t = w.triggers.as_ref().expect("triggers");
    let l = &sc.locations[0];
    assert_eq!(t.locations[0], (l.left, l.top, l.right, l.bottom));
    assert!(t.locations[0].2 <= sc.width as i32 * 32);
}

#[test]
fn backwater_station_ending_follows_the_retail_recording() {
    let Some(install) = RetailInstall::from_env() else {
        eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
        return;
    };
    // Retail `Screencast_20261105_113518`: after the Infested Command
    // Center falls, from the Adjutant's "Receiving incoming transmission"
    // (693.8 s): Duke +2.5 s, Raynor +19.9 s, Duke +29.0 s, Raynor +38.8 s,
    // the victory dialog +45.9 s.
    let (sc, mut w, human) = mission(&install, 3);
    for _ in 0..24 * 40 {
        w.step();
    }
    let shooter = w
        .units
        .iter()
        .find(|u| u.alive && u.owner as usize == human)
        .map(|u| u.id)
        .expect("a player unit");
    let ccs: Vec<u32> = w
        .units
        .iter()
        .filter(|u| u.alive && u.type_id == galaxycraft::ids::unit::INFESTED_COMMAND_CENTER)
        .map(|u| u.id)
        .collect();
    assert!(!ccs.is_empty(), "an Infested Command Center is placed");
    for id in ccs {
        if let Some(i) = w.units.iter().position(|u| u.id == id) {
            w.units[i].last_damage_source = shooter;
        }
        w.damage_unit(id, 10_000_000).expect("damage");
    }
    let mut lines: Vec<(u64, String)> = Vec::new();
    let mut seen = w.triggers.as_ref().expect("triggers").cues.len();
    let mut won = None;
    for _ in 0..24 * 70 {
        w.step();
        let t = w.triggers.as_ref().expect("triggers");
        for c in &t.cues[seen..] {
            if let TriggerCue::Text { player, string, .. } = *c
                && player == human
            {
                let text = sc.string(string).unwrap_or("").trim_start().to_string();
                lines.push((w.tick, text));
            }
        }
        seen = t.cues.len();
        if w.winner == Some(human as u8) {
            won = Some(w.tick);
            break;
        }
    }
    let secs = |t: u64| t as f32 / galaxycraft::sim::TICKS_PER_SECOND as f32;
    let first = lines.first().expect("a transmission").0;
    let expect = [
        ("ADVISOR", 0.0),
        ("DUKE", 2.5),
        ("RAYNOR", 19.9),
        ("DUKE", 29.0),
        ("RAYNOR", 38.8),
    ];
    assert_eq!(lines.len(), expect.len(), "{lines:?}");
    for ((at, text), (speaker, retail)) in lines.iter().zip(expect) {
        assert!(text.starts_with(speaker), "{text:?}");
        let gap = secs(at - first);
        assert!(
            (gap - retail).abs() <= 0.8,
            "{speaker}: +{gap:.2} s vs +{retail}"
        );
    }
    let victory = secs(won.expect("victory") - first);
    assert!(
        (victory - 45.9).abs() <= 1.25,
        "victory +{victory:.2} s vs +45.9"
    );
}

#[test]
fn backwater_station_opens_with_raynor_on_the_first_frame() {
    let Some(install) = RetailInstall::from_env() else {
        eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
        return;
    };
    // Retail `Screencast_20261105_113518`: "RAYNOR / Glad to see you, boys"
    // is on the first game frame (68.9 s) and leaves 4.3 s later.  Raynor
    // starts as a rescuable unit next to the player's Marines.
    let (sc, mut w, human) = mission(&install, 3);
    w.step();
    let t = w.triggers.as_ref().expect("triggers");
    let line = t.cues.iter().find_map(|c| match *c {
        TriggerCue::Text {
            player, string, ms, ..
        } if player == human => Some((string, ms)),
        _ => None,
    });
    let (string, ms) = line.expect("Raynor's line at tick 1");
    let text = sc.string(string).unwrap_or("");
    assert!(text.contains("Glad to see you"), "{text:?}");
    let life =
        galaxycraft::transmission::text_life_ms(ms, galaxycraft::transmission::text_chars(text));
    assert!(life.abs_diff(4_300) <= 200, "{life} ms on screen vs 4.3 s");
}

#[test]
fn desperate_alliance_follows_the_retail_recording() {
    let Some(install) = RetailInstall::from_env() else {
        eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
        return;
    };
    // Retail `Desperate Alliance.webm` (2026-11-05, video only).
    let (sc, mut w, human) = mission(&install, 4);
    let secs = |t: u64| t as f32 / galaxycraft::sim::TICKS_PER_SECOND as f32;
    // Ready room: six transmissions; from the first, the others start at
    // +4.8, +21.2, +52.3, +82.7, +104.5 s and the last ends at +115.1 s.
    let mut room = galaxycraft::ready_room::ReadyRoom::from_scenario(&sc, human).expect("MBRF");
    let mut starts = Vec::new();
    let mut end = 0;
    for t in (0..200_000u64).step_by(50) {
        room.advance(t);
        if let Some(l) = room.transmission {
            if starts.last() != Some(&l.start_ms) {
                starts.push(l.start_ms);
            }
            end = l.start_ms + l.ms as u64;
        }
        if room.finished() {
            break;
        }
    }
    let retail = [0.0, 4.8, 21.2, 52.3, 82.7, 104.5];
    assert_eq!(starts.len(), retail.len(), "{starts:?}");
    for (s, r) in starts.iter().zip(retail) {
        let got = (*s - starts[0]) as f32 / 1000.0;
        assert!((got - r).abs() <= 0.5, "room line at +{got:.2} s vs +{r}");
    }
    let last = (end - starts[0]) as f32 / 1000.0;
    // Line ends read 0.3-0.5 s late on the recording (the box fades), so
    // the end gets 0.6 s.
    assert!((last - 115.1).abs() <= 0.6, "room ends +{last:.2} s");
    // In game: VULTURE then MARINE 22.3 s later (both scripted at start);
    // the countdown starts at 30:00 and shows 29:56 at +2.75 s.
    let mut seen = 0;
    let mut lines: Vec<(u64, String)> = Vec::new();
    let take = |w: &World, seen: &mut usize, lines: &mut Vec<(u64, String)>| {
        let t = w.triggers.as_ref().expect("triggers");
        for c in &t.cues[*seen..] {
            if let TriggerCue::Text { player, string, .. } = *c
                && player == human
            {
                lines.push((
                    w.tick,
                    sc.string(string).unwrap_or("").trim_start().to_string(),
                ));
            }
        }
        *seen = t.cues.len();
    };
    for _ in 0..24 * 30 {
        w.step();
        take(&w, &mut seen, &mut lines);
        if w.tick == 65 {
            assert_eq!(
                w.triggers.as_ref().expect("triggers").countdown_timer,
                1796,
                "29:56"
            );
        }
    }
    let vulture = lines
        .iter()
        .find(|l| l.1.starts_with("VULTURE"))
        .expect("VULTURE")
        .0;
    let marine = lines
        .iter()
        .find(|l| l.1.starts_with("MARINE"))
        .expect("MARINE")
        .0;
    let gap = secs(marine - vulture);
    assert!((gap - 22.3).abs() <= 0.6, "MARINE {gap:.2} s after VULTURE");
    // The end: 60 s left -> "One minute"; at 0 -> "Sons of Korhal"
    // (retail +1.0 s after 0:00), the pilot +5.2 s later, the victory
    // dialog +5.2 s after the pilot.
    // Fast-forward the countdown through the 10- and 5-minute warnings so
    // they are spent before the last minute, as in a full game.
    for left in [605, 305] {
        w.triggers.as_mut().expect("triggers").countdown_timer = left;
        for _ in 0..24 * 10 {
            w.step();
        }
    }
    take(&w, &mut seen, &mut lines);
    w.triggers.as_mut().expect("triggers").countdown_timer = 64;
    lines.clear();
    let mut zero = None;
    let mut won = None;
    for _ in 0..24 * 90 {
        w.step();
        take(&w, &mut seen, &mut lines);
        if zero.is_none() && w.triggers.as_ref().expect("triggers").countdown_timer == 0 {
            zero = Some(w.tick);
        }
        if w.winner == Some(human as u8) {
            won = Some(w.tick);
            break;
        }
    }
    let at = |head: &str| lines.iter().find(|l| l.1.contains(head)).map(|l| l.0);
    let zero = zero.expect("countdown reaches 0");
    let one = at("One minute").expect("One minute");
    assert!(
        secs(zero - one) > 37.0,
        "One minute at 1:00 left (retail 1297.6 s, about 38.4 s before 0:00)"
    );
    let sons = at("Sons of Korhal").expect("Sons of Korhal");
    assert!(
        secs(sons - zero) <= 1.3,
        "Sons of Korhal {:.2} s after 0:00",
        secs(sons - zero)
    );
    let pilot = at("DROPSHIP PILOT").expect("pilot");
    assert!(
        (secs(pilot - sons) - 5.2).abs() <= 0.6,
        "pilot +{:.2} s",
        secs(pilot - sons)
    );
    let won = won.expect("victory");
    assert!(
        (secs(won - pilot) - 5.2).abs() <= 0.8,
        "victory +{:.2} s",
        secs(won - pilot)
    );
}

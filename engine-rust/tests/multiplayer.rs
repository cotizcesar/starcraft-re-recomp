//! Multiplayer game rules: lobby setup applied to the world, teams
//! (alliance, shared vision, team victory, AI targets) and the score sheet.

use std::sync::Arc;

use galaxycraft::data::GameData;
use galaxycraft::ids::unit as uid;
use galaxycraft::map::{Scenario, Terrain, TileData};
use galaxycraft::multiplayer::{
    GameSetup, GameType, ScoreTab, SetupOccupant, SetupSlot, bar_fraction, score_lines, score_table,
};
use galaxycraft::sim::{Order, Race, World};

fn data() -> Arc<GameData> {
    Arc::new(GameData::bundled().expect("bundled tables"))
}

fn boxer(races: &[Race], occupied: &[bool]) -> World {
    let raw = std::fs::read("assets/maps/Boxer.chk").expect("bundled Boxer");
    let vf4 = std::fs::read("assets/TileSet/platform.vf4").expect("bundled vf4");
    let cv5 = std::fs::read("assets/TileSet/platform.cv5").expect("bundled cv5");
    let sc = Scenario::parse(&raw).expect("parse");
    World::from_scenario_occupied(
        data(),
        &sc,
        &TileData::new(&cv5, &vf4),
        races,
        Some(occupied),
    )
}

#[test]
fn a_melee_start_scores_like_the_retail_score_screen() {
    // Retail 21:14:04: LinKa (Protoss, never mined) reads Units 200,
    // Structures 400, Resources 50, Total 650; Zerg starts read Structures
    // 300 (the Hatchery).
    let mut w = boxer(&[Race::Protoss, Race::Zerg], &[true, true]);
    w.step();
    let lines = score_lines(&w, &[0, 1]);
    let p = lines.iter().find(|l| l.player == 0).expect("protoss row");
    assert_eq!((p.values, p.total), ([200, 400, 50], 650));
    let z = lines.iter().find(|l| l.player == 1).expect("zerg row");
    assert_eq!(z.values[1], 300);
    assert_eq!(z.total, z.values.iter().sum::<i32>());
    // Credited once: more ticks do not add the starting units again.
    for _ in 0..24 {
        w.step();
    }
    let again = score_lines(&w, &[0]);
    assert_eq!(again[0].values[..2], [200, 400]);
}

#[test]
fn open_and_closed_slots_stay_empty() {
    let w = boxer(&[Race::Terran, Race::Terran], &[true, false]);
    assert!(w.units.iter().any(|u| u.alive && u.owner == 0));
    assert!(!w.units.iter().any(|u| u.alive && u.owner == 1));
    assert!(w.players[1].defeated);
}

fn setup(teams: &[u8]) -> GameSetup {
    GameSetup {
        game_type: GameType::TopVsBottom,
        slots: teams
            .iter()
            .map(|&team| SetupSlot {
                occupant: SetupOccupant::Computer,
                race: Race::Terran,
                team,
                color: 2,
            })
            .collect(),
    }
}

#[test]
fn allies_hold_fire_and_enemies_do_not() {
    let mut w = World::new(data(), Terrain::open(64, 64), 3);
    setup(&[0, 0, 1]).apply(&mut w);
    let a = w.spawn(0, uid::MARINE, 200, 200).unwrap();
    let b = w.spawn(1, uid::MARINE, 230, 200).unwrap();
    let e = w.spawn(2, uid::MARINE, 600, 600).unwrap();
    let (hp_a, hp_b) = (w.units[a].hp, w.units[b].hp);
    for _ in 0..96 {
        w.step();
    }
    assert_eq!(
        (w.units[a].hp, w.units[b].hp),
        (hp_a, hp_b),
        "allies never shoot"
    );
    // Move the enemy next to them: now there is a fight.
    let id = w.units[e].id;
    w.issue(id, Order::Move { pos: (260, 200) }).unwrap();
    for _ in 0..240 {
        w.step();
    }
    assert!(
        w.units[e].hp < 40 * 256 || !w.units[e].alive,
        "the enemy is engaged"
    );
}

#[test]
fn a_team_wins_together_and_never_targets_itself() {
    let mut w = World::new(data(), Terrain::open(64, 64), 3);
    setup(&[0, 0, 1]).apply(&mut w);
    let _ = w.spawn(0, uid::COMMAND_CENTER, 300, 300).unwrap();
    let _ = w.spawn(1, uid::COMMAND_CENTER, 900, 300).unwrap();
    let enemy_hall = w.spawn(2, uid::COMMAND_CENTER, 900, 900).unwrap();
    // An AI of player 1 is pointed at the enemy, not at its ally 0.
    assert_eq!(w.enemy_town_hall(1, 0), Some((900, 900)));
    assert_eq!(w.enemy_town_hall(2, 0), Some((300, 300)));
    w.step();
    assert_eq!(w.winner, None, "two sides still stand");
    w.players[2].defeated = true;
    w.units[enemy_hall].alive = false;
    w.step();
    assert!(w.has_won(0) && w.has_won(1), "the whole team wins");
    assert!(!w.has_won(2));
}

#[test]
fn teammates_share_vision() {
    let mut w = World::new(data(), Terrain::open(64, 64), 3);
    w.enable_fog();
    setup(&[0, 0, 1]).apply(&mut w);
    let _ = w.spawn(0, uid::MARINE, 100, 100).unwrap();
    let _ = w.spawn(1, uid::MARINE, 1500, 1500).unwrap();
    let _ = w.spawn(2, uid::MARINE, 100, 1500).unwrap();
    w.step();
    assert!(w.is_visible_to(0, 1500, 1500), "sees through its ally");
    assert!(!w.is_visible_to(0, 100, 1500), "not through the enemy");
    assert!(!w.is_visible_to(2, 1500, 1500));
}

#[test]
fn the_other_score_tabs_count_what_their_headers_say() {
    // Protoss start: a Nexus and four Probes produced; nothing killed.
    let mut w = boxer(&[Race::Protoss, Race::Zerg], &[true, true]);
    w.step();
    let units = score_table(&w, &[0], ScoreTab::Units);
    assert_eq!(units[0].values, [4, 0, 0], "Produced / Killed / Lost");
    let structures = score_table(&w, &[0], ScoreTab::Structures);
    assert_eq!(
        structures[0].values,
        [1, 0, 0],
        "Constructed / Razed / Lost"
    );
    let res = score_table(&w, &[0], ScoreTab::Resources);
    assert_eq!(res[0].values, [50, 0, 0], "Minerals / Gas / Spent");
    // Every tab keeps the Overview total and order.
    assert_eq!(units[0].total, 650);
    // Three Marines of player 0 against one of player 1, in open ground
    // away from the bases (a Drone would flee): a real kill.
    let victim = w.spawn(1, uid::MARINE, 1000, 1000).unwrap();
    let target = w.units[victim].id;
    for dx in [-60, -40, -20] {
        let m = w.spawn(0, uid::MARINE, 1000 + dx, 1000).unwrap();
        let id = w.units[m].id;
        w.issue(id, Order::AttackUnit { target }).unwrap();
    }
    for _ in 0..480 {
        w.step();
    }
    assert!(!w.units[victim].alive, "the marine died");
    let killed = score_table(&w, &[0], ScoreTab::Units)[0].values[1];
    let lost = score_table(&w, &[1], ScoreTab::Units)[0].values[2];
    assert!(killed >= 1 && killed == lost, "killed {killed} lost {lost}");
    assert!(w.players[0].scores[4] > 0, "kill score credited");
    assert_eq!(bar_fraction(400, 550), 400.0 / 550.0);
    assert_eq!(bar_fraction(5, 0), 0.0);
}

#[test]
fn an_empty_slots_melee_defeat_trigger_does_not_defeat_player_0() {
    // Melee maps carry "All Players: Current Player commands at most 0
    // buildings -> Defeat".  The empty slot must not run it, and Defeat
    // never applies to anyone but the player running the trigger.
    let mut w = boxer(&[Race::Protoss, Race::Zerg], &[true, false]);
    assert!(w.triggers.is_some(), "Boxer carries the melee triggers");
    for _ in 0..90 {
        w.step();
    }
    assert!(!w.players[0].defeated, "player 0 keeps playing");
    assert!(
        w.units
            .iter()
            .any(|u| u.alive && u.owner == 0 && u.type_id == uid::NEXUS)
    );
    // Resources stay on the map (they are bookkept under player 0).
    assert!(
        w.units
            .iter()
            .any(|u| u.alive && galaxycraft::sim::is_resource_type(u.type_id))
    );
}

#[test]
fn a_defeat_trigger_ends_the_game_only_for_the_player_running_it() {
    let mut w = boxer(&[Race::Terran, Race::Zerg], &[true, true]);
    for _ in 0..60 {
        w.step();
    }
    assert!(!w.players[0].defeated && !w.players[1].defeated);
    // Player 1 loses every building: its own Defeat trigger fires, and
    // player 0's "Non Allied Victory Players command 0 buildings" wins.
    let doomed: Vec<usize> = (0..w.units.len())
        .filter(|&i| w.units[i].owner == 1 && w.units[i].alive)
        .collect();
    for i in doomed {
        w.units[i].alive = false;
    }
    for _ in 0..60 {
        w.step();
    }
    // The game ends for player 1 (the sim stops at the winner); player 0
    // must not have been defeated by player 1's Defeat trigger.
    assert!(
        !w.players[0].defeated,
        "player 0 was not defeated in its place"
    );
    assert!(w.has_won(0) && !w.has_won(1));
}

#[test]
fn lobby_computers_run_identically_on_every_peer_and_after_a_resync() {
    use galaxycraft::ai::SimpleAi;
    use galaxycraft::net::{Lockstep, apply_resync};
    use galaxycraft::replay::{RecAct, state_hash};

    // Slot 0 is a human, slot 1 a computer on the other team.
    let setup = GameSetup {
        game_type: GameType::TopVsBottom,
        slots: vec![
            SetupSlot {
                occupant: SetupOccupant::Human { peer: 0 },
                race: Race::Terran,
                team: 0,
                color: 14,
            },
            SetupSlot {
                occupant: SetupOccupant::Computer,
                race: Race::Zerg,
                team: 1,
                color: 2,
            },
        ],
    };
    let build = || {
        let mut w = boxer(&setup.races(), &setup.occupied());
        setup.apply(&mut w);
        let mut lock = Lockstep::new(0, 1, 4);
        lock.computers = vec![SimpleAi::new(1)];
        (w, lock)
    };
    let scv = |w: &World| {
        w.units
            .iter()
            .find(|u| u.owner == 0 && u.type_id == uid::SCV)
            .map(|u| u.id)
            .unwrap()
    };
    // Two peers commit the same bundles: one human command, then idle turns.
    let run = || {
        let (mut w, mut lock) = build();
        let order = RecAct::Issue {
            unit: scv(&w),
            order: Order::Move { pos: (300, 300) },
        };
        lock.commit(&mut w, &[vec![order]]);
        for _ in 0..300 {
            lock.commit(&mut w, &[vec![]]);
        }
        (w, lock)
    };
    let (a, lock_a) = run();
    let (b, _) = run();
    assert_eq!(state_hash(&a), state_hash(&b), "peers agree");
    assert!(
        a.units.iter().filter(|u| u.alive && u.owner == 1).count() > 9,
        "the computer played (it trained units)"
    );
    // A rejoining peer rebuilds from the log with fresh computers.
    let (mut c, mut lock_c) = build();
    apply_resync(&mut c, &mut lock_c, lock_a.turn, &lock_a.recording());
    assert_eq!(state_hash(&c), state_hash(&a), "resync reproduces the game");
}

#[test]
fn lobby_colours_reach_the_game_and_its_drawing() {
    use galaxycraft::multiplayer::{
        COLOR_CHOICES, DEFAULT_COLORS, Lobby, Occupant, RaceChoice, color_rgb, color_score_group,
        score_group,
    };
    use galaxycraft::render::{PLAYER_COLORS, player_rgb};

    let mut l = Lobby::new_host("LinKa", "Boxer", 2, GameType::Melee, (1, 1));
    l.slots[0].race = RaceChoice::Protoss;
    l.set_occupant(1, Occupant::Computer);
    // Slot 1 picks Red, slot 0's default: Red is no longer offered to slot 0
    // and slot 0 (Random) falls to the first free colour.
    let red = COLOR_CHOICES.iter().position(|c| *c == "Red").unwrap();
    let green = COLOR_CHOICES.iter().position(|c| *c == "Green").unwrap();
    assert_eq!(DEFAULT_COLORS[0], red);
    assert!(l.free_colors(1).contains(&red));
    l.slots[1].color = red;
    assert!(
        !l.free_colors(0).contains(&red),
        "a taken colour is not offered"
    );
    assert!(l.free_colors(0).contains(&0), "Random is always offered");
    let resolved = l.resolved_colors();
    assert_eq!(resolved[1], red);
    assert_ne!(resolved[0], red);
    l.slots[0].color = green;
    let setup = l.resolve(1);
    assert_eq!(
        setup
            .slots
            .iter()
            .map(|s| s.color as usize)
            .collect::<Vec<_>>(),
        [green, red]
    );
    assert_eq!(GameSetup::decode(&setup.encode()), Some(setup.clone()));
    let mut w = boxer(&setup.races(), &setup.occupied());
    // Before the setup: classic slot colours.
    assert_eq!(player_rgb(&w, 0), PLAYER_COLORS[0]);
    setup.apply(&mut w);
    assert_eq!(
        player_rgb(&w, 0),
        color_rgb(green),
        "units drawn in the lobby colour"
    );
    assert_eq!(player_rgb(&w, 1), color_rgb(red));
    let dots = w.minimap_units(0, false);
    let enemy = dots.iter().find(|d| d.owner == 1 && !d.resource).unwrap();
    assert_eq!(enemy.color, color_rgb(red), "minimap blips too");
    // The eight default colours keep the classic look and their score bars.
    for (slot, &c) in DEFAULT_COLORS.iter().enumerate() {
        assert_eq!(color_rgb(c), PLAYER_COLORS[slot]);
        assert_eq!(
            color_score_group(c),
            slot,
            "score bar group of {}",
            COLOR_CHOICES[c]
        );
    }
    assert_eq!(color_score_group(green), 8);
    // Extra colours fall to the nearest group (Lime -> a yellow-green one).
    let lime = COLOR_CHOICES.iter().position(|c| *c == "Lime").unwrap();
    assert!([7, 8, 9].contains(&score_group(color_rgb(lime))));
    // A setup without a colour (0 = unresolved Random) is malformed.
    let mut bad = setup.encode();
    bad[6] = 0;
    assert_eq!(GameSetup::decode(&bad), None);
}

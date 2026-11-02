//! Menu flow coverage for the 44 `docs/original_screenshots/*.png` captures.
//!
//! The retail UI is: main menu (8 entries), registry, game type, episode
//! (I-VI), mission, saved games, replays, create lobby, options (5 tabs),
//! hotkeys (3 races), collections, credits, in-game game menu (7 entries),
//! mission objectives and tips, plus the two title intros.  This test pins
//! the state machine behind `sc_view` so a future change cannot silently drop
//! a screen the screenshots require.

use galaxycraft::menu::{
    episode_number, GameMenuItem, HotkeyRace, LobbyRace, MenuScreen, OptionsTab,
};

#[test]
fn all_retail_menu_screens_exist() {
    let screens = [
        MenuScreen::Main,
        MenuScreen::Registry,
        MenuScreen::GameType,
        MenuScreen::Episode,
        MenuScreen::Mission { episode: 1 },
        MenuScreen::SavedGames,
        MenuScreen::Replays,
        MenuScreen::Lobby { map: None },
        MenuScreen::Options {
            tab: OptionsTab::Speed,
        },
        MenuScreen::Hotkeys {
            race: HotkeyRace::Terran,
        },
        MenuScreen::Collections,
        MenuScreen::Credits,
        MenuScreen::Intro { expansion: false },
    ];
    assert_eq!(screens.len(), 13);
    // Every screen is distinct.
    for (i, a) in screens.iter().enumerate() {
        for (j, b) in screens.iter().enumerate() {
            if i != j {
                assert_ne!(a, b, "screens {i} and {j} collide");
            }
        }
    }
}

#[test]
fn options_covers_all_five_retail_tabs() {
    let tabs = OptionsTab::all();
    assert_eq!(tabs.len(), 5);
    let names: Vec<_> = tabs.iter().map(|t| t.name()).collect();
    for want in ["Speed", "Sound", "Video", "Game", "Network"] {
        assert!(names.contains(&want), "{names:?} missing {want}");
    }
}

#[test]
fn hotkeys_covers_all_three_races() {
    let races = HotkeyRace::all();
    assert_eq!(races.len(), 3);
    assert!(races.contains(&HotkeyRace::Terran));
    assert!(races.contains(&HotkeyRace::Protoss));
    assert!(races.contains(&HotkeyRace::Zerg));
}

#[test]
fn lobby_covers_the_retail_slot_races() {
    let races = LobbyRace::all();
    assert!(races.contains(&LobbyRace::Random));
    assert!(races.contains(&LobbyRace::Computer));
}

#[test]
fn game_menu_covers_the_f10_dialog() {
    assert_eq!(GameMenuItem::all().len(), 7);
}

#[test]
fn episodes_cover_all_six_retail_campaigns() {
    // Screenshots show I Terran, II Zerg, III Protoss, IV Protoss (BW),
    // V Terran (BW), VI Zerg (BW).
    let got: Vec<_> = [
        "terran",
        "zerg",
        "protoss",
        "expprotoss",
        "expterran",
        "expzerg",
    ]
    .iter()
    .map(|k| episode_number(k).unwrap())
    .collect();
    assert_eq!(got, vec![1, 2, 3, 4, 5, 6]);
}

#[test]
fn sc_view_supports_every_menu_shot_screen() {
    let src = include_str!("../src/bin/sc_view.rs");
    for shot in [
        "registry",
        "gametype",
        "gametype-hover-expansion",
        "episode",
        "episode-hover-zerg",
        "episode-hover-terran",
        "episode-hover-protoss",
        "episode-expansion-hover-zerg",
        "episode-expansion-hover-terran",
        "episode-expansion-hover-protoss",
        "mission",
        "lobby",
        "options",
        "hotkeys",
        "collections",
        "credits",
        "gamemenu",
        "objectives",
        "tips",
    ] {
        assert!(
            src.contains(&format!("\"{shot}\"")),
            "sc_view has no --shot-screen {shot}"
        );
    }
    // Both intros are reachable.
    assert!(src.contains("starintr.webm"));
    assert!(src.contains("starxintr.webm"));
}

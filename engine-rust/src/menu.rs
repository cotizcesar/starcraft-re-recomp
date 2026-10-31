//! Retail menu flow: main menu, registry, episode/mission select, lobby,
//! options, hotkeys, collections, credits and the in-game F10 submenus.
//!
//! The 44 `docs/original_screenshots/*.png` captures define the flow: the
//! animated main menu (`Single Player`, `Multiplayer`, `Options`,
//! `Collections`, `Hotkeys`, `Show Credits`, `View Intro`, `Exit`), the
//! `Registry` profile picker, the `Select Game Type` (Original vs Expansion)
//! dialog, the episode picker (Terran I / Zerg II / Protoss III plus Brood
//! War IV Protoss / V Terran / VI Zerg with `Load Saved`, `Play Custom`,
//! `Load Replay`), the mission list (`Cinematic: Wasteland Patrol`, ...),
//! the `Saved Games` / `Replays` browsers, the `Create` lobby (map list,
//! minimap preview, `Game Type` Free For All / Melee / Use Map Settings,
//! per-slot race dropdowns), the five-tab `Options` dialog
//! (Speed/Sound/Video/Game/Network), the per-race `Hotkeys` grids, the
//! `Collections` skin gallery, the scrolling `Credits`, and the in-game
//! `Game Menu` (Save/Load/Options/Help/Mission Objectives/End Mission) with
//! the `Mission Objectives` and `StarCraft Tips` dialogs.
//!
//! This module is the state machine behind `sc_view`'s menu.  It carries no
//! retail bytes; the art comes from the materialised `glue/` cache.  The
//! cosmetic Carbot/anniversary skins stay out of scope (see `EVIDENCE.md`).

/// Top-level menu screens reachable without a loaded map.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum MenuScreen {
    #[default]
    Main,
    Registry,
    GameType,
    Episode,
    Mission {
        episode: u8,
    },
    SavedGames,
    Replays,
    /// LAN lobby: discovery, hosting and joining.
    Lan,
    Lobby {
        map: Option<usize>,
    },
    Options {
        tab: OptionsTab,
    },
    Hotkeys {
        race: HotkeyRace,
    },
    Collections,
    Credits,
    Intro {
        expansion: bool,
    },
}

/// Where the `Select Game Type` dialog continues.  Retail opens the same
/// dialog from Single Player and Multiplayer; only the follow-up screen
/// differs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameTypeFollowup {
    Registry,
    MultiplayerLobby,
}

/// `Select Game Type` follow-up for the entry that opened the dialog.
pub const fn gametype_followup(multiplayer: bool) -> GameTypeFollowup {
    if multiplayer {
        GameTypeFollowup::MultiplayerLobby
    } else {
        GameTypeFollowup::Registry
    }
}

/// The five retail `Options` tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionsTab {
    Speed,
    Sound,
    Video,
    Game,
    Network,
}

impl OptionsTab {
    pub fn all() -> [Self; 5] {
        [
            Self::Speed,
            Self::Sound,
            Self::Video,
            Self::Game,
            Self::Network,
        ]
    }
    pub fn name(&self) -> &'static str {
        match self {
            Self::Speed => "Speed",
            Self::Sound => "Sound",
            Self::Video => "Video",
            Self::Game => "Game",
            Self::Network => "Network",
        }
    }
}

/// Hotkey viewer race tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotkeyRace {
    Terran,
    Protoss,
    Zerg,
}

impl HotkeyRace {
    pub fn all() -> [Self; 3] {
        [Self::Terran, Self::Protoss, Self::Zerg]
    }
    pub fn name(&self) -> &'static str {
        match self {
            Self::Terran => "Terran",
            Self::Protoss => "Protoss",
            Self::Zerg => "Zerg",
        }
    }
}

/// Functional gameplay options.  Game speed scales the 24 Hz tick; volumes
/// scale the mixer; `fog` toggles `World::enable_fog`; subtitles gate the
/// transmission captions.
#[derive(Clone, Debug, PartialEq)]
pub struct GameOptions {
    pub game_speed: f32,
    pub music_volume: f32,
    pub sfx_volume: f32,
    pub fog: bool,
    pub subtitles: bool,
    pub tips_at_startup: bool,
}

impl Default for GameOptions {
    fn default() -> Self {
        Self {
            game_speed: 1.0,
            music_volume: 0.8,
            sfx_volume: 0.8,
            fog: true,
            subtitles: true,
            tips_at_startup: false,
        }
    }
}

/// Episode numbers as retail presents them (screenshots + campaign order).
/// Original I Terran, II Zerg, III Protoss; Brood War IV Protoss, V Terran,
/// VI Zerg.
pub fn episode_number(episode_key: &str) -> Option<u8> {
    match episode_key {
        "terran" => Some(1),
        "zerg" => Some(2),
        "protoss" => Some(3),
        "expprotoss" => Some(4),
        "expterran" => Some(5),
        "expzerg" => Some(6),
        _ => None,
    }
}

pub fn episode_race(episode: u8) -> &'static str {
    match episode {
        1 | 5 => "Terran",
        2 | 6 => "Zerg",
        _ => "Protoss",
    }
}

/// First episode index opened by the `Select Game Type` popup: Original
/// starts at I (index 0), Expansion at IV (index 3).
pub fn episode_start(expansion: bool) -> usize {
    if expansion {
        3
    } else {
        0
    }
}

/// In-game F10 `Game Menu` entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameMenuItem {
    SaveGame,
    LoadGame,
    Options,
    Help,
    MissionObjectives,
    EndMission,
    ReturnToGame,
}

impl GameMenuItem {
    pub fn all() -> [Self; 7] {
        [
            Self::SaveGame,
            Self::LoadGame,
            Self::Options,
            Self::Help,
            Self::MissionObjectives,
            Self::EndMission,
            Self::ReturnToGame,
        ]
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::SaveGame => "Save Game",
            Self::LoadGame => "Load Game",
            Self::Options => "Options",
            Self::Help => "Help",
            Self::MissionObjectives => "Mission Objectives",
            Self::EndMission => "End Mission",
            Self::ReturnToGame => "Return to Game (Esc)",
        }
    }
}

/// Lobby slot race choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LobbyRace {
    Random,
    Terran,
    Zerg,
    Protoss,
    Computer,
}

impl LobbyRace {
    pub fn all() -> [Self; 5] {
        [
            Self::Random,
            Self::Terran,
            Self::Zerg,
            Self::Protoss,
            Self::Computer,
        ]
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::Random => "Random",
            Self::Terran => "Terran",
            Self::Zerg => "Zerg",
            Self::Protoss => "Protoss",
            Self::Computer => "Computer",
        }
    }
}

/// Clickable rectangles of the eight main-menu entries, in window pixels.
///
/// These are the shipped `glumain.ui.json` HOTBOX rectangles (640x480), not
/// estimates: Single Player `[0,30,320,150]` inside FLCBTN 21, Multiplayer
/// `[78,190,240,160]`, the five footer entries `[0,345..445,160,20]` and the
/// Exit hot area `[425,320,180,136]` inside FLCBTN 2.  Pure geometry so both
/// the drawing and the hit-test use one source of truth.
pub fn main_entry_rects(scale: f32, ox: i32, oy: i32) -> [(usize, (i32, i32, i32, i32)); 8] {
    let scale = scale.max(0.5);
    let hot = |x: i32, y: i32, w: i32, h: i32| -> (i32, i32, i32, i32) {
        (
            ox + (x as f32 * scale).round() as i32,
            oy + (y as f32 * scale).round() as i32,
            (w as f32 * scale).round() as i32,
            (h as f32 * scale).round() as i32,
        )
    };
    [
        (0, hot(0, 30, 320, 150)),
        (1, hot(78, 190, 240, 160)),
        (2, hot(0, 345, 160, 20)),
        (3, hot(0, 370, 160, 20)),
        (4, hot(0, 395, 160, 20)),
        (5, hot(0, 420, 160, 20)),
        (6, hot(0, 445, 160, 20)),
        (7, hot(425, 320, 180, 136)),
    ]
}

/// Label alignment of the retail main-menu entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelAlign {
    Left,
    Center,
}

/// Where each main-menu label is drawn, in 640x480 UI coordinates, measured
/// from the retail reference (3440x1440 capture, 3x UI scale).
///
/// `width` is the ink width measured on the reference; the client distributes
/// the difference between the face's natural advance width and this value as
/// tracking, so the labels reproduce the shipped letter spacing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MainLabel {
    pub idx: usize,
    /// Left edge for [`LabelAlign::Left`], centre for
    /// [`LabelAlign::Center`], in 640x480 UI pixels.
    pub x: f32,
    /// Top of the capital letters.
    pub y: f32,
    pub align: LabelAlign,
    /// Full reference ink width in UI pixels.  The first glyph is the
    /// highlighted hotkey letter (a wide Eurostile Extended cap); the rest
    /// uses the regular face until the whole label lights up on hover.
    pub width: f32,
}

/// One `flc.json` rectangle: clip name plus (x, y, w, h) in 640x480 UI pixels.
pub type FlcRect = (&'static str, (f32, f32, f32, f32));

/// Video rectangles from the shipped `rez/flc.json`, relative to each
/// FLCBTN's 640x480 origin.
pub const FLC_RECTS: [FlcRect; 6] = [
    ("single", (-20.0, 0.0, 425.0, 175.0)),
    ("singleon", (-40.0, -10.0, 425.0, 200.0)),
    ("multi", (-10.0, -5.0, 282.0, 270.0)),
    ("multion", (-52.0, -2.0, 330.0, 220.0)),
    ("exit", (-2.0, -4.0, 204.0, 182.0)),
    ("exiton", (5.0, 0.0, 176.0, 160.0)),
];

pub const MAIN_LABELS: [MainLabel; 8] = [
    MainLabel {
        idx: 0,
        x: 149.67,
        y: 116.33,
        align: LabelAlign::Left,
        width: 119.67,
    },
    MainLabel {
        idx: 1,
        x: 77.33,
        y: 195.33,
        align: LabelAlign::Left,
        width: 104.34,
    },
    MainLabel {
        idx: 2,
        x: 79.0,
        y: 347.33,
        align: LabelAlign::Center,
        width: 75.0,
    },
    MainLabel {
        idx: 3,
        x: 79.5,
        y: 374.33,
        align: LabelAlign::Center,
        width: 104.0,
    },
    MainLabel {
        idx: 4,
        x: 79.5,
        y: 397.33,
        align: LabelAlign::Center,
        width: 78.0,
    },
    MainLabel {
        idx: 5,
        x: 79.17,
        y: 424.33,
        align: LabelAlign::Center,
        width: 128.67,
    },
    MainLabel {
        idx: 6,
        x: 79.33,
        y: 449.33,
        align: LabelAlign::Center,
        width: 93.67,
    },
    MainLabel {
        idx: 7,
        x: 509.5,
        y: 325.33,
        align: LabelAlign::Center,
        width: 34.34,
    },
];

/// The entry whose rectangle contains the point, if any.
pub fn main_entry_at(scale: f32, ox: i32, oy: i32, x: i32, y: i32) -> Option<usize> {
    // The shipped hot areas overlap by five pixels (Multiplayer's hotbox is
    // 160 px tall and the footer column starts at y=345).  The footer
    // FLCBTNs come later in the dialog, so they are on top and win the hit
    // test, matching the retail z-order.
    main_entry_rects(scale, ox, oy)
        .iter()
        .rev()
        .find(|(_, r)| x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3)
        .map(|(i, _)| *i)
}

pub fn main_items() -> [&'static str; 8] {
    [
        "Single Player",
        "Multiplayer",
        "Options",
        "Collections",
        "Hotkeys",
        "Show Credits",
        "View Intro",
        "Exit",
    ]
}

/// `Select Game Type` popup geometry, in the 640x480 UI space.
///
/// The shipped `rez/glugamemode.ui.json` dialog is 360x200 and the retail
/// client centres popups in the screen: `(640-360)/2 = (480-200)/2 = 140`.
/// The three clickable controls are the two wordmark buttons (ids 6 and 7)
/// and `Cancel` (id 3).  Keeping the values here lets drawing and hit-testing
/// share one source of truth, exactly like [`main_entry_rects`].
pub const GAME_TYPE_DIALOG: crate::ui::UiRect = crate::ui::UiRect::new(140, 140, 360, 200);
pub const GAME_TYPE_BUTTONS: [(usize, crate::ui::UiRect); 3] = [
    // id 6, `gluGameMode-1` [26,38,190,56] inside the dialog.
    (0, crate::ui::UiRect::new(166, 178, 190, 56)),
    // id 7, `gluGameMode-3` [134,114,190,56].
    (1, crate::ui::UiRect::new(274, 254, 190, 56)),
    // id 3, `gluGameMode-4` [8,160,120,29] (`CTRL_ESCAPE`).
    (2, crate::ui::UiRect::new(148, 300, 120, 29)),
];

/// Clickable rectangles of the `Select Game Type` popup, in window pixels.
pub fn gametype_entry_rects(scale: f32, ox: i32, oy: i32) -> [(usize, (i32, i32, i32, i32)); 3] {
    GAME_TYPE_BUTTONS.map(|(idx, r)| {
        let s = r.scaled(scale.max(0.5), ox, oy);
        (idx, (s.x, s.y, s.w, s.h))
    })
}

/// Entry under a point in the popup: 0 Original, 1 Expansion, 2 Cancel.
pub fn gametype_entry_at(scale: f32, ox: i32, oy: i32, x: i32, y: i32) -> Option<usize> {
    gametype_entry_rects(scale, ox, oy)
        .iter()
        .rev()
        .find(|(_, r)| x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3)
        .map(|(i, _)| *i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_menu_entry_has_a_clickable_rect() {
        // 3440x1440 window: scale 3, UI space centred (ox = 760).
        let rects = main_entry_rects(3.0, 760, 0);
        for (i, label) in main_items().iter().enumerate() {
            let (idx, r) = rects
                .iter()
                .find(|(idx, _)| *idx == i)
                .copied()
                .expect("entry present");
            assert_eq!(idx, i, "{label} keeps its index");
            assert!(r.2 > 0 && r.3 > 0, "{label} rect is not empty");
        }
        assert_eq!(rects.len(), 8);
    }

    #[test]
    fn clicking_the_centre_of_a_label_returns_that_entry() {
        let (scale, ox, oy) = (3.0f32, 760, 0);
        for (idx, r) in main_entry_rects(scale, ox, oy).iter() {
            let (cx, cy) = (r.0 + r.2 / 2, r.1 + r.3 / 2);
            assert_eq!(
                main_entry_at(scale, ox, oy, cx, cy),
                Some(*idx),
                "centre of entry {idx} must hit entry {idx} (rect {r:?})"
            );
        }
        // Outside every entry.
        assert_eq!(main_entry_at(scale, ox, oy, 5, 5), None);
    }

    #[test]
    fn labels_sit_inside_their_shipped_hot_areas() {
        let (scale, ox, oy) = (3.0f32, 320, 0);
        let rects = main_entry_rects(scale, ox, oy);
        for label in MAIN_LABELS {
            let (_, r) = rects
                .iter()
                .find(|(idx, _)| *idx == label.idx)
                .expect("entry");
            let x = ox + (label.x * scale) as i32;
            let y = oy + (label.y * scale) as i32;
            assert!(
                r.0 - 16 <= x && x <= r.0 + r.2 + 16,
                "label {} starts outside its hot area ({x} vs {r:?})",
                label.idx
            );
            assert!(
                r.1 - 16 <= y && y <= r.1 + r.3 + 16,
                "label {} is outside its hot area ({y} vs {r:?})",
                label.idx
            );
            assert!(
                label.width > 20.0,
                "label {} has a measured width",
                label.idx
            );
        }
    }

    #[test]
    fn gametype_opens_the_chosen_campaign_set() {
        assert_eq!(episode_start(false), 0, "Original -> Episode I");
        assert_eq!(episode_start(true), 3, "Expansion -> Episode IV");
        assert_eq!(episode_race((episode_start(true) + 1) as u8), "Protoss");
    }

    #[test]
    fn gametype_rects_match_the_shipped_ui_json() {
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures/ui/glugamemode.ui.json"),
        )
        .expect("glugamemode fixture");
        let dialog = crate::ui::UiDialog::parse(text.as_bytes()).expect("valid dialog");
        let centered = crate::ui::UiRect::new(
            (640 - dialog.rect.w) / 2,
            (480 - dialog.rect.h) / 2,
            dialog.rect.w,
            dialog.rect.h,
        );
        assert_eq!(
            (centered.x, centered.y, centered.w, centered.h),
            (
                GAME_TYPE_DIALOG.x,
                GAME_TYPE_DIALOG.y,
                GAME_TYPE_DIALOG.w,
                GAME_TYPE_DIALOG.h
            ),
            "popup is centred in the 640x480 UI space"
        );
        for (idx, want) in GAME_TYPE_BUTTONS {
            let id = [6, 7, 3][idx];
            let c = dialog.control(id).expect("control id");
            let abs = c
                .abs
                .offset(-dialog.rect.x, -dialog.rect.y)
                .offset(GAME_TYPE_DIALOG.x, GAME_TYPE_DIALOG.y);
            assert_eq!(
                (abs.x, abs.y, abs.w, abs.h),
                (want.x, want.y, want.w, want.h),
                "control {id} rectangle"
            );
        }
    }

    #[test]
    fn every_gametype_button_is_clickable_at_its_centre() {
        let (scale, ox, oy) = (3.0f32, 320, 0);
        for (idx, r) in gametype_entry_rects(scale, ox, oy) {
            let (cx, cy) = (r.0 + r.2 / 2, r.1 + r.3 / 2);
            assert_eq!(
                gametype_entry_at(scale, ox, oy, cx, cy),
                Some(idx),
                "centre of game-type entry {idx} must hit itself"
            );
        }
        assert_eq!(gametype_entry_at(scale, ox, oy, 10, 10), None);
    }

    #[test]
    fn flc_rects_match_the_shipped_flc_json() {
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/ui/flc.json"),
        )
        .expect("flc.json fixture");
        let value: serde_json::Value = serde_json::from_str(&text).expect("valid json");
        let flc = value.get("flc").and_then(|v| v.as_object()).expect("flc");
        for (name, expect) in FLC_RECTS {
            let key = format!("glue\\\\mainmenu\\\\{name}.webm");
            let rect = flc
                .get(&key)
                .or_else(|| {
                    flc.iter()
                        .find(|(k, _)| k.to_ascii_lowercase().ends_with(&format!("{name}.webm")))
                        .map(|(_, v)| v)
                })
                .unwrap_or_else(|| panic!("{name} in flc.json"));
            let got: Vec<f32> = rect
                .as_array()
                .expect("rect")
                .iter()
                .map(|v| v.as_f64().expect("number") as f32)
                .collect();
            assert_eq!(
                (got[0], got[1], got[2], got[3]),
                expect,
                "{name} rectangle must match the shipped flc.json"
            );
        }
    }

    #[test]
    fn the_reference_label_widths_are_the_measured_ones() {
        // Layout widths (bright hotkey cap + Extended rest), tuned until
        // the rendered ink matches the captures within 1 px: Single 356,
        // Multiplayer 308, Options 219, Collections 305, Hotkeys 227,
        // Show Credits 379, View Intro 279, Exit 99 screen px.
        assert_eq!(
            MAIN_LABELS.map(|l| l.width.round() as i32),
            [120, 104, 75, 104, 78, 129, 94, 34]
        );
    }

    #[test]
    fn footer_entries_do_not_overlap() {
        let rects = main_entry_rects(3.0, 760, 0);
        for (i, a) in rects.iter().filter(|(i, _)| (2..=6).contains(i)) {
            for (j, b) in rects.iter().filter(|(j, _)| (2..=6).contains(j)) {
                if i >= j {
                    continue;
                }
                let overlap_x = (a.0 + a.2).min(b.0 + b.2) - a.0.max(b.0);
                let overlap_y = (a.1 + a.3).min(b.1 + b.3) - a.1.max(b.1);
                assert!(
                    overlap_x <= 0 || overlap_y <= 0,
                    "entries {i} and {j} overlap: {a:?} vs {b:?}"
                );
            }
        }
    }

    #[test]
    fn episodes_follow_the_retail_numbers() {
        assert_eq!(episode_number("terran"), Some(1));
        assert_eq!(episode_number("zerg"), Some(2));
        assert_eq!(episode_number("protoss"), Some(3));
        assert_eq!(episode_number("expprotoss"), Some(4));
        assert_eq!(episode_number("expterran"), Some(5));
        assert_eq!(episode_number("expzerg"), Some(6));
        assert_eq!(episode_race(1), "Terran");
        assert_eq!(episode_race(4), "Protoss");
        assert_eq!(episode_race(6), "Zerg");
    }

    #[test]
    fn main_menu_lists_all_retail_entries() {
        let items = main_items();
        assert_eq!(items.len(), 8);
        assert!(items.contains(&"Single Player"));
        assert!(items.contains(&"View Intro"));
        assert!(items.contains(&"Exit"));
    }

    #[test]
    fn options_default_to_retail_speed_and_volumes() {
        let o = GameOptions::default();
        assert!((o.game_speed - 1.0).abs() < 1e-6);
        assert!(o.fog && o.subtitles && !o.tips_at_startup);
    }

    #[test]
    fn game_menu_covers_the_f10_dialog() {
        let labels: Vec<_> = GameMenuItem::all().iter().map(|i| i.label()).collect();
        assert!(labels.contains(&"Save Game"));
        assert!(labels.contains(&"Mission Objectives"));
        assert!(labels.contains(&"Return to Game (Esc)"));
    }

    #[test]
    fn multiplayer_opens_gametype_before_the_lobby() {
        assert_eq!(
            gametype_followup(false),
            GameTypeFollowup::Registry,
            "Single Player -> Select Game Type -> registry"
        );
        assert_eq!(
            gametype_followup(true),
            GameTypeFollowup::MultiplayerLobby,
            "Multiplayer -> Select Game Type -> connection/lobby"
        );
    }
}

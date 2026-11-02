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

/// One episode-select FLC button, transcribed from the shipped
/// `glucmpgn.ui.json` / `gluexpcmpgn.ui.json` dialogs (640x480 UI space) and
/// their `rez/flc.json` clip rectangles (relative to the button origin).
///
/// `button` is the FLCBTN control rect; `portrait` the race clip; `on` the
/// mouse-over variant; `disk` the spinning hologram loop.  `idle` is the
/// `FLC_FADED` opacity the retail client keeps the portrait at until the
/// slot is hovered (measured 0.238-0.264 on captures 130744/130750).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CampaignSlot {
    /// FLCBTN control rect (x, y, w, h) in dialog space.
    pub button: (f32, f32, f32, f32),
    /// Video family stem (`terr`, `zerg`, `prot`, `xterr`, ...).
    pub art: &'static str,
    pub portrait: (f32, f32, f32, f32),
    pub on: (f32, f32, f32, f32),
    pub disk: (f32, f32, f32, f32),
    pub race: &'static str,
    pub episode: &'static str,
    /// Centre of the two labels in dialog space (measured ink centres on the
    /// 2560x1440 references, converted back to 640x480).
    pub label: (f32, f32),
    /// Measured ink widths of the race and episode labels (dialog pixels).
    pub label_width: (f32, f32),
}

/// Club portrait opacity before hover (the `FLC_FADED` state).
pub const CAMPAIGN_PORTRAIT_IDLE_ALPHA: f32 = 0.25;

/// Original-campaign slots: Zerg (left), Terran (centre), Protoss (right),
/// exactly as `glucmpgn.ui.json` places ids 8, 7 and 6.
pub const CAMPAIGN_SLOTS: [CampaignSlot; 3] = [
    CampaignSlot {
        button: (5.0, 41.0, 220.0, 330.0),
        art: "zerg",
        portrait: (10.0, 64.0, 196.0, 220.0),
        on: (10.0, -26.0, 196.0, 220.0),
        disk: (28.0, 237.0, 148.0, 60.0),
        race: "Zerg",
        episode: "Episode II",
        label: (113.33, 335.33),
        label_width: (44.67, 84.67),
    },
    CampaignSlot {
        button: (162.0, 4.0, 356.0, 278.0),
        art: "terr",
        portrait: (40.0, 36.0, 232.0, 208.0),
        on: (42.0, 0.0, 232.0, 208.0),
        disk: (78.0, 188.0, 148.0, 60.0),
        race: "Terran",
        episode: "Episode I",
        label: (318.33, 245.33),
        label_width: (62.33, 80.67),
    },
    CampaignSlot {
        button: (391.0, 7.0, 238.0, 364.0),
        art: "prot",
        portrait: (40.0, 102.0, 208.0, 212.0),
        on: (16.0, 6.0, 180.0, 212.0),
        disk: (71.0, 270.0, 148.0, 60.0),
        race: "Protoss",
        episode: "Episode III",
        label: (540.0, 328.33),
        label_width: (77.0, 89.0),
    },
];

/// Brood War slots from `gluexpcmpgn.ui.json`: the shipped dialog swaps the
/// screens — Terran V on the left (id 7), Protoss IV in the centre (id 6) and
/// Zerg VI on the right (id 8).  The measured rings on captures 130832/130840
/// place the disks at (643,922), (1258,723) and (1930,919) exactly as this
/// table predicts.
pub const CAMPAIGN_X_SLOTS: [CampaignSlot; 3] = [
    CampaignSlot {
        button: (5.0, 41.0, 240.0, 330.0),
        art: "xterr",
        portrait: (15.0, 27.0, 180.0, 252.0),
        on: (26.0, 26.0, 180.0, 252.0),
        disk: (28.0, 237.0, 148.0, 60.0),
        race: "Terran",
        episode: "Episode V",
        label: (123.17, 335.33),
        label_width: (62.67, 90.67),
    },
    CampaignSlot {
        button: (220.0, 4.0, 356.0, 297.0),
        art: "xprot",
        portrait: (8.0, 10.0, 180.0, 240.0),
        on: (40.0, 10.0, 180.0, 240.0),
        disk: (20.0, 208.0, 148.0, 60.0),
        race: "Protoss",
        episode: "Episode IV",
        label: (327.33, 260.33),
        label_width: (77.0, 94.67),
    },
    CampaignSlot {
        button: (391.0, 7.0, 238.0, 364.0),
        art: "xzerg",
        portrait: (30.0, 30.0, 204.0, 280.0),
        on: (16.0, 20.0, 240.0, 280.0),
        disk: (71.0, 270.0, 148.0, 60.0),
        race: "Zerg",
        episode: "Episode VI",
        label: (539.17, 328.33),
        label_width: (44.67, 93.67),
    },
];

/// The four bottom command buttons of the episode screen (`glucmpgn` ids 5,
/// 10, 11 and 9): label, dialog rect and the measured ink width in dialog
/// pixels (320/349/334/185 screen px on capture 130744).  Retail centres the
/// text in the control (`FB_VTEXTCENTER | FB_HTEXTCENTER`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CampaignButton {
    pub label: &'static str,
    pub rect: (f32, f32, f32, f32),
    /// Measured ink width in dialog pixels (320/349/334/185 screen px on
    /// capture 130744).
    pub ink_width: f32,
}

pub const CAMPAIGN_BUTTONS: [CampaignButton; 4] = [
    CampaignButton {
        label: "Load Saved",
        rect: (243.0, 368.0, 189.0, 20.0),
        ink_width: 106.7,
    },
    CampaignButton {
        label: "Play Custom",
        rect: (226.0, 404.0, 206.0, 20.0),
        ink_width: 116.3,
    },
    CampaignButton {
        label: "Load Replay",
        rect: (238.0, 443.0, 194.0, 20.0),
        ink_width: 111.3,
    },
    CampaignButton {
        label: "Cancel",
        rect: (470.0, 404.0, 158.0, 20.0),
        ink_width: 61.7,
    },
];

/// Dialog geometry of the campaign screen: a 640x480 dialog centred in the
/// 16:9 content box and scaled with its height (at 2560x1440 that is x=320,
/// scale 3).  The measured ring centres (641/1262/1929 on capture 130744)
/// match `x0 + 3 * disk_centre` to 1-2 px.
pub fn campaign_scale(content_width: i32, content_height: i32) -> (f32, f32) {
    let scale = content_height.max(1) as f32 / 480.0;
    let ox = ((content_width as f32 - 640.0 * scale) / 2.0).round();
    (scale, ox)
}

/// Screen rectangle of a dialog-space rect at the given content box.
pub fn campaign_rect(
    rect: (f32, f32, f32, f32),
    scale: f32,
    ox: f32,
    cx: i32,
    cy: i32,
) -> (i32, i32, i32, i32) {
    (
        cx + (ox + rect.0 * scale).round() as i32,
        cy + (rect.1 * scale).round() as i32,
        (rect.2 * scale).round() as i32,
        (rect.3 * scale).round() as i32,
    )
}

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

/// Escape / back destination of each menu screen in the retail flow.
/// `None` only for the main menu (Escape there quits the client).
pub fn back_destination(screen: &MenuScreen) -> Option<MenuScreen> {
    match screen {
        MenuScreen::Main => None,
        MenuScreen::Registry => Some(MenuScreen::Main),
        MenuScreen::GameType => Some(MenuScreen::Main),
        MenuScreen::Episode => Some(MenuScreen::GameType),
        MenuScreen::Mission { .. } => Some(MenuScreen::Episode),
        MenuScreen::SavedGames | MenuScreen::Replays => Some(MenuScreen::Episode),
        MenuScreen::Lan => Some(MenuScreen::GameType),
        MenuScreen::Lobby { .. } => Some(MenuScreen::Main),
        MenuScreen::Options { .. } => Some(MenuScreen::Main),
        MenuScreen::Hotkeys { .. } => Some(MenuScreen::Main),
        MenuScreen::Collections | MenuScreen::Credits => Some(MenuScreen::Main),
        MenuScreen::Intro { .. } => Some(MenuScreen::Main),
    }
}

/// Sound played when a screen transition starts: the shipped
/// `sound/glue/swishin.wav` (the retail "screen in" cue).
pub const TRANSITION_SOUND: &str = "ui_swish";
/// Sound played when a control is pressed: `sound/glue/mousedown2.wav`.
pub const PRESS_SOUND: &str = "ui_click";
/// Sound played when the pointer enters a control: `sound/glue/mouseover.wav`.
pub const HOVER_SOUND: &str = "ui_move";

/// Measured control rectangles in 2560x1440 content-box pixels, shared by the
/// drawing code and the hit-testing code so they cannot diverge.
pub type HitRect = (f32, f32, f32, f32);

/// `Select starting mission` modal: Ok and Cancel bays of `phist.dds`.
pub const MISSION_BUTTONS: [HitRect; 2] =
    [(910.0, 972.0, 355.0, 86.0), (1300.0, 972.0, 355.0, 86.0)];
/// Mission list rows (8 lines, 57 px pitch, first ink top at 478).
pub const MISSION_ROWS: HitRect = (748.0, 470.0, 1062.0, 8.0 * 57.0);

/// Create/Replays/Lobby shell: Ok and Cancel bays of `pok`/`pcancel`.
pub const CREATE_BUTTONS: [HitRect; 2] =
    [(1637.0, 1143.0, 463.0, 65.0), (1753.0, 1255.0, 423.0, 66.0)];
/// Create/Replays list rows (6 rows at y 406 + 57i).
pub const CREATE_ROWS: HitRect = (504.0, 398.0, 886.0, 6.0 * 57.0);

/// Options tab stack (five 93 px cells from y 345) and the Restore/Ok pair.
pub const OPTIONS_TABS: [HitRect; 5] = [
    (521.0, 345.0, 327.0, 93.0),
    (521.0, 438.0, 327.0, 93.0),
    (521.0, 531.0, 327.0, 93.0),
    (521.0, 624.0, 327.0, 93.0),
    (521.0, 717.0, 327.0, 93.0),
];
pub const OPTIONS_BUTTONS: [HitRect; 2] =
    [(513.0, 1212.0, 387.0, 84.0), (1672.0, 1212.0, 378.0, 84.0)];

/// Collections grid (11 cells, 353x224; the clipped 4th row keeps 34 px) and
/// the Ok button.  Cell order matches the reference: console x3, unit x2,
/// announcers x6.
pub const COLLECTION_CELLS: [HitRect; 11] = [
    (411.0, 274.0, 353.0, 224.0),
    (787.0, 274.0, 353.0, 224.0),
    (1161.0, 274.0, 353.0, 224.0),
    (411.0, 595.0, 353.0, 224.0),
    (787.0, 595.0, 353.0, 224.0),
    (411.0, 916.0, 353.0, 224.0),
    (787.0, 916.0, 353.0, 224.0),
    (1161.0, 916.0, 353.0, 224.0),
    (411.0, 1158.0, 353.0, 34.0),
    (787.0, 1158.0, 353.0, 34.0),
    (1161.0, 1158.0, 353.0, 34.0),
];
/// Collections grid content, in `COLLECTION_CELLS` order: thumbnail file in
/// `webui/dist/lib/images/` plus the title shown in the right-hand preview.
pub const COLLECTION_ITEMS: [(&str, &str); 11] = [
    ("default_console_thumbnail.jpg", "Default"),
    ("krc_silver_console_thumbnail.png", "Silver"),
    ("krc_gold_console_thumbnail.png", "Gold"),
    ("skins_default.jpg", "Default"),
    ("skins_carbot.png", "Carbot"),
    ("default_announcer.png", "Default"),
    ("um_announcer.png", "Um"),
    ("jeon_announcer.png", "Jeon"),
    ("kim_announcer.png", "Kim"),
    ("umjeonkim_announcer.png", "UmJeonKim"),
    ("heyjini_announcer.png", "HeyJini"),
];
pub const COLLECTION_BUTTONS: [HitRect; 2] =
    [(1837.0, 1245.0, 360.0, 66.0), (1688.0, 421.0, 410.0, 57.0)];

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
    fn campaign_slots_match_the_shipped_dialogs_and_flc_json() {
        // Control ids that carry each slot in the shipped dialogs.
        let original_ids = [8i32, 7, 6];
        let expansion_ids = [7i32, 6, 8];
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/ui/glucmpgn.ui.json"),
        )
        .expect("glucmpgn fixture");
        let dialog = crate::ui::UiDialog::parse(text.as_bytes()).expect("valid dialog");
        for (slot, id) in CAMPAIGN_SLOTS.iter().zip(original_ids) {
            let c = dialog.control(id).expect("control id");
            assert_eq!(
                (
                    c.abs.x as f32,
                    c.abs.y as f32,
                    c.abs.w as f32,
                    c.abs.h as f32
                ),
                slot.button,
                "glucmpgn control {id} must match the slot button"
            );
        }
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures/ui/gluexpcmpgn.ui.json"),
        )
        .expect("gluexpcmpgn fixture");
        let dialog = crate::ui::UiDialog::parse(text.as_bytes()).expect("valid dialog");
        for (slot, id) in CAMPAIGN_X_SLOTS.iter().zip(expansion_ids) {
            let c = dialog.control(id).expect("control id");
            assert_eq!(
                (
                    c.abs.x as f32,
                    c.abs.y as f32,
                    c.abs.w as f32,
                    c.abs.h as f32
                ),
                slot.button,
                "gluexpcmpgn control {id} must match the slot button"
            );
        }
        // Clip rectangles are the shipped `rez/flc.json` ones.
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/ui/flc.json"),
        )
        .expect("flc.json fixture");
        let value: serde_json::Value = serde_json::from_str(&text).expect("valid json");
        let flc = value.get("flc").and_then(|v| v.as_object()).expect("flc");
        let lookup = |stem: &str| -> (f32, f32, f32, f32) {
            let (_, rect) = flc
                .iter()
                .find(|(k, _)| k.to_ascii_lowercase().ends_with(&format!("\\{stem}.webm")))
                .unwrap_or_else(|| panic!("{stem}.webm in flc.json"));
            let a: Vec<f32> = rect
                .as_array()
                .expect("rect")
                .iter()
                .map(|v| v.as_f64().expect("number") as f32)
                .collect();
            (a[0], a[1], a[2], a[3])
        };
        for slot in CAMPAIGN_SLOTS.iter().chain(CAMPAIGN_X_SLOTS.iter()) {
            let prefix = slot.art;
            let disk = format!("disk{prefix}");
            let on = format!("{prefix}on");
            assert_eq!(lookup(prefix), slot.portrait, "{} portrait", slot.art);
            assert_eq!(lookup(&on), slot.on, "{} mouse-over", slot.art);
            assert_eq!(lookup(&disk), slot.disk, "{} disk", slot.art);
        }
    }

    #[test]
    fn campaign_layout_reproduces_the_measured_ring_centres() {
        // Capture 130744 at 2560x1440: disk centres measured at
        // (641,922), (1262,663) and (1929,918) (purple ring clusters).
        let (scale, ox) = campaign_scale(2560, 1440);
        assert_eq!((scale, ox), (3.0, 320.0));
        let want = [(641, 922), (1262, 663), (1929, 918)];
        for (slot, want) in CAMPAIGN_SLOTS.iter().zip(want) {
            let disk = (
                slot.button.0 + slot.disk.0,
                slot.button.1 + slot.disk.1,
                slot.disk.2,
                slot.disk.3,
            );
            let (x, y, w, h) = campaign_rect(disk, scale, ox, 0, 0);
            let (cx, cy) = (x + w / 2, y + h / 2);
            assert!(
                (cx - want.0).abs() <= 2 && (cy - want.1).abs() <= 3,
                "{} disk centre ({cx},{cy}) vs measured ({},{})",
                slot.art,
                want.0,
                want.1
            );
        }
        // The command text centres on the shipped control rects (measured
        // ink centres on 130744: 1330, 1304, 1325 and 1964).
        let want_buttons = [1330, 1304, 1325, 1964];
        for (button, want) in CAMPAIGN_BUTTONS.iter().zip(want_buttons) {
            let (x, _, w, _) = campaign_rect(button.rect, scale, ox, 0, 0);
            assert!(
                (x + w / 2 - want).abs() <= 4,
                "button centre {} vs measured {want}",
                x + w / 2
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

    #[test]
    fn every_sub_screen_has_a_back_destination() {
        let screens = [
            MenuScreen::Registry,
            MenuScreen::GameType,
            MenuScreen::Episode,
            MenuScreen::Mission { episode: 1 },
            MenuScreen::SavedGames,
            MenuScreen::Replays,
            MenuScreen::Lan,
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
        for screen in screens {
            let back = back_destination(&screen).expect("back destination");
            assert_ne!(back, screen, "back must leave {screen:?}");
        }
        assert_eq!(back_destination(&MenuScreen::Main), None);
        assert_eq!(
            back_destination(&MenuScreen::Mission { episode: 1 }),
            Some(MenuScreen::Episode)
        );
        assert_eq!(
            back_destination(&MenuScreen::Episode),
            Some(MenuScreen::GameType)
        );
    }

    #[test]
    fn menu_hit_rects_match_the_measured_controls() {
        let inside = |r: HitRect| {
            r.2 > 0.0
                && r.3 > 0.0
                && r.0 >= 0.0
                && r.1 >= 0.0
                && r.0 + r.2 <= 2560.0
                && r.1 + r.3 <= 1440.0
        };
        for r in MISSION_BUTTONS.iter().chain(CREATE_BUTTONS.iter()) {
            assert!(inside(*r), "{r:?} outside the content box");
        }
        // Mission Ok/Cancel live on the phist plate (708..1848 x 342..1095).
        for r in MISSION_BUTTONS {
            assert!(r.0 >= 708.0 && r.0 + r.2 <= 1848.0);
            assert!(r.1 >= 342.0 && r.1 + r.3 <= 1095.0);
        }
        // Create Ok/Cancel are the measured pok/pcancel bays.
        assert_eq!(CREATE_BUTTONS[0], (1637.0, 1143.0, 463.0, 65.0));
        assert_eq!(CREATE_BUTTONS[1], (1753.0, 1255.0, 423.0, 66.0));
        // Options cells share one x and a 93 px pitch.
        assert!(OPTIONS_TABS.iter().all(|r| r.0 == 521.0 && r.2 == 327.0));
        for pair in OPTIONS_TABS.windows(2) {
            assert_eq!(pair[1].1 - pair[0].1, 93.0);
        }
        for r in COLLECTION_CELLS.iter().chain(COLLECTION_BUTTONS.iter()) {
            assert!(inside(*r), "{r:?} outside the content box");
        }
        assert_eq!(COLLECTION_CELLS[0], (411.0, 274.0, 353.0, 224.0));
    }

    #[test]
    fn menu_sounds_are_the_shipped_glue_cues() {
        assert_eq!(TRANSITION_SOUND, "ui_swish");
        assert_eq!(PRESS_SOUND, "ui_click");
        assert_eq!(HOVER_SOUND, "ui_move");
    }

    #[test]
    fn collections_items_cover_every_hit_cell() {
        assert_eq!(COLLECTION_ITEMS.len(), COLLECTION_CELLS.len());
        assert_eq!(COLLECTION_ITEMS[0].0, "default_console_thumbnail.jpg");
        assert_eq!(COLLECTION_ITEMS[3].0, "skins_default.jpg");
        assert_eq!(COLLECTION_ITEMS[10].0, "heyjini_announcer.png");
    }

    #[test]
    fn menu_flow_round_trip_matches_the_capture_sequence() {
        // Main -> Single Player opens `Select Game Type` in single mode.
        assert_eq!(gametype_followup(false), GameTypeFollowup::Registry);
        // Multiplayer goes to the connection/lobby instead.
        assert_eq!(gametype_followup(true), GameTypeFollowup::MultiplayerLobby);
        // Original starts at Episode I, Expansion at Episode IV.
        assert_eq!(episode_start(false), 0);
        assert_eq!(episode_start(true), 3);
        // Episode <-> Mission modal, then back to Game Type and the main menu.
        assert_eq!(
            back_destination(&MenuScreen::Mission { episode: 1 }),
            Some(MenuScreen::Episode)
        );
        let mut screen = MenuScreen::Episode;
        for want in [MenuScreen::GameType, MenuScreen::Main] {
            screen = back_destination(&screen).expect("back path");
            assert_eq!(screen, want);
        }
        // Saved / Replays (opened from Episode) return to Episode; the footer
        // screens and the intro return to the main menu.
        for screen in [MenuScreen::SavedGames, MenuScreen::Replays] {
            assert_eq!(back_destination(&screen), Some(MenuScreen::Episode));
        }
        for screen in [
            MenuScreen::Options {
                tab: OptionsTab::Speed,
            },
            MenuScreen::Hotkeys {
                race: HotkeyRace::Terran,
            },
            MenuScreen::Collections,
            MenuScreen::Credits,
            MenuScreen::Intro { expansion: true },
            MenuScreen::Registry,
        ] {
            assert_eq!(back_destination(&screen), Some(MenuScreen::Main));
        }
    }
}

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
    /// 0..1 slider positions of the Speed tab (retail seven-stop rails).
    pub key_scroll: f32,
    pub mouse_scroll: f32,
    pub mouse_sensitivity: f32,
    pub music_volume: f32,
    pub sfx_volume: f32,
    pub use_mouse_sensitivity: bool,
    pub mouse_scaling: bool,
    pub hardware_cursor: bool,
    pub unit_speech: bool,
    pub unit_acks: bool,
    pub sound_background: bool,
    pub building_sounds: bool,
    pub cinematic_subtitles: bool,
    pub original_voice_overs: bool,
    pub original_1998_campaign: bool,
    /// Video tab state (all rendered, so the mouse can toggle them).
    pub video_fps_cap: bool,
    pub video_vsync: bool,
    pub video_hd: bool,
    pub video_environment: bool,
    pub video_lighting: bool,
    pub video_smooth_turning: bool,
    pub video_shadow_stacking: bool,
    pub video_pillarbox: bool,
    /// Game tab state: four gameplay checks, the friends notification check,
    /// the portrait mode (0 animated, 1 still, 2 disabled) and the mini-map
    /// position (0 left corner, 1 standard).
    pub game_timer: bool,
    pub game_color_cycling: bool,
    pub game_language_filter: bool,
    pub game_bonus_skins: bool,
    pub game_friend_notifications: bool,
    pub portraits: u8,
    pub minimap_position: u8,
    /// Network tab: prefer the retail discovery port.
    pub network_prefer_6112: bool,
    pub fog: bool,
    pub subtitles: bool,
    pub tips_at_startup: bool,
}

impl Default for GameOptions {
    fn default() -> Self {
        Self {
            game_speed: 1.0,
            key_scroll: 0.5,
            mouse_scroll: 0.5,
            mouse_sensitivity: 0.5,
            music_volume: 0.25,
            sfx_volume: 0.5,
            use_mouse_sensitivity: true,
            mouse_scaling: false,
            hardware_cursor: true,
            unit_speech: true,
            unit_acks: true,
            sound_background: true,
            building_sounds: true,
            cinematic_subtitles: true,
            original_voice_overs: false,
            original_1998_campaign: false,
            video_fps_cap: true,
            video_vsync: true,
            video_hd: true,
            video_environment: true,
            video_lighting: true,
            video_smooth_turning: true,
            video_shadow_stacking: true,
            video_pillarbox: false,
            game_timer: false,
            game_color_cycling: true,
            game_language_filter: true,
            game_bonus_skins: true,
            game_friend_notifications: true,
            portraits: 0,
            minimap_position: 0,
            network_prefer_6112: false,
            fog: true,
            subtitles: true,
            tips_at_startup: false,
        }
    }
}

/// Key/value codec of [`GameOptions`] for the profile store
/// (`crate::profile`).  Keys are the field names, values `f32` (bools 0/1,
/// modes their index), so new options load from old files with their
/// default and unknown keys are ignored.  The exhaustive destructure makes a
/// field missing from this list a compile error.
macro_rules! settings_fields {
    ($( $kind:ident $field:ident ),* $(,)?) => {
        impl GameOptions {
            pub fn to_pairs(&self) -> Vec<(&'static str, f32)> {
                let GameOptions { $( $field ),* } = self;
                vec![$( (stringify!($field), settings_fields!(@get $kind *$field)) ),*]
            }

            /// Apply one stored value; `false` for an unknown key.  Values
            /// are clamped to the range the UI can produce.
            pub fn set_pair(&mut self, key: &str, value: f32) -> bool {
                if !value.is_finite() {
                    return false;
                }
                match key {
                    $( stringify!($field) => {
                        self.$field = settings_fields!(@set $kind value);
                        true
                    } )*
                    _ => false,
                }
            }
        }
    };
    (@get speed $e:expr) => { $e };
    (@get unit $e:expr) => { $e };
    (@get bool $e:expr) => { if $e { 1.0 } else { 0.0 } };
    (@get mode3 $e:expr) => { $e as f32 };
    (@get mode2 $e:expr) => { $e as f32 };
    (@set speed $v:expr) => { $v.clamp(0.1, 3.0) };
    (@set unit $v:expr) => { $v.clamp(0.0, 1.0) };
    (@set bool $v:expr) => { $v != 0.0 };
    (@set mode3 $v:expr) => { $v.clamp(0.0, 2.0) as u8 };
    (@set mode2 $v:expr) => { $v.clamp(0.0, 1.0) as u8 };
}

settings_fields! {
    speed game_speed,
    unit key_scroll,
    unit mouse_scroll,
    unit mouse_sensitivity,
    unit music_volume,
    unit sfx_volume,
    bool use_mouse_sensitivity,
    bool mouse_scaling,
    bool hardware_cursor,
    bool unit_speech,
    bool unit_acks,
    bool sound_background,
    bool building_sounds,
    bool cinematic_subtitles,
    bool original_voice_overs,
    bool original_1998_campaign,
    bool video_fps_cap,
    bool video_vsync,
    bool video_hd,
    bool video_environment,
    bool video_lighting,
    bool video_smooth_turning,
    bool video_shadow_stacking,
    bool video_pillarbox,
    bool game_timer,
    bool game_color_cycling,
    bool game_language_filter,
    bool game_bonus_skins,
    bool game_friend_notifications,
    mode3 portraits,
    mode2 minimap_position,
    bool network_prefer_6112,
    bool fog,
    bool subtitles,
    bool tips_at_startup,
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
    /// `TEXTBOX` child `[x0, x1]` (inclusive, relative to the button) that
    /// centres the label; `None` centres it in the button rect.
    pub textbox: Option<(f32, f32)>,
}

impl CampaignSlot {
    /// Horizontal centre of the label text in dialog pixels.
    pub fn text_center(&self) -> f32 {
        match self.textbox {
            Some((x0, x1)) => self.button.0 + (x0 + x1 + 1.0) / 2.0,
            None => self.button.0 + self.button.2 / 2.0,
        }
    }
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
        textbox: None,
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
        textbox: Some((105.0, 210.0)),
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
        textbox: Some((93.0, 205.0)),
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
        textbox: None,
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
        textbox: Some((45.0, 170.0)),
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
        textbox: Some((93.0, 205.0)),
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
    /// Index of the hotkey glyph drawn bright (`\x04..\x01` span of the
    /// shipped `rc3.json` string).
    pub hot: usize,
}

pub const CAMPAIGN_BUTTONS: [CampaignButton; 4] = [
    CampaignButton {
        label: "Load Saved",
        rect: (243.0, 368.0, 189.0, 20.0),
        ink_width: 106.7,
        hot: 0,
    },
    CampaignButton {
        label: "Play Custom",
        rect: (226.0, 404.0, 206.0, 20.0),
        ink_width: 116.3,
        hot: 6,
    },
    CampaignButton {
        label: "Load Replay",
        rect: (238.0, 443.0, 194.0, 20.0),
        ink_width: 111.3,
        hot: 5,
    },
    CampaignButton {
        label: "Cancel",
        rect: (470.0, 404.0, 158.0, 20.0),
        ink_width: 61.7,
        hot: 0,
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
        // Live client: Cancel on the campaign screen returns to the Registry.
        MenuScreen::Episode => Some(MenuScreen::Registry),
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

/// Retail CEF menu cues (verified against the live client on 2026-11-02):
/// hover -> `sound/glue/mouseover.wav`, press -> `sound/glue/mousedown2.wav`.
/// Screen changes have no dedicated cue: the press that triggers them is the
/// only sound (the CEF bundle sends only `GLUSND_MOUSE_OVER` and
/// `GLUSND_MOUSE_DOWN`, and the capture has no swishin transient).
pub const PRESS_SOUND: &str = "ui_click";
/// Sound played when the pointer enters a control: `sound/glue/mouseover.wav`.
pub const HOVER_SOUND: &str = "ui_move";

/// Seven-stop slider value from a pointer x inside `x0..x1` (the retail
/// rails have seven detents between their 16/17 px end pads).
pub fn slider_step(x: f32, x0: f32, x1: f32) -> usize {
    let sx0 = x0 + 16.0;
    let sx1 = x1 - 17.0;
    let t = ((x - sx0) / (sx1 - sx0).max(1.0)).clamp(0.0, 1.0);
    (t * 6.0).round() as usize
}

/// Fraction 0..1 of a seven-stop slider step.
pub fn slider_frac(step: usize) -> f32 {
    step.min(6) as f32 / 6.0
}

/// Measured control rectangles in 2560x1440 content-box pixels, shared by the
/// drawing code and the hit-testing code so they cannot diverge.
pub type HitRect = (f32, f32, f32, f32);

/// A classic glue list dialog: the `glue/<name>/plist.dds` plate with the
/// title, the list box and its `FLCBTN` buttons, plus the shared `PalNl`
/// Ok/Cancel plates.  Rects are the shipped `rez/*.ui.json` controls in
/// 640x480 UI pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlueListDialog {
    /// `pList` plate (id 1, `RC_ANCHOR_TOPRIGHT`: its right edge sits on
    /// the control's right edge, the 4x art drawn at `scale / 4`).
    pub plate: HitRect,
    /// Title `LTEXT`; `title_bigger` is `RC_BIGGERTEXT` (extended face).
    pub title: HitRect,
    pub title_bigger: bool,
    pub list: HitRect,
    pub ok: HitRect,
    pub cancel: HitRect,
    /// `New ID` (Registry only).
    pub new_id: Option<HitRect>,
    pub delete: HitRect,
}

/// Ok / Cancel `FLCBTN`s over the shared `PalNl` plates: the same rects in
/// `glulogin` (ids 4/5), `gluload` (4/5) and `glucreat` (12/13).
pub const PALNL_OK_BUTTON: HitRect = (441.0, 383.0, 152.0, 22.0);
pub const PALNL_CANCEL_BUTTON: HitRect = (477.0, 419.0, 132.0, 22.0);

/// `rez/glulogin.ui.json` (Single Player Registry): ids 1, 9, 8, 4, 5, 6, 7.
pub const LOGIN_DIALOG: GlueListDialog = GlueListDialog {
    plate: (0.0, 0.0, 362.0, 360.0),
    title: (48.0, 60.0, 132.0, 30.0),
    title_bigger: true,
    list: (60.0, 100.0, 288.0, 210.0),
    ok: PALNL_OK_BUTTON,
    cancel: PALNL_CANCEL_BUTTON,
    new_id: Some((44.0, 335.0, 165.0, 22.0)),
    delete: (224.0, 335.0, 133.0, 22.0),
};

/// `rez/gluload.ui.json` (Load Saved Game): ids 1, -1, 6, 4, 5, 7.
pub const LOAD_DIALOG: GlueListDialog = GlueListDialog {
    plate: (0.0, 0.0, 362.0, 356.0),
    title: (40.0, 68.0, 136.0, 20.0),
    title_bigger: false,
    list: (64.0, 92.0, 286.0, 218.0),
    ok: PALNL_OK_BUTTON,
    cancel: PALNL_CANCEL_BUTTON,
    new_id: None,
    delete: (44.0, 332.0, 164.0, 20.0),
};

/// Classic glue "swish": the dialog plates slide in when a list dialog
/// opens and out when it closes.  Measured on a 60 fps recording of the
/// retail client (2026-11-04, Single Player → Original → Registry → Ok):
/// - the plates move in steps of 1/30 s;
/// - in: each plate starts just outside the 16:9 box (the list plate from
///   the left, Ok from the right, Cancel from the bottom) and the remaining
///   distance is multiplied by 0.75 per step (1408 px at 1440p, then 1056,
///   792, 594, 446, 336, 252, 188, ... measured), snapping into place once
///   under 2 UI px; the text appears when every plate is in place;
/// - out: the text disappears at once and each plate moves away by
///   2^n − 1 UI px after n steps (−3, −9, −21, −45, −93, −189, −381 px at
///   1440p; measured −2, −8, −20, −44, −92, −188, −380); the next screen
///   appears one step after the farthest plate has left the box.
pub const SWISH_STEPS_PER_SEC: f32 = 30.0;

/// Remaining offset (UI px) of a plate sliding in from `distance`.
pub fn swish_in_offset(distance: f32, step: u32) -> f32 {
    let o = distance * 0.75f32.powi(step.min(64) as i32);
    if o.abs() < 2.0 {
        0.0
    } else {
        o
    }
}

/// Steps until a plate sliding in from `distance` is in place.
pub fn swish_in_steps(distance: f32) -> u32 {
    (0..64)
        .find(|&n| swish_in_offset(distance, n) == 0.0)
        .unwrap_or(64)
}

/// Offset (UI px) of a plate sliding out after `step` steps.
pub fn swish_out_offset(step: u32) -> f32 {
    2f32.powi(step.min(30) as i32) - 1.0
}

/// Steps a swish-out lasts: the farthest plate (`distance` UI px to leave
/// the box) is out, plus one step on the empty backdrop.
pub fn swish_out_steps(distance: f32) -> u32 {
    (0..31)
        .find(|&n| swish_out_offset(n) >= distance)
        .unwrap_or(30)
        + 1
}

/// List rows of the glue list dialogs: first row top `list.y + 16/3` and a
/// 19 UI px pitch (57 px at 1440p, measured on 130734 / 130848).
pub const GLUE_LIST_ROW_PITCH: f32 = 19.0;
pub const GLUE_LIST_TEXT_INSET: (f32, f32) = (7.0 / 3.0, 16.0 / 3.0);

/// Control of a glue list dialog under a UI point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlueListHit {
    Ok,
    Cancel,
    NewId,
    Delete,
    /// A list row (0 = first visible row).
    Row(usize),
}

impl GlueListDialog {
    /// Hit test in 640x480 UI pixels; buttons win over the list.
    pub fn hit(&self, x: f32, y: f32, rows: usize) -> Option<GlueListHit> {
        let inside = |r: HitRect| x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3;
        if inside(self.ok) {
            return Some(GlueListHit::Ok);
        }
        if inside(self.cancel) {
            return Some(GlueListHit::Cancel);
        }
        if self.new_id.is_some_and(inside) {
            return Some(GlueListHit::NewId);
        }
        if inside(self.delete) {
            return Some(GlueListHit::Delete);
        }
        if inside(self.list) {
            let top = self.list.1 + GLUE_LIST_TEXT_INSET.1;
            if y >= top {
                let row = ((y - top) / GLUE_LIST_ROW_PITCH) as usize;
                if row < rows {
                    return Some(GlueListHit::Row(row));
                }
            }
        }
        None
    }
}

/// `Select starting mission` modal: Ok and Cancel bays of `phist.dds`.
pub const MISSION_BUTTONS: [HitRect; 2] =
    [(910.0, 972.0, 355.0, 86.0), (1300.0, 972.0, 355.0, 86.0)];
/// Mission list rows (8 lines, 57 px pitch, first ink top at 478), left of
/// the scrollbar column (x 1771..1819 on the live client).
pub const MISSION_ROWS: HitRect = (748.0, 470.0, 1022.0, 8.0 * 57.0);

/// Mission-list scrollbar, measured on the live client (2560x1440 content
/// px): up arrow y 459..510, track box 516..876, down arrow 882..930,
/// 42 px octagonal thumb whose top runs from 519 (top) to 834 (last page).
pub const MISSION_SCROLL_X: (f32, f32) = (1771.0, 1819.0);
pub const MISSION_SCROLL_UP: (f32, f32) = (459.0, 511.0);
pub const MISSION_SCROLL_DOWN: (f32, f32) = (881.0, 931.0);
pub const MISSION_THUMB_TOP: (f32, f32) = (519.0, 834.0);
pub const MISSION_THUMB_H: f32 = 42.0;

/// Part of the mission scrollbar under a content-box point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollHit {
    Up,
    Down,
    PageUp,
    PageDown,
    Thumb,
}

/// Scroll fraction for a thumb whose top is at content y `top`.
pub fn mission_thumb_fraction(top: f32) -> f32 {
    ((top - MISSION_THUMB_TOP.0) / (MISSION_THUMB_TOP.1 - MISSION_THUMB_TOP.0)).clamp(0.0, 1.0)
}

/// Content y of the thumb top for a scroll fraction.
pub fn mission_thumb_top(fraction: f32) -> f32 {
    MISSION_THUMB_TOP.0 + (MISSION_THUMB_TOP.1 - MISSION_THUMB_TOP.0) * fraction
}

/// Hit-test the mission scrollbar; `thumb` is the scroll fraction 0..=1.
pub fn mission_scrollbar_hit(x: f32, y: f32, thumb: f32) -> Option<ScrollHit> {
    if x < MISSION_SCROLL_X.0 || x >= MISSION_SCROLL_X.1 {
        return None;
    }
    if y >= MISSION_SCROLL_UP.0 && y < MISSION_SCROLL_UP.1 {
        return Some(ScrollHit::Up);
    }
    if y >= MISSION_SCROLL_DOWN.0 && y < MISSION_SCROLL_DOWN.1 {
        return Some(ScrollHit::Down);
    }
    if y < MISSION_SCROLL_UP.1 || y >= MISSION_SCROLL_DOWN.0 {
        return None;
    }
    let top = MISSION_THUMB_TOP.0 + (MISSION_THUMB_TOP.1 - MISSION_THUMB_TOP.0) * thumb;
    Some(if y < top {
        ScrollHit::PageUp
    } else if y < top + MISSION_THUMB_H {
        ScrollHit::Thumb
    } else {
        ScrollHit::PageDown
    })
}

/// `glupokcancel` (360x200) centred in the 640x480 campaign dialog: Ok and
/// Cancel bays (17,159) / (186,159), 158x29, in dialog pixels.
pub const PROMPT_RECT: (f32, f32, f32, f32) = (140.0, 140.0, 360.0, 200.0);
pub const PROMPT_BUTTONS: [(f32, f32, f32, f32); 2] =
    [(157.0, 299.0, 158.0, 29.0), (326.0, 299.0, 158.0, 29.0)];
/// Body text box of the prompt (`gluPOkCancel-2`, `RC_BIGTEXT`).
pub const PROMPT_TEXT: (f32, f32, f32, f32) = (150.0, 150.0, 340.0, 140.0);

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

/// Video tab: Enable FPS Cap (top of the right column) and the seven
/// checkboxes at y 668 + 54i.
pub const OPTIONS_CHECKS_VIDEO: [HitRect; 8] = [
    (1456.0, 341.0, 430.0, 29.0),
    (1456.0, 668.0, 430.0, 29.0),
    (1456.0, 722.0, 430.0, 29.0),
    (1456.0, 776.0, 430.0, 29.0),
    (1456.0, 830.0, 430.0, 29.0),
    (1456.0, 884.0, 430.0, 29.0),
    (1456.0, 938.0, 430.0, 29.0),
    (1456.0, 992.0, 430.0, 29.0),
];
/// Game tab: the four gameplay checkboxes, the friends checkbox and the two
/// radio groups (portraits 900/1075/1190 at y 752, mini-map 900/1096 at 869).
pub const OPTIONS_CHECKS_GAME: [HitRect; 10] = [
    (900.0, 389.0, 430.0, 29.0),
    (900.0, 443.0, 430.0, 29.0),
    (900.0, 497.0, 430.0, 29.0),
    (900.0, 551.0, 430.0, 29.0),
    (900.0, 941.0, 430.0, 29.0),
    (896.0, 746.0, 165.0, 46.0),
    (1071.0, 746.0, 110.0, 46.0),
    (1186.0, 746.0, 120.0, 46.0),
    (896.0, 863.0, 190.0, 46.0),
    (1092.0, 863.0, 160.0, 46.0),
];
/// Network tab: `Prefer port 6112`.
pub const OPTIONS_CHECKS_NETWORK: [HitRect; 1] = [(900.0, 341.0, 430.0, 29.0)];

/// LAN panel geometry (dev lockstep screen): the plate is 360 UI px wide,
/// 60 + 22*lines tall, centred in the content box; rows are 18 px high.
pub fn lan_panel_rect(content_w: i32, content_h: i32, lines: usize) -> (i32, i32, i32, i32) {
    let scale = content_h.max(1) as f32 / 480.0;
    let bw = (360.0 * scale) as i32;
    let bh = 60 + lines as i32 * 22;
    ((content_w - bw) / 2, (content_h - bh) / 2, bw, bh)
}

pub fn lan_row_rect(panel: (i32, i32, i32, i32), i: usize) -> HitRect {
    let (bx, by, bw, _) = panel;
    (
        (bx + 12) as f32,
        (by + 44 + i as i32 * 22 - 2) as f32,
        (bw - 24) as f32,
        18.0,
    )
}

/// Lobby slot rows: the race cell of each of the eight player rows, measured
/// on 134831 (first ink row y 960, 48 px pitch, race column starts at x 902).
pub fn lobby_race_bay(i: usize) -> HitRect {
    (902.0, 958.0 + 48.0 * i as f32, 621.0, 46.0)
}

/// Live race dropdown: a 5-item list opened from `slot` (same box geometry
/// as the shipped popup, 55 px rows).
pub fn lobby_race_item(slot: usize, k: usize) -> HitRect {
    (
        902.0,
        960.0 + 48.0 * slot as f32 + 5.0 + 55.0 * k as f32,
        441.0,
        52.0,
    )
}

/// Options sliders (rail boxes) and checkboxes, measured on the Options
/// captures.  Speed: game speed, key scroll, mouse scroll (left) and mouse
/// sensitivity (right); Sound: music and digital volume.
pub const OPTIONS_SLIDERS_SPEED: [HitRect; 4] = [
    (900.0, 371.0, 516.0, 40.0),
    (900.0, 524.0, 516.0, 40.0),
    (900.0, 677.0, 516.0, 40.0),
    (1456.0, 374.0, 516.0, 40.0),
];
pub const OPTIONS_CHECKS_SPEED: [HitRect; 4] = [
    (1456.0, 341.0, 430.0, 29.0),
    (1456.0, 497.0, 430.0, 29.0),
    (1456.0, 551.0, 430.0, 29.0),
    (1456.0, 605.0, 430.0, 29.0),
];
pub const OPTIONS_SLIDERS_SOUND: [HitRect; 2] =
    [(900.0, 374.0, 516.0, 40.0), (900.0, 530.0, 516.0, 40.0)];
pub const OPTIONS_CHECKS_SOUND: [HitRect; 8] = [
    (1456.0, 341.0, 430.0, 29.0),
    (1456.0, 395.0, 430.0, 29.0),
    (1456.0, 449.0, 430.0, 29.0),
    (1456.0, 503.0, 430.0, 29.0),
    (1456.0, 557.0, 430.0, 29.0),
    (1456.0, 611.0, 430.0, 29.0),
    (1456.0, 665.0, 430.0, 29.0),
    (1456.0, 719.0, 430.0, 29.0),
];

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
    fn glue_list_dialogs_match_the_shipped_ui_json() {
        let rect = |dialog: &crate::ui::UiDialog, id: i32| {
            let c = dialog.control(id).expect("control id");
            (
                c.abs.x as f32,
                c.abs.y as f32,
                c.abs.w as f32,
                c.abs.h as f32,
            )
        };
        let load = |name: &str| {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("fixtures/ui/{name}.ui.json"));
            let text = std::fs::read_to_string(path).expect("dialog fixture");
            crate::ui::UiDialog::parse(text.as_bytes()).expect("valid dialog")
        };
        let login = load("glulogin");
        let d = LOGIN_DIALOG;
        assert_eq!(rect(&login, 1), d.plate);
        assert_eq!(rect(&login, 9), d.title);
        assert_eq!(rect(&login, 8), d.list);
        assert_eq!(rect(&login, 4), d.ok);
        assert_eq!(rect(&login, 5), d.cancel);
        assert_eq!(Some(rect(&login, 6)), d.new_id);
        assert_eq!(rect(&login, 7), d.delete);
        let gl = load("gluload");
        let d = LOAD_DIALOG;
        assert_eq!(rect(&gl, 1), d.plate);
        assert_eq!(rect(&gl, -1), d.title);
        assert_eq!(rect(&gl, 6), d.list);
        assert_eq!(rect(&gl, 4), d.ok);
        assert_eq!(rect(&gl, 5), d.cancel);
        assert_eq!(rect(&gl, 7), d.delete);
        let create = load("glucreat");
        assert_eq!(rect(&create, 12), PALNL_OK_BUTTON);
        assert_eq!(rect(&create, 13), PALNL_CANCEL_BUTTON);
    }

    #[test]
    fn swish_follows_the_retail_recording() {
        // In, list plate: 1408 px at 1440p = 469.33 UI px; recording values
        // in screen px at steps 1..8.
        let px = |o: f32| (o * 3.0).round();
        let seen = [1056.0, 792.0, 594.0, 446.0, 336.0, 252.0, 188.0, 144.0];
        for (n, &want) in seen.iter().enumerate() {
            let got = px(swish_in_offset(1408.0 / 3.0, n as u32 + 1));
            assert!((got - want).abs() <= 4.0, "step {}: {got} vs {want}", n + 1);
        }
        // Locks at step 19 (measured: last visible offset 8 px at step 18).
        assert_eq!(swish_in_steps(1408.0 / 3.0), 19);
        // Ok plate (932 px): 524 px after two steps.
        assert!((px(swish_in_offset(932.0 / 3.0, 2)) - 524.0).abs() <= 2.0);
        // Out: 2^n - 1 UI px; measured -2 -8 -20 -44 -92 -188 -380 px.
        let out = [2.0, 8.0, 20.0, 44.0, 92.0, 188.0, 380.0];
        for (n, &want) in out.iter().enumerate() {
            let got = px(swish_out_offset(n as u32 + 1));
            assert!((got - want).abs() <= 1.0, "out step {}: {got}", n + 1);
        }
        // The next screen 10 steps (0.333 s) after the click.
        assert_eq!(swish_out_steps(1408.0 / 3.0), 10);
    }

    #[test]
    fn glue_list_hits_buttons_and_rows() {
        let d = LOGIN_DIALOG;
        assert_eq!(d.hit(500.0, 390.0, 2), Some(GlueListHit::Ok));
        assert_eq!(d.hit(540.0, 430.0, 2), Some(GlueListHit::Cancel));
        assert_eq!(d.hit(100.0, 345.0, 2), Some(GlueListHit::NewId));
        assert_eq!(d.hit(250.0, 345.0, 2), Some(GlueListHit::Delete));
        // Rows: first at 105.33, 19 px pitch; only existing rows hit.
        assert_eq!(d.hit(100.0, 110.0, 2), Some(GlueListHit::Row(0)));
        assert_eq!(d.hit(100.0, 126.0, 2), Some(GlueListHit::Row(1)));
        assert_eq!(d.hit(100.0, 150.0, 2), None);
        assert_eq!(LOAD_DIALOG.hit(100.0, 340.0, 1), Some(GlueListHit::Delete));
        assert_eq!(LOAD_DIALOG.new_id, None);
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
        // Live client (2026-11-02): campaign Cancel returns to the Registry.
        assert_eq!(
            back_destination(&MenuScreen::Episode),
            Some(MenuScreen::Registry)
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
        assert_eq!(PRESS_SOUND, "ui_click");
        assert_eq!(HOVER_SOUND, "ui_move");
        // The remastered CEF menu has no dedicated transition cue; the press
        // is the only sound on a screen change (live capture, 2026-11-02).
    }

    #[test]
    fn collections_items_cover_every_hit_cell() {
        assert_eq!(COLLECTION_ITEMS.len(), COLLECTION_CELLS.len());
        assert_eq!(COLLECTION_ITEMS[0].0, "default_console_thumbnail.jpg");
        assert_eq!(COLLECTION_ITEMS[3].0, "skins_default.jpg");
        assert_eq!(COLLECTION_ITEMS[10].0, "heyjini_announcer.png");
    }

    #[test]
    fn mission_scrollbar_hits_follow_the_live_geometry() {
        assert_eq!(
            mission_scrollbar_hit(1794.0, 480.0, 0.0),
            Some(ScrollHit::Up)
        );
        assert_eq!(
            mission_scrollbar_hit(1794.0, 906.0, 0.0),
            Some(ScrollHit::Down)
        );
        assert_eq!(
            mission_scrollbar_hit(1794.0, 540.0, 0.0),
            Some(ScrollHit::Thumb)
        );
        assert_eq!(
            mission_scrollbar_hit(1794.0, 700.0, 0.0),
            Some(ScrollHit::PageDown)
        );
        assert_eq!(
            mission_scrollbar_hit(1794.0, 700.0, 1.0),
            Some(ScrollHit::PageUp)
        );
        assert_eq!(
            mission_scrollbar_hit(1794.0, 850.0, 1.0),
            Some(ScrollHit::Thumb)
        );
        // Rows end before the scrollbar column; outside it nothing hits.
        assert!(MISSION_ROWS.0 + MISSION_ROWS.2 <= MISSION_SCROLL_X.0);
        assert_eq!(mission_scrollbar_hit(1700.0, 700.0, 0.0), None);
        assert_eq!(mission_scrollbar_hit(1794.0, 440.0, 0.0), None);
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
        // Episode <-> Mission modal, then back to the Registry and the main
        // menu (live client: campaign Cancel -> Registry, Registry Cancel ->
        // Main).
        assert_eq!(
            back_destination(&MenuScreen::Mission { episode: 1 }),
            Some(MenuScreen::Episode)
        );
        let mut screen = MenuScreen::Episode;
        for want in [MenuScreen::Registry, MenuScreen::Main] {
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

    #[test]
    fn overlapping_footer_hot_areas_win_over_the_tall_entries() {
        // Options label ink starts at y=347.33 UI, inside the 15 px band where
        // Multiplayer's 160 px hotbox overlaps the footer column.  The footer
        // entries are later in the dialog, so they must win the hit test.
        let (scale, ox, oy) = (3.0f32, 320, 0);
        let options = 320 + (79.0 * scale) as i32;
        let y = (347.6 * scale) as i32; // inside both Multiplayer and Options
        assert_eq!(main_entry_at(scale, ox, oy, options, y), Some(2));
        // Every footer label centre hits its own entry even in that band.
        for label in MAIN_LABELS.iter().filter(|l| l.idx >= 2 && l.idx <= 6) {
            let x = ox + (label.x * scale) as i32;
            let top = oy + (label.y * scale) as i32;
            assert_eq!(
                main_entry_at(scale, ox, oy, x, top + 4),
                Some(label.idx),
                "footer entry {} must win the overlap band",
                label.idx
            );
        }
    }

    #[test]
    fn slider_steps_map_to_the_seven_retail_detents() {
        assert_eq!(slider_step(900.0, 900.0, 1416.0), 0);
        assert_eq!(slider_step(1416.0, 900.0, 1416.0), 6);
        // Centre of the rail is the middle detent.
        assert_eq!(slider_step(1158.0, 900.0, 1416.0), 3);
        for step in 0..=6 {
            assert!((slider_frac(step) - step as f32 / 6.0).abs() < 1e-6);
        }
        assert_eq!(slider_frac(9), 1.0, "clamped");
    }

    #[test]
    fn options_control_rects_are_inside_the_panel() {
        // The plate is 515..2047 x 175..1150; every control must sit inside.
        for r in OPTIONS_SLIDERS_SPEED
            .iter()
            .chain(OPTIONS_CHECKS_SPEED.iter())
            .chain(OPTIONS_SLIDERS_SOUND.iter())
            .chain(OPTIONS_CHECKS_SOUND.iter())
        {
            assert!(r.0 >= 515.0 && r.0 + r.2 <= 2047.0, "{r:?}");
            assert!(r.1 >= 175.0 && r.1 + r.3 <= 1150.0, "{r:?}");
        }
        assert_eq!(OPTIONS_SLIDERS_SPEED[0], (900.0, 371.0, 516.0, 40.0));
        assert_eq!(OPTIONS_SLIDERS_SPEED[3].0, 1456.0);
    }

    #[test]
    fn lobby_race_hit_rects_follow_the_measured_rows() {
        let bay0 = lobby_race_bay(0);
        assert_eq!(bay0, (902.0, 958.0, 621.0, 46.0));
        let bay7 = lobby_race_bay(7);
        assert_eq!(bay7.1, 958.0 + 48.0 * 7.0);
        // Rows do not overlap and stay inside the slot panel (855..1434).
        for i in 0..7 {
            let a = lobby_race_bay(i);
            let b = lobby_race_bay(i + 1);
            assert!(a.1 + a.3 <= b.1, "row {i} overlaps the next");
        }
        assert!(bay7.1 + bay7.3 <= 1434.0);
        // The dropdown items sit under their slot and keep a 55 px pitch.
        let item0 = lobby_race_item(1, 0);
        let item1 = lobby_race_item(1, 1);
        assert_eq!(item1.1 - item0.1, 55.0);
        assert_eq!(item0.0, 902.0);
        assert!(item0.1 >= lobby_race_bay(1).1);
    }

    #[test]
    fn remaining_option_controls_are_reachable() {
        for r in OPTIONS_CHECKS_VIDEO
            .iter()
            .chain(OPTIONS_CHECKS_GAME.iter())
            .chain(OPTIONS_CHECKS_NETWORK.iter())
        {
            assert!(r.0 >= 515.0 && r.0 + r.2 <= 2047.0, "{r:?}");
            assert!(r.1 >= 175.0 && r.1 + r.3 <= 1150.0, "{r:?}");
        }
        assert_eq!(OPTIONS_CHECKS_VIDEO[0].1, 341.0);
        assert_eq!(OPTIONS_CHECKS_VIDEO[7].1, 992.0);
        assert_eq!(OPTIONS_CHECKS_GAME[5].0, 896.0);
        assert_eq!(OPTIONS_CHECKS_NETWORK[0], (900.0, 341.0, 430.0, 29.0));
    }

    #[test]
    fn lan_rows_are_clickable_inside_the_panel() {
        let panel = lan_panel_rect(2560, 1440, 4);
        assert_eq!(panel.2, 1080);
        let row = lan_row_rect(panel, 2);
        assert!(row.0 >= panel.0 as f32 && row.0 + row.2 <= (panel.0 + panel.2) as f32);
        assert!(row.1 >= panel.1 as f32);
        let next = lan_row_rect(panel, 3);
        assert_eq!(next.1 - row.1, 22.0);
    }
}

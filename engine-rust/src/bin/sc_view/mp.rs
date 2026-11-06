//! Multiplayer front end, as the retail client shows it on the user's
//! capture of 2026-11-04 21:11:48 (Select Connection -> Registry -> Games ->
//! Create -> "Creating game..." -> lobby -> countdown -> game) and the score
//! screen of 21:14:04.  Rules live in `galaxycraft::multiplayer`; this file
//! only draws them and routes input.
//!
//! Geometry: plates use the shipped `rez/*.ui.json` rects (640x480 UI, 4x
//! art drawn at `scale / 4`, `RC_ANCHOR_TOPRIGHT` plates right-aligned);
//! CEF panel content (lobby rows, map info) is measured on the capture in
//! 2560x1440 content px and scaled with the content box.

use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;

use galaxycraft::dds::{DdsGrp, DdsImage};
use galaxycraft::multiplayer::{
    self as mpl, GameType, Lobby, OCCUPANT_CHOICES, Occupant, RaceChoice, ScoreTab, countdown,
};
use galaxycraft::net::{Announce, Msg, TcpTransport, Transport};

use super::*;

/// Retail palette (glue `FLCBTN`/list colours and the CEF panel CSS).
const TITLE: Rgb = Rgb::new(0xc6, 0xda, 0xff);
const ROW: Rgb = Rgb::new(0x5c, 0xed, 0x30);
const ROW_SELECTED: Rgb = Rgb::new(0xda, 0xff, 0x8c);
const CEF_GREEN: Rgb = Rgb::new(0x4c, 0xc4, 0x28);
const CEF_HOVER: Rgb = Rgb::new(0xb4, 0xfc, 0x74);
const CEF_INFO: Rgb = Rgb::new(0xa2, 0xb1, 0xf5);
const CEF_BORDER: Rgb = Rgb::new(0x6c, 0x00, 0x00);
const CEF_BORDER_HOT: Rgb = Rgb::new(0xfc, 0x10, 0x00);
const VALUE: Rgb = Rgb::new(0xe6, 0xe6, 0xe6);
const DISABLED: Rgb = Rgb::new(0x80, 0x80, 0x80);
const ARROW: Rgb = Rgb::new(0xc8, 0xb4, 0x00);

/// "Creating game..." stays up 0.7 s (capture 22.25-22.95 s).
const CREATING_SECS: f32 = 0.7;
/// After the countdown the plates leave and the screen stays black until
/// the game shows (57.30 s out, 58.63 s game).
const LAUNCH_SECS: f32 = 1.33;
/// Visible rows of the Create map list (capture: six).
const MAP_ROWS: usize = 6;
/// Retail game speeds, ms per frame, Slowest..Fastest.
const SPEED_MS: [f32; 7] = [167.0, 111.0, 83.0, 67.0, 56.0, 48.0, 42.0];
const SPEED_NAMES: [&str; 7] = [
    "Slowest", "Slower", "Slow", "Normal", "Fast", "Faster", "Fastest",
];
/// A LAN game disappears from the list when its announce stops this long.
const GAME_TIMEOUT: Duration = Duration::from_secs(3);

/// A dropdown control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Drop {
    GameType,
    Teams,
    Player(usize),
    Race(usize),
    Color(usize),
    Observer(usize),
}

/// Something under the pointer, registered while drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MpHit {
    Row(usize),
    Ok,
    Cancel,
    CreateGame,
    ScrollUp,
    ScrollDown,
    Combo(Drop),
    Item(usize),
    SpeedDown,
    SpeedUp,
    PopupCancel,
    Tab(usize),
    SaveReplay,
    WatchReplay,
}

/// An open dropdown list.
#[derive(Clone, Debug)]
pub(super) struct OpenDrop {
    which: Drop,
    /// The control's rect in content px.
    rect: (f32, f32, f32, f32),
    items: Vec<String>,
}

/// One row of the Create map browser.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum MapRow {
    Up,
    Folder(String),
    Map(usize),
}

/// What the Create info panel shows for a map.
#[derive(Clone, Debug)]
pub(super) struct MapInfo {
    index: usize,
    title: String,
    description: String,
    suggested: String,
    size: (usize, usize),
    tileset: &'static str,
    slots: usize,
    /// 128x128 terrain preview (RGB) plus overlay points in map tiles.
    preview: Vec<u32>,
    starts: Vec<(u8, i32, i32)>,
    resources: Vec<(i32, i32)>,
}

/// Shipped plates of the multiplayer screens, read straight from CASC.
#[derive(Default)]
pub(super) struct MpArt {
    conn_list: Option<DdsImage>,
    conn_info: Option<DdsImage>,
    conn_gateway: Option<DdsImage>,
    games_list: Option<DdsImage>,
    games_info: Option<DdsImage>,
    lobby_list: Option<DdsImage>,
    lobby_chat: Option<DdsImage>,
    lobby_info: Option<DdsImage>,
    icreate: Vec<DdsImage>,
}

/// Score screen art of one race/result (`glue/score<r><v|d>/`).
#[derive(Default)]
pub(super) struct ScoreArt {
    backgnd: Option<DdsImage>,
    pmain: Option<DdsImage>,
    pinset: Option<DdsImage>,
    scorebox: Option<DdsImage>,
    iscore: Vec<DdsImage>,
}

/// A joined LAN game, client side: the host's link and what the lobby
/// needs until the host sends `Setup`.
pub(super) struct ClientLobby {
    link: TcpTransport,
    addr: SocketAddr,
    map_hash: [u8; 32],
    /// Our peer index, from the first `LobbyState`.
    you: Option<u8>,
    speed: u8,
}

#[derive(Default)]
pub(super) struct MpState {
    /// Lobby setup of the game being played (or last played), `None` for
    /// single-player games.  Saved with saves and replays.
    pub setup: Option<galaxycraft::multiplayer::GameSetup>,
    /// Select Connection: Registry Ok continues to the Games list.
    pub registry_next: bool,
    conn_sel: usize,
    /// When the selection changed: the lower plate swishes out and back
    /// (capture 5.8-7.0 s).
    conn_swap: Option<Instant>,
    games: Vec<(SocketAddr, Announce, Instant)>,
    games_sel: Option<usize>,
    discovery: Option<UdpSocket>,
    create_dir: Vec<String>,
    create_map: Option<usize>,
    create_top: usize,
    game_type: Option<GameType>,
    split: Option<(u8, u8)>,
    speed: Option<usize>,
    info: Option<MapInfo>,
    creating: Option<Instant>,
    lobby: Option<Lobby>,
    lobby_map: usize,
    countdown: Option<Instant>,
    launching: Option<Instant>,
    chat: Vec<String>,
    chat_input: String,
    /// Set while this client sits in someone else's lobby.
    client: Option<ClientLobby>,
    /// Host: the lobby bytes last sent to joined players, and when.
    lobby_sent: Option<(Instant, Vec<u8>)>,
    open: Option<OpenDrop>,
    hits: Vec<((i32, i32, i32, i32), MpHit)>,
    hover: Option<MpHit>,
    art: Option<Arc<MpArt>>,
    score_art: Option<((u8, bool), Arc<ScoreArt>)>,
    score_tab: usize,
}

impl MpState {
    pub fn enter_connection(&mut self) {
        // Retail opens with Battle.net selected and Ok grey (3.6-5.6 s).
        self.conn_sel = 0;
        self.conn_swap = None;
        self.registry_next = false;
    }

    fn speed_index(&self) -> usize {
        self.speed.unwrap_or(SPEED_MS.len() - 1)
    }
}

fn load_dds(install: &RetailInstall, path: &str) -> Option<DdsImage> {
    galaxycraft::dds::parse(&install.read(path)?)
}

fn load_grp(install: &RetailInstall, path: &str) -> Vec<DdsImage> {
    install
        .read(path)
        .and_then(|b| DdsGrp::parse(&b))
        .map(|g| g.frames)
        .unwrap_or_default()
}

fn race_letter(race: Race) -> char {
    match race {
        Race::Zerg => 'z',
        Race::Protoss => 'p',
        _ => 't',
    }
}

fn tileset_name(era: u16) -> &'static str {
    match era {
        0 => "Badlands",
        1 => "Space Platform",
        2 => "Installation",
        3 => "Ash World",
        4 => "Jungle",
        5 => "Desert",
        6 => "Ice",
        _ => "Twilight",
    }
}

fn inside(r: (i32, i32, i32, i32), x: i32, y: i32) -> bool {
    x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Face {
    Reg,
    Ext,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Align {
    Left,
    Center,
    Right,
}

impl App {
    // ------------------------------------------------------------------
    // Drawing primitives (content px at 1440p unless noted)
    // ------------------------------------------------------------------

    fn mp_k(fb: &Framebuffer) -> (i32, f32) {
        let (cx, _, _, ch) = Self::content_rect(fb);
        (cx, ch as f32 / 1440.0)
    }

    /// Screen rect of a content-px rect.
    fn mp_rect(fb: &Framebuffer, r: (f32, f32, f32, f32)) -> (i32, i32, i32, i32) {
        let (cx, k) = Self::mp_k(fb);
        (
            cx + (r.0 * k).round() as i32,
            (r.1 * k).round() as i32,
            ((r.2 * k).round() as i32).max(1),
            ((r.3 * k).round() as i32).max(1),
        )
    }

    /// Screen rect of a 640x480 UI rect.
    fn mp_ui(fb: &Framebuffer, r: (f32, f32, f32, f32)) -> (i32, i32, i32, i32) {
        Self::mp_rect(fb, (320.0 + r.0 * 3.0, r.1 * 3.0, r.2 * 3.0, r.3 * 3.0))
    }

    fn mp_hit(&mut self, r: (i32, i32, i32, i32), hit: MpHit) {
        self.mp.hits.push((r, hit));
    }

    /// A shipped 4x plate at its UI rect, shifted by `(dx, dy)` UI px.
    fn mp_plate(
        &self,
        fb: &mut Framebuffer,
        art: Option<&DdsImage>,
        rect: (f32, f32, f32, f32),
        right: bool,
        dx: f32,
        dy: f32,
    ) {
        let Some(art) = art else {
            return;
        };
        let (scale, ox, oy) = self.ui_metrics(fb);
        let w = (art.width as f32 * scale / 4.0).round() as i32;
        let h = (art.height as f32 * scale / 4.0).round() as i32;
        let x = if right {
            ox + ((rect.0 + rect.2 + dx) * scale).round() as i32 - w
        } else {
            ox + ((rect.0 + dx) * scale).round() as i32
        };
        art.draw_scaled(fb, x, oy + ((rect.1 + dy) * scale).round() as i32, w, h);
    }

    /// The `PalNl` Ok/Cancel plates at their measured places.
    fn mp_okcancel_plates(&self, fb: &mut Framebuffer, ok_dx: f32, cancel_dy: f32) {
        let (scale, _, _) = self.ui_metrics(fb);
        let Some(glue) = self.glue.as_ref() else {
            return;
        };
        let px = |ui: f32| (ui * scale).round() as i32;
        if let Some(art) = glue.palnl_ok.as_ref() {
            let r = Self::mp_rect(fb, (1627.0, 1006.0, 1062.0, 214.0));
            art.draw_scaled(fb, r.0 + px(ok_dx), r.1, r.2, r.3);
        }
        if let Some(art) = glue.palnl_cancel.as_ref() {
            let r = Self::mp_rect(fb, (1734.0, 1068.0, 940.0, 273.0));
            art.draw_scaled(fb, r.0, r.1 + px(cancel_dy), r.2, r.3);
        }
    }

    /// Ok/Cancel labels and hit areas (`PALNL_*_BUTTON`).
    fn mp_okcancel(&mut self, fb: &mut Framebuffer, ok: bool, cancel: bool) {
        use galaxycraft::menu::{PALNL_CANCEL_BUTTON, PALNL_OK_BUTTON};
        self.draw_glue_button(fb, "Ok", PALNL_OK_BUTTON, ok);
        self.draw_glue_button(fb, "Cancel", PALNL_CANCEL_BUTTON, cancel);
        if ok {
            let r = Self::mp_ui(fb, PALNL_OK_BUTTON);
            self.mp_hit(r, MpHit::Ok);
        }
        if cancel {
            let r = Self::mp_ui(fb, PALNL_CANCEL_BUTTON);
            self.mp_hit(r, MpHit::Cancel);
        }
    }

    /// Draw `text` with its cap top at `top`; returns its width (content px).
    #[allow(clippy::too_many_arguments)]
    fn mp_text(
        &mut self,
        fb: &mut Framebuffer,
        text: &str,
        x: f32,
        top: f32,
        px: f32,
        face: Face,
        color: Rgb,
        align: Align,
    ) -> f32 {
        let (cx, k) = Self::mp_k(fb);
        let size = (px * k).max(6.0);
        let font = match face {
            Face::Reg => self.ui_font.as_mut(),
            Face::Ext => self.ui_font_hover.as_mut().or(self.ui_font.as_mut()),
        };
        let Some(font) = font else {
            return 0.0;
        };
        let w = font.measure(text, size, 0.0);
        let shift = match align {
            Align::Left => 0,
            Align::Center => w / 2,
            Align::Right => w,
        };
        font.draw_top(
            fb,
            cx + (x * k).round() as i32 - shift,
            (top * k).round() as i32,
            text,
            size,
            0.0,
            color,
            None,
        );
        w as f32 / k
    }

    /// Darken a rect (translucent black, as the CEF controls).
    fn mp_shade(fb: &mut Framebuffer, r: (i32, i32, i32, i32), alpha: u8) {
        for y in r.1.max(0)..(r.1 + r.3).min(fb.height) {
            for x in r.0.max(0)..(r.0 + r.2).min(fb.width) {
                let c = fb.get(x, y);
                fb.put(x, y, c.blend(Rgb::new(0, 0, 0), alpha));
            }
        }
    }

    /// 2 px border with clipped corners (the CEF `border-radius: 3px`).
    fn mp_border(fb: &mut Framebuffer, r: (i32, i32, i32, i32), color: Rgb) {
        let t = ((r.3 as f32) / 22.0).round().clamp(1.0, 3.0) as i32;
        Self::mp_border_t(fb, r, t, t + 1, color);
    }

    /// Border of thickness `t` with corners clipped by `c` px.
    fn mp_border_t(fb: &mut Framebuffer, r: (i32, i32, i32, i32), t: i32, c: i32, color: Rgb) {
        fb.rect(r.0 + c, r.1, r.2 - 2 * c, t, color);
        fb.rect(r.0 + c, r.1 + r.3 - t, r.2 - 2 * c, t, color);
        fb.rect(r.0, r.1 + c, t, r.3 - 2 * c, color);
        fb.rect(r.0 + r.2 - t, r.1 + c, t, r.3 - 2 * c, color);
        for i in 0..c {
            fb.rect(r.0 + c - i, r.1 + i + 1, t, 1, color);
            fb.rect(r.0 + r.2 - c + i - t + 1, r.1 + i + 1, t, 1, color);
            fb.rect(r.0 + c - i, r.1 + r.3 - i - 2, t, 1, color);
            fb.rect(r.0 + r.2 - c + i - t + 1, r.1 + r.3 - i - 2, t, 1, color);
        }
    }

    /// Filled triangle centred on `(cx, cy)` (screen px), pointing down or up.
    fn mp_triangle(fb: &mut Framebuffer, cx: i32, cy: i32, w: i32, h: i32, down: bool, c: Rgb) {
        for row in 0..h.max(1) {
            let t = if down { h - row } else { row + 1 };
            let half = (w * t / h.max(1)) / 2;
            fb.rect(cx - half, cy - h / 2 + row, (2 * half).max(1), 1, c);
        }
    }

    /// Thick triangle outline (the Create list's scroll arrows): pixels
    /// inside the triangle but farther than `t` from every edge stay clear.
    fn mp_triangle_ring(fb: &mut Framebuffer, r: (i32, i32, i32, i32), down: bool, t: f32, c: Rgb) {
        let (x0, y0, w, h) = (r.0 as f32, r.1 as f32, r.2 as f32, r.3 as f32);
        let (apex, base) = if down { (y0 + h, y0) } else { (y0, y0 + h) };
        let pts = [(x0, base), (x0 + w, base), (x0 + w / 2.0, apex)];
        let edge = |a: (f32, f32), b: (f32, f32), p: (f32, f32)| {
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            ((p.0 - a.0) * dy - (p.1 - a.1) * dx) / (dx * dx + dy * dy).sqrt()
        };
        for y in r.1..=r.1 + r.3 {
            for x in r.0..=r.0 + r.2 {
                let p = (x as f32 + 0.5, y as f32 + 0.5);
                let d: Vec<f32> = (0..3).map(|i| edge(pts[i], pts[(i + 1) % 3], p)).collect();
                let sign = if d.iter().sum::<f32>() >= 0.0 {
                    1.0
                } else {
                    -1.0
                };
                let inside = d.iter().all(|v| v * sign >= 0.0);
                if inside && d.iter().any(|v| v * sign < t) {
                    fb.put(x, y, c);
                }
            }
        }
    }

    /// A CEF dropdown: shaded box, red border (bright when hovered), label
    /// and arrow.  Registers the hit.
    #[allow(clippy::too_many_arguments)]
    fn mp_dropdown(
        &mut self,
        fb: &mut Framebuffer,
        rect: (f32, f32, f32, f32),
        label: &str,
        color: Rgb,
        enabled: bool,
        which: Drop,
        px: f32,
    ) {
        let r = Self::mp_rect(fb, rect);
        let hot = enabled && self.mp.hover == Some(MpHit::Combo(which));
        Self::mp_shade(fb, r, 150);
        Self::mp_border(fb, r, if hot { CEF_BORDER_HOT } else { CEF_BORDER });
        let colour = if !enabled {
            DISABLED
        } else if hot {
            CEF_HOVER
        } else {
            color
        };
        let top = rect.1 + (rect.3 - px * 0.62) / 2.0;
        self.mp_text(
            fb,
            label,
            rect.0 + 10.0,
            top,
            px,
            Face::Reg,
            colour,
            Align::Left,
        );
        let (_, k) = Self::mp_k(fb);
        let aw = (28.0 * k).round() as i32;
        let ah = (15.0 * k).round() as i32;
        let acx = r.0 + r.2 - (33.0 * k).round() as i32;
        let arrow = if !enabled {
            DISABLED
        } else if hot {
            Rgb::new(0xff, 0xff, 0xff)
        } else {
            ARROW
        };
        Self::mp_triangle(fb, acx, r.1 + r.3 / 2 + 1, aw, ah, true, arrow);
        if enabled {
            self.mp_hit(r, MpHit::Combo(which));
        }
    }

    fn mp_open(&mut self, which: Drop, rect: (f32, f32, f32, f32), items: Vec<String>) {
        self.mp.open = Some(OpenDrop { which, rect, items });
        self.play("ui_click");
    }

    /// The open dropdown's list, drawn last.
    fn mp_draw_open(&mut self, fb: &mut Framebuffer) {
        let Some(open) = self.mp.open.clone() else {
            return;
        };
        let pitch = (open.rect.3 * 0.8).max(30.0);
        let list = (
            open.rect.0,
            open.rect.1 + open.rect.3,
            open.rect.2,
            pitch * open.items.len() as f32 + 8.0,
        );
        let r = Self::mp_rect(fb, list);
        Self::mp_shade(fb, r, 235);
        Self::mp_border(fb, r, CEF_BORDER_HOT);
        for (i, item) in open.items.iter().enumerate() {
            let row = (list.0, list.1 + 4.0 + pitch * i as f32, list.2, pitch);
            let rr = Self::mp_rect(fb, row);
            let hot = self.mp.hover == Some(MpHit::Item(i));
            if hot {
                let inner = (rr.0 + 3, rr.1, rr.2 - 6, rr.3);
                fb.rect(
                    inner.0,
                    inner.1,
                    inner.2,
                    inner.3,
                    Rgb::new(0x40, 0x08, 0x04),
                );
            }
            let top = row.1 + (pitch - 24.0 * 0.62) / 2.0;
            self.mp_text(
                fb,
                item,
                row.0 + 10.0,
                top,
                24.0,
                Face::Reg,
                if hot { CEF_HOVER } else { CEF_GREEN },
                Align::Left,
            );
            self.mp_hit(rr, MpHit::Item(i));
        }
    }

    // ------------------------------------------------------------------
    // Art
    // ------------------------------------------------------------------

    fn mp_ensure_art(&mut self) -> Arc<MpArt> {
        if let Some(a) = &self.mp.art {
            return a.clone();
        }
        let art = match self.install.as_ref() {
            Some(i) => MpArt {
                conn_list: load_dds(i, "glue/selconn/plistsml.dds"),
                conn_info: load_dds(i, "glue/selconn/pinfo.dds"),
                conn_gateway: load_dds(i, "glue/battlenet/connection/pgateway.dds")
                    .or_else(|| load_dds(i, "glue/selconn/pgateway.dds")),
                games_list: load_dds(i, "glue/gamesel/plistlrg.dds"),
                games_info: load_dds(i, "glue/gamesel/pjoin.dds"),
                lobby_list: load_dds(i, "glue/chatroom/plist.dds"),
                lobby_chat: load_dds(i, "glue/chatroom/pchat.dds"),
                lobby_info: load_dds(i, "glue/chatroom/pjoin.dds"),
                icreate: load_grp(i, "glue/create/icreate.dds.grp"),
            },
            None => MpArt::default(),
        };
        let art = Arc::new(art);
        self.mp.art = Some(art.clone());
        art
    }

    fn mp_score_art(&mut self, race: Race, victory: bool) -> Arc<ScoreArt> {
        let key = (race_letter(race) as u8, victory);
        if let Some((k, a)) = &self.mp.score_art
            && *k == key
        {
            return a.clone();
        }
        let dir = format!(
            "glue/score{}{}",
            race_letter(race),
            if victory { 'v' } else { 'd' }
        );
        let art = match self.install.as_ref() {
            Some(i) => ScoreArt {
                // The scene behind the sheet is the race/result palette's
                // backdrop (`glue/palpd/backgnd.dds` for a Protoss defeat).
                backgnd: load_dds(
                    i,
                    &format!(
                        "glue/pal{}{}/backgnd.dds",
                        race_letter(race),
                        if victory { 'v' } else { 'd' }
                    ),
                ),
                pmain: load_dds(i, &format!("{dir}/pmain.dds")),
                pinset: load_dds(i, &format!("{dir}/pinset.dds")),
                scorebox: load_dds(i, &format!("{dir}/scorebox.dds")),
                iscore: load_grp(i, &format!("{dir}/iscore.dds.grp")),
            },
            None => ScoreArt::default(),
        };
        let art = Arc::new(art);
        self.mp.score_art = Some((key, art.clone()));
        art
    }

    // ------------------------------------------------------------------
    // Screens
    // ------------------------------------------------------------------

    /// Draw one of the multiplayer screens (also the outgoing one while its
    /// plates swish out).
    pub(super) fn draw_mp(&mut self, fb: &mut Framebuffer, screen: &MenuScreen) {
        self.mp.hits.clear();
        let art = self.mp_ensure_art();
        self.draw_moon_backdrop(fb);
        match screen {
            MenuScreen::Connection => self.draw_mp_connection(fb, &art),
            MenuScreen::Games => self.draw_mp_games(fb, &art),
            MenuScreen::MpCreate => self.draw_mp_create(fb, &art),
            MenuScreen::MpLobby => self.draw_mp_lobby(fb, &art),
            _ => {}
        }
        self.mp_draw_open(fb);
    }

    /// Plate offsets (left plates, right plates/Ok, Cancel) of the swish.
    fn mp_swish(&self) -> (f32, f32, f32) {
        (
            -self.swish_offset(SWISH_LIST_UI),
            self.swish_offset(SWISH_OK_UI),
            self.swish_offset(SWISH_CANCEL_UI),
        )
    }

    fn draw_mp_connection(&mut self, fb: &mut Framebuffer, art: &MpArt) {
        use galaxycraft::menu::{GLUE_LIST_ROW_PITCH, GLUE_LIST_TEXT_INSET, LabelAlign};
        let (l, r, c) = self.mp_swish();
        self.mp_plate(
            fb,
            art.conn_list.as_ref(),
            (0.0, 0.0, 342.0, 224.0),
            true,
            l,
            0.0,
        );
        // The lower plate leaves and comes back when the row changes.
        let swap = self.mp.conn_swap.map(|t| {
            use galaxycraft::menu::{swish_in_offset, swish_out_offset, swish_out_steps};
            let n = swish_step(t);
            let out = swish_out_steps(SWISH_LIST_UI);
            if n < out {
                -swish_out_offset(n)
            } else {
                -swish_in_offset(SWISH_LIST_UI, n - out)
            }
        });
        if swap == Some(0.0) {
            self.mp.conn_swap = None;
        }
        self.mp_plate(
            fb,
            art.conn_gateway.as_ref(),
            (0.0, 230.0, 340.0, 236.0),
            true,
            l + swap.unwrap_or(0.0),
            0.0,
        );
        self.mp_plate(
            fb,
            art.conn_info.as_ref(),
            (366.0, 31.0, 274.0, 336.0),
            false,
            r,
            0.0,
        );
        self.mp_okcancel_plates(fb, r, c);
        if !self.swish_text_shown() {
            return;
        }
        let (scale, ox, oy) = self.ui_metrics(fb);
        let title = "Select Connection";
        let w = self.hd_advance_ui(title, scale, true);
        self.draw_hd_text(
            fb,
            title,
            50.0,
            59.0 + GLUE_LIST_TEXT_INSET.1,
            LabelAlign::Left,
            w,
            16.0,
            scale,
            ox,
            oy,
            TITLE,
            true,
        );
        for (i, name) in mpl::CONNECTIONS.iter().enumerate() {
            let w = self.hd_advance_ui(name, scale, false);
            let top = 92.0 + GLUE_LIST_TEXT_INSET.1 + GLUE_LIST_ROW_PITCH * i as f32;
            self.draw_hd_text(
                fb,
                name,
                53.0 + GLUE_LIST_TEXT_INSET.0,
                top,
                LabelAlign::Left,
                w,
                16.0,
                scale,
                ox,
                oy,
                if i == self.mp.conn_sel {
                    ROW_SELECTED
                } else {
                    ROW
                },
                false,
            );
            let hit = Self::mp_ui(
                fb,
                (53.0, 92.0 + GLUE_LIST_ROW_PITCH * i as f32, 280.0, 19.0),
            );
            self.mp_hit(hit, MpHit::Row(i));
        }
        let ok = mpl::connection_ok_enabled(self.mp.conn_sel);
        self.mp_okcancel(fb, ok, true);
    }

    fn draw_mp_games(&mut self, fb: &mut Framebuffer, art: &MpArt) {
        use galaxycraft::menu::{GLUE_LIST_ROW_PITCH, GLUE_LIST_TEXT_INSET, LabelAlign};
        let (l, r, c) = self.mp_swish();
        self.mp_plate(
            fb,
            art.games_list.as_ref(),
            (0.0, 0.0, 362.0, 356.0),
            true,
            l,
            0.0,
        );
        self.mp_plate(
            fb,
            art.games_info.as_ref(),
            (400.0, 0.0, 240.0, 333.0),
            false,
            r,
            0.0,
        );
        self.mp_okcancel_plates(fb, r, c);
        if !self.swish_text_shown() {
            return;
        }
        let (scale, ox, oy) = self.ui_metrics(fb);
        let w = self.hd_advance_ui("Games", scale, true);
        self.draw_hd_text(
            fb,
            "Games",
            58.0,
            60.0 + GLUE_LIST_TEXT_INSET.1,
            LabelAlign::Left,
            w,
            16.0,
            scale,
            ox,
            oy,
            TITLE,
            true,
        );
        let names: Vec<String> = self.mp.games.iter().map(|g| g.1.name.clone()).collect();
        for (i, name) in names.iter().enumerate().take(11) {
            let w = self.hd_advance_ui(name, scale, false);
            let top = 92.0 + GLUE_LIST_TEXT_INSET.1 + GLUE_LIST_ROW_PITCH * i as f32;
            self.draw_hd_text(
                fb,
                name,
                64.0 + GLUE_LIST_TEXT_INSET.0,
                top,
                LabelAlign::Left,
                w,
                16.0,
                scale,
                ox,
                oy,
                if Some(i) == self.mp.games_sel {
                    ROW_SELECTED
                } else {
                    ROW
                },
                false,
            );
            let hit = Self::mp_ui(
                fb,
                (64.0, 92.0 + GLUE_LIST_ROW_PITCH * i as f32, 286.0, 19.0),
            );
            self.mp_hit(hit, MpHit::Row(i));
        }
        // Game information (measured cap tops, CTEXT centred on the plate).
        let centre = 320.0 + 507.0 * 3.0;
        for (label, top) in [
            ("Game Title:", 179.0),
            ("Game Type:", 419.0),
            ("Map Name:", 615.0),
        ] {
            self.mp_text(
                fb,
                label,
                centre,
                top,
                35.0,
                Face::Reg,
                TITLE,
                Align::Center,
            );
        }
        for (label, top) in [("Map Size:", 788.0), ("Game Speed:", 842.0)] {
            self.mp_text(fb, label, 1604.0, top, 35.0, Face::Reg, TITLE, Align::Left);
        }
        if let Some(g) = self
            .mp
            .games_sel
            .and_then(|i| self.mp.games.get(i))
            .cloned()
        {
            let a = &g.1;
            self.mp_text(
                fb,
                &a.name,
                centre,
                236.0,
                40.0,
                Face::Reg,
                VALUE,
                Align::Center,
            );
            if let Some(t) = GameType::from_tag(a.game_type) {
                self.mp_text(
                    fb,
                    t.name(),
                    centre,
                    470.0,
                    35.0,
                    Face::Reg,
                    VALUE,
                    Align::Center,
                );
            }
            self.mp_text(
                fb,
                &a.map_name,
                centre,
                668.0,
                35.0,
                Face::Reg,
                VALUE,
                Align::Center,
            );
            let size = format!("{}x{}", a.map_size.0, a.map_size.1);
            self.mp_text(
                fb,
                &size,
                1928.0,
                788.0,
                35.0,
                Face::Reg,
                VALUE,
                Align::Left,
            );
            let speed = SPEED_NAMES[(a.speed as usize).min(6)];
            self.mp_text(
                fb,
                speed,
                1928.0,
                842.0,
                35.0,
                Face::Reg,
                VALUE,
                Align::Left,
            );
        }
        let create = (44.0, 331.0, 312.0, 22.0);
        self.draw_glue_button(fb, "Create Game", create, true);
        let hit = Self::mp_ui(fb, create);
        self.mp_hit(hit, MpHit::CreateGame);
        // Retail: "Using UDP ports: 5353 and 6112."  This client's LAN
        // protocol listens on TCP 7777 and announces on UDP 7778.
        let footer = format!(
            "Using ports: {} (TCP) and {} (UDP).",
            galaxycraft::net::LOBBY_PORT,
            galaxycraft::net::DISCOVERY_PORT
        );
        self.mp_text(
            fb,
            &footer,
            328.0,
            1388.0,
            33.5,
            Face::Reg,
            TITLE,
            Align::Left,
        );
        self.mp_okcancel(fb, true, true);
    }

    /// Rows of the Create map browser for the current folder: parent, then
    /// sub-folders, then maps, each group by name (retail file browser).
    pub(super) fn mp_map_rows(&self) -> Vec<MapRow> {
        let melee: Vec<usize> = (0..self.maps.len())
            .filter(|&i| self.maps[i].campaign.is_none())
            .collect();
        let root = melee
            .iter()
            .filter_map(|&i| self.maps[i].path.parent())
            .min_by_key(|p| p.components().count())
            .map(Path::to_path_buf);
        let Some(root) = root else {
            return Vec::new();
        };
        let dir = self
            .mp
            .create_dir
            .iter()
            .fold(root.clone(), |p, d| p.join(d));
        let mut folders: Vec<String> = Vec::new();
        let mut maps: Vec<usize> = Vec::new();
        for &i in &melee {
            let Ok(rel) = self.maps[i].path.strip_prefix(&dir) else {
                continue;
            };
            let parts: Vec<_> = rel.components().collect();
            if parts.len() == 1 {
                maps.push(i);
            } else if let Some(first) = parts.first() {
                let name = first.as_os_str().to_string_lossy().into_owned();
                if !folders.contains(&name) {
                    folders.push(name);
                }
            }
        }
        folders.sort();
        maps.sort_by_key(|&i| self.mp_map_file(i).to_lowercase());
        let mut rows = Vec::new();
        if !self.mp.create_dir.is_empty() {
            rows.push(MapRow::Up);
        }
        rows.extend(folders.into_iter().map(MapRow::Folder));
        rows.extend(maps.into_iter().map(MapRow::Map));
        rows
    }

    /// The file name the browser shows: retail lists the `.scm`/`.scx`
    /// archives the cached CHKs were extracted from.
    fn mp_map_file(&self, i: usize) -> String {
        let Some(m) = self.maps.get(i) else {
            return String::new();
        };
        let stem = m
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| m.name.clone());
        let expansion = m
            .path
            .components()
            .any(|c| c.as_os_str().eq_ignore_ascii_case("broodwar"));
        format!("{stem}.{}", if expansion { "scx" } else { "scm" })
    }

    fn mp_map_info(&self, index: usize) -> Option<MapInfo> {
        let entry = self.maps.get(index)?;
        let raw = std::fs::read(&entry.path).ok()?;
        let sc = Scenario::parse(&raw).ok()?;
        let mut preview = vec![0u32; 128 * 128];
        if let Some(tiles) = Tileset::load(&self.tiles_dir, sc.era, &sc) {
            let mut fbp = Framebuffer::new(128, 128);
            tiles.draw_minimap_with_terrain(
                &mut fbp,
                (0, 0, 128, 128),
                &[],
                (0, 0, 0, 0),
                None,
                0,
                galaxycraft::render::MinimapTerrain::Tiles,
            );
            preview = fbp.pixels;
        }
        let full = sc.description().unwrap_or("").replace('\r', "");
        // "Suggested Players: 2-8" heads the description of Blizzard maps;
        // the panel shows it as a statistic and the rest as the text.
        let (suggested, description) = match full.strip_prefix("Suggested Players:") {
            Some(rest) => {
                let mut lines = rest.splitn(2, '\n');
                let s = lines.next().unwrap_or("").trim().to_string();
                (s, lines.next().unwrap_or("").trim().to_string())
            }
            None => (entry.players.to_string(), full.trim().to_string()),
        };
        let starts = sc
            .units
            .iter()
            .filter(|u| u.unit_id == uid::START_LOCATION)
            .map(|u| (u.owner, u.x, u.y))
            .collect();
        let resources = sc
            .units
            .iter()
            .filter(|u| galaxycraft::sim::is_resource_type(u.unit_id))
            .map(|u| (u.x, u.y))
            .collect();
        Some(MapInfo {
            index,
            title: sc
                .title()
                .map(str::to_string)
                .unwrap_or_else(|| entry.name.clone()),
            description,
            suggested,
            size: (sc.width, sc.height),
            tileset: tileset_name(sc.era),
            slots: entry.players.max(sc.start_locations().len()).clamp(1, 8),
            preview,
            starts,
            resources,
        })
    }

    fn mp_info(&mut self, index: usize) -> Option<MapInfo> {
        if self.mp.info.as_ref().is_none_or(|i| i.index != index) {
            self.mp.info = self.mp_map_info(index);
        }
        self.mp.info.clone()
    }

    /// The retail minimap: terrain, cyan resources, coloured start squares.
    fn mp_minimap(fb: &mut Framebuffer, info: &MapInfo, rect: (f32, f32, f32, f32)) {
        let (mw, mh) = (info.size.0.max(1) as f32, info.size.1.max(1) as f32);
        let s = (rect.2 / mw).min(rect.3 / mh);
        let (w, h) = (mw * s, mh * s);
        let inner = (
            rect.0 + (rect.2 - w) / 2.0,
            rect.1 + (rect.3 - h) / 2.0,
            w,
            h,
        );
        let r = Self::mp_rect(fb, inner);
        for y in 0..r.3 {
            let sy = (y as usize * 128 / r.3.max(1) as usize).min(127);
            for x in 0..r.2 {
                let sx = (x as usize * 128 / r.2.max(1) as usize).min(127);
                fb.put(
                    r.0 + x,
                    r.1 + y,
                    Rgb(info.preview[sy * 128 + sx] & 0x00ff_ffff),
                );
            }
        }
        let to = |px: i32, py: i32| {
            (
                r.0 + (px as f32 / (mw * 32.0) * r.2 as f32) as i32,
                r.1 + (py as f32 / (mh * 32.0) * r.3 as f32) as i32,
            )
        };
        let dot = (r.2 as f32 / 100.0).ceil() as i32;
        for &(x, y) in &info.resources {
            let (sx, sy) = to(x, y);
            fb.rect(
                sx - dot,
                sy - dot / 2,
                dot * 2,
                dot,
                Rgb::new(0x00, 0xe4, 0xfc),
            );
        }
        for &(owner, x, y) in &info.starts {
            let (sx, sy) = to(x, y);
            let c = galaxycraft::render::PLAYER_COLORS
                [owner as usize % galaxycraft::render::PLAYER_COLORS.len()];
            fb.rect(sx - dot * 2, sy - dot * 2, dot * 4, dot * 3, c);
        }
    }

    fn draw_mp_create(&mut self, fb: &mut Framebuffer, _art: &MpArt) {
        let (l, r, c) = self.mp_swish();
        let (create_map, create_info) = match self.glue.as_ref() {
            Some(g) => (g.create_map.clone(), g.create_info.clone()),
            None => (None, None),
        };
        self.mp_plate(
            fb,
            create_map.as_ref(),
            (0.0, 0.0, 362.0, 356.0),
            true,
            l,
            0.0,
        );
        self.mp_plate(
            fb,
            create_info.as_ref(),
            (400.0, 0.0, 240.0, 333.0),
            false,
            r,
            0.0,
        );
        self.mp_okcancel_plates(fb, r, c);
        if !self.swish_text_shown() {
            return;
        }
        let icons = self.mp.art.clone().unwrap_or_default();
        let icon = |fb: &mut Framebuffer, frame: usize, x: f32, y: f32| {
            if let Some(img) = icons.icreate.get(frame) {
                let r = Self::mp_rect(
                    fb,
                    (x, y, img.width as f32 * 0.75, img.height as f32 * 0.75),
                );
                img.draw_scaled(fb, r.0, r.1, r.2, r.3);
            }
        };
        self.mp_text(
            fb,
            "Create",
            442.0,
            220.0,
            48.0,
            Face::Ext,
            TITLE,
            Align::Left,
        );
        icon(fb, 2, 492.0, 318.0);
        let folder = self
            .mp
            .create_dir
            .last()
            .cloned()
            .unwrap_or_else(|| "Maps".to_string());
        self.mp_text(
            fb,
            &folder,
            541.0,
            324.0,
            48.0,
            Face::Reg,
            TITLE,
            Align::Left,
        );
        // Map list: 57 px rows from y 400, icons at x 505, names at x 555.
        let rows = self.mp_map_rows();
        self.mp.create_top = self.mp.create_top.min(rows.len().saturating_sub(MAP_ROWS));
        for (n, row) in rows
            .iter()
            .enumerate()
            .skip(self.mp.create_top)
            .take(MAP_ROWS)
        {
            let i = n - self.mp.create_top;
            let y = 400.0 + 57.0 * i as f32;
            let (frame, label) = match row {
                MapRow::Up => (0, "[Up One Level]".to_string()),
                MapRow::Folder(f) => (0, format!("[{f}]")),
                MapRow::Map(m) => {
                    let sel = Some(*m) == self.mp.create_map;
                    (if sel { 4 } else { 3 }, self.mp_map_file(*m))
                }
            };
            let selected = matches!(row, MapRow::Map(m) if Some(*m) == self.mp.create_map);
            icon(fb, frame, 505.0, y);
            self.mp_text(
                fb,
                &label,
                555.0,
                y + 6.0,
                48.0,
                Face::Reg,
                if selected { ROW_SELECTED } else { ROW },
                Align::Left,
            );
            let hit = Self::mp_rect(fb, (498.0, y, 800.0, 57.0));
            self.mp_hit(hit, MpHit::Row(n));
        }
        // Scroll bar (measured on 20.5 s): thick outlined arrows above and
        // below a dark-red track, the thumb a thick rounded square on it.
        let (_, k) = Self::mp_k(fb);
        let red = Rgb::new(0xd2, 0x08, 0x0a);
        let up = Self::mp_rect(fb, (1315.0, 391.0, 46.0, 46.0));
        Self::mp_triangle_ring(fb, up, false, 5.0 * k, red);
        self.mp_hit(up, MpHit::ScrollUp);
        let down = Self::mp_rect(fb, (1315.0, 690.0, 46.0, 46.0));
        Self::mp_triangle_ring(fb, down, true, 5.0 * k, red);
        self.mp_hit(down, MpHit::ScrollDown);
        let track = Self::mp_rect(fb, (1315.0, 444.0, 46.0, 238.0));
        let t2 = (2.0 * k).round().max(1.0) as i32;
        Self::mp_border_t(fb, track, t2, 0, Rgb::new(0x6c, 0x04, 0x06));
        let span = rows.len().saturating_sub(MAP_ROWS) as f32;
        let at = if span > 0.0 {
            self.mp.create_top as f32 / span
        } else {
            0.0
        };
        let thumb = Self::mp_rect(fb, (1318.0, 444.0 + at * 198.0, 41.0, 40.0));
        let t4 = (4.0 * k).round().max(1.0) as i32;
        Self::mp_border_t(fb, thumb, t4, (9.0 * k) as i32, red);
        // Game options.
        let gt = self.mp.game_type.unwrap_or(GameType::TopVsBottom);
        self.mp_text(
            fb,
            "Game Type",
            494.0,
            795.0,
            35.0,
            Face::Reg,
            TITLE,
            Align::Left,
        );
        self.mp_dropdown(
            fb,
            (860.0, 786.0, 514.0, 44.0),
            gt.name(),
            CEF_GREEN,
            true,
            Drop::GameType,
            35.0,
        );
        let info = self.mp.create_map.and_then(|m| self.mp_info(m));
        if gt == GameType::TopVsBottom {
            let slots = info.as_ref().map(|i| i.slots).unwrap_or(2);
            let split = self.mp.split.unwrap_or_else(|| mpl::default_split(slots));
            self.mp_text(
                fb,
                "Teams:",
                494.0,
                861.0,
                35.0,
                Face::Reg,
                TITLE,
                Align::Left,
            );
            self.mp_dropdown(
                fb,
                (860.0, 852.0, 514.0, 44.0),
                &format!("{} vs {}", split.0, split.1),
                CEF_GREEN,
                true,
                Drop::Teams,
                35.0,
            );
        }
        self.mp_text(
            fb,
            "Speed:",
            494.0,
            927.0,
            35.0,
            Face::Reg,
            TITLE,
            Align::Left,
        );
        let speed = self.mp.speed_index();
        self.mp_text(
            fb,
            SPEED_NAMES[speed],
            726.0,
            927.0,
            35.0,
            Face::Reg,
            TITLE,
            Align::Left,
        );
        self.mp_text(fb, "-", 870.0, 927.0, 35.0, Face::Reg, TITLE, Align::Left);
        self.mp_text(fb, "+", 1353.0, 927.0, 35.0, Face::Reg, TITLE, Align::Left);
        let track = Self::mp_rect(fb, (888.0, 934.0, 440.0, 3.0));
        fb.rect(
            track.0,
            track.1,
            track.2,
            track.3,
            Rgb::new(0x49, 0x0a, 0x13),
        );
        for t in 0..7 {
            let tick = Self::mp_rect(fb, (888.0 + 72.0 * t as f32, 932.0, 4.0, 7.0));
            fb.rect(tick.0, tick.1, tick.2, tick.3, Rgb::new(0x60, 0x10, 0x18));
        }
        let knob = Self::mp_rect(fb, (1319.0 - 72.0 * (6 - speed) as f32, 928.0, 18.0, 18.0));
        fb.circle_outline(knob.0 + knob.2 / 2, knob.1 + knob.3 / 2, knob.2 / 2, ARROW);
        for rr in 0..knob.2 / 2 {
            fb.circle_outline(
                knob.0 + knob.2 / 2,
                knob.1 + knob.3 / 2,
                rr,
                Rgb::new(0xf5, 0xf4, 0x38),
            );
        }
        let minus = Self::mp_rect(fb, (850.0, 912.0, 40.0, 50.0));
        self.mp_hit(minus, MpHit::SpeedDown);
        let plus = Self::mp_rect(fb, (1340.0, 912.0, 40.0, 50.0));
        self.mp_hit(plus, MpHit::SpeedUp);
        // Map information plate.
        if let Some(info) = &info {
            Self::mp_minimap(fb, info, (1686.0, 184.0, 314.0, 314.0));
            self.mp_text(
                fb,
                &info.title,
                1843.0,
                520.0,
                30.0,
                Face::Ext,
                VALUE,
                Align::Center,
            );
            for (i, line) in wrap_text(&info.description, 40).iter().enumerate().take(8) {
                self.mp_text(
                    fb,
                    line,
                    1552.0,
                    571.0 + 34.0 * i as f32,
                    30.0,
                    Face::Reg,
                    CEF_INFO,
                    Align::Left,
                );
            }
            let size = format!("{}x{}", info.size.0, info.size.1);
            for (i, (label, value)) in [
                ("Size:", size.as_str()),
                ("Tileset:", info.tileset),
                ("Suggested Players:", info.suggested.as_str()),
            ]
            .iter()
            .enumerate()
            {
                let top = 890.0 + 30.0 * i as f32;
                self.mp_text(
                    fb,
                    label,
                    1829.0,
                    top,
                    24.0,
                    Face::Reg,
                    CEF_INFO,
                    Align::Right,
                );
                self.mp_text(fb, value, 1853.0, top, 24.0, Face::Reg, VALUE, Align::Left);
            }
        }
        let ok = self.mp.create_map.is_some();
        self.mp_okcancel(fb, ok, true);
        if self.mp.creating.is_some() {
            // The popup is modal: only its Cancel answers.
            self.mp.hits.clear();
            self.draw_mp_creating(fb);
        }
    }

    /// The "Creating game..." popup (22.3 s): black box, red hairline,
    /// a centred title and a Cancel button.
    fn draw_mp_creating(&mut self, fb: &mut Framebuffer) {
        let r = Self::mp_rect(fb, (752.0, 427.0, 1061.0, 581.0));
        fb.rect(r.0, r.1, r.2, r.3, Rgb::new(0, 0, 0));
        Self::mp_border(fb, (r.0, r.1, r.2, r.3), CEF_BORDER);
        self.mp_text(
            fb,
            "Creating game...",
            1280.0,
            466.0,
            48.0,
            Face::Reg,
            TITLE,
            Align::Center,
        );
        let b = Self::mp_rect(fb, (1045.0, 885.0, 472.0, 86.0));
        Self::mp_border(fb, b, CEF_BORDER_HOT);
        self.mp_text(
            fb,
            "Cancel",
            1280.0,
            910.0,
            48.0,
            Face::Ext,
            ROW,
            Align::Center,
        );
        self.mp_hit(b, MpHit::PopupCancel);
    }

    /// Offset of the lobby plates: the menu swish, or the launch swish out
    /// after the countdown.
    fn mp_lobby_offset(&self, distance: f32) -> f32 {
        match self.mp.launching {
            Some(t) => galaxycraft::menu::swish_out_offset(swish_step(t)).min(distance * 4.0),
            None => self.swish_offset(distance),
        }
    }

    fn draw_mp_lobby(&mut self, fb: &mut Framebuffer, art: &MpArt) {
        let l = -self.mp_lobby_offset(SWISH_LIST_UI);
        let r = self.mp_lobby_offset(SWISH_OK_UI);
        let c = self.mp_lobby_offset(SWISH_CANCEL_UI);
        if let Some(t) = self.mp.launching {
            use galaxycraft::menu::swish_out_steps;
            if swish_step(t) >= swish_out_steps(SWISH_LIST_UI) {
                // Everything is out: black until the game shows.
                let (cx, _, cw, ch) = Self::content_rect(fb);
                fb.rect(cx, 0, cw, ch, Rgb::new(0, 0, 0));
                return;
            }
        }
        self.mp_plate(
            fb,
            art.lobby_chat.as_ref(),
            (0.0, 308.0, 372.0, 169.0),
            true,
            l,
            0.0,
        );
        self.mp_plate(
            fb,
            art.lobby_list.as_ref(),
            (0.0, 0.0, 372.0, 323.0),
            true,
            l,
            0.0,
        );
        self.mp_plate(
            fb,
            art.lobby_info.as_ref(),
            (400.0, 0.0, 240.0, 333.0),
            false,
            r,
            0.0,
        );
        self.mp_okcancel_plates(fb, r, c);
        if !self.swish_text_shown() || self.mp.launching.is_some() {
            return;
        }
        let Some(lobby) = self.mp.lobby.clone() else {
            // A joiner before the host's first update.
            self.mp_text(
                fb,
                "Joining game...",
                470.0,
                150.0,
                24.0,
                Face::Reg,
                CEF_GREEN,
                Align::Left,
            );
            self.mp_okcancel(fb, false, true);
            return;
        };
        let elapsed = self.mp.countdown.map(|t| t.elapsed().as_secs_f32());
        let editable = elapsed.is_none_or(countdown::dropdowns_enabled);
        let is_host = self.mp.client.is_none();
        let local = self.mp.client.as_ref().and_then(|c| c.you).unwrap_or(0);
        // Team headers and slot rows: 50.67 px pitch, 44 px controls.
        let mut y = 96.0;
        for (name, range) in lobby.teams() {
            if !name.is_empty() {
                let h = Self::mp_rect(fb, (408.0, y, 464.0, 38.0));
                Self::mp_shade(fb, h, 120);
                Self::mp_border(fb, h, CEF_BORDER);
                self.mp_text(
                    fb,
                    name,
                    419.0,
                    y + 11.0,
                    24.0,
                    Face::Reg,
                    CEF_GREEN,
                    Align::Left,
                );
                y += 46.0;
            }
            for i in range {
                let slot = &lobby.slots[i];
                let me = matches!(slot.occupant, Occupant::Human { peer, .. } if peer == local);
                let human = matches!(slot.occupant, Occupant::Human { .. });
                self.mp_dropdown(
                    fb,
                    (470.0, y, 402.0, 44.0),
                    slot.occupant.label(),
                    CEF_GREEN,
                    editable && (is_host || me) && (!human || me),
                    Drop::Player(i),
                    24.0,
                );
                if slot.occupant.plays() {
                    let race_colour = if slot.race == RaceChoice::Unselected {
                        DISABLED
                    } else {
                        CEF_GREEN
                    };
                    self.mp_dropdown(
                        fb,
                        (892.0, y, 202.0, 44.0),
                        slot.race.label(),
                        race_colour,
                        editable && (me || is_host && slot.occupant == Occupant::Computer),
                        Drop::Race(i),
                        24.0,
                    );
                    let colour = mpl::COLOR_CHOICES[slot.color.min(mpl::COLOR_CHOICES.len() - 1)];
                    self.mp_dropdown(
                        fb,
                        (1114.0, y, 202.0, 44.0),
                        colour,
                        VALUE,
                        editable && (me || is_host && slot.occupant == Occupant::Computer),
                        Drop::Color(i),
                        24.0,
                    );
                }
                y += 50.67;
            }
            y += 1.0;
        }
        // Observers: header at y 802, two columns of two seats.
        let h = Self::mp_rect(fb, (408.0, 802.0, 124.0, 38.0));
        Self::mp_shade(fb, h, 120);
        Self::mp_border(fb, h, CEF_BORDER);
        self.mp_text(
            fb,
            "Observers",
            419.0,
            812.0,
            24.0,
            Face::Reg,
            CEF_GREEN,
            Align::Left,
        );
        for (i, seat) in lobby.observers.iter().enumerate() {
            let x = if i % 2 == 0 { 470.0 } else { 974.0 };
            let y = if i < 2 { 846.0 } else { 898.0 };
            self.mp_dropdown(
                fb,
                (x, y, 332.0, 44.0),
                seat.label(),
                CEF_GREEN,
                editable && is_host,
                Drop::Observer(i),
                24.0,
            );
        }
        // Chat log and input line.
        let log: Vec<String> = self.mp.chat.iter().rev().take(8).rev().cloned().collect();
        for (i, line) in log.iter().enumerate() {
            self.mp_text(
                fb,
                line,
                396.0,
                1018.0 + 34.0 * i as f32,
                24.0,
                Face::Reg,
                VALUE,
                Align::Left,
            );
        }
        let input = format!("{}_", self.mp.chat_input);
        self.mp_text(
            fb,
            &input,
            372.0,
            1392.0,
            24.0,
            Face::Reg,
            VALUE,
            Align::Left,
        );
        // Map information panel.
        if let Some(info) = self.mp_info(self.mp.lobby_map) {
            Self::mp_minimap(fb, &info, (1674.0, 170.0, 312.0, 314.0));
        }
        let speed = SPEED_NAMES[self.mp.speed_index()];
        for (label, value, top) in [
            ("Game Name", lobby.game_name.as_str(), 506.0),
            ("Map Name", lobby.map_name.as_str(), 575.0),
            ("Game Type", lobby.game_type.name(), 644.0),
        ] {
            self.mp_text(
                fb,
                label,
                1830.0,
                top,
                24.0,
                Face::Reg,
                CEF_INFO,
                Align::Center,
            );
            self.mp_text(
                fb,
                value,
                1830.0,
                top + 27.0,
                27.0,
                Face::Ext,
                VALUE,
                Align::Center,
            );
        }
        let size = self
            .mp
            .info
            .as_ref()
            .map(|i| format!("{}x{}", i.size.0, i.size.1))
            .unwrap_or_default();
        for (i, (label, value)) in [
            ("Size:", size.as_str()),
            ("Speed:", speed),
            ("Turn Rate:", "24"),
            ("Custom Hotkeys:", "Enabled"),
            ("Unit Limits:", "Extended"),
        ]
        .iter()
        .enumerate()
        {
            let top = 725.0 + 30.0 * i as f32;
            self.mp_text(
                fb,
                label,
                1821.0,
                top,
                24.0,
                Face::Reg,
                CEF_INFO,
                Align::Right,
            );
            self.mp_text(fb, value, 1840.0, top, 24.0, Face::Reg, VALUE, Align::Left);
        }
        if let Some(e) = elapsed {
            let text = countdown::text(e);
            self.mp_text(
                fb,
                &text,
                1542.0,
                934.0,
                24.0,
                Face::Reg,
                VALUE,
                Align::Left,
            );
        }
        // Only the host starts the game.
        let ok = is_host && elapsed.is_none() && lobby.can_start();
        let cancel = elapsed.is_none_or(countdown::cancel_enabled);
        self.mp_okcancel(fb, ok, cancel);
    }

    // ------------------------------------------------------------------
    // Flow
    // ------------------------------------------------------------------

    /// Open the Games list and start listening for LAN games.
    pub(super) fn mp_enter_games(&mut self) {
        self.mp_leave_lobby();
        self.menu_screen = MenuScreen::Games;
        self.mp.games.clear();
        self.mp.games_sel = None;
        if self.mp.discovery.is_none() {
            let sock = galaxycraft::net::discovery_socket(galaxycraft::net::DISCOVERY_PORT)
                .or_else(|_| galaxycraft::net::discovery_socket(0));
            if let Ok(s) = sock {
                let _ = s.set_nonblocking(true);
                self.mp.discovery = Some(s);
            }
        }
    }

    fn mp_enter_create(&mut self) {
        self.mp.discovery = None;
        self.menu_screen = MenuScreen::MpCreate;
        self.mp.open = None;
        self.mp.creating = None;
        self.mp.create_dir.clear();
        // Start on the current map when it is a melee map, else the first.
        let current = Some(self.map_index)
            .filter(|&i| self.maps.get(i).is_some_and(|m| m.campaign.is_none()));
        let rows = self.mp_map_rows();
        let first = rows.iter().find_map(|r| match r {
            MapRow::Map(m) => Some(*m),
            _ => None,
        });
        self.mp.create_map = current
            .filter(|c| rows.contains(&MapRow::Map(*c)))
            .or(first);
        self.mp_scroll_to_selection();
    }

    /// Keep the selected map visible (retail shows it on the last row).
    fn mp_scroll_to_selection(&mut self) {
        let rows = self.mp_map_rows();
        if let Some(pos) = rows
            .iter()
            .position(|r| matches!(r, MapRow::Map(m) if Some(*m) == self.mp.create_map))
            && (pos < self.mp.create_top || pos >= self.mp.create_top + MAP_ROWS)
        {
            self.mp.create_top = (pos + 1).saturating_sub(MAP_ROWS);
        }
    }

    /// Create pressed Ok: build the lobby, start hosting, open the lobby.
    fn mp_open_lobby(&mut self) {
        let Some(index) = self.mp.create_map else {
            return;
        };
        let Some(info) = self.mp_info(index) else {
            return;
        };
        let gt = self.mp.game_type.unwrap_or(GameType::TopVsBottom);
        let split = self
            .mp
            .split
            .unwrap_or_else(|| mpl::default_split(info.slots));
        let host = self.active_name();
        let host = if host.is_empty() {
            "Player".to_string()
        } else {
            host
        };
        self.mp.lobby = Some(Lobby::new_host(&host, &info.title, info.slots, gt, split));
        self.mp.lobby_map = index;
        self.mp.countdown = None;
        self.mp.launching = None;
        self.mp.chat.clear();
        self.mp.chat_input.clear();
        self.map_index = index;
        self.lan.map_index = index;
        self.mp_host();
        self.menu_screen = MenuScreen::MpLobby;
    }

    /// Bind the LAN lobby so other players can join (best effort: a game
    /// with computers still works when the port is taken).
    fn mp_host(&mut self) {
        self.lan.listener = None;
        self.lan.queue = None;
        let Some(entry) = self.maps.get(self.mp.lobby_map).cloned() else {
            return;
        };
        let Ok(raw) = std::fs::read(&entry.path) else {
            return;
        };
        let mut map_hash = [0u8; 32];
        map_hash.copy_from_slice(&Sha256::digest(&raw));
        let addr = format!("0.0.0.0:{}", galaxycraft::net::LOBBY_PORT);
        if let Ok(l) = galaxycraft::net::Lobby::bind(&addr, map_hash, galaxycraft::net::WORLD_SEED)
        {
            let open = self
                .mp
                .lobby
                .as_ref()
                .map(|l| l.slots.len())
                .unwrap_or(2)
                .clamp(2, 8);
            self.lan.queue = Some(galaxycraft::net::JoinQueue::new(open as u8));
            self.lan.listener = Some(l);
            self.lan.announce_sock = announcer().ok();
            self.lan.last_announce = Instant::now() - Duration::from_secs(5);
        }
    }

    fn mp_leave_lobby(&mut self) {
        if let Some(mut c) = self.mp.client.take() {
            let _ = c.link.send(&Msg::Bye);
        }
        self.mp.lobby_sent = None;
        self.mp.lobby = None;
        self.mp.countdown = None;
        self.mp.launching = None;
        self.lan.listener = None;
        self.lan.queue = None;
        self.lan.announce_sock = None;
    }

    /// Per-frame work of the multiplayer screens: discovery, hosting,
    /// popup and countdown timers.
    fn mp_tick(&mut self) {
        if let Some(sock) = self.mp.discovery.as_ref() {
            let mut buf = [0u8; 1500];
            while let Ok((n, from)) = sock.recv_from(&mut buf) {
                if let Some(a) = Announce::decode(&buf[..n]) {
                    galaxycraft::net::upsert_game(&mut self.mp.games, from, a, Instant::now());
                }
            }
            let before = self.mp.games.len();
            self.mp.games.retain(|g| g.2.elapsed() < GAME_TIMEOUT);
            if self.mp.games.len() != before {
                self.mp.games_sel = None;
            }
        }
        if let Some(t) = self.mp.creating
            && t.elapsed().as_secs_f32() >= CREATING_SECS
        {
            self.mp.creating = None;
            self.mp_open_lobby();
        }
        if matches!(self.menu_screen, MenuScreen::MpLobby) {
            if self.mp.client.is_some() {
                self.mp_poll_client();
                // The joiner runs the host's countdown and launch swish,
                // then stays black until the host's `Setup` arrives.
                if let Some(t) = self.mp.countdown
                    && countdown::finished(t.elapsed().as_secs_f32())
                {
                    self.mp.countdown = None;
                    self.mp.launching = Some(Instant::now());
                    self.mp.open = None;
                }
                return;
            }
            self.mp_poll_host();
            if let Some(t) = self.mp.countdown
                && countdown::finished(t.elapsed().as_secs_f32())
            {
                self.mp.countdown = None;
                self.mp.launching = Some(Instant::now());
                self.mp.open = None;
            }
            if let Some(t) = self.mp.launching
                && t.elapsed().as_secs_f32() >= LAUNCH_SECS
            {
                self.mp.launching = None;
                self.mp_start_game();
            }
        }
    }

    /// Hosting: announce the game every second and seat joining players in
    /// the first Open slot.
    fn mp_poll_host(&mut self) {
        let Some(lobby_state) = self.mp.lobby.clone() else {
            return;
        };
        let (Some(listener), Some(queue)) = (self.lan.listener.as_ref(), self.lan.queue.as_mut())
        else {
            return;
        };
        if self.lan.last_announce.elapsed() >= Duration::from_secs(1) {
            if let Some(sock) = self.lan.announce_sock.as_ref() {
                let size = self.mp.info.as_ref().map(|i| i.size).unwrap_or((0, 0));
                let a = Announce {
                    name: lobby_state.game_name.clone(),
                    map_hash: listener.map_hash,
                    port: galaxycraft::net::LOBBY_PORT,
                    players: lobby_state
                        .slots
                        .iter()
                        .filter(|s| s.occupant.plays())
                        .count() as u8,
                    started: self.mp.launching.is_some(),
                    game_type: lobby_state.game_type.tag(),
                    map_name: lobby_state.map_name.clone(),
                    map_size: (size.0 as u16, size.1 as u16),
                    speed: self.mp.speed_index() as u8,
                };
                let _ = broadcast(sock, &a);
            }
            self.lan.last_announce = Instant::now();
        }
        // Joined players: race picks, chat, departures.
        let (msgs, mut gone) = queue.poll_lobby();
        let mut changed = false;
        for (peer, msg) in msgs {
            match msg {
                Msg::LobbyRace { race } => {
                    if let (Some(lobby), Some(r)) =
                        (self.mp.lobby.as_mut(), mpl::race_choice_from(race))
                        && let Some(slot) = lobby.slot_of_peer(peer)
                        && r != RaceChoice::Unselected
                    {
                        lobby.slots[slot].race = r;
                        changed = true;
                    }
                }
                Msg::LobbyColor { color } => {
                    if let Some(lobby) = self.mp.lobby.as_mut()
                        && let Some(slot) = lobby.slot_of_peer(peer)
                        && lobby.free_colors(slot).contains(&(color as usize))
                    {
                        lobby.slots[slot].color = color as usize;
                        changed = true;
                    }
                }
                Msg::Chat { text, .. } => {
                    let name = queue.name(peer).unwrap_or("Player").to_string();
                    let line = format!("{name}: {text}");
                    queue.send_all(&Msg::Chat {
                        from: peer,
                        text: line.clone(),
                    });
                    self.mp.chat.push(line);
                }
                Msg::Bye => gone.push(peer),
                _ => {}
            }
        }
        gone.sort_unstable();
        gone.dedup();
        for &peer in gone.iter().rev() {
            let name = queue.name(peer).unwrap_or("Player").to_string();
            queue.remove(peer);
            if let Some(lobby) = self.mp.lobby.as_mut() {
                lobby.remove_peer(peer);
            }
            self.mp.chat.push(format!("{name} has left the game."));
            changed = true;
        }
        let open = self
            .mp
            .lobby
            .as_ref()
            .is_some_and(|l| l.first_open().is_some());
        if open
            && self.mp.countdown.is_none()
            && let Ok(Some(_)) = listener.poll_join(queue, false)
        {
            let peer = queue.players() - 1;
            let name = queue
                .name(peer)
                .filter(|n| !n.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| format!("Player {}", peer + 1));
            if let Some(lobby) = self.mp.lobby.as_mut()
                && let Some(slot) = lobby.first_open()
            {
                lobby.slots[slot].occupant = Occupant::Human {
                    name: name.clone(),
                    peer,
                };
                lobby.slots[slot].race = RaceChoice::Random;
            }
            self.mp.chat.push(format!("{name} has joined the game."));
            self.audio.play_named(&self.sounds, "ui_click");
            changed = true;
        }
        // Keep joined players' view current (on change, else every second).
        if let Some(lobby) = self.mp.lobby.as_ref() {
            let bytes = lobby.encode();
            let due = self.mp.lobby_sent.as_ref().is_none_or(|(t, b)| {
                changed || *b != bytes || t.elapsed() >= Duration::from_secs(1)
            });
            if due {
                let failed = queue.send_lobby(lobby);
                self.mp.lobby_sent = Some((Instant::now(), bytes));
                for &peer in failed.iter().rev() {
                    queue.remove(peer);
                    if let Some(l) = self.mp.lobby.as_mut() {
                        l.remove_peer(peer);
                    }
                }
            }
        }
    }

    /// Client in a host's lobby: apply `LobbyState`, start on `Setup`.
    fn mp_poll_client(&mut self) {
        loop {
            let Some(c) = self.mp.client.as_mut() else {
                return;
            };
            match c.link.try_recv() {
                Ok(Some(Msg::LobbyState { you, lobby })) => {
                    c.you = Some(you);
                    self.mp.lobby = Some(lobby);
                }
                Ok(Some(Msg::Chat { text, .. })) => self.mp.chat.push(text),
                Ok(Some(Msg::LobbyCountdown { on })) => {
                    self.mp.countdown = on.then(Instant::now);
                    self.mp.open = None;
                }
                Ok(Some(Msg::Setup {
                    races, you, setup, ..
                })) => {
                    let Some(c) = self.mp.client.take() else {
                        return;
                    };
                    let index = self.mp.lobby_map;
                    let speed = SPEED_MS[6] / SPEED_MS[(c.speed as usize).min(6)];
                    self.mp.lobby = None;
                    if self.start_net_match(
                        NetRole::Client,
                        vec![c.link],
                        races,
                        index,
                        you,
                        c.map_hash,
                        None,
                        c.addr.to_string(),
                        setup,
                    ) {
                        self.speed = speed;
                    }
                    return;
                }
                Ok(Some(_)) => {}
                Ok(None) => return,
                Err(e) => {
                    self.status(format!("Lost the host: {e}"));
                    self.mp.client = None;
                    self.mp_enter_games();
                    return;
                }
            }
        }
    }

    /// The countdown ended: build the world from the lobby (and start the
    /// LAN match when other players joined).
    fn mp_start_game(&mut self) {
        let Some(lobby) = self.mp.lobby.clone() else {
            return;
        };
        let setup = lobby.resolve(galaxycraft::net::WORLD_SEED);
        let index = self.mp.lobby_map;
        let speed = SPEED_MS[6] / SPEED_MS[self.mp.speed_index()];
        let humans = setup
            .slots
            .iter()
            .filter(|s| {
                matches!(
                    s.occupant,
                    galaxycraft::multiplayer::SetupOccupant::Human { .. }
                )
            })
            .count();
        self.mp.discovery = None;
        if humans > 1
            && let (Some(listener), Some(queue)) = (self.lan.listener.take(), self.lan.queue.take())
        {
            self.lan.announce_sock = None;
            match listener.start_with_setup(queue, &setup) {
                Ok((clients, races)) => {
                    let hash = listener.map_hash;
                    if self.start_net_match(
                        NetRole::Host,
                        clients,
                        races,
                        index,
                        0,
                        hash,
                        Some(listener),
                        String::new(),
                        Some(setup),
                    ) {
                        self.speed = speed;
                        self.mp.lobby = None;
                    }
                }
                Err(e) => self.status(format!("start failed: {e}")),
            }
            return;
        }
        self.mp_leave_lobby();
        let Some(entry) = self.maps.get(index).cloned() else {
            return;
        };
        let Ok(raw) = std::fs::read(&entry.path) else {
            return;
        };
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&Sha256::digest(&raw));
        let save = SaveFile {
            map_name: entry.name.clone(),
            map_chk: raw,
            races: setup.races(),
            local_player: setup.slot_of_peer(0).unwrap_or(0),
            tick: 0,
            rec: Recording::new(galaxycraft::net::WORLD_SEED, hash),
            setup: Some(setup),
        };
        if self.build_from_save(&save) {
            self.world.recorder = Some(Recorder::new());
            self.map_index = index;
            self.replay = None;
            self.speed = speed;
        }
    }

    // ------------------------------------------------------------------
    // Input
    // ------------------------------------------------------------------

    pub(super) fn handle_mp_input(&mut self, input: &MenuInput, screen: &MenuScreen) {
        self.mp_tick();
        let hit = input.mouse.and_then(|(x, y)| {
            self.mp
                .hits
                .iter()
                .rev()
                .find(|(r, _)| inside(*r, x, y))
                .map(|(_, h)| *h)
        });
        if hit != self.mp.hover {
            self.mp.hover = hit;
            if hit.is_some() {
                self.play("ui_move");
            }
        }
        let clicked = input.mouse_down && !self.mouse_was_down;
        self.mouse_was_down = input.mouse_down;
        if input.wheel != 0 && matches!(screen, MenuScreen::MpCreate) {
            self.mp_scroll(input.wheel);
        }
        if clicked {
            if let Some(open) = self.mp.open.take() {
                if let Some(MpHit::Item(i)) = hit {
                    self.mp_pick(open.which, i);
                }
                return;
            }
            if let Some(h) = hit {
                self.mp_click(screen, h);
            }
        }
        for key in input.keys.iter().copied() {
            self.mp_key(screen, key);
        }
    }

    fn mp_scroll(&mut self, delta: i32) {
        let rows = self.mp_map_rows().len();
        let top = self.mp.create_top as i32 + delta;
        self.mp.create_top = top.clamp(0, rows.saturating_sub(MAP_ROWS) as i32) as usize;
    }

    fn mp_ok(&mut self, screen: &MenuScreen) {
        match screen {
            MenuScreen::Connection => {
                if mpl::connection_ok_enabled(self.mp.conn_sel) {
                    self.mp.registry_next = true;
                    self.profile_sel = self.active_index().unwrap_or(0);
                    self.menu_screen = MenuScreen::Registry;
                    self.play("ui_click");
                }
            }
            MenuScreen::Games => {
                if let Some(i) = self.mp.games_sel {
                    self.play("ui_click");
                    self.mp_join(i);
                }
            }
            MenuScreen::MpCreate => {
                if self.mp.create_map.is_some() && self.mp.creating.is_none() {
                    self.play("ui_click");
                    self.mp.creating = Some(Instant::now());
                }
            }
            MenuScreen::MpLobby => {
                let ready = self.mp.client.is_none()
                    && self.mp.lobby.as_ref().is_some_and(|l| l.can_start());
                if ready && self.mp.countdown.is_none() {
                    self.play("ui_click");
                    self.mp.countdown = Some(Instant::now());
                    self.mp.open = None;
                    if let Some(q) = self.lan.queue.as_mut() {
                        q.send_all(&Msg::LobbyCountdown { on: true });
                    }
                }
            }
            _ => {}
        }
    }

    fn mp_cancel(&mut self, screen: &MenuScreen) {
        self.play("ui_click");
        match screen {
            MenuScreen::Connection => self.menu_screen = MenuScreen::Main,
            MenuScreen::Games => {
                self.mp.discovery = None;
                self.mp.enter_connection();
                self.menu_screen = MenuScreen::Connection;
            }
            MenuScreen::MpCreate => {
                if self.mp.creating.take().is_none() {
                    self.mp_enter_games();
                }
            }
            MenuScreen::MpLobby => match self.mp.countdown.map(|t| t.elapsed().as_secs_f32()) {
                // A joiner cannot stop the host's countdown.
                Some(_) if self.mp.client.is_some() => {}
                Some(e) if countdown::cancel_enabled(e) => {
                    self.mp.countdown = None;
                    if let Some(q) = self.lan.queue.as_mut() {
                        q.send_all(&Msg::LobbyCountdown { on: false });
                    }
                }
                Some(_) => {}
                None => self.mp_enter_games(),
            },
            _ => {}
        }
    }

    fn mp_click(&mut self, screen: &MenuScreen, hit: MpHit) {
        match hit {
            MpHit::Ok => self.mp_ok(screen),
            MpHit::Cancel | MpHit::PopupCancel => self.mp_cancel(screen),
            MpHit::Row(i) => match screen {
                MenuScreen::Connection => {
                    if i != self.mp.conn_sel {
                        self.mp.conn_sel = i;
                        self.mp.conn_swap = Some(Instant::now());
                    }
                    self.play("ui_click");
                }
                MenuScreen::Games => {
                    if self.mp.games_sel == Some(i) {
                        self.mp_ok(screen);
                    } else {
                        self.mp.games_sel = Some(i);
                        self.play("ui_click");
                    }
                }
                MenuScreen::MpCreate => self.mp_pick_row(i),
                _ => {}
            },
            MpHit::CreateGame => {
                self.play("ui_click");
                self.mp_enter_create();
            }
            MpHit::ScrollUp => self.mp_scroll(-1),
            MpHit::ScrollDown => self.mp_scroll(1),
            MpHit::SpeedDown => {
                self.mp.speed = Some(self.mp.speed_index().saturating_sub(1));
                self.play("ui_click");
            }
            MpHit::SpeedUp => {
                self.mp.speed = Some((self.mp.speed_index() + 1).min(SPEED_MS.len() - 1));
                self.play("ui_click");
            }
            MpHit::Combo(which) => self.mp_open_combo(which),
            _ => {}
        }
    }

    fn mp_pick_row(&mut self, n: usize) {
        let rows = self.mp_map_rows();
        match rows.get(n) {
            Some(MapRow::Up) => {
                self.mp.create_dir.pop();
                self.mp.create_top = 0;
            }
            Some(MapRow::Folder(f)) => {
                self.mp.create_dir.push(f.clone());
                self.mp.create_top = 0;
            }
            Some(MapRow::Map(m)) => {
                if self.mp.create_map == Some(*m) {
                    self.mp_ok(&MenuScreen::MpCreate);
                    return;
                }
                self.mp.create_map = Some(*m);
                // A new map resets Teams to its even split (19.2 s).
                self.mp.split = None;
            }
            None => return,
        }
        self.play("ui_click");
    }

    fn mp_open_combo(&mut self, which: Drop) {
        let lobby = self.mp.lobby.clone();
        let slot_rect = |i: usize, x: f32, w: f32| {
            let mut y = 96.0;
            if let Some(l) = &lobby {
                for (name, range) in l.teams() {
                    if !name.is_empty() {
                        y += 46.0;
                    }
                    for j in range {
                        if j == i {
                            return (x, y, w, 44.0);
                        }
                        y += 50.67;
                    }
                    y += 1.0;
                }
            }
            (x, y, w, 44.0)
        };
        match which {
            Drop::GameType => {
                let items = GameType::ALL.iter().map(|g| g.name().to_string()).collect();
                self.mp_open(which, (860.0, 786.0, 514.0, 44.0), items);
            }
            Drop::Teams => {
                let slots = self
                    .mp
                    .create_map
                    .and_then(|m| self.mp_info(m))
                    .map(|i| i.slots)
                    .unwrap_or(2);
                let items = mpl::team_splits(slots)
                    .iter()
                    .map(|(a, b)| format!("{a} vs {b}"))
                    .collect();
                self.mp_open(which, (860.0, 852.0, 514.0, 44.0), items);
            }
            Drop::Player(i) => {
                let items = OCCUPANT_CHOICES.iter().map(|s| s.to_string()).collect();
                self.mp_open(which, slot_rect(i, 470.0, 402.0), items);
            }
            Drop::Race(i) => {
                let items = RaceChoice::CHOICES
                    .iter()
                    .map(|r| r.label().to_string())
                    .collect();
                self.mp_open(which, slot_rect(i, 892.0, 202.0), items);
            }
            Drop::Color(i) => {
                let items = lobby
                    .as_ref()
                    .map(|l| l.free_colors(i))
                    .unwrap_or_default()
                    .iter()
                    .map(|&c| mpl::COLOR_CHOICES[c].to_string())
                    .collect();
                self.mp_open(which, slot_rect(i, 1114.0, 202.0), items);
            }
            Drop::Observer(i) => {
                let x = if i % 2 == 0 { 470.0 } else { 974.0 };
                let y = if i < 2 { 846.0 } else { 898.0 };
                let items = vec!["Open".to_string(), "Closed".to_string()];
                self.mp_open(which, (x, y, 332.0, 44.0), items);
            }
        }
    }

    fn mp_pick(&mut self, which: Drop, i: usize) {
        self.play("ui_click");
        match which {
            Drop::GameType => {
                self.mp.game_type = GameType::ALL.get(i).copied();
                self.mp.split = None;
            }
            Drop::Teams => {
                let slots = self
                    .mp
                    .create_map
                    .and_then(|m| self.mp_info(m))
                    .map(|i| i.slots)
                    .unwrap_or(2);
                self.mp.split = mpl::team_splits(slots).get(i).copied();
            }
            Drop::Player(s) => {
                let occupant = match i {
                    0 => Occupant::Open,
                    1 => Occupant::Closed,
                    _ => Occupant::Computer,
                };
                if let Some(l) = self.mp.lobby.as_mut() {
                    l.set_occupant(s, occupant);
                }
            }
            Drop::Race(s) => {
                if let (Some(l), Some(r)) = (self.mp.lobby.as_mut(), RaceChoice::CHOICES.get(i)) {
                    if let Some(slot) = l.slots.get_mut(s) {
                        slot.race = *r;
                    }
                    if let Some(c) = self.mp.client.as_mut() {
                        let _ = c.link.send(&Msg::LobbyRace {
                            race: mpl::race_choice_byte(*r),
                        });
                    }
                }
            }
            Drop::Color(s) => {
                let Some(l) = self.mp.lobby.as_mut() else {
                    return;
                };
                let Some(&color) = l.free_colors(s).get(i) else {
                    return;
                };
                if let Some(slot) = l.slots.get_mut(s) {
                    slot.color = color;
                }
                if let Some(c) = self.mp.client.as_mut() {
                    let _ = c.link.send(&Msg::LobbyColor { color: color as u8 });
                }
            }
            Drop::Observer(s) => {
                if let Some(l) = self.mp.lobby.as_mut()
                    && let Some(seat) = l.observers.get_mut(s)
                {
                    *seat = if i == 0 {
                        Occupant::Open
                    } else {
                        Occupant::Closed
                    };
                }
            }
        }
    }

    fn mp_key(&mut self, screen: &MenuScreen, key: Key) {
        if self.mp.open.is_some() && key == Key::Escape {
            self.mp.open = None;
            return;
        }
        if matches!(screen, MenuScreen::MpLobby) {
            match key {
                Key::Enter if !self.mp.chat_input.is_empty() => {
                    let text = std::mem::take(&mut self.mp.chat_input);
                    let line = format!("{}: {text}", self.active_name());
                    match self.mp.client.as_mut() {
                        Some(c) => {
                            let _ = c.link.send(&Msg::Chat { from: 0, text });
                        }
                        None => {
                            if let Some(q) = self.lan.queue.as_mut() {
                                q.send_all(&Msg::Chat {
                                    from: 0,
                                    text: line.clone(),
                                });
                            }
                        }
                    }
                    self.mp.chat.push(line);
                    return;
                }
                Key::Backspace => {
                    self.mp.chat_input.pop();
                    return;
                }
                Key::Space if !self.mp.chat_input.is_empty() => {
                    self.mp.chat_input.push(' ');
                    return;
                }
                _ => {}
            }
            let ch = key_letter(key)
                .map(|c| c.to_ascii_lowercase())
                .or_else(|| key_digit(key));
            if let Some(c) = ch {
                if self.mp.chat_input.len() < 80 {
                    self.mp.chat_input.push(c);
                }
                return;
            }
        }
        match key {
            Key::Enter => self.mp_ok(screen),
            Key::Escape => self.mp_cancel(screen),
            Key::Up | Key::Down => {
                let down = key == Key::Down;
                match screen {
                    MenuScreen::Connection => {
                        let n = if down { 1 } else { 0 };
                        if n != self.mp.conn_sel {
                            self.mp.conn_sel = n;
                            self.mp.conn_swap = Some(Instant::now());
                            self.play("ui_move");
                        }
                    }
                    MenuScreen::Games if !self.mp.games.is_empty() => {
                        let last = self.mp.games.len() - 1;
                        self.mp.games_sel = Some(match self.mp.games_sel {
                            None => 0,
                            Some(i) if down => (i + 1).min(last),
                            Some(i) => i.saturating_sub(1),
                        });
                    }
                    MenuScreen::MpCreate => {
                        let maps: Vec<usize> = self
                            .mp_map_rows()
                            .iter()
                            .filter_map(|r| match r {
                                MapRow::Map(m) => Some(*m),
                                _ => None,
                            })
                            .collect();
                        let pos = maps.iter().position(|m| Some(*m) == self.mp.create_map);
                        let next = match pos {
                            None => 0,
                            Some(p) if down => (p + 1).min(maps.len().saturating_sub(1)),
                            Some(p) => p.saturating_sub(1),
                        };
                        if let Some(m) = maps.get(next) {
                            self.mp.create_map = Some(*m);
                            self.mp.split = None;
                            self.mp_scroll_to_selection();
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// Join a LAN game from the Games list: connect, then sit in the host's
    /// lobby (its `LobbyState` updates) until the host starts.
    fn mp_join(&mut self, i: usize) {
        let Some((addr, announce, _)) = self.mp.games.get(i).cloned() else {
            return;
        };
        let index = self.maps.iter().position(|m| {
            std::fs::read(&m.path)
                .map(|raw| Sha256::digest(&raw).as_slice() == announce.map_hash)
                .unwrap_or(false)
        });
        let Some(index) = index else {
            self.status("You do not have the map of this game".to_string());
            return;
        };
        let name = self.active_name();
        match galaxycraft::net::connect_lobby(
            &addr.to_string(),
            announce.map_hash,
            galaxycraft::net::WORLD_SEED,
            self.menu_race,
            &name,
        ) {
            Ok(link) => {
                self.mp.discovery = None;
                self.mp.lobby = None;
                self.mp.lobby_map = index;
                self.mp.countdown = None;
                self.mp.launching = None;
                self.mp.chat.clear();
                self.mp.speed = Some((announce.speed as usize).min(6));
                self.mp.client = Some(ClientLobby {
                    link,
                    addr,
                    map_hash: announce.map_hash,
                    you: None,
                    speed: announce.speed,
                });
                self.menu_screen = MenuScreen::MpLobby;
            }
            Err(e) => self.status(format!("join failed: {e}")),
        }
    }

    // ------------------------------------------------------------------
    // Score screen (`gluscore`), retail capture 21:14:04
    // ------------------------------------------------------------------

    /// Players shown on the score screen: every occupied lobby slot.
    fn mp_score_players(&self) -> Vec<u8> {
        match &self.mp.setup {
            Some(s) => s
                .slots
                .iter()
                .enumerate()
                .filter(|(_, x)| x.occupant != galaxycraft::multiplayer::SetupOccupant::Empty)
                .map(|(i, _)| i as u8)
                .collect(),
            None => (0..self.world.players.len() as u8)
                .filter(|&p| self.world.units.iter().any(|u| u.owner == p))
                .collect(),
        }
    }

    pub(super) fn draw_mp_score(&mut self, fb: &mut Framebuffer) {
        self.mp.hits.clear();
        let won = self.game_result == Some(true);
        let race = self
            .world
            .players
            .get(self.local_player)
            .map(|p| p.race)
            .unwrap_or(Race::Terran);
        let art = self.mp_score_art(race, won);
        let (cx, _, cw, ch) = Self::content_rect(fb);
        fb.rect(cx, 0, cw, ch, Rgb::new(0, 0, 0));
        if let Some(bg) = &art.backgnd {
            let w = (bg.width as f32 * ch as f32 / bg.height as f32).round() as i32;
            bg.draw_scaled(fb, cx + (cw - w) / 2, 0, w, ch);
        }
        self.mp_plate(
            fb,
            art.pmain.as_ref(),
            (0.0, 0.0, 640.0, 480.0),
            true,
            0.0,
            0.0,
        );
        let tab = ScoreTab::ALL[self.mp.score_tab.min(3)];
        let players = self.mp_score_players();
        let lines = mpl::score_table(&self.world, &players, tab);
        let title = if won { "Victory!" } else { "Defeat!" };
        self.mp_text(
            fb,
            title,
            674.0,
            72.0,
            48.0,
            Face::Ext,
            VALUE,
            Align::Center,
        );
        for (i, t) in ScoreTab::ALL.iter().enumerate() {
            let rect = (232.0 + 99.0 * i as f32, 12.0, 93.0, 32.0);
            let r = Self::mp_ui(fb, rect);
            let colour = if i == self.mp.score_tab {
                ROW_SELECTED
            } else if self.mp.hover == Some(MpHit::Tab(i)) {
                CEF_HOVER
            } else {
                ROW
            };
            let centre = 320.0 + (rect.0 + rect.2 / 2.0) * 3.0;
            self.mp_text(
                fb,
                t.label(),
                centre,
                72.0,
                40.0,
                Face::Reg,
                colour,
                Align::Center,
            );
            self.mp_hit(r, MpHit::Tab(i));
        }
        for (c, name) in tab.columns().iter().enumerate() {
            let centre = 320.0 + (188.0 + 104.0 * c as f32 + 48.0) * 3.0;
            self.mp_text(
                fb,
                name,
                centre,
                164.0,
                35.0,
                Face::Reg,
                TITLE,
                Align::Center,
            );
        }
        self.mp_text(
            fb,
            "Total Score",
            1985.0,
            157.0,
            47.0,
            Face::Ext,
            VALUE,
            Align::Center,
        );
        let maxes: Vec<i32> = (0..3)
            .map(|c| lines.iter().map(|l| l.values[c]).max().unwrap_or(0))
            .collect();
        let local = self.local_player as u8;
        for (row, line) in lines.iter().enumerate().take(8) {
            let band_y = 76.0 + 40.0 * row as f32;
            self.mp_plate(
                fb,
                art.pinset.as_ref(),
                (30.0, band_y, 588.0, 37.0),
                false,
                0.0,
                0.0,
            );
            let name = if line.player == local {
                self.active_name()
            } else if self
                .mp
                .setup
                .as_ref()
                .and_then(|s| s.slots.get(line.player as usize))
                .is_some_and(|s| s.occupant == galaxycraft::multiplayer::SetupOccupant::Computer)
            {
                "Computer".to_string()
            } else {
                format!("Player {}", line.player + 1)
            };
            let colour = if line.player == local { VALUE } else { TITLE };
            self.mp_text(
                fb,
                &name,
                426.0,
                band_y * 3.0 + 22.0,
                48.0,
                Face::Reg,
                colour,
                Align::Left,
            );
            let portrait = match line.race {
                Race::Zerg => 3,
                Race::Terran => 4,
                _ => 5,
            };
            if let Some(img) = art.iscore.get(portrait) {
                let r = Self::mp_rect(fb, (448.0, band_y * 3.0 + 66.0, 84.0, 42.0));
                img.draw_scaled(fb, r.0, r.1, r.2, r.3);
            }
            let colour_index = match self
                .world
                .players
                .get(line.player as usize)
                .and_then(|p| p.color)
            {
                Some(c) => mpl::color_score_group(c as usize),
                None => line.player as usize % 12,
            };
            for (c, (&value, &max)) in line.values.iter().zip(&maxes).enumerate() {
                let x = 320.0 + (188.0 + 104.0 * c as f32) * 3.0;
                let y = (80.0 + 40.0 * row as f32) * 3.0;
                let frac = mpl::bar_fraction(value, max);
                self.mp_score_bar(fb, &art, colour_index, (x, y, 288.0, 54.0), frac);
                let text = value.to_string();
                self.mp_text(
                    fb,
                    &text,
                    x + 144.0,
                    y + 15.0,
                    30.0,
                    Face::Reg,
                    VALUE,
                    Align::Center,
                );
            }
            let total_y = (80.0 + 40.0 * row as f32) * 3.0;
            if let Some(b) = &art.scorebox {
                let r = Self::mp_rect(fb, (1832.0, total_y, 288.0, 90.0));
                b.draw_scaled(fb, r.0, r.1, r.2, r.3);
            }
            self.mp_text(
                fb,
                &line.total.to_string(),
                1980.0,
                total_y + 22.0,
                47.0,
                Face::Reg,
                VALUE,
                Align::Center,
            );
        }
        let elapsed = mpl::elapsed_text(self.elapsed_ticks());
        self.mp_text(
            fb,
            &elapsed,
            429.0,
            1186.0,
            48.0,
            Face::Reg,
            TITLE,
            Align::Left,
        );
        for (label, rect, hit) in [
            ("Save Replay", (40.0, 430.0, 210.0, 30.0), MpHit::SaveReplay),
            (
                "Watch Replay",
                (260.0, 430.0, 210.0, 30.0),
                MpHit::WatchReplay,
            ),
            ("Ok", (480.0, 430.0, 138.0, 30.0), MpHit::Ok),
        ] {
            self.draw_glue_button(fb, label, rect, true);
            let r = Self::mp_ui(fb, rect);
            self.mp_hit(r, hit);
        }
    }

    /// One score bar: the colour's outline (3-slice) and its fill scaled
    /// to `frac` of the inner width.
    fn mp_score_bar(
        &self,
        fb: &mut Framebuffer,
        art: &ScoreArt,
        colour: usize,
        rect: (f32, f32, f32, f32),
        frac: f32,
    ) {
        let base = 6 + 6 * colour;
        let three = |fb: &mut Framebuffer, frames: [usize; 3], r: (f32, f32, f32, f32)| {
            let imgs: Vec<&DdsImage> = frames.iter().filter_map(|&f| art.iscore.get(f)).collect();
            if imgs.len() != 3 || r.2 <= 0.0 {
                return;
            }
            let cap_l = imgs[0].width as f32 * 0.75;
            let cap_r = imgs[2].width as f32 * 0.75;
            let squeeze = (r.2 / (cap_l + cap_r)).min(1.0);
            let (cl, cr) = (cap_l * squeeze, cap_r * squeeze);
            let parts = [
                (imgs[0], r.0, cl),
                (imgs[1], r.0 + cl, (r.2 - cl - cr).max(0.0)),
                (imgs[2], r.0 + r.2 - cr, cr),
            ];
            for (img, x, w) in parts {
                if w <= 0.0 {
                    continue;
                }
                let s = Self::mp_rect(fb, (x, r.1, w, r.3));
                img.draw_scaled(fb, s.0, s.1, s.2, s.3);
            }
        };
        let inset = 6.0;
        let fill_w = (rect.2 - 2.0 * inset) * frac;
        if fill_w > 0.5 {
            three(
                fb,
                [base, base + 1, base + 2],
                (rect.0 + inset, rect.1 + inset, fill_w, rect.3 - 2.0 * inset),
            );
        }
        three(fb, [base + 3, base + 4, base + 5], rect);
    }

    pub(super) fn handle_mp_score(&mut self, window: &Window) {
        let mouse = window
            .get_unscaled_mouse_pos(MouseMode::Clamp)
            .map(|(x, y)| (x as i32, y as i32));
        let down = window.get_mouse_down(MouseButton::Left);
        let keys = window.get_keys_pressed(KeyRepeat::No);
        self.handle_mp_score_input(mouse, down, &keys);
    }

    pub(super) fn handle_mp_score_input(
        &mut self,
        mouse: Option<(i32, i32)>,
        down: bool,
        keys: &[Key],
    ) {
        let hit = mouse.and_then(|(x, y)| {
            self.mp
                .hits
                .iter()
                .rev()
                .find(|(r, _)| inside(*r, x, y))
                .map(|(_, h)| *h)
        });
        if hit != self.mp.hover {
            self.mp.hover = hit;
            if hit.is_some() {
                self.play("ui_move");
            }
        }
        let clicked = down && !self.mouse_was_down;
        self.mouse_was_down = down;
        let mut action = if clicked { hit } else { None };
        for k in keys {
            match k {
                Key::Enter | Key::Escape | Key::O => action = Some(MpHit::Ok),
                Key::S => action = Some(MpHit::SaveReplay),
                Key::W => action = Some(MpHit::WatchReplay),
                _ => {}
            }
        }
        match action {
            Some(MpHit::Tab(i)) => {
                self.mp.score_tab = i;
                self.play("ui_click");
            }
            Some(MpHit::SaveReplay) => {
                self.play("ui_click");
                let _ = self.save_replay();
            }
            Some(MpHit::WatchReplay) => {
                self.play("ui_click");
                if let Some(path) = self.save_replay() {
                    self.load_save(&path, true);
                }
            }
            Some(MpHit::Ok) => {
                self.play("ui_click");
                self.mp.score_tab = 0;
                self.net = None;
                self.open_menu();
                self.mp_enter_games();
            }
            _ => {}
        }
    }
}

impl App {
    /// Walkthrough log line (`--menu-script` TSV state column).
    pub(super) fn mp_state_line(&self) -> String {
        let teams: Vec<u8> = self.world.players.iter().map(|p| p.team).collect();
        let computers = self
            .mp
            .setup
            .as_ref()
            .map(|s| {
                s.slots
                    .iter()
                    .filter(|x| x.occupant == galaxycraft::multiplayer::SetupOccupant::Computer)
                    .count()
            })
            .unwrap_or(0);
        let defeated: String = self
            .world
            .players
            .iter()
            .map(|p| if p.defeated { 'D' } else { '.' })
            .collect();
        let units: Vec<usize> = (0..self.world.players.len() as u8)
            .map(|p| {
                self.world
                    .units
                    .iter()
                    .filter(|u| u.alive && u.owner == p)
                    .count()
            })
            .collect();
        let net = match &self.net {
            Some(n) => format!(
                "{} desync={} hash={:016x}",
                if matches!(n.role, NetRole::Host) {
                    "host"
                } else {
                    "client"
                },
                n.desync,
                state_hash(&self.world)
            ),
            None => "none".to_string(),
        };
        let colors: Vec<i32> = self
            .world
            .players
            .iter()
            .map(|p| p.color.map(i32::from).unwrap_or(-1))
            .collect();
        let lobby_colors: Vec<usize> = self
            .mp
            .lobby
            .as_ref()
            .map(|l| l.slots.iter().map(|s| s.color).collect())
            .unwrap_or_default();
        format!(
            "colors={colors:?} lobby_colors={lobby_colors:?} net={net} defeated={defeated} units={units:?} winner={:?} in_game={} tick={} lobby_game={} local={} computers={} ais={} teams={:?} countdown={} result={:?}",
            self.world.winner,
            !self.menu,
            self.world.tick,
            self.mp.setup.is_some(),
            self.local_player,
            computers,
            self.ais.len(),
            teams,
            self.mp.countdown.is_some(),
            self.game_result
        )
    }

    /// Headless captures of each step of the retail recording
    /// (`--shot-screen mp-*`, compared by `scripts/mp_visual_diff.py`):
    /// mp-connection (3.6 s), mp-connection-lan (7.5 s), mp-games (12.5 s),
    /// mp-create (20.5 s), mp-creating (22.5 s), mp-lobby (24.5 s),
    /// mp-lobby-ready (48.0 s), mp-countdown (55.0 s), score-defeat /
    /// score-victory (21:14:04 screenshot).
    pub(super) fn mp_shot(&mut self, name: &str, fb: &mut Framebuffer) {
        self.intro = false;
        self.menu = true;
        if let Some(i) = self
            .maps
            .iter()
            .position(|m| m.name.contains("The Hunters"))
        {
            self.map_index = i;
        }
        match name {
            "mp-connection" | "mp-connection-lan" => {
                self.mp.enter_connection();
                if name.ends_with("lan") {
                    self.mp.conn_sel = mpl::CONNECTION_LAN;
                }
                self.menu_screen = MenuScreen::Connection;
            }
            "mp-games" => {
                self.menu_screen = MenuScreen::Games;
            }
            "mp-create" | "mp-creating" => {
                self.mp.game_type = Some(GameType::TopVsBottom);
                self.mp_enter_create();
                if name == "mp-creating" {
                    self.mp.creating = Some(Instant::now());
                }
            }
            _ => {
                // Lobby states and the score screen start from the lobby the
                // recording builds: (8)The Hunters, Top vs Bottom 4 vs 4.
                self.mp.game_type = Some(GameType::TopVsBottom);
                self.mp_enter_create();
                let Some(index) = self.mp.create_map else {
                    return;
                };
                let Some(info) = self.mp_info(index) else {
                    return;
                };
                let host = if self.active_name().is_empty() {
                    "LinKa".to_string()
                } else {
                    self.active_name()
                };
                let mut lobby = Lobby::new_host(
                    &host,
                    &info.title,
                    info.slots,
                    GameType::TopVsBottom,
                    mpl::default_split(info.slots),
                );
                if name != "mp-lobby" {
                    lobby.slots[0].race = RaceChoice::Protoss;
                    for i in 1..lobby.slots.len() {
                        lobby.set_occupant(i, Occupant::Computer);
                    }
                    for seat in lobby.observers.iter_mut() {
                        *seat = Occupant::Open;
                    }
                }
                self.mp.lobby = Some(lobby);
                self.mp.lobby_map = index;
                self.menu_screen = MenuScreen::MpLobby;
                if name == "mp-countdown" {
                    self.mp.countdown = Instant::now().checked_sub(Duration::from_millis(2300));
                }
                if name.starts_with("score-") {
                    self.mp_start_game();
                    // 01:18 of play, as on the reference sheet.
                    let local = self.local_player as u8;
                    let mut ais = std::mem::take(&mut self.ais);
                    for _ in 0..(78 * 1000 / 42) {
                        galaxycraft::ai::run_computers(&mut self.world, &mut ais, local);
                        self.world.step();
                    }
                    self.ais = ais;
                    self.menu = false;
                    self.game_result = Some(name == "score-victory");
                    self.draw(fb);
                    return;
                }
            }
        }
        self.draw_menu(fb);
    }
}

/// `0`..`9` as typed in the lobby chat.
fn key_digit(key: Key) -> Option<char> {
    Some(match key {
        Key::Key0 => '0',
        Key::Key1 => '1',
        Key::Key2 => '2',
        Key::Key3 => '3',
        Key::Key4 => '4',
        Key::Key5 => '5',
        Key::Key6 => '6',
        Key::Key7 => '7',
        Key::Key8 => '8',
        Key::Key9 => '9',
        _ => return None,
    })
}

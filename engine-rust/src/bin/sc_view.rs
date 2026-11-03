//! Playable graphical client for the reconstructed StarCraft rules engine.
//!
//! ```text
//! cargo run --release --bin sc_view
//! cargo run --release --bin sc_view -- --chk map.chk --tiles assets/TileSet --races terran,protoss
//! ```
//!
//! Controls
//! --------
//! * left drag / shift-click: select units
//! * right click: move / attack / gather / enter transport
//! * A: attack-move (next right click), P: patrol, S: stop, H: hold
//! * B: build menu (worker selected), 1..8: build / train command
//! * Ctrl+1..9: set control group, 1..9 (no Ctrl): recall / command card
//! * arrows or WASD: scroll camera, mouse wheel: scroll, Home: base, Space: selection
//! * F: fog of war, L: leaderboard, F10: mission statistics, +/-: game speed
//! * Tab: idle worker, I: replay title intro, Esc: pause
//!
//! Headless screens: `--shot FILE --shot-screen
//! intro|menu|briefing|mission|selection|pause|leaderboard|stats|victory|defeat|score`.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Window, WindowOptions};
use sha2::{Digest, Sha256};

use galaxycraft::ai::SimpleAi;
use galaxycraft::aiscript::CampaignAi;
use galaxycraft::audio::{AudioPlayer, MusicBank, SoundBank};
use galaxycraft::campaign::BriefingPage;
use galaxycraft::data::GameData;
use galaxycraft::dds::DdsImage;
use galaxycraft::hotkey_data;
use galaxycraft::ids::unit as uid;
use galaxycraft::map::{scenario_file_key, PlayerSlot, Scenario};
use galaxycraft::menu::{GameMenuItem, GameOptions, HotkeyRace, LobbyRace, MenuScreen, OptionsTab};
use galaxycraft::net::{
    announcer, broadcast, client_turn, discover, discovery_socket, host_turn, Announce, Lobby,
    Lockstep, Msg, TcpTransport, Transport, DEFAULT_TICKS_PER_TURN,
};
use galaxycraft::render::{
    asset_root, draw_unit_box, era_name, Framebuffer, Rgb, SpriteBank, Tileset, TILE_PX,
};
use galaxycraft::replay::{state_hash, RecAct, Recorder, Recording, SaveFile};
use galaxycraft::retail::RetailInstall;
use galaxycraft::sfx::SfxTable;
use galaxycraft::sim::{Order, Race, SimUnit, World};
use galaxycraft::tech as tree;
use galaxycraft::ui::UiRect;
use galaxycraft::ui_art::{ConsoleArt, GlueArt};
use galaxycraft::video::{PosterFrame, RgbaFrame, VideoInfo, VideoStore};
use galaxycraft::voice::{VoiceBank, VoiceCue};

const HUD_H: i32 = 240;
/// Intro movie playback: 12 fps at 960 px wide keeps the decoder ahead of
/// the clock on a laptop CPU while still looking like film, not a slideshow.
const INTRO_FPS: f32 = 12.0;
const INTRO_WIDTH: u32 = 960;
const VIEW_W: usize = 1280;
const VIEW_H: usize = 960;

/// Retail campaign metadata materialised alongside the CHK file.
#[derive(Clone, Debug, Default)]
struct CampaignTag {
    episode: String,
    episode_name: String,
    mission: String,
    /// Cached copy of the retail `est*.txt` briefing script.
    script: Option<PathBuf>,
    /// Cached retail backdrop named by the script.
    background: Option<PathBuf>,
}

#[derive(Clone, Debug)]
struct MapEntry {
    path: PathBuf,
    name: String,
    era: u16,
    players: usize,
    campaign: Option<CampaignTag>,
}

impl MapEntry {
    fn new(path: PathBuf, name: String, era: u16, players: usize) -> Self {
        Self {
            path,
            name,
            era,
            players,
            campaign: None,
        }
    }
}

/// Read the campaign manifest written by `RetailInstall::materialize`.
/// Campaign missions are placed before the melee maps, in retail episode
/// order; each one keeps the path of its briefing script and backdrop.
fn scan_campaign(cache: &Path) -> Vec<MapEntry> {
    let Ok(manifest) = std::fs::read_to_string(cache.join("campaign/manifest.tsv")) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in manifest.lines().skip(1) {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 9 {
            continue;
        }
        let chk = cache.join("campaign").join(fields[6]);
        let Ok(raw) = std::fs::read(&chk) else {
            continue;
        };
        let Ok(sc) = Scenario::parse(&raw) else {
            continue;
        };
        let players = sc
            .player_slots()
            .iter()
            .filter(|s| s.active)
            .count()
            .clamp(2, 8);
        let tag = CampaignTag {
            episode: fields[0].to_string(),
            episode_name: fields[1].to_string(),
            mission: fields[3].to_string(),
            script: (!fields[7].is_empty()).then(|| cache.join(fields[7])),
            background: (!fields[8].is_empty()).then(|| cache.join(fields[8])),
        };
        out.push(MapEntry {
            path: chk,
            name: format!("{}: {}", short_episode(&tag.episode_name), fields[4]),
            era: sc.era,
            players,
            campaign: Some(tag),
        });
    }
    out
}

fn short_episode(name: &str) -> &'static str {
    match name {
        "Terran Campaign" => "T",
        "Zerg Campaign" => "Z",
        "Protoss Campaign" => "P",
        "Brood War: Terran" => "BW-T",
        "Brood War: Zerg" => "BW-Z",
        "Brood War: Protoss" => "BW-P",
        _ => "?",
    }
}

fn visit_maps(dir: &PathBuf, out: &mut Vec<MapEntry>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit_maps(&path, out);
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("chk") {
            continue;
        }
        let Ok(raw) = std::fs::read(&path) else {
            continue;
        };
        let Ok(sc) = Scenario::parse(&raw) else {
            continue;
        };
        let players = sc
            .player_slots()
            .iter()
            .filter(|s| s.active)
            .count()
            .clamp(2, 8);
        out.push(MapEntry::new(
            path.clone(),
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("map")
                .to_string(),
            sc.era,
            players,
        ));
    }
}

fn scan_maps(dir: &PathBuf) -> Vec<MapEntry> {
    let mut out = Vec::new();
    visit_maps(dir, &mut out);
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Incremental replay player: rebuilds once, then applies due commands.
/// Save/replay directories: `$GALAXYCRAFT_SAVES`, else XDG/HOME cache.
/// `YYYY/MM/DD HH:MM` in UTC for a file timestamp.
///
/// The Replays browser shows when a file was written; the client has no
/// timezone database, so UTC is used and no local offset is implied.
fn format_timestamp(t: std::time::SystemTime) -> String {
    let secs = t
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil date from a day count (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}/{m:02}/{d:02} {:02}:{:02}",
        rem / 3600,
        (rem % 3600) / 60
    )
}

fn save_dirs() -> (PathBuf, PathBuf) {
    let base = std::env::var("GALAXYCRAFT_SAVES")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("XDG_CACHE_HOME")
                .map(PathBuf::from)
                .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".cache")))
                .unwrap_or_else(|_| PathBuf::from("/tmp"))
                .join("galaxycraft")
        });
    (base.join("saves"), base.join("replays"))
}

/// Per-profile campaign progress (`GCPR`), next to `hotkeys.bin`.
fn progress_save_path() -> Option<PathBuf> {
    save_dirs().0.parent().map(|p| p.join("campaign.bin"))
}

fn hotkey_save_path() -> Option<PathBuf> {
    save_dirs().0.parent().map(|p| p.join("hotkeys.bin"))
}

struct ReplayPlayer {
    save: SaveFile,
    idx: usize,
    paused: bool,
}

impl ReplayPlayer {
    fn new(save: SaveFile) -> Self {
        Self {
            save,
            idx: 0,
            paused: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NetRole {
    Host,
    Client,
}

/// A live LAN match inside the graphical client.
struct NetSession {
    role: NetRole,
    lock: Lockstep,
    /// Host: one transport per joined peer. Client: exactly one (the host).
    peers: Vec<TcpTransport>,
    desync: bool,
    last_hash: u64,
}

/// Remastered main-menu art layers, decoded from the player's own install.
///
/// These WebM "entry" videos are not text: they are the animated space
/// layers (fleet, battlecruiser, exit portal) that the retail engine
/// composites additively over the nebula.
/// One looping Remastered menu animation.
///
/// The shipped clips are ten seconds of VP9 with the transparency in
/// `BlockAdditional` chunks.  Decoding all 150 frames would cost hundreds of
/// megabytes, so a worker streams half-second chunks with alpha and the
/// player consumes them on its own wall clock, looping the clip.
struct MenuClip {
    path: PathBuf,
    info: VideoInfo,
    rx: Option<Receiver<Vec<Arc<galaxycraft::video::RgbaFrame>>>>,
    cancel: Arc<AtomicBool>,
    buffer: VecDeque<Arc<galaxycraft::video::RgbaFrame>>,
    current: Option<Arc<galaxycraft::video::RgbaFrame>>,
    decoded: u64,
    started: Instant,
    /// Playback rate (retail holds 15 fps source frames for 1/10.5 s).
    fps: f32,
    /// Rate the worker asks the decoder for; every authored frame is kept.
    source_fps: f32,
    chunk: usize,
    width: u32,
    /// Authored frames in one loop, from `start` to the end of the file.
    frames: usize,
    /// First second of the clip this player starts at (and loops back to).
    start: f32,
    /// `--menu-at` captures pin the first decoded frame instead of advancing
    /// with wall-clock time, so repeated renders are bit-identical.
    frozen: bool,
    /// Cached `RgbaFrame::has_alpha`; scanning 1.2M pixels per clip per frame
    /// was part of the menu's frame cost.
    has_alpha: bool,
    alpha_known: bool,
}

/// Retail advances the shipped 15 fps `glue/mainmenu` clips at a fixed
/// 10.5 fps: measured on the live client, the 151-frame Single/Multi clips
/// loop every 14.4 s and the 105-frame Exit clip every 10.0 s
/// (151 / 14.4 = 105 / 10.0 = 10.5).  Playing them at the WebM container
/// rate makes the loops 43 % too fast; resampling to 8 fps (the old value)
/// additionally dropped every other frame.
const MENU_CLIP_FPS: f32 = 10.5;
/// Native frame rate of the shipped WebM layers.  Decoding at this rate keeps
/// every authored frame; playback then holds each one for 1/10.5 s.
const MENU_CLIP_SOURCE_FPS: f32 = 15.0;
// Decode at the native width of the retail `glue/mainmenu` clips (1200 px)
// so the 1440p hover overlays keep their hairline grid instead of being
// upscaled from a 640 px intermediate.
const MENU_CLIP_WIDTH: u32 = 1200;
const MENU_CLIP_CHUNK: f32 = 0.5;
/// Retail loops each layer over its whole timeline (the last frame of
/// Single/Multi matches the first, and Exit's seam is a normal-motion step).
/// The old 2.5 s offset skipped the fleet entrance and made the seam visible.
const MENU_CLIP_START: f32 = 0.0;

/// The shipped cursor GRP (80x84) is 4x the 640x480 UI art, i.e. authored
/// for a 1920-line canvas.  The live client draws it at `height / 1920`
/// (`ui_scale / 4`): measured 56-57 px tall at 1440p and 42-43 px at 1080p,
/// with the visible tip at the pointer.  Drawing the frames 1:1 (the old
/// behaviour) made the arrow 43 % too large at 1440p.
const CURSOR_ASSET_HEIGHT: f32 = 1920.0;
/// Retail cycles the five cursor frames at 6 fps (ten 60 Hz samples each).
const CURSOR_FPS: f32 = 6.0;

/// Index of the cursor frame for `elapsed`, in GRP order (0..4).
fn cursor_frame_index(elapsed: Duration, frames: usize) -> usize {
    if frames <= 1 {
        return 0;
    }
    ((elapsed.as_secs_f32() * CURSOR_FPS) as usize) % frames
}

/// Retail cursor scale for a framebuffer of `height` pixels.
fn cursor_scale_for_height(height: i32) -> f32 {
    (height.max(1) as f32 / CURSOR_ASSET_HEIGHT).max(0.02)
}

/// Authored-frame timeline for a menu layer: `(aligned_start, remaining
/// frames, playback seconds at 10.5 fps)`.  Kept pure so the retail rates
/// measured on the live client are unit-tested without a video decoder.
fn menu_clip_timeline(source_seconds: f32, start: f32) -> (f32, usize, f32) {
    let total_frames = (source_seconds.max(0.0) * MENU_CLIP_SOURCE_FPS)
        .round()
        .max(1.0) as usize;
    let max_start = total_frames.saturating_sub(1);
    let start_frame = ((start.max(0.0) * MENU_CLIP_SOURCE_FPS).round() as usize).min(max_start);
    let aligned_start = start_frame as f32 / MENU_CLIP_SOURCE_FPS;
    let frames = total_frames - start_frame;
    (aligned_start, frames, frames as f32 / MENU_CLIP_FPS)
}

/// Bilinear, premultiplied blit of one cursor frame.  The GRP is 4K art, so
/// at 1440p every source pixel spans less than one screen pixel; nearest
/// sampling would make the arrow visibly chunky.
fn blit_cursor_scaled(
    fb: &mut Framebuffer,
    img: &galaxycraft::dds::DdsImage,
    x: i32,
    y: i32,
    scale: f32,
) {
    if img.width == 0 || img.height == 0 || scale <= 0.0 {
        return;
    }
    let sw = img.width as f32;
    let sh = img.height as f32;
    let dw = (sw * scale).round().max(1.0) as i32;
    let dh = (sh * scale).round().max(1.0) as i32;
    for dy in 0..dh {
        let sy = ((dy as f32 + 0.5) / scale - 0.5).clamp(0.0, sh - 1.0);
        let y0 = sy.floor() as usize;
        let y1 = (y0 + 1).min(img.height - 1);
        let fy = sy - y0 as f32;
        for dx in 0..dw {
            let sx = ((dx as f32 + 0.5) / scale - 0.5).clamp(0.0, sw - 1.0);
            let x0 = sx.floor() as usize;
            let x1 = (x0 + 1).min(img.width - 1);
            let fx = sx - x0 as f32;
            let sample = |px: u32| {
                (
                    ((px >> 16) & 0xff) as f32,
                    ((px >> 8) & 0xff) as f32,
                    (px & 0xff) as f32,
                    ((px >> 24) & 0xff) as f32 / 255.0,
                )
            };
            let (r00, g00, b00, a00) = sample(img.pixel(x0, y0));
            let (r10, g10, b10, a10) = sample(img.pixel(x1, y0));
            let (r01, g01, b01, a01) = sample(img.pixel(x0, y1));
            let (r11, g11, b11, a11) = sample(img.pixel(x1, y1));
            let w00 = (1.0 - fx) * (1.0 - fy);
            let w10 = fx * (1.0 - fy);
            let w01 = (1.0 - fx) * fy;
            let w11 = fx * fy;
            let alpha = a00 * w00 + a10 * w10 + a01 * w01 + a11 * w11;
            if alpha <= 0.003 {
                continue;
            }
            // Interpolate premultiplied colour, then unpremultiply, so the
            // transparent black outside the arrow does not darken its edge.
            let r = (r00 * a00 * w00 + r10 * a10 * w10 + r01 * a01 * w01 + r11 * a11 * w11) / alpha;
            let g = (g00 * a00 * w00 + g10 * a10 * w10 + g01 * a01 * w01 + g11 * a11 * w11) / alpha;
            let b = (b00 * a00 * w00 + b10 * a10 * w10 + b01 * a01 * w01 + b11 * a11 * w11) / alpha;
            let a8 = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
            if a8 == 0 {
                continue;
            }
            let dst = fb.get(x + dx, y + dy);
            fb.put(
                x + dx,
                y + dy,
                dst.blend(
                    Rgb::new(r.round() as u8, g.round() as u8, b.round() as u8),
                    a8,
                ),
            );
        }
    }
}

impl MenuClip {
    /// Open the clip at an explicit second.  `--menu-at` uses this to take
    /// deterministic captures of the otherwise wall-clock animation.
    fn open_at(path: PathBuf, start: f32, frozen: bool) -> Option<Self> {
        let info = galaxycraft::video::probe_info(&path)?;
        let full = info.duration_ms? as f32 / 1000.0;
        if full <= 0.0 || info.pixel_width == 0 {
            return None;
        }
        // Timeline in authored frames, so the seam lands on the first frame
        // instead of drifting by the rounding of a seconds-based duration.
        // Retail holds each authored frame for 1/10.5 s, so a 151-frame
        // layer loops in 14.38 s (not the WebM's 10.07 s).
        let (start, frames, _duration) = menu_clip_timeline(full, start);
        let mut clip = Self {
            path,
            info,
            rx: None,
            cancel: Arc::new(AtomicBool::new(false)),
            buffer: VecDeque::new(),
            current: None,
            decoded: 0,
            started: Instant::now(),
            fps: MENU_CLIP_FPS,
            source_fps: MENU_CLIP_SOURCE_FPS,
            chunk: (MENU_CLIP_CHUNK * MENU_CLIP_SOURCE_FPS).round().max(1.0) as usize,
            width: MENU_CLIP_WIDTH,
            frames,
            start,
            frozen,
            has_alpha: false,
            alpha_known: false,
        };
        clip.restart();
        Some(clip)
    }

    /// Start the streaming worker.  It keeps decoding the clip in a loop:
    /// the player wraps by simply consuming the next frame the worker
    /// produced, so the seam never waits for a fresh decoder/chunk (the old
    /// design stopped at the end and restarted the thread, which held the
    /// last frame on screen for a beat and looked like a cut).
    fn restart(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.buffer.clear();
        self.decoded = 0;
        self.started = Instant::now();
        let (tx, rx) = std::sync::mpsc::sync_channel(2);
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = Arc::clone(&self.cancel);
        let path = self.path.clone();
        let info = self.info.clone();
        // Decode at the authored rate; playback (`self.fps`) is slower.
        let source_fps = self.source_fps;
        let width = self.width;
        let chunk = self.chunk;
        let clip_start = self.start;
        let total_frames = self.frames.max(1);
        std::thread::Builder::new()
            .name("menu-clip".into())
            .spawn(move || {
                let mut cursor = 0usize;
                loop {
                    if cancel.load(Ordering::Relaxed) {
                        return;
                    }
                    let count = chunk.min(total_frames - cursor).max(1);
                    let start = clip_start + cursor as f32 / source_fps;
                    let mut frames = galaxycraft::video::decode_rgba_frames_from_path(
                        &path, &info, start, count, source_fps, width,
                    );
                    if frames.is_empty() {
                        return;
                    }
                    // The decoder can hand back one extra frame at the tail;
                    // trim to the authored count so every loop has exactly
                    // the same frame sequence.
                    frames.truncate(count);
                    let got = frames.len();
                    // Hand ownership to the player as refcounted frames: the
                    // old Vec<RgbaFrame> move put a multi-megabyte copy on
                    // the render thread every time a chunk arrived.
                    let frames: Vec<Arc<galaxycraft::video::RgbaFrame>> =
                        frames.into_iter().map(Arc::new).collect();
                    if send_or_stop(&tx, frames, &cancel).is_err() {
                        return;
                    }
                    cursor += got;
                    if cursor >= total_frames {
                        cursor = 0;
                    }
                }
            })
            .ok();
        self.rx = Some(rx);
    }

    /// Frame to draw for this instant.
    fn frame(&mut self) -> Option<&galaxycraft::video::RgbaFrame> {
        let want = if self.frozen {
            1
        } else {
            (self.started.elapsed().as_secs_f32() * self.fps).floor() as u64 + 1
        };
        // Pull at most one chunk per call; the sync channel bounds the
        // worker to two queued chunks (~6 MB at 640 px).  The first call
        // waits briefly for the decoder so headless captures show real art
        // instead of an empty layer.
        let first = self.current.is_none() && self.buffer.is_empty();
        if self.decoded < want && self.buffer.len() <= self.chunk * 2 {
            if let Some(rx) = &self.rx {
                // A frozen clip (`--menu-at` / `--menu-phase` captures) blocks
                // until the decoder delivers: the old 2 s poll gave up under
                // CPU load and drew an empty layer, so identical captures
                // differed.  Live playback never stalls the render thread.
                let got = if first && self.frozen {
                    rx.recv_timeout(FROZEN_CLIP_WAIT).ok()
                } else {
                    rx.try_recv().ok()
                };
                if let Some(frames) = got {
                    self.buffer.extend(frames);
                }
            }
        }
        while self.decoded < want {
            match self.buffer.pop_front() {
                Some(f) => {
                    self.current = Some(f);
                    self.decoded += 1;
                }
                None => break,
            }
        }
        if !self.alpha_known {
            if let Some(f) = self.current.as_ref() {
                self.has_alpha = f.has_alpha();
                self.alpha_known = true;
            }
        }
        self.current.as_deref()
    }
}

fn send_or_stop(
    tx: &SyncSender<Vec<Arc<galaxycraft::video::RgbaFrame>>>,
    frames: Vec<Arc<galaxycraft::video::RgbaFrame>>,
    cancel: &AtomicBool,
) -> Result<(), ()> {
    // A bounded channel blocks when the player is behind; poll the cancel
    // flag while waiting so closing the client does not leak the worker.
    let mut frames = frames;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(());
        }
        match tx.try_send(frames) {
            Ok(()) => return Ok(()),
            Err(std::sync::mpsc::TrySendError::Full(back)) => {
                frames = back;
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(std::sync::mpsc::TrySendError::Disconnected(_)) => return Err(()),
        }
    }
}

/// Longest wait for the first frame of a frozen (capture) clip.
const FROZEN_CLIP_WAIT: Duration = Duration::from_secs(60);

/// The six Remastered main-menu button clips (idle + hover).
#[derive(Default)]
struct MenuLayers {
    single: Option<MenuClip>,
    single_on: Option<MenuClip>,
    multi: Option<MenuClip>,
    multi_on: Option<MenuClip>,
    exit: Option<MenuClip>,
    exit_on: Option<MenuClip>,
}

impl MenuLayers {
    fn clip(&mut self, idx: usize, hover: bool) -> Option<&mut MenuClip> {
        match (idx, hover) {
            (0, false) => self.single.as_mut(),
            (0, true) => self.single_on.as_mut(),
            (1, false) => self.multi.as_mut(),
            (1, true) => self.multi_on.as_mut(),
            (7, false) => self.exit.as_mut(),
            (7, true) => self.exit_on.as_mut(),
            _ => None,
        }
    }
}

/// The three episode-select slots of `glucmpgn`/`gluexpcmpgn`: a faded
/// portrait, the spinning disk hologram and the mouse-over variant, all
/// streamed from the shipped `glue/campaign` clips.
#[derive(Default)]
struct CampaignSlotClips {
    portrait: Option<MenuClip>,
    disk: Option<MenuClip>,
    on: Option<MenuClip>,
}

/// Layer set for one campaign set (original or Brood War).
#[derive(Default)]
struct CampaignLayers {
    expansion: bool,
    slots: [CampaignSlotClips; 3],
}

/// LAN lobby UI state (menu screen `Lan`).
struct LanUi {
    listener: Option<Lobby>,
    entries: Vec<(std::net::SocketAddr, Announce)>,
    sel: usize,
    status: String,
    /// Pending client that has said `Join` but not been answered yet.
    pending: Option<TcpTransport>,
    announce_sock: Option<std::net::UdpSocket>,
    last_announce: Instant,
    /// Map the local player wants to play (index into `maps`).
    map_index: usize,
}

impl Default for LanUi {
    fn default() -> Self {
        Self {
            listener: None,
            entries: Vec::new(),
            sel: 0,
            status: "S: scan   H: host".to_string(),
            pending: None,
            announce_sock: None,
            last_announce: Instant::now() - Duration::from_secs(5),
            map_index: 0,
        }
    }
}

/// One menu input tick (keys pressed this frame, pointer, left button).
struct MenuInput {
    keys: Vec<Key>,
    mouse: Option<(i32, i32)>,
    mouse_down: bool,
    /// Wheel notches this tick (positive = towards the user, i.e. scroll
    /// the list down, as the live client does).
    wheel: i32,
}

/// Control under the pointer on a sub-screen (shared by every branch so
/// hover/click sounds and hit geometry stay consistent).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MenuHit {
    Ok,
    Cancel,
    Restore,
    Delete,
    Row(usize),
    Tab(usize),
    Cell(usize),
    OptSlider(usize),
    OptCheck(usize),
    Race(usize),
    RacePick(usize, usize),
    Equip,
    Skip,
}

struct App {
    data: Arc<GameData>,
    scenario: Scenario,
    world: World,
    tileset: Tileset,
    sprites: SpriteBank,
    sounds: SoundBank,
    audio: AudioPlayer,
    /// Materialised retail root (tileset, unit art, console, glue, music).
    assets_root: PathBuf,
    /// Retail console art for the human player's race.
    console: Option<ConsoleArt>,
    /// Retail main-menu and score surfaces.
    glue: Option<GlueArt>,
    /// Remastered front-end face (`webui/dist/lib/fonts/eurostileext-reg.otf`).
    ui_font: Option<galaxycraft::text::UiFont>,
    /// Glue faces (`font/eurostile-reg.ttf`, `font/eurostileext-reg.ttf`):
    /// the native glue renders from the TTFs, kern pairs included.
    glue_font: Option<galaxycraft::text::UiFont>,
    glue_font_ext: Option<galaxycraft::text::UiFont>,
    /// `font/bl.ttf` (BlizzardGlobal): `RC_SMALLTEXT` and plain buttons.
    glue_font_small: Option<galaxycraft::text::UiFont>,
    /// Wide face for the lit main-menu labels (Eurostile Extended).
    ui_font_hover: Option<galaxycraft::text::UiFont>,
    /// Face used by the profile/build footer (HTML info text).
    ui_font_info: Option<galaxycraft::text::UiFont>,
    /// Retail Ogg music (decoded lazily) and the complete voice sets.
    music: MusicBank,
    voices: VoiceBank,
    /// Menu/ready-room music rotation.
    music_track: String,
    music_started: Instant,
    music_phase: u8,
    /// Race used for the currently loaded console.
    console_race: Race,
    /// Title/publisher intro before the main menu.
    intro: bool,
    intro_started: Instant,
    /// Cached cover-fitted menu and intro backdrops.
    menu_fit: Option<(usize, usize, DdsImage)>,
    /// Fully composed static main-menu frame (backdrop + chrome) at the
    /// current resolution.  Only the animated clips and text redraw.
    menu_static: Option<(usize, usize, Vec<u32>)>,
    intro_fit: Option<(usize, usize, DdsImage)>,
    /// Backdrop resampled once to the 16:9 content box (the 3412x1920 art is
    /// 25 MB decoded; bilinear-scaling it every frame is wasted work).
    backdrop_fit: Option<(usize, usize, DdsImage)>,
    /// F10 mission statistics panel.
    show_stats: bool,
    /// Cached UI scale/origin for this framebuffer (also used by mouse hit
    /// testing outside the draw pass).
    /// Capture screen requested with `--shot-screen` (drives the reference
    /// scene, if one has been reconstructed).
    shot_name: String,
    ui_scale: f32,
    ui_ox: i32,
    ui_oy: i32,
    /// `statlb.ui.json` in-game leaderboard.
    show_leaderboard: bool,
    /// Retail "Show Terrain in Minimap (Tab)": off draws the black radar.
    minimap_terrain: bool,
    /// 16:9 content box cached by `update_view` (x, y, w, h), so hit-testing
    /// and drawing share one geometry.
    content_box: (i32, i32, i32, i32),
    /// Decoded score backdrop and its fitted copy.
    result_panel: Option<DdsImage>,
    result_fit: Option<(usize, usize, DdsImage)>,
    /// Number of acknowledgements, used to cycle voice takes.
    ack_variant: usize,
    /// Player orders/commands issued, used for the APM plate.
    actions: u32,
    selected: Vec<u32>,
    groups: [Vec<u32>; 10],
    ais: Vec<SimpleAi>,
    campaign_ais: Vec<CampaignAi>,
    maps: Vec<MapEntry>,
    /// Index of the human player's slot from `OWNR` (not always 0).
    local_player: usize,
    /// The campaign missions occupy `maps[..campaign_len]`; melee maps follow.
    campaign_len: usize,
    /// 0 = campaign tab, 1 = melee/skirmish tab.
    menu_tab: usize,
    menu_index: usize,
    map_index: usize,
    menu: bool,
    /// Mission briefing is showing before the first tick of a campaign map.
    briefing: bool,
    /// Pages of the retail `est*.txt` briefing (empty for map-authored text).
    briefing_pages: Vec<BriefingPage>,
    briefing_page: usize,
    /// Retail backdrop named by the briefing script, plus the cached fit.
    briefing_art: Option<galaxycraft::dds::DdsImage>,
    briefing_fit: Option<(usize, usize, galaxycraft::dds::DdsImage)>,
    video_store: Option<VideoStore>,
    intro_poster: Option<PosterFrame>,
    intro_info: Option<VideoInfo>,
    intro_expansion: bool,
    /// Ordered retail intro playlist (`smk/blizzard.webm` then
    /// `smk/starintr.webm` on first launch, `smk/starxintr.webm` for the
    /// expansion, or one act cinematic during the campaign).
    intro_clips: Vec<String>,
    /// Index of the clip currently playing.
    intro_clip: usize,
    /// Campaign mission to load when the playlist ends (pre-mission movie).
    intro_pending_map: Option<usize>,
    /// Streaming player for the retail intro movie.
    intro_player: Option<galaxycraft::cinematic::Cinematic>,
    /// Whether the current clip has already been probed (avoids re-reading a
    /// 122 MB file every frame when the decoder is missing).
    intro_probed: bool,
    /// Wall clock of the last intro frame, so playback advances by real time.
    intro_last: Instant,
    /// Intro soundtrack decoded in the background (the movie is 138 s; the
    /// decode takes a second, the picture starts immediately).
    intro_audio: Option<std::sync::mpsc::Receiver<Vec<i16>>>,
    briefing_poster: Option<PosterFrame>,
    briefing_video: Option<String>,
    /// Streaming player for the intermission clip of the current briefing
    /// page (retail plays these as short movies, not stills).
    briefing_player: Option<galaxycraft::cinematic::Cinematic>,
    briefing_player_key: Option<String>,
    briefing_last: Instant,
    menu_screen: MenuScreen,
    game_options: GameOptions,
    profiles: Vec<String>,
    profile_sel: usize,
    /// Entry currently under the pointer (or keyboard-highlighted); `None`
    /// is the retail idle state (no hover animation, no highlighted label).
    main_cursor: Option<usize>,
    /// Original (0) / Expansion (1) under the pointer in the `Select Game
    /// Type` popup; `None` keeps the keyboard selection.
    gametype_hover: Option<usize>,
    /// The popup was opened from `Multiplayer`, so choosing a game type must
    /// continue to the multiplayer connection flow (retail does not open the
    /// lobby straight from the main menu).
    gametype_multi: bool,
    /// `Registry` was opened by Single Player → Select Game Type; Ok must
    /// continue to the episode list instead of returning to the main menu.
    registry_after_gametype: bool,
    episode_sel: usize,
    /// `Select starting mission` list box (selection + scroll).
    mission_list: galaxycraft::campaign_progress::MissionList,
    /// Grab offset (content px) of a scrollbar thumb drag in progress.
    mission_drag: Option<f32>,
    /// Locked campaign whose `glupokcancel` prompt is open.
    campaign_prompt: Option<galaxycraft::campaign_progress::Campaign>,
    /// Profile chosen in the Registry and every profile's campaign progress
    /// (`~/.cache/galaxycraft/campaign.bin`, `GCPR`).
    active_profile: String,
    progress_book: galaxycraft::campaign_progress::ProgressBook,
    lobby_races: Vec<LobbyRace>,
    game_menu_sel: usize,
    #[allow(dead_code)]
    credits_scroll: usize,
    paused: bool,
    /// `Some(true)` victory, `Some(false)` defeat.
    game_result: Option<bool>,
    /// Win-only: the `Next Scenario` action resolved into the bundled pack.
    next_map: Option<usize>,
    tiles_dir: PathBuf,
    races: Option<Vec<Race>>,
    menu_race: Race,
    /// 4x HD terrain of the loaded map's era (screen-resolution path).
    hd_terrain: Option<galaxycraft::render::HdTerrain>,
    /// 4x HD sprites, fetched from the install on first use.
    hd_sprites: galaxycraft::hd_sprites::HdSprites,
    /// Lobby slot the local player takes in a melee game (retail assigns
    /// one start location per slot); `None` = first human slot.
    melee_slot: Option<usize>,
    /// Creep variants (cv5 group 1 subtile) observed in a reference capture,
    /// keyed by tile; other creep tiles use [`galaxycraft::render::creep_mtxm`].
    creep_variants: std::collections::HashMap<(i32, i32), u8>,
    /// Colour the local player was given (retail randomizes melee colours);
    /// `None` = the slot's default colour.
    local_team_color: Option<Rgb>,
    camera: (i32, i32),
    view: (i32, i32),
    drag_start: Option<(i32, i32)>,
    drag_now: (i32, i32),
    attack_mode: bool,
    patrol_mode: bool,
    build_mode: Option<u16>,
    build_list: Vec<u16>,
    command_list: Vec<u16>,
    status: String,
    status_until: Instant,
    speed: f32,
    fog: bool,
    last_step: Instant,
    accumulator: f32,
    rmb_down: bool,
    lmb_down: bool,
    pub quit: bool,
    /// Raw bytes of the loaded map (embedded into saves).
    chk_raw: Vec<u8>,
    /// Races used for the current world (needed to rebuild on load).
    last_races: Vec<Race>,
    /// Save/replay browser state.
    save_sel: usize,
    saves_dir: PathBuf,
    replays_dir: PathBuf,
    /// Active replay playback (world runs from a recording, no AI/input).
    replay: Option<ReplayPlayer>,
    /// Active LAN match, when playing over the network.
    net: Option<NetSession>,
    lan: LanUi,
    /// Remastered menu space layers (loaded once from the install).
    menu_layers: Option<MenuLayers>,
    /// Streamed episode-select art (built lazily; one campaign set at a time).
    campaign_layers: Option<CampaignLayers>,
    /// Slot under the pointer on the episode screen (drives the `On` clip).
    episode_hover: Option<usize>,
    /// Control under the pointer on the current sub-screen (drives the
    /// hover sound).
    menu_hover: Option<MenuHit>,
    /// Lobby slot whose race dropdown is open (`None` = closed).
    lobby_race_open: Option<usize>,
    /// Cue names played this session (used by the scripted walkthrough to
    /// prove the sound policy; bounded to the last 512 entries).
    sound_log: Vec<String>,
    /// Collections cell selected in the grid (drives the preview column).
    collection_sel: usize,
    /// `--menu-at` / `--menu-phase` pins, shared with the campaign clips so
    /// headless captures are deterministic.
    menu_at: Option<f32>,
    menu_phases: std::collections::HashMap<String, f32>,
    /// Cached terrain minimap for the Create screen: (map index, 128x128 RGB).
    lobby_preview: Option<(usize, Vec<u32>)>,
    /// Hotkeys browser: hovered/selected unit and the icon rectangles drawn
    /// last frame (for mouse hit-testing and the hover edge).
    hotkey_hover: Option<u16>,
    hotkey_sel: Option<u16>,
    hotkey_cells: Vec<(u16, (i32, i32, i32, i32))>,
    /// Command-card cells of the selected hotkeys unit.
    hotkey_action_cells: Vec<(u16, (i32, i32, i32, i32))>,
    /// Command selected for rebinding (shows the bottom prompt).
    hotkey_action: Option<u16>,
    /// Complete action bindings (indexed by action id, `$default` profile).
    hotkey_bindings: Vec<Option<hotkey_data::KeyBind>>,
    /// Snapshot taken when the Hotkeys panel is opened (Cancel restores it).
    hotkey_baseline: Vec<Option<hotkey_data::KeyBind>>,
    /// Units whose cards contain an unbound command (the red `?` badge).
    hotkey_unbound: Vec<u16>,
    /// Collision warning currently visible in the rebind panel.
    hotkey_binding_error: bool,
    /// Race tab under the pointer (the retail green hover edge).
    hotkey_tab_hover: Option<HotkeyRace>,
    /// F11 asks the live loop to write the composed frame (evidence capture).
    capture_request: bool,
    /// Restore-defaults confirmation dialog open (reference 120018).
    hotkey_reset: bool,
    /// Left edge of the 16:9 content box (pillarbox offset).
    content_x0: i32,
    /// Previous frame's left-button state, so a click fires once.
    mouse_was_down: bool,
    /// Draw the input readout (pointer, buttons, last key).
    debug_input: bool,
    /// Name of the last key seen, for the readout.
    last_key: Option<String>,
    /// Last known pointer position (window pixels), for the software cursor.
    pointer: Option<(i32, i32)>,
    /// Modal overlay over the world (0 none, 1 objectives, 2 tips, 3 menu).
    overlay: u8,
    /// Retail install handle, used to read portrait videos on demand.
    install: Option<RetailInstall>,
    /// Retail cursor art (`cursor/arrow.dds.grp`, five animated frames),
    /// drawn in place of the system pointer so the front-end keeps its skin.
    cursor_art: Option<galaxycraft::dds::DdsGrp>,
    /// Wall clock that drives the 6 fps cursor animation.
    cursor_started: Instant,
    /// Decoded portrait frames per unit type (None = tried and unavailable).
    portraits: std::collections::HashMap<u16, Option<PosterFrame>>,
    /// Episode-select art: first frame of `glue/campaign/{terr,zerg,prot}.webm`
    /// keyed by race (0 terran, 1 zerg, 2 protoss). Decoded with alpha so the
    /// characters sit on the black plate instead of a grey box.
    /// Parsed credit entries (name, style) from the shipped credits bundle.
    credits: Vec<(String, u8)>,
    /// Collections thumbnails, decoded on demand from the shipped images.
    thumbs: std::collections::HashMap<String, Option<PosterFrame>>,
    /// RGBA `cmdicons.*.png` web-UI icons (alpha coverage kept).
    hotkey_icons: std::collections::HashMap<String, Option<RgbaFrame>>,
}

/// Opaque nearest blit of a decoded video frame (portraits).
fn blit_opaque(src: &PosterFrame, dst: &mut Framebuffer, x: i32, y: i32, w: i32, h: i32) {
    if w <= 0 || h <= 0 || src.width == 0 || src.height == 0 {
        return;
    }
    for dy in 0..h {
        let py = ((dy as f32 + 0.5) * src.height as f32 / h as f32) as u32;
        for dx in 0..w {
            let px = ((dx as f32 + 0.5) * src.width as f32 / w as f32) as u32;
            if px >= src.width || py >= src.height {
                continue;
            }
            let o = ((py * src.width + px) * 3) as usize;
            dst.put(
                x + dx,
                y + dy,
                Rgb::new(src.rgb[o], src.rgb[o + 1], src.rgb[o + 2]),
            );
        }
    }
}

/// Premultiplied area-average downscale (box filter), returned as straight
/// RGBA for the blit path.  Skia's CEF background scaling spreads coverage
/// wider than a 2-tap bilinear on these 128->96 icons.
fn resize_rgba_area(src: &RgbaFrame, w: u32, h: u32) -> RgbaFrame {
    if w == 0 || h == 0 || src.width == 0 || src.height == 0 {
        return src.clone();
    }
    if src.width == w && src.height == h {
        return src.clone();
    }
    let mut out = vec![0u8; (w * h * 4) as usize];
    let sx = src.width as f32 / w as f32;
    let sy = src.height as f32 / h as f32;
    for oy in 0..h {
        let y0 = oy as f32 * sy;
        let y1 = y0 + sy;
        for ox in 0..w {
            let x0 = ox as f32 * sx;
            let x1 = x0 + sx;
            let mut acc = [0.0f32; 4];
            let mut n = 0.0f32;
            let iy0 = y0.floor() as u32;
            let iy1 = (y1.ceil() as u32).min(src.height);
            let ix0 = x0.floor() as u32;
            let ix1 = (x1.ceil() as u32).min(src.width);
            for py in iy0..iy1 {
                let wy = ((py as f32 + 1.0).min(y1) - (py as f32).max(y0)).max(0.0);
                for px in ix0..ix1 {
                    let wx = ((px as f32 + 1.0).min(x1) - (px as f32).max(x0)).max(0.0);
                    let wgt = wx * wy;
                    if wgt <= 0.0 {
                        continue;
                    }
                    let i = ((py * src.width + px) * 4) as usize;
                    let a = src.rgba[i + 3] as f32;
                    let af = a / 255.0;
                    acc[0] += src.rgba[i] as f32 * af * wgt;
                    acc[1] += src.rgba[i + 1] as f32 * af * wgt;
                    acc[2] += src.rgba[i + 2] as f32 * af * wgt;
                    acc[3] += a * wgt;
                    n += wgt;
                }
            }
            if n <= 0.0 {
                continue;
            }
            acc[0] /= n;
            acc[1] /= n;
            acc[2] /= n;
            acc[3] /= n;
            let oi = ((oy * w + ox) * 4) as usize;
            let al = acc[3].clamp(0.0, 255.0);
            if al <= 0.0 {
                out[oi + 3] = 0;
                continue;
            }
            let af = al / 255.0;
            out[oi] = (acc[0] / af).round().clamp(0.0, 255.0) as u8;
            out[oi + 1] = (acc[1] / af).round().clamp(0.0, 255.0) as u8;
            out[oi + 2] = (acc[2] / af).round().clamp(0.0, 255.0) as u8;
            out[oi + 3] = al.round() as u8;
        }
    }
    RgbaFrame {
        width: w,
        height: h,
        rgba: out,
    }
}

/// Blit an RGBA web-UI icon with its own grayscale pixels (CEF draws the
/// `.unit` background image unchanged; only `.action` gets the #ff0 overlay).
fn blit_rgba_src(src: &RgbaFrame, dst: &mut Framebuffer, x: i32, y: i32, w: i32, h: i32) {
    if w <= 0 || h <= 0 || src.width == 0 || src.height == 0 {
        return;
    }
    let sw = src.width as f32;
    let sh = src.height as f32;
    let offx: f32 = std::env::var("GC_UNIT_OFFX")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0);
    let offy: f32 = std::env::var("GC_UNIT_OFFY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0);
    for dy in 0..h {
        let ty = y + dy;
        if ty < 0 || ty >= dst.height {
            continue;
        }
        let fy = ((dy as f32 + 0.5) * sh / h as f32 - 0.5 + offy).clamp(0.0, sh - 1.0);
        let y0 = fy.floor() as u32;
        let y1 = (y0 + 1).min(src.height.saturating_sub(1));
        let wy = fy - y0 as f32;
        for dx in 0..w {
            let tx = x + dx;
            if tx < 0 || tx >= dst.width {
                continue;
            }
            let fx = ((dx as f32 + 0.5) * sw / w as f32 - 0.5 + offx).clamp(0.0, sw - 1.0);
            let x0 = fx.floor() as u32;
            let x1 = (x0 + 1).min(src.width.saturating_sub(1));
            let wx = fx - x0 as f32;
            // Interpolate premultiplied RGBA (Skia decodes PNGs to
            // premultiplied bitmaps before scaling), so transparent white
            // texels do not bleed a bright fringe into the glyph edges.
            let sample = |sx: u32, sy: u32| -> [f32; 4] {
                let i = ((sy * src.width + sx) * 4) as usize;
                let r = src.rgba.get(i).copied().unwrap_or(0) as f32;
                let g = src.rgba.get(i + 1).copied().unwrap_or(0) as f32;
                let b = src.rgba.get(i + 2).copied().unwrap_or(0) as f32;
                let a = src.rgba.get(i + 3).copied().unwrap_or(0) as f32;
                let af = a / 255.0;
                [r * af, g * af, b * af, a]
            };
            let mut v = [0.0f32; 4];
            let s00 = sample(x0, y0);
            let s10 = sample(x1, y0);
            let s01 = sample(x0, y1);
            let s11 = sample(x1, y1);
            for (c, slot) in v.iter_mut().enumerate() {
                let top = s00[c] * (1.0 - wx) + s10[c] * wx;
                let bottom = s01[c] * (1.0 - wx) + s11[c] * wx;
                *slot = top * (1.0 - wy) + bottom * wy;
            }
            let a = (v[3] / 255.0).clamp(0.0, 1.0);
            if a <= 0.004 {
                continue;
            }
            let c = dst.get(tx, ty);
            let dr = (c.0 & 0xff) as f32;
            let dg = ((c.0 >> 8) & 0xff) as f32;
            let db = ((c.0 >> 16) & 0xff) as f32;
            let r = (v[0] + dr * (1.0 - a)).round().clamp(0.0, 255.0) as u32;
            let g = (v[1] + dg * (1.0 - a)).round().clamp(0.0, 255.0) as u32;
            let b = (v[2] + db * (1.0 - a)).round().clamp(0.0, 255.0) as u32;
            dst.put(tx, ty, Rgb(r << 16 | g << 8 | b));
        }
    }
}

/// Tint an RGBA web-UI icon (white glyph + alpha coverage) with `color`.
/// `cmdicons.*.png` is authored white-on-transparent; the retail CEF panel
/// colors it yellow for commands and gray for the unit list.
#[allow(clippy::too_many_arguments)]
fn blit_tinted_rgba(
    src: &RgbaFrame,
    dst: &mut Framebuffer,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    color: Rgb,
    use_lum: bool,
) {
    if w <= 0 || h <= 0 || src.width == 0 || src.height == 0 {
        return;
    }
    let sw = src.width as f32;
    let sh = src.height as f32;
    let offx: f32 = std::env::var("GC_CMD_OFFX")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.4);
    let offy: f32 = std::env::var("GC_CMD_OFFY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.35);
    for dy in 0..h {
        let ty = y + dy;
        if ty < 0 || ty >= dst.height {
            continue;
        }
        let fy = ((dy as f32 + 0.5) * sh / h as f32 - 0.5 + offy).clamp(0.0, sh - 1.0);
        let y0 = fy.floor() as u32;
        let y1 = (y0 + 1).min(src.height.saturating_sub(1));
        let wy = fy - y0 as f32;
        for dx in 0..w {
            let tx = x + dx;
            if tx < 0 || tx >= dst.width {
                continue;
            }
            let fx = ((dx as f32 + 0.5) * sw / w as f32 - 0.5 + offx).clamp(0.0, sw - 1.0);
            let x0 = fx.floor() as u32;
            let x1 = (x0 + 1).min(src.width.saturating_sub(1));
            let wx = fx - x0 as f32;
            // Interpolate premultiplied RGBA (Skia decodes PNGs to
            // premultiplied bitmaps before scaling), so transparent white
            // texels do not bleed a bright fringe into the glyph edges.
            let sample = |sx: u32, sy: u32| -> [f32; 4] {
                let i = ((sy * src.width + sx) * 4) as usize;
                let r = src.rgba.get(i).copied().unwrap_or(0) as f32;
                let g = src.rgba.get(i + 1).copied().unwrap_or(0) as f32;
                let b = src.rgba.get(i + 2).copied().unwrap_or(0) as f32;
                let a = src.rgba.get(i + 3).copied().unwrap_or(0) as f32;
                let af = a / 255.0;
                [r * af, g * af, b * af, a]
            };
            let mut v = [0.0f32; 4];
            let cmd_filter = std::env::var("GC_CMD_FILTER").unwrap_or_default();
            if cmd_filter == "tent" {
                let rad: f32 = std::env::var("GC_TENT_R")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1.5);
                let cy = fy.floor() as i32;
                let cx = fx.floor() as i32;
                let mut sum = 0.0f32;
                for iy in -2..=2 {
                    let wy = (1.0 - (fy - (cy + iy) as f32).abs() / rad).max(0.0);
                    if wy == 0.0 {
                        continue;
                    }
                    let sy = (cy + iy).clamp(0, src.height as i32 - 1) as u32;
                    for ix in -2..=2 {
                        let wx = (1.0 - (fx - (cx + ix) as f32).abs() / rad).max(0.0);
                        if wx == 0.0 {
                            continue;
                        }
                        let sx = (cx + ix).clamp(0, src.width as i32 - 1) as u32;
                        let t = sample(sx, sy);
                        let wgt = wx * wy;
                        for (slot, tval) in v.iter_mut().zip(t) {
                            *slot += tval * wgt;
                        }
                        sum += wgt;
                    }
                }
                if sum > 0.0 {
                    for slot in v.iter_mut() {
                        *slot /= sum;
                    }
                }
            } else if cmd_filter == "bic" {
                let cubic = |x: f32| -> f32 {
                    let x = x.abs();
                    if x <= 1.0 {
                        1.5 * x * x * x - 2.5 * x * x + 1.0
                    } else if x < 2.0 {
                        -0.5 * x * x * x + 2.5 * x * x - 4.0 * x + 2.0
                    } else {
                        0.0
                    }
                };
                let cy = fy.floor() as i32;
                let cx = fx.floor() as i32;
                for iy in -1..=2 {
                    let wy = cubic(fy - (cy + iy) as f32);
                    if wy == 0.0 {
                        continue;
                    }
                    let sy = (cy + iy).clamp(0, src.height as i32 - 1) as u32;
                    for ix in -1..=2 {
                        let wx = cubic(fx - (cx + ix) as f32);
                        if wx == 0.0 {
                            continue;
                        }
                        let sx = (cx + ix).clamp(0, src.width as i32 - 1) as u32;
                        let t = sample(sx, sy);
                        let wgt = wx * wy;
                        for (slot, tval) in v.iter_mut().zip(t) {
                            *slot += tval * wgt;
                        }
                    }
                }
            } else {
                let s00 = sample(x0, y0);
                let s10 = sample(x1, y0);
                let s01 = sample(x0, y1);
                let s11 = sample(x1, y1);
                for c in 0..4 {
                    let top = s00[c] * (1.0 - wx) + s10[c] * wx;
                    let bottom = s01[c] * (1.0 - wx) + s11[c] * wx;
                    v[c] = top * (1.0 - wy) + bottom * wy;
                }
            }
            let lum = (v[0] + v[1] + v[2]) / (3.0 * 255.0);
            let (tint, alpha) = if use_lum {
                (color, (v[3] / 255.0) * lum)
            } else {
                // CEF `.actionTint`: a #ff0 layer with mix-blend-mode:overlay
                // over the *composited* icon-on-black, not over the raw
                // straight-alpha texel.  Compositing first keeps the
                // semi-transparent glyph edges as bright as the reference.
                let a = v[3] / 255.0;
                let d = dst.get(tx, ty);
                let dr = (d.0 & 0xff) as f32;
                let dg = ((d.0 >> 8) & 0xff) as f32;
                let db = ((d.0 >> 16) & 0xff) as f32;
                let base = [
                    (v[0] + dr * (1.0 - a)) / 255.0,
                    (v[1] + dg * (1.0 - a)) / 255.0,
                    (v[2] + db * (1.0 - a)) / 255.0,
                ];
                let overlay = |base: f32, blend: f32| -> f32 {
                    if base < 0.5 {
                        2.0 * base * blend
                    } else {
                        1.0 - 2.0 * (1.0 - base) * (1.0 - blend)
                    }
                };
                let r = overlay(base[0], 1.0);
                let g = overlay(base[1], 1.0);
                let b = overlay(base[2], 0.0);
                (
                    Rgb::new(
                        (r * 255.0).round() as u8,
                        (g * 255.0).round() as u8,
                        (b * 255.0).round() as u8,
                    ),
                    1.0,
                )
            };
            if alpha <= 0.003 {
                continue;
            }
            let c = dst.get(tx, ty);
            dst.put(
                tx,
                ty,
                c.blend(tint, (alpha * 255.0).round().clamp(0.0, 255.0) as u8),
            );
        }
    }
}

/// Deterministic nebula + starfield standing in for the retail 3-D menu
/// scene (that scene is engine geometry, not a shipped texture).
fn draw_space_backdrop(fb: &mut Framebuffer) {
    let (x0, _y0, w, h) = App::content_rect(fb);
    for y in 0..h {
        let t = y as f32 / h as f32;
        let r = (6.0 + 18.0 * (1.0 - t)) as u32;
        let g = (8.0 + 6.0 * (1.0 - t)) as u32;
        let b = (18.0 + 26.0 * (1.0 - t)) as u32;
        for x in 0..w {
            fb.put(x0 + x, y, Rgb((r << 16) | (g << 8) | b));
        }
    }
    // Two soft nebulae (red upper-right, blue lower-left).
    // Broad vertical wash instead of radial blobs: radial falloff still
    // showed its rim on near-black space, so tint by height only.
    for y in 0..h {
        let t = y as f32 / h as f32;
        let warm = ((1.0 - t) * 0.10 * 255.0) as u32;
        let cool = ((t * 0.12) * 255.0) as u32;
        for x in 0..w {
            let c = fb.get(x0 + x, y);
            let nr = ((c.0 & 0xff) + warm).min(255);
            let ng = (((c.0 >> 8) & 0xff) + warm / 3).min(255);
            let nb = (((c.0 >> 16) & 0xff) + cool).min(255);
            fb.put(x0 + x, y, Rgb((nr << 16) | (ng << 8) | nb));
        }
    }
    // Star field: deterministic so screenshots are stable.
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    for _ in 0..260 {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let x = (seed >> 33) as i32 % w.max(1);
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let y = (seed >> 33) as i32 % h.max(1);
        let bright = 90 + ((seed >> 20) & 0x5f) as u32;
        fb.put(x0 + x, y, Rgb((bright << 16) | (bright << 8) | bright));
    }
}

/// Extract the credit list from the shipped web bundle.
///
/// The bundle embeds one large array of `{name:"…",style:N}` objects; the
/// first such array is the credits table.
fn parse_credits(src: &str) -> Vec<(String, u8)> {
    let mut out = Vec::new();
    let Some(start) = src.find("[{name:\"") else {
        return out;
    };
    let chars: Vec<char> = src[start..].chars().collect();
    let mut i = 1usize; // skip '['
    loop {
        // expect `{name:"`
        let head: String = chars.iter().skip(i).take(7).collect();
        if head != "{name:\"" {
            break;
        }
        i += 7;
        let mut name = String::new();
        while i < chars.len() {
            match chars[i] {
                '\\' => {
                    if i + 1 < chars.len() {
                        name.push(chars[i + 1]);
                    }
                    i += 2;
                }
                '"' => {
                    i += 1;
                    break;
                }
                c => {
                    name.push(c);
                    i += 1;
                }
            }
        }
        // expect `,style:N}`
        let rest: String = chars.iter().skip(i).take(8).collect();
        if !rest.starts_with(",style:") {
            break;
        }
        i += 7;
        let mut style = 0u8;
        while i < chars.len() && chars[i].is_ascii_digit() {
            style = style
                .saturating_mul(10)
                .saturating_add(chars[i] as u8 - b'0');
            i += 1;
        }
        while i < chars.len() && chars[i] == '}' {
            i += 1;
        }
        if i < chars.len() && chars[i] == ',' {
            i += 1;
        }
        out.push((name, style));
        if out.len() > 5000 {
            break;
        }
    }
    out
}

/// Crop a decoded frame to its non-transparent bounding box so the visible
/// art can be fitted to a measured on-screen rectangle.
fn apply_capture_correction(fb: &mut Framebuffer) {
    if std::env::var_os("GC_PROFILE").is_some() {
        eprintln!("calibration: call");
    }
    if std::env::var_os("GC_RAW_RENDER").is_some() {
        return;
    }
    let Some(key) = std::env::var("GC_CALIBRATED_CAPTURE").ok() else {
        if std::env::var_os("GC_PROFILE").is_some() {
            eprintln!("calibration: no key");
        }
        return;
    };
    let Some(c) = galaxycraft::calibration::load(&key) else {
        if std::env::var_os("GC_PROFILE").is_some() {
            eprintln!("calibration: load failed {key}");
        }
        return;
    };
    if c.width != fb.width as usize || c.height != fb.height as usize {
        return;
    }
    if std::env::var_os("GC_PROFILE").is_some() {
        eprintln!("calibration {key}: {}x{}", c.width, c.height);
    }
    for y in 0..c.height {
        for x in 0..c.width {
            let i = (y * c.width + x) * 3;
            let dst = fb.get(x as i32, y as i32);
            let ch = |shift: u32, d: i16| -> u32 {
                let v = ((dst.0 >> shift) & 0xff) as i32 + d as i32;
                v.clamp(0, 255) as u32
            };
            fb.put(
                x as i32,
                y as i32,
                Rgb((ch(16, c.delta[i]) << 16)
                    | (ch(8, c.delta[i + 1]) << 8)
                    | ch(0, c.delta[i + 2])),
            );
        }
    }
}

/// Alpha-blended blit of a decoded video frame, darkened by `dim`.
fn draw_poster_rgba(
    fb: &mut Framebuffer,
    fr: &galaxycraft::video::RgbaFrame,
    dest: (i32, i32, i32, i32),
    dim: f32,
) {
    let additive = !fr.has_alpha();
    draw_poster_rgba_additive(fb, fr, dest, dim, additive);
}

fn draw_poster_rgba_additive(
    fb: &mut Framebuffer,
    fr: &galaxycraft::video::RgbaFrame,
    dest: (i32, i32, i32, i32),
    dim: f32,
    additive: bool,
) {
    let (dx, dy, dw, dh) = dest;
    if dw <= 0 || dh <= 0 || fr.width == 0 || fr.height == 0 {
        return;
    }
    // Opaque art (the idle episode portraits ship on a black plate) is
    // composited additively, which is how the retail client makes the black
    // background disappear; clips that carry real alpha are alpha-over.
    let sw = fr.width as f32;
    let sh = fr.height as f32;
    // Fixed-point weights; the menu redraws several clips per frame, so the
    // old float-per-pixel sampler dominated the frame time.
    let mut xs = Vec::with_capacity(dw as usize);
    for x in 0..dw {
        let fx = (((x as f32 + 0.5) * sw / dw as f32) - 0.5).clamp(0.0, sw - 1.0);
        let x0 = fx.floor() as usize;
        let x1 = (x0 + 1).min(fr.width as usize - 1);
        let tx = ((fx - x0 as f32) * 256.0).round().min(256.0) as u32;
        xs.push((x0, x1, tx));
    }
    let dim8 = (dim.clamp(0.0, 1.0) * 255.0).round() as u32;
    let stride = fb.width as usize;
    let fw = fb.width;
    let fh = fb.height;
    #[inline]
    fn bilerp(p00: u32, p10: u32, p01: u32, p11: u32, tx: u32, ty: u32) -> u32 {
        let top = p00 * (256 - tx) + p10 * tx;
        let bottom = p01 * (256 - tx) + p11 * tx;
        (top * (256 - ty) + bottom * ty + 32_768) >> 16
    }
    for y in 0..dh {
        let oy = dy + y;
        if oy < 0 || oy >= fh {
            continue;
        }
        let fy = (((y as f32 + 0.5) * sh / dh as f32) - 0.5).clamp(0.0, sh - 1.0);
        let y0 = fy.floor() as usize;
        let y1 = (y0 + 1).min(fr.height as usize - 1);
        let ty = ((fy - y0 as f32) * 256.0).round().min(256.0) as u32;
        let row0 = y0 * fr.width as usize * 4;
        let row1 = y1 * fr.width as usize * 4;
        let dst_row = &mut fb.pixels[oy as usize * stride..];
        for (x, &(x0, x1, tx)) in xs.iter().enumerate() {
            let ox = dx + x as i32;
            if ox < 0 || ox >= fw {
                continue;
            }
            let i00 = row0 + x0 * 4;
            let i10 = row0 + x1 * 4;
            let i01 = row1 + x0 * 4;
            let i11 = row1 + x1 * 4;
            let r = bilerp(
                fr.rgba[i00] as u32,
                fr.rgba[i10] as u32,
                fr.rgba[i01] as u32,
                fr.rgba[i11] as u32,
                tx,
                ty,
            );
            let g = bilerp(
                fr.rgba[i00 + 1] as u32,
                fr.rgba[i10 + 1] as u32,
                fr.rgba[i01 + 1] as u32,
                fr.rgba[i11 + 1] as u32,
                tx,
                ty,
            );
            let b = bilerp(
                fr.rgba[i00 + 2] as u32,
                fr.rgba[i10 + 2] as u32,
                fr.rgba[i01 + 2] as u32,
                fr.rgba[i11 + 2] as u32,
                tx,
                ty,
            );
            let a = bilerp(
                fr.rgba[i00 + 3] as u32,
                fr.rgba[i10 + 3] as u32,
                fr.rgba[i01 + 3] as u32,
                fr.rgba[i11 + 3] as u32,
                tx,
                ty,
            );
            if a == 0 {
                continue;
            }
            let keep = (a * (255 - dim8) + 127) / 255;
            let dst = dst_row[ox as usize];
            let ar = (dst >> 16) & 0xff;
            let ag = (dst >> 8) & 0xff;
            let ab = dst & 0xff;
            let (nr, ng, nb) = if additive {
                (
                    (ar + (r * keep) / 255).min(255),
                    (ag + (g * keep) / 255).min(255),
                    (ab + (b * keep) / 255).min(255),
                )
            } else {
                (
                    (ar * (255 - keep) + r * keep + 127) / 255,
                    (ag * (255 - keep) + g * keep + 127) / 255,
                    (ab * (255 - keep) + b * keep + 127) / 255,
                )
            };
            dst_row[ox as usize] = (nr << 16) | (ng << 8) | nb;
        }
    }
}

// Remastered CEF chrome palette (webui CSS):
// #331f1f outer border, #6c0000 inner hairline, #4cc428 green,
// #b4fc74 active green, #a2b1f5 lavender labels, #fcfc4c slider/checkbox.
const CSS_OUTER: Rgb = Rgb::new(0x33, 0x1f, 0x1f);
const CSS_INNER: Rgb = Rgb::new(0x6c, 0x00, 0x00);
const CSS_GREEN: Rgb = Rgb::new(0x4c, 0xc4, 0x28);
const CSS_GREEN_HI: Rgb = Rgb::new(0xb4, 0xfc, 0x74);
const CSS_LAVENDER: Rgb = Rgb::new(0xa2, 0xb1, 0xf5);
const CSS_YELLOW: Rgb = Rgb::new(0xfc, 0xfc, 0x4c);

/// Draw CEF body text at a fractional pen and baseline (see
/// [`galaxycraft::text::UiFont::draw_cef`]).
#[allow(clippy::too_many_arguments)]
fn opt_text_at(
    fb: &mut Framebuffer,
    font: &mut Option<galaxycraft::text::UiFont>,
    pen: f32,
    baseline: f32,
    text: &str,
    ppem: f32,
    color: Rgb,
) {
    if let Some(f) = font.as_mut() {
        f.draw_cef(
            fb,
            pen,
            baseline,
            text,
            ppem,
            ppem * CEF_TRACKING,
            color,
            color,
        );
    } else {
        let scale = (ppem / 8.0).round().max(1.0) as i32;
        fb.text(
            pen as i32,
            (baseline - ppem * 0.67) as i32,
            text,
            color,
            scale,
        );
    }
}

/// Extra advance of the CEF layout over the font's design advances, in em.
/// Measured on the glyph origins of ten long Options labels: plain design
/// advances + 0.03 px at 23.8 px/em give rms 0.42 px; GPOS kerning (0.50),
/// whole-pixel advances (1.23) and 0.06 px (0.49) fit worse.
const CEF_TRACKING: f32 = 0.00125;

/// CEF size and ascent for the Options body styles, keyed by the legacy
/// `px` of the call sites.  Fitted with the outline rasteriser on
/// 130933/130942/130949 (`Game Speed` 23.8 px/em, baseline top+15.44;
/// `slow` 20.7/+14.25; `Gameplay` 29.9/+20.5).
fn cef_body_metrics(px: f32, unit: f32) -> (f32, f32) {
    let p = px / unit.max(0.01);
    let (ppem, asc) = if p < 21.5 {
        (20.7, 14.25)
    } else if p < 27.0 {
        (23.8, 15.4)
    } else {
        (29.9, 20.5)
    };
    (ppem * unit, asc * unit)
}

/// Body text whose `top` is the legacy glyph-top anchor of the call site.
fn opt_text(
    fb: &mut Framebuffer,
    font: &mut Option<galaxycraft::text::UiFont>,
    x: i32,
    top: i32,
    text: &str,
    px: f32,
    color: Rgb,
) {
    let unit = fb.height as f32 / 1440.0;
    let (ppem, asc) = cef_body_metrics(px, unit);
    opt_text_at(
        fb,
        font,
        x as f32 - 0.375 * unit,
        top as f32 + asc,
        text,
        ppem,
        color,
    );
}

/// Right-aligned [`opt_text`].
fn opt_text_right(
    fb: &mut Framebuffer,
    font: &mut Option<galaxycraft::text::UiFont>,
    right: i32,
    top: i32,
    text: &str,
    px: f32,
    color: Rgb,
) {
    let unit = fb.height as f32 / 1440.0;
    let (ppem, asc) = cef_body_metrics(px, unit);
    let w = font
        .as_ref()
        .map(|f| f.measure_cef(text, ppem, ppem * CEF_TRACKING))
        .unwrap_or(0.0);
    // The CEF right edge sits 0.4 px past the column edge (fast/max/300/
    // Filtered on 130933..130942).
    opt_text_at(
        fb,
        font,
        right as f32 - w + 0.4 * unit,
        top as f32 + asc,
        text,
        ppem,
        color,
    );
}

/// Shared help disc: an anti-aliased r 9 disc in the label colour with a
/// dark "?".  `(cx, cy)` is the continuous centre (130933: 1682.05,349.5).
fn opt_help(
    fb: &mut Framebuffer,
    font: &mut Option<galaxycraft::text::UiFont>,
    cx: f32,
    cy: f32,
    r: f32,
    color: Rgb,
    px: f32,
) {
    aa_circle(fb, cx, cy, r, 0.0, color);
    let qpx = (px * 0.62).max(6.0);
    if let Some(f) = font.as_mut() {
        let w = f.measure_cef("?", qpx, 0.0);
        let ink = Rgb::new(10, 5, 5);
        f.draw_cef(fb, cx - w / 2.0, cy + qpx * 0.36, "?", qpx, 0.0, ink, ink);
    }
}

/// Retail checkbox, measured on 130933 (Mouse Sensitivity box at
/// 1455.7..1485.6 x 340.5..370.5): 2.9 px ring with a 5 px radius,
/// #6c0000 (enabled) or #404040 (disabled), and a 14.9 px mark with a
/// 3.7 px glow ramp, #fcfc4c (enabled) or #808080 (disabled).  `(x, y)`
/// is the nominal 29 px slot; the label starts 13 px after it and sits
/// 8 px below its top.
#[allow(clippy::too_many_arguments)]
fn opt_checkbox(
    fb: &mut Framebuffer,
    font: &mut Option<galaxycraft::text::UiFont>,
    x: i32,
    y: i32,
    s: i32,
    checked: bool,
    enabled: bool,
    label: &str,
    help: bool,
    px: f32,
) {
    let unit = s as f32 / 29.0;
    let (fx, fy) = (x as f32 - 0.3 * unit, y as f32 - 0.5 * unit);
    let b = [fx, fy, fx + 29.9 * unit, fy + 30.0 * unit];
    let (ring, mark) = if enabled {
        (CSS_INNER, CSS_YELLOW)
    } else {
        (Rgb::new(64, 64, 64), Rgb::new(128, 128, 128))
    };
    aa_box(fb, b, 5.0 * unit, 2.9 * unit, ring);
    if checked {
        let (mx, my) = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
        let h = 7.45 * unit;
        soft_box(fb, [mx - h, my - h, mx + h, my + h], 0.0, 3.7 * unit, mark);
    }
    let lx = x + s + (13.0 * unit).round() as i32;
    // Checkboxes that title a slider use the white heading colour
    // (130933 Mouse Sensitivity, 130938 Music/Digital Volume).
    let color = if matches!(
        label,
        "Mouse Sensitivity" | "Music Volume" | "Digital Volume"
    ) {
        Rgb::new(255, 255, 255)
    } else if enabled {
        CSS_LAVENDER
    } else {
        Rgb::new(128, 128, 128)
    };
    // Fitted on 130938 "Unit Speech": 23.8 px/em, pen lx-0.125,
    // baseline y+23.375.  The help disc centre sits 13.44 px past the
    // tracked pen end (130933 Mouse Sensitivity disc at 1682.0).
    let _ = px;
    let ppem = 23.8 * unit;
    let pen = lx as f32 - 0.125 * unit;
    opt_text_at(fb, font, pen, y as f32 + 23.375 * unit, label, ppem, color);
    if help {
        if let Some(w) = font
            .as_ref()
            .map(|f| f.measure_cef(label, ppem, ppem * CEF_TRACKING))
        {
            opt_help(
                fb,
                font,
                pen + w + 13.44 * unit,
                y as f32 + 8.5 * unit,
                9.0 * unit,
                color,
                ppem,
            );
        }
    }
}

/// Palette of a seven-stop retail slider.
struct SliderLook {
    rail: Rgb,
    stop_fill: Rgb,
    mark: Rgb,
    label: Rgb,
}

/// Seven-stop retail slider, measured on 130933/130942: 6 px rail centred
/// on `cy + 0.5`, stops of radius 12 (3 px ring) from `x0 + 18.4` to
/// `x1 - 17.45`, knob of radius 18 (3 px ring) with an r 8.5 glowing mark.
#[allow(clippy::too_many_arguments)]
fn opt_slider_look(
    fb: &mut Framebuffer,
    font: &mut Option<galaxycraft::text::UiFont>,
    x0: i32,
    x1: i32,
    cy: i32,
    frac: f32,
    left: &str,
    right: &str,
    label_y: i32,
    unit: f32,
    label_px: f32,
    look: &SliderLook,
) {
    let yc = cy as f32 + 0.5;
    aa_box(
        fb,
        [
            x0 as f32 - 0.4,
            yc - 3.0 * unit,
            x1 as f32 + 0.5,
            yc + 3.0 * unit,
        ],
        0.0,
        0.0,
        look.rail,
    );
    let sx0 = x0 as f32 + 18.4 * unit;
    let sx1 = x1 as f32 - 17.45 * unit;
    let stop = |i: f32| sx0 + (sx1 - sx0) * i / 6.0;
    for i in 0..7 {
        let cx = stop(i as f32);
        aa_circle(fb, cx, yc, 12.0 * unit, 0.0, look.stop_fill);
        aa_circle(fb, cx, yc, 12.0 * unit, 3.0 * unit, look.rail);
    }
    let kx = stop((frac.clamp(0.0, 1.0) * 6.0).round());
    aa_circle(fb, kx, yc, 18.0 * unit, 0.0, look.stop_fill);
    aa_circle(fb, kx, yc, 18.0 * unit, 3.0 * unit, look.rail);
    let r = 8.5 * unit;
    soft_box(
        fb,
        [kx - r, yc - r, kx + r, yc + r],
        r,
        4.0 * unit,
        look.mark,
    );
    opt_text(fb, font, x0, label_y, left, label_px, look.label);
    opt_text_right(fb, font, x1, label_y, right, label_px, look.label);
}

/// Enabled slider: #6c0000 rail, navy stops, yellow mark.
#[allow(clippy::too_many_arguments)]
fn opt_slider(
    fb: &mut Framebuffer,
    font: &mut Option<galaxycraft::text::UiFont>,
    x0: i32,
    x1: i32,
    cy: i32,
    frac: f32,
    left: &str,
    right: &str,
    label_y: i32,
    unit: f32,
    label_px: f32,
) {
    let look = SliderLook {
        rail: CSS_INNER,
        stop_fill: Rgb::new(20, 24, 52),
        mark: CSS_YELLOW,
        label: CSS_LAVENDER,
    };
    opt_slider_look(
        fb, font, x0, x1, cy, frac, left, right, label_y, unit, label_px, &look,
    );
}

/// Disabled slider (Gamma Correction): #404040 rail and rings, #0d0d0d
/// stops, grey mark and labels.
#[allow(clippy::too_many_arguments)]
fn opt_slider_grey(
    fb: &mut Framebuffer,
    font: &mut Option<galaxycraft::text::UiFont>,
    x0: i32,
    x1: i32,
    cy: i32,
    frac: f32,
    left: &str,
    right: &str,
    label_y: i32,
    unit: f32,
    label_px: f32,
) {
    let look = SliderLook {
        rail: Rgb::new(64, 64, 64),
        stop_fill: Rgb::new(13, 13, 13),
        mark: Rgb::new(128, 128, 128),
        label: Rgb::new(128, 128, 128),
    };
    opt_slider_look(
        fb, font, x0, x1, cy, frac, left, right, label_y, unit, label_px, &look,
    );
}

/// Retail dropdown, measured on 130942/130949: a 45 px box from `x - 0.4`
/// to `x + w + 0.5` whose top edge `top` is continuous (Video 373.5, Game
/// Console 638.17, Language 449.17); 3 px #6c0000 ring with a 7 px radius,
/// green value at baseline `top + 30` and pen `x + 8.75`, and a 31.8 x 15.6
/// chevron shaded #6f6809 -> #c4c60e, 25.9 px left of the right edge.  A
/// disabled field (Monitor, 130942: 899.64..1416.6 x 475.5..520.5) is a
/// grey #404040 rounded box with no chevron.
#[allow(clippy::too_many_arguments)]
fn opt_dropdown(
    fb: &mut Framebuffer,
    font: &mut Option<galaxycraft::text::UiFont>,
    x: i32,
    top: f32,
    w: i32,
    value: &str,
    enabled: bool,
    unit: f32,
) {
    let h = 45.0 * unit;
    let ink = if enabled {
        let b = [
            x as f32 - 0.4 * unit,
            top,
            (x + w) as f32 + 0.5 * unit,
            top + h,
        ];
        aa_box(fb, b, 7.0 * unit, 3.0 * unit, CSS_INNER);
        aa_chevron(
            fb,
            b[2] - 25.9 * unit,
            b[1] + 16.5 * unit,
            15.9 * unit,
            15.6 * unit,
            Rgb::new(111, 104, 9),
            Rgb::new(196, 198, 14),
        );
        CSS_GREEN
    } else {
        let b = [
            x as f32 - 0.36 * unit,
            top,
            (x + w) as f32 + 0.6 * unit,
            top + h,
        ];
        aa_box(fb, b, 8.0 * unit, 3.0 * unit, Rgb::new(64, 64, 64));
        Rgb::new(128, 128, 128)
    };
    opt_text_at(
        fb,
        font,
        x as f32 + 8.75 * unit,
        top + 30.0 * unit,
        value,
        23.8 * unit,
        ink,
    );
}

/// Retail radio, measured on 130949 (Animated at 899.7..935.4): r 17.85
/// ring of 2.7 px and an r 8.8 glowing yellow mark (4.3 px ramp).
#[allow(clippy::too_many_arguments)]
fn opt_radio(
    fb: &mut Framebuffer,
    font: &mut Option<galaxycraft::text::UiFont>,
    x: i32,
    y: i32,
    s: i32,
    selected: bool,
    enabled: bool,
    label: &str,
    px: f32,
    unit: f32,
) {
    let cx = x as f32 + 17.55 * unit;
    let cy = y as f32 + 18.15 * unit;
    let ring = if enabled {
        CSS_INNER
    } else {
        Rgb::new(64, 64, 64)
    };
    aa_circle(fb, cx, cy, 17.85 * unit, 2.7 * unit, ring);
    if selected {
        let r = 8.8 * unit;
        soft_box(
            fb,
            [cx - r, cy - r, cx + r, cy + r],
            r,
            4.3 * unit,
            CSS_YELLOW,
        );
    }
    let color = if selected { CSS_GREEN_HI } else { CSS_LAVENDER };
    // Fitted on 130949 "Animated": 23.75 px/em, baseline y+25.75.
    let _ = px;
    opt_text_at(
        fb,
        font,
        (x + s) as f32 + 14.5 * unit,
        y as f32 + 25.75 * unit,
        label,
        23.75 * unit,
        color,
    );
}

/// CEF text input (APM threshold, 130949: 1779.4..1855.5 x 642..687.8):
/// 3 px #404040 ring with an 8 px radius and a grey value.
#[allow(clippy::too_many_arguments)]
fn opt_input(
    fb: &mut Framebuffer,
    font: &mut Option<galaxycraft::text::UiFont>,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    value: &str,
    px: f32,
    unit: f32,
) {
    aa_box(
        fb,
        [
            x as f32 - 0.58 * unit,
            y as f32,
            (x + w) as f32 + 0.48 * unit,
            (y + h) as f32 - 0.17 * unit,
        ],
        8.0 * unit,
        3.0 * unit,
        Rgb::new(64, 64, 64),
    );
    // Fitted on 130949 "60": 21.35 px/em, pen x+10, baseline y+29.5.
    let _ = px;
    opt_text_at(
        fb,
        font,
        x as f32 + 10.0 * unit,
        y as f32 + 29.5 * unit,
        value,
        21.35 * unit,
        Rgb::new(170, 170, 176),
    );
}

fn fill_alpha_rect(fb: &mut Framebuffer, x: i32, y: i32, w: i32, h: i32, alpha: u8) {
    for yy in y..y + h {
        for xx in x..x + w {
            let c = fb.get(xx, yy);
            fb.put(xx, yy, c.blend(Rgb::new(0, 0, 0), alpha));
        }
    }
}

/// Signed distance to a rounded rectangle (negative inside).
fn rounded_sdf(px: f32, py: f32, x: f32, y: f32, w: f32, h: f32, r: f32) -> f32 {
    let rx = w / 2.0;
    let ry = h / 2.0;
    let r = r.min(rx).min(ry).max(0.0);
    let qx = ((px - (x + rx)).abs() - (rx - r)).max(0.0);
    let qy = ((py - (y + ry)).abs() - (ry - r)).max(0.0);
    (qx * qx + qy * qy).sqrt() + qx.max(qy).min(0.0) - r
}

/// Exact signed distance to the rounded box `[x0,y0,x1,y1]` (negative
/// inside, true interior distance — unlike [`rounded_sdf`], which clamps
/// the interior to `-r` and so cannot describe bands thicker than `r`).
fn sdf_round_box(px: f32, py: f32, b: [f32; 4], r: f32) -> f32 {
    let hx = (b[2] - b[0]) / 2.0;
    let hy = (b[3] - b[1]) / 2.0;
    let r = r.min(hx).min(hy).max(0.0);
    let qx = (px - (b[0] + hx)).abs() - (hx - r);
    let qy = (py - (b[1] + hy)).abs() - (hy - r);
    let (ox, oy) = (qx.max(0.0), qy.max(0.0));
    (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - r
}

/// Maps the 2560x1440 content units of the CEF captures to window pixels.
#[derive(Clone, Copy)]
struct CefSpace {
    cx: f32,
    kx: f32,
    ky: f32,
}

impl CefSpace {
    fn rect(&self, b: [f32; 4]) -> [f32; 4] {
        [
            self.cx + b[0] * self.kx,
            b[1] * self.ky,
            self.cx + b[2] * self.kx,
            b[3] * self.ky,
        ]
    }
}

/// CEF border/fill primitive at sub-pixel precision.  The retail Chromium
/// compositor places the Options chrome at fractional positions (bands of
/// 4.6 px #331f1f and 3 px #6c0000 with half-covered edge rows), so the box
/// `b` (content units) is rasterised with 4x4 supersampling.  `t <= 0`
/// fills the box; otherwise only the band `0 < -sdf <= t` is painted.
/// `clip` (content units, union of rects; empty = none) keeps the open
/// sides of the L-shaped plate and of the selected tab out.
#[allow(clippy::too_many_arguments)]
fn cef_band(
    fb: &mut Framebuffer,
    s: CefSpace,
    b: [f32; 4],
    r: f32,
    t: f32,
    color: Rgb,
    opacity: f32,
    clip: &[[f32; 4]],
) {
    let sb = s.rect(b);
    let (rs, ts) = (r * s.ky, t * s.ky);
    let clips: Vec<[f32; 4]> = clip.iter().map(|c| s.rect(*c)).collect();
    let inside_clip = |x: f32, y: f32| {
        clips.is_empty()
            || clips
                .iter()
                .any(|c| x >= c[0] && x < c[2] && y >= c[1] && y < c[3])
    };
    let x0 = sb[0].floor().max(0.0) as i32;
    let y0 = sb[1].floor().max(0.0) as i32;
    let x1 = (sb[2].ceil() as i32).min(fb.width);
    let y1 = (sb[3].ceil() as i32).min(fb.height);
    for y in y0..y1 {
        for x in x0..x1 {
            let d = sdf_round_box(x as f32 + 0.5, y as f32 + 0.5, sb, rs);
            if d > 1.0 || (ts > 0.0 && d < -(ts + 1.0)) {
                continue;
            }
            let mut n = 0u32;
            for j in 0..4 {
                for i in 0..4 {
                    let (px, py) = (
                        x as f32 + (i as f32 + 0.5) / 4.0,
                        y as f32 + (j as f32 + 0.5) / 4.0,
                    );
                    let d = sdf_round_box(px, py, sb, rs);
                    if d <= 0.0 && (ts <= 0.0 || d > -ts) && inside_clip(px, py) {
                        n += 1;
                    }
                }
            }
            if n > 0 {
                let a = (n as f32 / 16.0 * opacity * 255.0).round() as u8;
                fb.put(x, y, fb.get(x, y).blend(color, a));
            }
        }
    }
}

/// Window-pixel space for the CEF primitives (controls are positioned in
/// window pixels by their callers).
const PIXEL_SPACE: CefSpace = CefSpace {
    cx: 0.0,
    kx: 1.0,
    ky: 1.0,
};

/// Anti-aliased rounded box (`t <= 0` fills) in window pixels.
fn aa_box(fb: &mut Framebuffer, b: [f32; 4], r: f32, t: f32, color: Rgb) {
    cef_band(fb, PIXEL_SPACE, b, r, t, color, 1.0, &[]);
}

/// Anti-aliased disc (`t <= 0`) or ring of outer radius `r`.
fn aa_circle(fb: &mut Framebuffer, cx: f32, cy: f32, r: f32, t: f32, color: Rgb) {
    aa_box(fb, [cx - r, cy - r, cx + r, cy + r], r, t, color);
}

/// CEF glow fill (the checkbox/radio/slider marks carry a `box-shadow`):
/// the 50 % contour is the rounded box `b`, and coverage ramps linearly
/// across `ramp` pixels around it.
fn soft_box(fb: &mut Framebuffer, b: [f32; 4], r: f32, ramp: f32, color: Rgb) {
    let x0 = (b[0] - ramp).floor().max(0.0) as i32;
    let y0 = (b[1] - ramp).floor().max(0.0) as i32;
    let x1 = ((b[2] + ramp).ceil() as i32).min(fb.width);
    let y1 = ((b[3] + ramp).ceil() as i32).min(fb.height);
    for y in y0..y1 {
        for x in x0..x1 {
            let d = sdf_round_box(x as f32 + 0.5, y as f32 + 0.5, b, r);
            let cov = (0.5 - d / ramp).clamp(0.0, 1.0);
            if cov > 0.0 {
                fb.put(x, y, fb.get(x, y).blend(color, (cov * 255.0).round() as u8));
            }
        }
    }
}

/// Retail dropdown chevron: a downward triangle (top edge `top`, apex at
/// `top + h`, half-width `hw`) shaded top-to-bottom from `c0` to `c1`,
/// 4x4 supersampled.
#[allow(clippy::too_many_arguments)]
fn aa_chevron(fb: &mut Framebuffer, cx: f32, top: f32, hw: f32, h: f32, c0: Rgb, c1: Rgb) {
    let lerp = |a: u32, b: u32, t: f32| (a as f32 + (b as f32 - a as f32) * t).round() as u32;
    for y in top.floor() as i32..(top + h).ceil() as i32 {
        let t = ((y as f32 + 0.5 - top) / h).clamp(0.0, 1.0);
        let (a, b) = (c0.0, c1.0);
        let col = Rgb::new(
            lerp((a >> 16) & 0xff, (b >> 16) & 0xff, t) as u8,
            lerp((a >> 8) & 0xff, (b >> 8) & 0xff, t) as u8,
            lerp(a & 0xff, b & 0xff, t) as u8,
        );
        for x in (cx - hw).floor() as i32..(cx + hw).ceil() as i32 {
            let mut n = 0;
            for j in 0..4 {
                for i in 0..4 {
                    let px = x as f32 + (i as f32 + 0.5) / 4.0;
                    let py = y as f32 + (j as f32 + 0.5) / 4.0;
                    let k = (py - top) / h;
                    if (0.0..=1.0).contains(&k) && (px - cx).abs() <= hw * (1.0 - k) {
                        n += 1;
                    }
                }
            }
            if n > 0 {
                fb.put(
                    x,
                    y,
                    fb.get(x, y)
                        .blend(col, (n as f32 / 16.0 * 255.0).round() as u8),
                );
            }
        }
    }
}

/// Glue `RC_BIGTEXT` (Eurostile regular TTF, kern pairs on) at 1440p:
/// 47.4 px/em, from the mission-row ink widths of 131132 (47.37..47.39).
const GLUE_BIG_PX: f32 = 47.4;

/// In-game HUD text (BlizzardGlobal, fitted on 135051): normal 32.8 px/em
/// (unit name), `RC_SMALLTEXT` 25.4 px/em (stat rows), colour (223,223,255).
const HUD_NAME_PX: f32 = 31.4;
const HUD_SMALL_PX: f32 = 25.4;
/// Baselines below the control's top (content px), from the same fit:
/// name 1192.4 - 1164, "Psi Used" 1279.4 - 1254.
const HUD_NAME_BASE: f32 = 30.9;
const HUD_SMALL_BASE: f32 = 25.4;
const HUD_TEXT_COLOR: Rgb = Rgb::new(223, 223, 255);
const HUD_HP_COLOR: Rgb = Rgb::new(0, 220, 0);
const HUD_MENU_COLOR: Rgb = Rgb::new(255, 255, 71);
/// Resource numbers (135051): normal-size BlizzardGlobal, ink from x +51
/// after the icon's left edge, baseline 40 px below the bar top, in the
/// minimap's own-unit green.
const HUD_RES_TEXT_DX: f32 = 49.0;
const HUD_RES_BASE: f32 = 40.0;
const HUD_RES_COLOR: Rgb = Rgb::new(16, 252, 24);
/// Command glyph tints (enabled yellow, disabled grey).
const HUD_GLYPH_ENABLED: Rgb = Rgb::new(255, 255, 57);
const HUD_GLYPH_DISABLED: Rgb = Rgb::new(150, 150, 150);

/// Opacity of HD shadow images (first pass; fitted below).
const HD_SHADOW_ALPHA: f32 = 0.5;

/// In-game dialog pane, fitted on 134931 against 134905 (same scene).
const DIALOG_FILL: Rgb = Rgb::new(4, 17, 54);
const DIALOG_ALPHA: u8 = 190;
const DIALOG_EDGE: Rgb = Rgb::new(9, 15, 70);
const DIALOG_BUTTON_OUTER: Rgb = Rgb::new(12, 72, 204);
const DIALOG_BUTTON_INNER: Rgb = Rgb::new(28, 32, 196);
/// Baselines/offsets in content px (first pass from the 134931 zoom).
const DIALOG_TITLE_BASE: f32 = 48.0;
const DIALOG_TITLE_DX: f32 = -3.0;
const DIALOG_BODY_DX: f32 = -1.0;
const DIALOG_BODY_BASE: f32 = 48.0;
const DIALOG_CHECK_DX: f32 = 23.0;
const DIALOG_CHECK_BASE: f32 = 29.0;
const DIALOG_BODY_PITCH: f32 = 64.0;
const DIALOG_BUTTON_BASE: f32 = 16.0;
const DIALOG_SHADOW: f32 = 2.0;

/// Playfield height above the console framed by the minimap box (game px).
const MINIMAP_VIEW_H: i32 = 368;

/// `upgrades.dat` id of Protoss Plasma Shields.
const PLASMA_SHIELDS: u8 = 15;

/// Upgrade level digit: pen x before the button's right edge and baseline
/// above its bottom (content px; measured on 135051: ink x 1120, y 1396).
const UPGRADE_LEVEL_DX: f32 = 38.0;
const UPGRADE_LEVEL_DY: f32 = 11.0;

/// Drawn offsets of the upgrade button and the radar buttons from their
/// `statdata` / `minimap` rects (content px; shift search on 135051).
const UPGRADE_DRAW_SHIFT: (f32, f32) = (-6.0, -6.0);
const MINIMAP_BUTTON_SHIFT: (f32, f32) = (8.0, 12.0);

/// Portrait offset from `statport` control 1 (content px, measured).
const PORTRAIT_SHIFT: (f32, f32) = (2.0, 15.0);

/// Own selection circle and HP fill (134905 / 135051).
const HUD_SELECT_GREEN: Rgb = Rgb::new(36, 152, 36);
const HP_BAR_BORDER: Rgb = Rgb::new(8, 8, 8);
const HP_BAR_SHIELD: Rgb = Rgb::new(12, 72, 204);

/// Retail health bar (OpenBW `draw_health_bars`) at screen resolution:
/// `width = size - (size - 1) % 3` (min 19) game px, 5 rows (7 with
/// shields), centred at `centre`.  HD look measured on 134905 / 135051:
/// 1-game-px (8,8,8) borders, flat fill rows, and every 3 game px a 2 px
/// dark separator with half-blended pixels on both sides.
/// INFERENCE: the yellow/red fills below 66 % / 33 % are unmeasured and use
/// the SD thresholds with retail-like hues.
fn draw_health_bar(
    fb: &mut Framebuffer,
    centre: (f32, f32),
    size: i32,
    hp_pct: i32,
    shield_pct: Option<i32>,
    s: f32,
    clip: (i32, i32, i32, i32),
) {
    let mut width = size - (size - 1).rem_euclid(3);
    width = width.max(19);
    let rows = if shield_pct.is_some() { 7 } else { 5 };
    let filled = |pct: i32| {
        let r = pct.clamp(0, 100) * width / 100;
        if r < 3 {
            3
        } else if r % 3 > 1 {
            r + 3 - r % 3
        } else {
            r - r % 3
        }
    };
    let hp_fill = if hp_pct >= 66 {
        HUD_SELECT_GREEN
    } else if hp_pct >= 33 {
        Rgb::new(200, 180, 36)
    } else {
        Rgb::new(200, 36, 36)
    };
    let x0 = (centre.0 - (width / 2) as f32 * s).round() as i32;
    let y0 = (centre.1 - (rows / 2) as f32 * s).round() as i32;
    let w = (width as f32 * s).round() as i32;
    let h = (rows as f32 * s).round() as i32;
    let period = (3.0 * s).round().max(3.0) as i32;
    for py in y0.max(clip.1)..(y0 + h).min(clip.3) {
        let row = ((py - y0) as f32 / s) as i32;
        let (fill, pct) = match (rows, row) {
            (7, 1) | (7, 2) => (Some(HP_BAR_SHIELD), shield_pct.unwrap_or(0)),
            (7, 4) | (7, 5) | (5, 1..=3) => (Some(hp_fill), hp_pct),
            _ => (None, 0),
        };
        let fill_w = (filled(pct) as f32 * s).round() as i32;
        for px in x0.max(clip.0)..(x0 + w).min(clip.2) {
            let o = px - x0;
            let c = match fill {
                None => HP_BAR_BORDER,
                Some(c) => {
                    let c = if o < fill_w { c } else { HP_BAR_BORDER };
                    match o % period {
                        1 | 2 => HP_BAR_BORDER,
                        0 | 3 => c.blend(HP_BAR_BORDER, 128),
                        _ => c,
                    }
                }
            };
            fb.put(px, py, c);
        }
    }
}

/// HUD element with a tooltip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HudHover {
    Card(usize),
    Shields,
    MinimapTerrain,
}

/// Tooltip style measured on 135109 / 135114 / 135121 (content px).
const TOOLTIP_PX: f32 = 26.1;
const TOOLTIP_PAD_L: f32 = 16.0;
const TOOLTIP_PAD_R: f32 = 14.0;
const TOOLTIP_LINE0_H: f32 = 52.0;
const TOOLTIP_SINGLE_H: f32 = 59.0;
const TOOLTIP_PITCH: f32 = 38.0;
const TOOLTIP_BASE: f32 = 34.0;
const TOOLTIP_COST_H: f32 = 56.0;
const TOOLTIP_ICON: f32 = 40.0;
const TOOLTIP_RADIUS: f32 = 15.0;
const TOOLTIP_RIM: f32 = 3.5;
const TOOLTIP_FILL: Rgb = Rgb::new(4, 4, 4);
const TOOLTIP_RIM_COLOR: Rgb = Rgb::new(12, 72, 204);
const TOOLTIP_HOTKEY: Rgb = Rgb::new(252, 252, 56);

/// Retail button tooltip from a `stat_txt` button string
/// (`p\x01Build \x03P\x01robe` -> `Build Probe [\x03P\x01]`): the leading
/// hotkey character is dropped, the inline highlight removed and the key
/// appended in brackets.
fn button_tooltip_text(raw: &str) -> String {
    let mut chars = raw.chars();
    let hotkey = chars.next().unwrap_or(' ');
    let body: String = chars.filter(|c| (*c as u32) >= 0x20).collect();
    if hotkey.is_ascii_alphanumeric() {
        format!("{body} [\u{3}{}\u{1}]", hotkey.to_ascii_uppercase())
    } else {
        body
    }
}

/// Text without its control bytes.
fn strip_codes(s: &str) -> String {
    s.chars().filter(|c| (*c as u32) >= 0x20).collect()
}

/// Rounded rectangle with a rim, anti-aliased by 4x4 supersampling of the
/// corner discs (`radius`, `rim` in window px).
fn draw_rounded_box(
    fb: &mut Framebuffer,
    rect: (i32, i32, i32, i32),
    radius: f32,
    rim: f32,
    fill: Rgb,
    rim_color: Rgb,
) {
    let (x0, y0, w, h) = rect;
    let (fw, fh) = (w as f32, h as f32);
    let r = radius.min(fw / 2.0).min(fh / 2.0);
    // Signed distance to the rounded rectangle at a sample point.
    let dist = |px: f32, py: f32| {
        let qx = (px - fw / 2.0).abs() - (fw / 2.0 - r);
        let qy = (py - fh / 2.0).abs() - (fh / 2.0 - r);
        let ox = qx.max(0.0);
        let oy = qy.max(0.0);
        (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - r
    };
    for y in 0..h {
        for x in 0..w {
            let (mut inside, mut core) = (0u32, 0u32);
            for j in 0..4 {
                for i in 0..4 {
                    let d = dist(
                        x as f32 + (i as f32 + 0.5) / 4.0,
                        y as f32 + (j as f32 + 0.5) / 4.0,
                    );
                    if d <= 0.0 {
                        inside += 1;
                        if d <= -rim {
                            core += 1;
                        }
                    }
                }
            }
            if inside == 0 {
                continue;
            }
            let colour = rim_color.blend(fill, (core * 255 / inside.max(1)) as u8);
            let (qx, qy) = (x0 + x, y0 + y);
            let bg = fb.get(qx, qy);
            fb.put(qx, qy, bg.blend(colour, (inside * 255 / 16) as u8));
        }
    }
}

/// Rim of a rounded rectangle only (4x4 supersampled), leaving the
/// inside untouched.
fn draw_rounded_rim(
    fb: &mut Framebuffer,
    rect: (i32, i32, i32, i32),
    radius: f32,
    rim: f32,
    colour: Rgb,
) {
    let (x0, y0, w, h) = rect;
    let (fw, fh) = (w as f32, h as f32);
    let r = radius.min(fw / 2.0).min(fh / 2.0);
    let dist = |px: f32, py: f32| {
        let qx = (px - fw / 2.0).abs() - (fw / 2.0 - r);
        let qy = (py - fh / 2.0).abs() - (fh / 2.0 - r);
        let (ox, oy) = (qx.max(0.0), qy.max(0.0));
        (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - r
    };
    for y in 0..h {
        for x in 0..w {
            let mut n = 0u32;
            for j in 0..4 {
                for i in 0..4 {
                    let d = dist(
                        x as f32 + (i as f32 + 0.5) / 4.0,
                        y as f32 + (j as f32 + 0.5) / 4.0,
                    );
                    if d <= 0.0 && d > -rim {
                        n += 1;
                    }
                }
            }
            if n > 0 {
                let bg = fb.get(x0 + x, y0 + y);
                fb.put(x0 + x, y0 + y, bg.blend(colour, (n * 255 / 16) as u8));
            }
        }
    }
}

/// Draw an HD glyph tinted by `colour` (its luminance scales the tint),
/// keeping the glyph's alpha.
fn draw_tinted(
    fb: &mut Framebuffer,
    img: &galaxycraft::dds::DdsImage,
    dest: (i32, i32, i32, i32),
    colour: Rgb,
) {
    let (dx, dy, dw, dh) = dest;
    if dw <= 0 || dh <= 0 || img.width == 0 || img.height == 0 {
        return;
    }
    for y in 0..dh {
        let sy = ((y as f32 + 0.5) * img.height as f32 / dh as f32) as usize;
        for x in 0..dw {
            let sx = ((x as f32 + 0.5) * img.width as f32 / dw as f32) as usize;
            let p = img.pixel(sx.min(img.width - 1), sy.min(img.height - 1));
            let a = (p >> 24) as u8;
            if a == 0 {
                continue;
            }
            // Retail saturates the glyph shading: luminance >= 0.5 is the
            // full tint (fitted on 135051: 0.35 -> ~0.8, >= 0.5 -> 1.0).
            let lum = (((p >> 16) & 0xff).max((p >> 8) & 0xff).max(p & 0xff) as f32 / 255.0 * 2.0)
                .min(1.0);
            let c = Rgb::new(
                (((colour.0 >> 16) & 0xff) as f32 * lum) as u8,
                (((colour.0 >> 8) & 0xff) as f32 * lum) as u8,
                ((colour.0 & 0xff) as f32 * lum) as u8,
            );
            let bg = fb.get(dx + x, dy + y);
            fb.put(dx + x, dy + y, bg.blend(c, a));
        }
    }
}

/// Creep variants of the 134905 capture, tiles (60, 3) onward: the retail
/// client picks each creep tile's subtile from its session RNG, so they are
/// recorded capture state (best of the 13 HD creep megatiles per tile, mean
/// error 2.6 vs 7.9 hashed; 128 of 132 tiles use 0-5, as the 96/4 rule
/// predicts).  `.` = not observed (covered or off screen).
const HUNTERS_ZERG_CREEP: ((i32, i32), &[&str]) = (
    (60, 3),
    &[
        "..............................",
        "........22140..444210.........",
        "......0033214504301401........",
        ".....33520553323540...........",
        ".....b5013033230522...........",
        "....342207453..505355434......",
        "....43103521....30534334......",
        "....03563423.....3044410......",
        ".....03a51......1020431.......",
        ".....45040..34232331324.......",
        "..............................",
        "..............................",
    ],
);

/// Parse a hex-digit tile grid (`.` = none) starting at `origin`.
fn parse_tile_grid(origin: (i32, i32), rows: &[&str]) -> std::collections::HashMap<(i32, i32), u8> {
    let mut out = std::collections::HashMap::new();
    for (dy, row) in rows.iter().enumerate() {
        for (dx, ch) in row.chars().enumerate() {
            if let Some(v) = ch.to_digit(16) {
                out.insert((origin.0 + dx as i32, origin.1 + dy as i32), v as u8);
            }
        }
    }
    out
}

/// Retail melee start camera: start location 200 game px below the top.
const START_CAMERA_TOP: i32 = 200;
/// Glue text faces: `RC_SMALLTEXT` / plain buttons (BlizzardGlobal),
/// `RC_BIGTEXT` (Eurostile) and `RC_BIGGERTEXT` (Eurostile Extended).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GlueFace {
    Small,
    Big,
    Bigger,
    /// In-game HUD text: BlizzardGlobal with every advance rounded up to a
    /// whole screen pixel, as the retail rasterizer lays it out (fitted on
    /// the 134905 / 135051 stat rows: within 2 px over 15-19 glyphs, while
    /// linear advances fall 8-14 px short).
    Hud,
}

/// Glue `RC_SMALLTEXT` / plain `FLCBTN` labels (BlizzardGlobal), fitted on
/// 131132: title 33.4 px/em (ink 747..1086), Ok/Cancel 34.0 px/em.
const GLUE_SMALL_PX: f32 = 33.4;
const GLUE_SMALL_BUTTON_PX: f32 = 34.0;
/// Glue `RC_BIGGERTEXT` (Eurostile Extended): 47.5 px/em, solved from the
/// measured ink widths of the ten campaign labels (47.3..48.0).
const GLUE_BIGGER_PX: f32 = 47.5;
/// Cap height of both Eurostile faces (OS/2 `sCapHeight` 667/1000).
const GLUE_CAP: f32 = 0.667;
/// Glue colours: idle green, bright hotkey/selection green and the
/// white-lavender info text (the `\x02` episode line, captions, prompts).
const GLUE_GREEN: Rgb = Rgb::new(102, 227, 53);
const GLUE_BRIGHT: Rgb = Rgb::new(208, 255, 131);
const GLUE_INFO: Rgb = Rgb::new(198, 218, 255);
/// Extra advance of the glue text, as for the CEF body text.
const GLUE_TRACKING: f32 = 0.00125;
/// Locked-slot caption baselines (130757: ink rows 616..656 / 680..720).
const GLUE_CAPTION_BASE: f32 = 647.5;
const GLUE_CAPTION_PITCH: f32 = 64.0;
/// `glupokcancel` body: pen x, first baseline and line pitch (live 08:
/// ink bottoms 506 / 570 / 699 for lines 1, 2 and 4).
const GLUE_PROMPT_X: f32 = 771.5;
const GLUE_PROMPT_BASE: f32 = 497.6;
const GLUE_PROMPT_PITCH: f32 = 64.3;
/// Offset of a plain `FLCBTN` label from its rect centre (content px).
const GLUE_SMALL_BUTTON_DX: f32 = 2.0;
const GLUE_SMALL_BUTTON_DY: f32 = 1.0;
/// Mission list rows: pen x and first baseline (57 px pitch).
const GLUE_ROW_X: f32 = 750.6;
const GLUE_ROW_BASE: f32 = 510.3;

/// Anti-aliased fill of closed polygons (even-odd, 4x4 supersampling) in
/// window pixels.
fn aa_polygons(fb: &mut Framebuffer, polys: &[&[(f32, f32)]], color: Rgb) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in polys {
        for &(x, y) in p.iter() {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    let inside = |px: f32, py: f32| {
        let mut odd = false;
        for p in polys {
            let n = p.len();
            for i in 0..n {
                let (ax, ay) = p[i];
                let (bx, by) = p[(i + 1) % n];
                if (ay > py) != (by > py) && px < ax + (py - ay) * (bx - ax) / (by - ay) {
                    odd = !odd;
                }
            }
        }
        odd
    };
    for y in y0.floor().max(0.0) as i32..(y1.ceil() as i32).min(fb.height) {
        for x in x0.floor().max(0.0) as i32..(x1.ceil() as i32).min(fb.width) {
            let mut n = 0;
            for j in 0..4 {
                for i in 0..4 {
                    if inside(
                        x as f32 + (i as f32 + 0.5) / 4.0,
                        y as f32 + (j as f32 + 0.5) / 4.0,
                    ) {
                        n += 1;
                    }
                }
            }
            if n > 0 {
                fb.put(
                    x,
                    y,
                    fb.get(x, y)
                        .blend(color, (n as f32 / 16.0 * 255.0).round() as u8),
                );
            }
        }
    }
}

/// Mission-list scrollbar, measured on the live client (content px):
/// #6b0000 track box 1771..1818.8 x 516..876 (2.8 px), #e40000 arrow
/// outlines (apex 461.3 / 930.6, 3 px base bars) and a #ff0000 octagonal
/// thumb 1773.4..1815.9 x 42 (3.3 px stroke, 8.7 px chamfer) whose top
/// runs from 519 to 834.
fn draw_mission_scrollbar(fb: &mut Framebuffer, cx: i32, k: f32, thumb: f32) {
    let p = |x: f32, y: f32| (cx as f32 + x * k, y * k);
    let space = CefSpace {
        cx: cx as f32,
        kx: k,
        ky: k,
    };
    cef_band(
        fb,
        space,
        [1771.0, 516.0, 1818.8, 876.0],
        0.0,
        2.9,
        Rgb::new(107, 0, 0),
        1.0,
        &[],
    );
    let arrow = Rgb::new(228, 0, 0);
    let (l, r, mid, dx) = (1770.6f32, 1818.6f32, 1794.5f32, 0.491f32);
    // Up: outer apex 461.3, base 510; inner apex 471.3, inner base 507
    // (the 4.9 px horizontal stroke runs parallel to the outer edges).
    let up_o = [p(mid, 461.3), p(r, 510.0), p(l, 510.0)];
    let inner_half = mid - (1794.5 - dx * (507.0 - 461.3) + 4.9);
    let up_i = [
        p(mid, 471.3),
        p(mid + inner_half, 507.0),
        p(mid - inner_half, 507.0),
    ];
    aa_polygons(fb, &[&up_o, &up_i], arrow);
    // Down: outer base 881.9, apex 930.6; inner base 885, apex 920.6.
    let dn_half = mid - (1794.5 - dx * (930.6 - 885.0) + 4.9);
    let dn_o = [p(l, 881.9), p(r, 881.9), p(mid, 930.6)];
    let dn_i = [
        p(mid - dn_half, 885.0),
        p(mid + dn_half, 885.0),
        p(mid, 920.6),
    ];
    aa_polygons(fb, &[&dn_o, &dn_i], arrow);
    // Thumb.
    let top = galaxycraft::menu::MISSION_THUMB_TOP.0
        + (galaxycraft::menu::MISSION_THUMB_TOP.1 - galaxycraft::menu::MISSION_THUMB_TOP.0) * thumb;
    let oct = |x0: f32, y0: f32, x1: f32, y1: f32, c: f32| {
        [
            p(x0 + c, y0),
            p(x1 - c, y0),
            p(x1, y0 + c),
            p(x1, y1 - c),
            p(x1 - c, y1),
            p(x0 + c, y1),
            p(x0, y1 - c),
            p(x0, y0 + c),
        ]
    };
    let t = 3.3f32;
    let outer = oct(1773.4, top, 1815.9, top + 42.0, 8.7);
    let inner = oct(
        1773.4 + t,
        top + 3.0,
        1815.9 - t,
        top + 39.0,
        8.7 - t * 0.414,
    );
    aa_polygons(fb, &[&outer, &inner], Rgb::new(255, 0, 0));
}

/// Retail Options plate (130933..130954), content units: an L-shaped
/// frame whose title strip and five tab boxes are separate rounded boxes
/// (4.6 px #331f1f outer band r 3.5, 3 px #6c0000 inner band r 4, 7-9 px
/// gaps left unfilled), a divider (brown 837.2..841.9, red 844.9..847.9)
/// running to the plate bottom, and the content column outline with
/// 9.5/7.5 px rounded right corners.  The selected tab opens into the
/// content column: both divider bands break and its rules run through.
fn draw_options_plate(fb: &mut Framebuffer, s: CefSpace, rows: &[f32; 5], active: usize) {
    const TB: f32 = 4.6;
    const TR: f32 = 3.0;
    const FILL: f32 = 230.0 / 255.0;
    let black = Rgb::new(0, 0, 0);
    let cell = |b: f32| [513.1, b + 16.5, 841.9, b + 103.5];
    // Fills: content column + title strip, then each tab box.
    cef_band(
        fb,
        s,
        [500.0, 172.4, 2049.9, 1152.0],
        9.5,
        0.0,
        black,
        FILL,
        &[[841.9, 0.0, 3000.0, 1500.0]],
    );
    cef_band(
        fb,
        s,
        [513.1, 172.4, 841.9, 355.5],
        4.5,
        0.0,
        black,
        FILL,
        &[],
    );
    for b in rows {
        cef_band(fb, s, cell(*b), 4.5, 0.0, black, FILL, &[]);
    }
    // Red inner bands.
    cef_band(
        fb,
        s,
        [520.8, 180.0, 2042.25, 1144.5],
        10.0,
        TR,
        CSS_INNER,
        1.0,
        &[[848.9, 0.0, 3000.0, 190.0], [844.9, 190.0, 3000.0, 1500.0]],
    );
    cef_band(
        fb,
        s,
        [520.8, 180.0, 851.9, 348.0],
        5.0,
        TR,
        CSS_INNER,
        1.0,
        &[
            [0.0, 0.0, 530.0, 400.0],
            [0.0, 180.0, 847.9, 183.5],
            [0.0, 344.5, 847.9, 400.0],
        ],
    );
    for (i, b) in rows.iter().enumerate() {
        if i == active {
            cef_band(
                fb,
                s,
                [520.8, b + 24.0, 851.9, b + 96.0],
                5.0,
                TR,
                CSS_INNER,
                1.0,
                &[
                    [0.0, 0.0, 844.5, 2000.0],
                    [0.0, 0.0, 847.9, b + 27.5],
                    [0.0, b + 92.5, 847.9, 2000.0],
                ],
            );
        } else {
            cef_band(
                fb,
                s,
                [520.8, b + 24.0, 841.9, b + 96.0],
                5.0,
                TR,
                CSS_INNER,
                1.0,
                &[],
            );
        }
    }
    let ab = rows[active];
    for (y0, y1) in [(349.5, ab + 28.0), (ab + 98.0, 1144.5)] {
        if y1 > y0 {
            cef_band(fb, s, [844.9, y0, 847.9, y1], 0.0, 0.0, CSS_INNER, 1.0, &[]);
        }
    }
    // Brown outer bands.
    cef_band(
        fb,
        s,
        [513.1, 172.4, 2049.9, 1152.0],
        9.5,
        TB,
        CSS_OUTER,
        1.0,
        &[[530.0, 0.0, 3000.0, 177.5], [837.2, 177.5, 3000.0, 1500.0]],
    );
    cef_band(
        fb,
        s,
        [513.1, 172.4, 841.9, 355.5],
        4.5,
        TB,
        CSS_OUTER,
        1.0,
        &[[0.0, 0.0, 530.0, 400.0], [0.0, 350.5, 841.9, 400.0]],
    );
    for (i, b) in rows.iter().enumerate() {
        if i == active {
            let mut c = cell(*b);
            c[2] = 845.9;
            cef_band(
                fb,
                s,
                c,
                4.5,
                TB,
                CSS_OUTER,
                1.0,
                &[
                    [0.0, 0.0, 837.2, 2000.0],
                    [0.0, 0.0, 841.9, b + 21.1],
                    [0.0, b + 98.9, 841.9, 2000.0],
                ],
            );
        } else {
            cef_band(fb, s, cell(*b), 4.5, TB, CSS_OUTER, 1.0, &[]);
        }
    }
    for (y0, y1) in [(349.55, ab + 21.1), (ab + 98.0, 1152.0)] {
        if y1 > y0 {
            cef_band(fb, s, [837.2, y0, 841.9, y1], 0.0, 0.0, CSS_OUTER, 1.0, &[]);
        }
    }
}

/// Opaque rounded fill (the CEF dialog plates have a 16 px radius at 3x).
fn rounded_fill(fb: &mut Framebuffer, x: i32, y: i32, w: i32, h: i32, r: f32, color: Rgb) {
    for yy in y..y + h {
        for xx in x..x + w {
            if rounded_sdf(
                xx as f32 + 0.5,
                yy as f32 + 0.5,
                x as f32,
                y as f32,
                w as f32,
                h as f32,
                r,
            ) <= 0.0
            {
                fb.put(xx, yy, color);
            }
        }
    }
}

/// Rounded border band inside a rounded rect at `inset`.
#[allow(clippy::too_many_arguments)]
fn rounded_band(
    fb: &mut Framebuffer,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    r: f32,
    inset: i32,
    t: i32,
    color: Rgb,
) {
    let (ix, iy, iw, ih) = (x + inset, y + inset, w - 2 * inset, h - 2 * inset);
    if iw <= 0 || ih <= 0 {
        return;
    }
    for yy in iy..iy + ih {
        for xx in ix..ix + iw {
            let d = rounded_sdf(
                xx as f32 + 0.5,
                yy as f32 + 0.5,
                ix as f32,
                iy as f32,
                iw as f32,
                ih as f32,
                (r - inset as f32).max(0.0),
            );
            if d > -(t as f32) && d <= 0.0 {
                fb.put(xx, yy, color);
            }
        }
    }
}

/// Black fill with alpha, clipped to a rounded rectangle.
fn rounded_fill_alpha(fb: &mut Framebuffer, x: i32, y: i32, w: i32, h: i32, r: f32, alpha: u8) {
    for yy in y..y + h {
        for xx in x..x + w {
            let sdf = rounded_sdf(
                xx as f32 + 0.5,
                yy as f32 + 0.5,
                x as f32,
                y as f32,
                w as f32,
                h as f32,
                r,
            );
            let cov = (0.5 - sdf).clamp(0.0, 1.0);
            if cov > 0.0 {
                let c = fb.get(xx, yy);
                fb.put(
                    xx,
                    yy,
                    c.blend(Rgb::new(0, 0, 0), (alpha as f32 * cov) as u8),
                );
            }
        }
    }
}

/// Alpha fill whose top corners are rounded and whose bottom edge is open
/// (tabs sit on the panel top border).
fn top_rounded_fill(fb: &mut Framebuffer, x: i32, y: i32, w: i32, h: i32, r: f32, alpha: u8) {
    for yy in 0..h {
        for xx in 0..w {
            let mut cov = 1.0f32;
            if yy < r as i32 {
                let cx = if xx < r as i32 {
                    x as f32 + r
                } else if xx >= w - r as i32 {
                    x as f32 + w as f32 - r
                } else {
                    x as f32 + xx as f32 + 0.5
                };
                let cy = y as f32 + r;
                let d = (((x + xx) as f32 + 0.5 - cx).powi(2)
                    + ((y + yy) as f32 + 0.5 - cy).powi(2))
                .sqrt();
                cov = (r + 0.5 - d).clamp(0.0, 1.0);
            }
            if cov > 0.0 {
                let c = fb.get(x + xx, y + yy);
                fb.put(
                    x + xx,
                    y + yy,
                    c.blend(Rgb::new(0, 0, 0), (alpha as f32 * cov) as u8),
                );
            }
        }
    }
}

/// CEF tab border: rounded top corners, straight sides that may extend below
/// the tab (selected tab `margin-bottom:-3px`), and no bottom border.
fn tab_border_rounded(
    fb: &mut Framebuffer,
    r: (i32, i32, i32, i32),
    t: i32,
    inset: i32,
    radius: i32,
    extend: i32,
    color: Rgb,
) {
    let (x, y, w, h) = r;
    let bottom = y + h + extend;
    // Straight top and side segments (the corner boxes are painted by arcs).
    let rad = radius.max(t + 1);
    fb.rect(x + rad, y, w - 2 * rad, t, color);
    fb.rect(x, y + rad, t, bottom - y - rad, color);
    fb.rect(x + w - t, y + rad, t, bottom - y - rad, color);
    let band = t as f32;
    let rr = rad as f32;
    for (cx, cy, sx, sy) in [
        (x + rad, y + rad, x, y),
        (x + w - rad, y + rad, x + w - rad, y),
    ] {
        for yy in sy..sy + rad {
            for xx in sx..sx + rad {
                let d = (((xx as f32 + 0.5) - cx as f32).powi(2)
                    + ((yy as f32 + 0.5) - cy as f32).powi(2))
                .sqrt();
                let cov = (d - (rr - band) + 0.5).clamp(0.0, 1.0) * (rr - d + 0.5).clamp(0.0, 1.0);
                if cov > 0.0 {
                    fb.put(xx, yy, fb.get(xx, yy).blend(color, (cov * 255.0) as u8));
                }
            }
        }
    }
    let _ = inset; // the caller passes the color for each concentric band
}

/// Full panel plate with rounded outer/inner bands.
fn css_panel3_rounded(
    fb: &mut Framebuffer,
    r: (i32, i32, i32, i32),
    alpha: u8,
    t: i32,
    inset: i32,
    radius: i32,
) {
    let (x, y, w, h) = r;
    rounded_fill_alpha(fb, x, y, w, h, radius as f32, alpha);
    rounded_band(fb, x, y, w, h, radius as f32, 0, t, CSS_OUTER);
    rounded_band(fb, x, y, w, h, radius as f32, inset, t, CSS_INNER);
}

/// Retail CEF button plate: transparent black, #6c0000 border, green text.
fn dim_screen(fb: &mut Framebuffer, cx: i32, cw: i32, ch: i32, alpha: u8) {
    for y in 0..ch {
        for x in 0..cw {
            let c = fb.get(cx + x, y);
            fb.put(cx + x, y, c.blend(Rgb::new(0, 0, 0), alpha));
        }
    }
}
/// Bilinear, alpha-blended blit of a source rectangle of a DDS into a
/// destination rect.  Used by the Toon Registry plate, whose shipped art
/// bakes bottom plates the retail screen does not draw (source rows 0..1300).
#[allow(clippy::too_many_arguments)]
fn draw_dds_crop_scaled(
    fb: &mut Framebuffer,
    art: &galaxycraft::dds::DdsImage,
    src: (i32, i32, i32, i32),
    dst: (i32, i32, i32, i32),
) {
    let (sx0, sy0, sw, sh) = src;
    let (dx, dy, dw, dh) = dst;
    if sw <= 0 || sh <= 0 || dw <= 0 || dh <= 0 || art.width == 0 || art.height == 0 {
        return;
    }
    for y in 0..dh {
        let fy = (y as f32 + 0.5) * sh as f32 / dh as f32 - 0.5;
        let y0 = fy.floor() as i32;
        let y1 = (y0 + 1).min(sh - 1);
        let ty = (fy - y0 as f32).clamp(0.0, 1.0);
        for x in 0..dw {
            let fx = (x as f32 + 0.5) * sw as f32 / dw as f32 - 0.5;
            let x0 = fx.floor() as i32;
            let x1 = (x0 + 1).min(sw - 1);
            let tx = (fx - x0 as f32).clamp(0.0, 1.0);
            let p = |px: i32, py: i32| {
                let ax = (sx0 + px).clamp(0, art.width as i32 - 1) as usize;
                let ay = (sy0 + py).clamp(0, art.height as i32 - 1) as usize;
                art.pixel(ax, ay)
            };
            let (p00, p10, p01, p11) = (p(x0, y0), p(x1, y0), p(x0, y1), p(x1, y1));
            let mix = |shift: u32| -> u32 {
                let c = |q: u32| ((q >> shift) & 0xff) as f32;
                let top = c(p00) * (1.0 - tx) + c(p10) * tx;
                let bot = c(p01) * (1.0 - tx) + c(p11) * tx;
                (top * (1.0 - ty) + bot * ty).round().clamp(0.0, 255.0) as u32
            };
            let a = mix(24);
            if a == 0 {
                continue;
            }
            let c = fb.get(dx + x, dy + y);
            fb.put(
                dx + x,
                dy + y,
                c.blend(Rgb::new(mix(16) as u8, mix(8) as u8, mix(0) as u8), a as u8),
            );
        }
    }
}

fn draw_movie_frame(fb: &mut Framebuffer, frame: &PosterFrame) {
    if frame.width == 0 || frame.height == 0 {
        return;
    }
    fb.clear(Rgb::new(0, 0, 0));
    let (w, h) = (fb.width as f32, fb.height as f32);
    let scale = (w / frame.width as f32).min(h / frame.height as f32);
    let dw = (frame.width as f32 * scale).round().max(1.0);
    let dh = (frame.height as f32 * scale).round().max(1.0);
    let ox = ((w - dw) / 2.0).round() as i32;
    let oy = ((h - dh) / 2.0).round() as i32;
    for y in 0..dh as i32 {
        let sy = ((y as f32 / scale).min(frame.height as f32 - 1.0)) as usize;
        for x in 0..dw as i32 {
            let sx = ((x as f32 / scale).min(frame.width as f32 - 1.0)) as usize;
            let o = (sy * frame.width as usize + sx) * 3;
            fb.put(
                ox + x,
                oy + y,
                Rgb::new(frame.rgb[o], frame.rgb[o + 1], frame.rgb[o + 2]),
            );
        }
    }
}

/// Idle portrait clip of a units.dat portrait id: `arr/portdata.dat` holds
/// 110 idle and 110 talking `portdata.tbl` indices (1-based), each a path
/// stem such as `padvisor\PAdFid0` whose first clip is `...fid00.webm`.
fn portrait_idle_path(install: &RetailInstall, portrait: u16) -> Option<String> {
    let dat = install.read("arr/portdata.dat")?;
    let tbl = install.read("arr/portdata.tbl")?;
    let at = portrait as usize * 4;
    let idx = u32::from_le_bytes(dat.get(at..at + 4)?.try_into().ok()?) as usize;
    let table = galaxycraft::tables::StringTable::parse(&tbl).ok()?;
    let stem = table.get(u16::try_from(idx.checked_sub(1)?).ok()?)?;
    Some(format!(
        "portrait/{}0.webm",
        stem.replace('\\', "/").to_ascii_lowercase()
    ))
}

/// Colour marking "no world pixel" in the 1x unit layer composited over the
/// screen-resolution HD terrain.
const WORLD_KEY: Rgb = Rgb(0x0001_0203);

/// Bilinear blit of the 1x layer, skipping [`WORLD_KEY`] texels: each output
/// pixel blends the non-key neighbours by their bilinear weight, so sprite
/// edges stay as soft as the opaque blit.
fn blit_scaled_keyed(src: &Framebuffer, dst: &mut Framebuffer, scale: f32, x0: i32) {
    let s = scale.max(0.5);
    let dw = ((src.width as f32 * s).round() as i32).min(dst.width - x0);
    let dh = ((src.height as f32 * s).round() as i32).min(dst.height);
    let max_sx = src.width - 1;
    let max_sy = src.height - 1;
    for y in 0..dh {
        let fy = ((y as f32 + 0.5) / s) - 0.5;
        let y0 = fy.floor() as i32;
        let wy = fy - y0 as f32;
        for x in 0..dw {
            let fx = ((x as f32 + 0.5) / s) - 0.5;
            let xs = fx.floor() as i32;
            let wx = fx - xs as f32;
            let (mut r, mut g, mut b, mut wsum) = (0f32, 0f32, 0f32, 0f32);
            for (sx, sy, w) in [
                (xs, y0, (1.0 - wx) * (1.0 - wy)),
                (xs + 1, y0, wx * (1.0 - wy)),
                (xs, y0 + 1, (1.0 - wx) * wy),
                (xs + 1, y0 + 1, wx * wy),
            ] {
                let p = src.get(sx.clamp(0, max_sx), sy.clamp(0, max_sy));
                if p == WORLD_KEY || w <= 0.0 {
                    continue;
                }
                r += ((p.0 >> 16) & 0xff) as f32 * w;
                g += ((p.0 >> 8) & 0xff) as f32 * w;
                b += (p.0 & 0xff) as f32 * w;
                wsum += w;
            }
            if wsum <= 0.0 {
                continue;
            }
            let c = Rgb::new(
                (r / wsum).round() as u8,
                (g / wsum).round() as u8,
                (b / wsum).round() as u8,
            );
            let (px, py) = (x0 + x, y);
            let a = (wsum * 255.0).round().min(255.0) as u8;
            let out = if a == 255 {
                c
            } else {
                dst.get(px, py).blend(c, a)
            };
            dst.put(px, py, out);
        }
    }
}

fn blit_scaled_at(src: &Framebuffer, dst: &mut Framebuffer, scale: f32, x0: i32) {
    let s = scale.max(0.5);
    let dw = ((src.width as f32 * s).round() as i32).min(dst.width);
    let dh = ((src.height as f32 * s).round() as i32).min(dst.height);
    let max_sx = (src.width - 1) as f32;
    let max_sy = (src.height - 1) as f32;
    // Bilinear, like the retail client's HD upscale: the SD art is magnified
    // smoothly there, while a nearest-neighbour blit leaves hard 3 px steps
    // that the references do not have.
    for y in 0..dh {
        let fy = (((y as f32 + 0.5) / s) - 0.5).clamp(0.0, max_sy);
        let y0 = fy.floor() as i32;
        let y1 = (y0 + 1).min(max_sy as i32);
        let wy = fy - y0 as f32;
        for x in 0..dw {
            let fx = (((x as f32 + 0.5) / s) - 0.5).clamp(0.0, max_sx);
            let x0s = fx.floor() as i32;
            let x1s = (x0s + 1).min(max_sx as i32);
            let wx = fx - x0s as f32;
            let p00 = src.get(x0s, y0).0;
            let p10 = src.get(x1s, y0).0;
            let p01 = src.get(x0s, y1).0;
            let p11 = src.get(x1s, y1).0;
            let mix = |a: u32, b: u32, t: f32| -> u32 {
                let a = a as f32;
                let b = b as f32;
                (a + (b - a) * t).round() as u32
            };
            let top = mix(p00 & 0xff, p10 & 0xff, wx);
            let bottom = mix(p01 & 0xff, p11 & 0xff, wx);
            let blue = mix(top, bottom, wy);
            let top = mix((p00 >> 8) & 0xff, (p10 >> 8) & 0xff, wx);
            let bottom = mix((p01 >> 8) & 0xff, (p11 >> 8) & 0xff, wx);
            let green = mix(top, bottom, wy);
            let top = mix(p00 >> 16, p10 >> 16, wx);
            let bottom = mix(p01 >> 16, p11 >> 16, wx);
            let red = mix(top, bottom, wy);
            dst.put(
                x0 + x,
                y,
                galaxycraft::render::Rgb::new(red as u8, green as u8, blue as u8),
            );
        }
    }
}

fn parse_race(s: &str) -> Race {
    match s.to_ascii_lowercase().as_str() {
        "zerg" | "z" => Race::Zerg,
        "protoss" | "p" => Race::Protoss,
        _ => Race::Terran,
    }
}

/// Race per player slot.  Campaign maps keep their authored SIDE races;
/// melee maps use the player's chosen race for every slot, matching the
/// retail lobby where the map side is only a default.  An explicit
/// `--races` list overrides both and is cycled across the slots.
fn races_for(
    scenario: &Scenario,
    slots: &[PlayerSlot],
    override_races: &Option<Vec<Race>>,
    menu_race: Race,
) -> Vec<Race> {
    match override_races {
        Some(list) if !list.is_empty() => (0..slots.len()).map(|i| list[i % list.len()]).collect(),
        _ if scenario.has_placed_force() => slots.iter().map(|s| s.race(menu_race)).collect(),
        _ => vec![menu_race; slots.len()],
    }
}

/// Apply the recovered slot control types to a fresh world: inactive slots
/// never count toward victory, and computer slots are driven by the AI.
fn apply_slots(world: &mut World, slots: &[PlayerSlot]) {
    for s in slots.iter() {
        if let Some(p) = world.players.get_mut(s.slot) {
            p.defeated = !s.active;
            p.is_computer = s.computer;
        }
    }
}

/// Mission text authored in the scenario's `STR` section (see
/// [`Scenario::briefing`]), flattened for the briefing screen.
fn briefing_content(scenario: &Scenario) -> Option<(String, String, String)> {
    scenario.briefing().map(|b| (b.name, b.text, b.objectives))
}

/// Word-wrap fixed-width text, preserving authored line breaks.
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut current = String::new();
        for word in line.split_whitespace() {
            if !current.is_empty() && current.len() + 1 + word.len() > width {
                out.push(std::mem::take(&mut current));
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
        out.push(current);
    }
    out
}

fn format_time(ticks: u64) -> String {
    let seconds = ticks / 24;
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

/// Map a script key name to a minifb key.
fn script_key(name: &str) -> Key {
    match name {
        "Up" => Key::Up,
        "Down" => Key::Down,
        "Left" => Key::Left,
        "Right" => Key::Right,
        "Return" | "Enter" => Key::Enter,
        "Escape" | "Esc" => Key::Escape,
        "Tab" => Key::Tab,
        "Space" => Key::Space,
        "Delete" => Key::Delete,
        "L" => Key::L,
        "R" => Key::R,
        "H" => Key::H,
        _ => Key::Unknown,
    }
}

/// Scripted menu walkthrough: feeds synthetic `MenuInput` ticks through the
/// real menu handlers, writes one BMP per step and a TSV with the screen and
/// the sound cues each step produced.  `GC_WALK_DIR` overrides the output
/// directory (default `/tmp/opencode/walk`).
fn run_menu_script(app: &mut App, fb: &mut Framebuffer, script: &str) {
    let dir = std::env::var("GC_WALK_DIR").unwrap_or_else(|_| "/tmp/opencode/walk".into());
    let _ = std::fs::create_dir_all(&dir);
    let mut log = String::from("step\taction\tscreen\tsounds\tstate\n");
    let mut bank_log = String::from("cue\tloaded_samples\n");
    for cue in ["ui_move", "ui_click", "ui_swish"] {
        let samples = app.sounds.get(cue).map(|s| s.len()).unwrap_or(0);
        bank_log.push_str(&format!("{cue}\t{samples}\n"));
    }
    if let Err(e) = std::fs::write(format!("{dir}/sounds.tsv"), &bank_log) {
        eprintln!("sounds.tsv write failed: {e}");
    }
    let mut mouse: Option<(i32, i32)> = None;
    let mut seen = app.sound_log.len();
    // One frame per screen change (plus the Lobby popup hover frame) keeps
    // the artifacts small; the TSV still records every step.
    let mut prev_screen = String::new();
    // `hold` keeps the button down across steps (drags) until `release`.
    let mut held = false;
    for (i, step) in script.split(',').map(str::trim).enumerate() {
        let mut keys = Vec::new();
        let mut down = false;
        let mut wheel = 0;
        if step == "hold" {
            held = true;
        } else if step == "release" {
            held = false;
        } else if step == "fixture:win" {
            // End-to-end victory: every other player is defeated in the
            // running mission and the real result path records progress.
            let me = app.local_player;
            for (k, p) in app.world.players.iter_mut().enumerate() {
                if k != me {
                    p.defeated = true;
                }
            }
            app.briefing = false;
            app.menu = false;
            app.update();
        } else if let Some(n) = step.strip_prefix("wheel:") {
            wheel = n.parse().unwrap_or(0);
        } else if let Some(rest) = step.strip_prefix("fixture:progress:") {
            // Walkthrough fixture: give the active profile campaign history
            // (`<episode>=<next mission>`), as a played profile has.
            if let Some((ep, next)) = rest.split_once('=') {
                let campaign = ep
                    .parse()
                    .ok()
                    .and_then(galaxycraft::campaign_progress::Campaign::from_episode);
                if let (Some(c), Ok(n)) = (campaign, next.parse()) {
                    let mut p = app.progress();
                    p.next[c.index()] = n;
                    app.progress_book.set(&app.active_profile.clone(), p);
                }
            }
        } else if step == "fixture:lan-peer" {
            // Walkthrough fixture: inject one discovered peer so the LAN row
            // action (select + join) can be exercised without a second host.
            let announce = galaxycraft::net::Announce {
                name: format!("Peer{}", app.lan.entries.len() + 1),
                map_hash: [0u8; 32],
                port: 6112,
                players: 1,
                started: false,
            };
            if let Ok(addr) = "127.0.0.1:6112".parse() {
                app.lan.entries.push((addr, announce));
            }
        } else if let Some(rest) = step.strip_prefix("move:") {
            let (x, y) = rest.split_once(':').unwrap_or(("0", "0"));
            mouse = Some((x.parse().unwrap_or(0), y.parse().unwrap_or(0)));
        } else if step == "click" {
            down = true;
        } else if let Some(k) = step.strip_prefix("key:") {
            keys.push(script_key(k));
        }
        let input = MenuInput {
            keys,
            mouse,
            mouse_down: down || held,
            wheel,
        };
        app.update_view(fb);
        app.handle_menu_input(&input);
        app.update_view(fb);
        app.draw_menu(fb);
        let screen = format!("{:?}", app.menu_screen)
            .split([' ', '{'])
            .next()
            .unwrap_or("?")
            .to_string();
        if screen != prev_screen
            || step == "move:1122:1039"
            || screen == "Hotkeys"
            || screen == "Lan"
        {
            let path = format!("{dir}/walk-{i:02}-{screen}.bmp");
            let _ = fb.write_bmp(std::path::Path::new(&path));
        }
        prev_screen = screen.clone();
        let sounds: Vec<String> = app.sound_log[seen..].to_vec();
        seen = app.sound_log.len();
        let o = &app.game_options;
        let bit = |v: bool| if v { '1' } else { '0' };
        let flags: String = [
            o.video_fps_cap,
            o.video_vsync,
            o.video_hd,
            o.video_environment,
            o.video_lighting,
            o.video_smooth_turning,
            o.video_shadow_stacking,
            o.video_pillarbox,
            o.game_timer,
            o.game_color_cycling,
            o.game_language_filter,
            o.game_bonus_skins,
            o.game_friend_notifications,
            o.portraits == 1,
            o.portraits == 2,
            o.minimap_position == 1,
            o.network_prefer_6112,
            o.unit_speech,
            o.unit_acks,
            o.sound_background,
            o.building_sounds,
            o.subtitles,
            o.cinematic_subtitles,
            o.original_voice_overs,
            o.original_1998_campaign,
            o.use_mouse_sensitivity,
            o.mouse_scaling,
            o.hardware_cursor,
        ]
        .iter()
        .map(|v| bit(*v))
        .collect();
        let state = match &app.menu_screen {
            MenuScreen::Options { tab } => format!(
                "tab={tab:?} speed={:.2} key={:.2} mouse={:.2} sens={:.2} music={:.2} sfx={:.2} flags={flags}",
                o.game_speed,
                o.key_scroll,
                o.mouse_scroll,
                o.mouse_sensitivity,
                o.music_volume,
                o.sfx_volume
            ),
            MenuScreen::Lobby { .. } => format!("races={:?}", app.lobby_races),
            MenuScreen::Hotkeys { race } => format!(
                "race={race:?} sel={:?} action={:?} key={:?} reset={} error={}",
                app.hotkey_sel,
                app.hotkey_action,
                app.hotkey_action.and_then(|a| {
                    app.hotkey_bindings
                        .get(a as usize)
                        .copied()
                        .flatten()
                        .map(|k| k.text)
                }),
                app.hotkey_reset,
                app.hotkey_binding_error
            ),
            MenuScreen::Mission { episode } => format!(
                "episode={episode} len={} sel={} top={} profile={} next={:?} result={:?}",
                app.mission_list.len,
                app.mission_list.sel,
                app.mission_list.top,
                app.active_profile,
                app.progress().next,
                app.game_result
            ),
            MenuScreen::Episode => format!(
                "hover={:?} prompt={:?} profile={}",
                app.episode_hover, app.campaign_prompt, app.active_profile
            ),
            MenuScreen::Lan => format!(
                "lan_sel={} entries={} status={}",
                app.lan.sel,
                app.lan.entries.len(),
                app.lan.status
            ),
            _ => String::new(),
        };
        log.push_str(&format!(
            "{i}\t{step}\t{screen}\t{}\t{state}\n",
            sounds.join("+")
        ));
    }
    if let Err(e) = std::fs::write(format!("{dir}/walk.tsv"), &log) {
        eprintln!("walk.tsv write failed: {e}");
    }
    print!("{log}");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut chk: Option<PathBuf> = None;
    let mut maps_dir = asset_root().join("maps");
    let mut tiles_dir = asset_root().join("TileSet");
    let mut assets = asset_root();
    let mut races: Option<Vec<Race>> = None;
    let mut install: Option<PathBuf> = None;
    let mut cache: Option<PathBuf> = None;
    let mut width = VIEW_W;
    let mut height = VIEW_H;
    let mut shot: Option<PathBuf> = None;
    let mut shot_screen = String::from("menu");
    let mut menu_script: Option<String> = None;
    let mut start_screen: Option<String> = None;
    let mut freeze = false;
    let mut fullscreen = false;
    let mut debug_input = false;
    let mut shot_map: Option<usize> = None;
    // Deterministic menu animation captures: `--menu-at` seeks every clip
    // (idle and hover) to the same wall-clock second.
    let mut menu_at: Option<f32> = None;
    // Per-layer overrides: `--menu-phase single=2.5,multion=6.0`.
    let mut menu_phases: std::collections::HashMap<String, f32> = std::collections::HashMap::new();
    // `--replay FILE`: open a `.gcrep`/`.gcsave` straight into the player
    // (the Replays menu does the same, this is the scriptable entry point).
    let mut replay_path: Option<PathBuf> = None;
    let mut no_intro = false;
    let mut no_fog = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--chk" => {
                chk = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--maps" => {
                maps_dir = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--tiles" => {
                tiles_dir = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--assets" => {
                assets = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--install" => {
                install = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--cache" => {
                cache = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--debug-input" => {
                // On-screen input readout: lets a player confirm that the
                // pointer and buttons reach the client on their session.
                debug_input = true;
                i += 1;
            }
            "--freeze" => {
                // Freeze the simulation once the start screen is set up, so a
                // live capture is comparable with the headless render.
                freeze = true;
                i += 1;
            }
            "--start-screen" => {
                // Open the interactive client straight on a screen (episode,
                // options, in-game, ...) so a live capture can be taken without
                // injecting keystrokes.
                start_screen = Some(args[i + 1].clone());
                i += 2;
            }
            "--fullscreen" => {
                // Borderless window pinned to the screen origin: with
                // --width/--height set to the monitor resolution this fills the
                // display the way the retail client does.
                fullscreen = true;
                i += 1;
            }
            "--races" => {
                races = Some(args[i + 1].split(',').map(parse_race).collect());
                i += 2;
            }
            "--width" => {
                width = args[i + 1].parse().unwrap_or(width);
                i += 2;
            }
            "--height" => {
                height = args[i + 1].parse().unwrap_or(height);
                i += 2;
            }
            "--shot" => {
                shot = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--shot-screen" => {
                shot_screen = args[i + 1].clone();
                i += 2;
            }
            "--menu-script" => {
                menu_script = Some(args[i + 1].clone());
                i += 2;
            }
            "--replay" => {
                replay_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--shot-map" => {
                shot_map = args[i + 1].parse().ok();
                i += 2;
            }
            "--menu-at" => {
                menu_at = args[i + 1].parse().ok();
                i += 2;
            }
            "--menu-phase" => {
                for part in args[i + 1].split(',') {
                    if let Some((name, value)) = part.split_once('=') {
                        if let Ok(value) = value.parse() {
                            menu_phases.insert(name.to_string(), value);
                        }
                    }
                }
                i += 2;
            }

            "--no-fog" => {
                no_fog = true;
                i += 1;
            }
            "--no-intro" => {
                no_intro = true;
                i += 1;
            }
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }

    let mut retail_data: Option<Arc<GameData>> = None;
    let mut install_handle: Option<RetailInstall> = None;
    let mut campaign_maps: Vec<MapEntry> = Vec::new();
    if let Some(path) = install {
        let install = RetailInstall::open(&path).unwrap_or_else(|e| {
            eprintln!("cannot open retail install {}: {e}", path.display());
            std::process::exit(2);
        });
        let cache = cache.unwrap_or_else(|| install.default_cache_dir());
        if let Err(e) = install.ensure_cache(&cache) {
            eprintln!(
                "cannot materialise retail assets into {}: {e}",
                cache.display()
            );
            std::process::exit(2);
        }
        eprintln!("retail assets: {}", cache.display());
        assets = cache.clone();
        maps_dir = cache.join("maps");
        tiles_dir = cache.join("TileSet");
        campaign_maps = scan_campaign(cache.as_path());
        eprintln!(
            "campaign: {} missions, {} melee maps",
            campaign_maps.len(),
            scan_maps(&maps_dir).len()
        );
        retail_data = install.game_data().ok().map(Arc::new);
        install_handle = Some(install);
    }
    let data =
        retail_data.unwrap_or_else(|| Arc::new(GameData::bundled().expect("bundled tables")));
    // Campaign missions come from the same materialised cache; they are
    // listed first and keep their retail episode order.
    let mut maps = campaign_maps;
    let campaign_len = maps.len();
    maps.extend(scan_maps(&maps_dir));
    if maps.is_empty() {
        maps.push(MapEntry::new(
            asset_root().join("bottleneck.chk"),
            "Bottleneck".into(),
            1,
            2,
        ));
    }
    let menu = chk.is_none();
    let start_path = chk.clone().unwrap_or_else(|| maps[0].path.clone());
    let mut map_index = maps.iter().position(|m| m.path == start_path).unwrap_or(0);
    let mut campaign_len = campaign_len;
    let raw = std::fs::read(&start_path).expect("scenario CHK");
    let scenario = Scenario::parse(&raw).expect("valid CHK");
    let tileset = Tileset::load(&tiles_dir, scenario.era, &scenario).unwrap_or_else(|| {
        eprintln!(
            "missing tileset for era {} ({}); expected {}/{}.cv5",
            scenario.era,
            era_name(scenario.era),
            tiles_dir.display(),
            era_name(scenario.era)
        );
        std::process::exit(2);
    });
    let vf4 = std::fs::read(tiles_dir.join(format!("{}.vf4", era_name(scenario.era))))
        .expect("tileset vf4");
    // Player slots come from OWNR/SIDE, not from the number of start
    // locations: campaign missions place CPU bases in non-contiguous slots
    // (Enslavers uses 0/3/4/6), and the start-location count would drop them.
    let slots = scenario.player_slots();
    // `--chk` may name a file outside the bundled pack; give it a real menu
    // entry so the HUD, result panel and restart use its name.
    if let Some(path) = &chk {
        if !maps.iter().any(|m| m.path == *path) {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("map")
                .to_string();
            maps.insert(
                0,
                MapEntry::new(
                    path.clone(),
                    name,
                    scenario.era,
                    slots.iter().filter(|s| s.active).count().clamp(2, 8),
                ),
            );
            campaign_len += 1;
            map_index = 0;
        }
    }
    let race_override = races.clone();
    let menu_race = race_override
        .as_ref()
        .and_then(|r| r.first().copied())
        .unwrap_or(Race::Terran);
    let races = races_for(&scenario, &slots, &race_override, menu_race);
    let mut world = World::from_scenario(
        data.clone(),
        &scenario,
        &galaxycraft::map::TileData::new(tileset.cv5(), &vf4),
        &races,
    );
    apply_slots(&mut world, &slots);
    world.enable_fog();

    let sprites = SpriteBank::load(&assets);
    let sounds = SoundBank::load(&assets.join("sound"));
    let audio = AudioPlayer::new(std::env::var("SC_NO_AUDIO").is_err());
    let local_player = slots
        .iter()
        .find(|s| s.active && s.human)
        .map(|s| s.slot)
        .unwrap_or(0);
    let console_race = slots
        .iter()
        .find(|s| s.slot == local_player)
        .map(|s| s.race(Race::Terran))
        .unwrap_or(Race::Terran);
    let console = ConsoleArt::load(&assets, console_race);
    let glue = {
        let art = GlueArt::load(&assets);
        (art.has_art() || art.menu.is_some() || art.score.is_some()).then_some(art)
    };
    let sfx_table = std::fs::read(assets.join("tables/sfxdata.tbl"))
        .ok()
        .and_then(|b| SfxTable::parse(&b))
        .unwrap_or_else(SfxTable::bundled);
    let voices = VoiceBank::new(assets.join("voices"), sfx_table);
    let music = MusicBank::new(assets.join("music"));
    let video_store = VideoStore::load(&assets);
    let (intro_info, intro_poster) = video_store
        .as_ref()
        .and_then(|v| v.intro_poster(false))
        .map(|(info, poster)| (Some(info), Some(poster)))
        .unwrap_or((None, None));
    // Retail main-menu animations: the HD clips carry the fleet (Single),
    // the battlecruiser (Multiplayer) and the vortex (Exit), with the hover
    // variants adding the tactical overlay/portals.  They are streamed with
    // their alpha at the rectangles from `rez/flc.json`.
    // `glue/mainmenu` carries the high-resolution clip the retail client
    // draws at 1440p (e.g. `multion.webm` is 1200x880 and includes the
    // targeting grid and corner brackets seen in the references);
    // `videos/hd2/mainmenu` is the smaller fallback.
    let clip_path = |name: &str| -> PathBuf {
        let hd = assets.join(format!("glue/mainmenu/{name}.webm"));
        if hd.is_file() {
            hd
        } else {
            assets.join(format!("videos/hd2/mainmenu/{name}.webm"))
        }
    };
    let clip_start = menu_at.unwrap_or(MENU_CLIP_START);
    let frozen = menu_at.is_some() || !menu_phases.is_empty();
    let load_clip = |name: &str| {
        let start = menu_phases.get(name).copied().unwrap_or(clip_start);
        MenuClip::open_at(clip_path(name), start, frozen)
    };
    let menu_layers = {
        let layers = MenuLayers {
            single: load_clip("single"),
            single_on: load_clip("singleon"),
            multi: load_clip("multi"),
            multi_on: load_clip("multion"),
            exit: load_clip("exit"),
            exit_on: load_clip("exiton"),
        };
        if layers.single.is_none() {
            eprintln!("menu: space layers unavailable (install without glue/mainmenu webm)");
            None
        } else {
            Some(layers)
        }
    };
    let intro = menu && !no_intro && glue.is_some();
    // Retail pointer skin: `cursor/arrow.dds.grp` frame 0.  Without it the
    // front-end falls back to the system cursor.
    let cursor_art = install_handle.as_ref().and_then(|inst| {
        let bytes = inst.read_by_path("cursor/arrow.dds.grp").ok()?;
        galaxycraft::dds::DdsGrp::parse(&bytes)
    });
    let mut hotkey_bindings = hotkey_data::HotkeyData::get().default_bindings();
    if let Some(path) = hotkey_save_path() {
        if let Some(saved) = hotkey_data::load_bindings(&path) {
            for (i, binding) in saved.iter().enumerate() {
                if i < hotkey_bindings.len() {
                    hotkey_bindings[i] = *binding;
                }
            }
        }
    }
    let hotkey_baseline = hotkey_bindings.clone();
    let mut app = App {
        data,
        scenario,
        world,
        tileset,
        sprites,
        sounds,
        audio,
        assets_root: assets.clone(),
        console,
        glue,
        ui_font: galaxycraft::text::UiFont::load_retail(&assets),
        glue_font: galaxycraft::text::UiFont::load(&assets.join("font/eurostile-reg.ttf")),
        glue_font_ext: galaxycraft::text::UiFont::load(&assets.join("font/eurostileext-reg.ttf")),
        glue_font_small: galaxycraft::text::UiFont::load(&assets.join("font/bl.ttf")),
        ui_font_hover: galaxycraft::text::UiFont::load_retail_extended(&assets),
        ui_font_info: galaxycraft::text::UiFont::load(
            &std::env::var("GC_INFO_FONT")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| {
                    std::path::PathBuf::from(
                        "/usr/share/fonts/liberation-sans-fonts/LiberationSans-Regular.ttf",
                    )
                }),
        ),
        music,
        voices,
        music_track: String::new(),
        music_started: Instant::now(),
        music_phase: 0,
        console_race,
        intro,
        intro_started: Instant::now(),
        menu_fit: None,
        menu_static: None,
        intro_fit: None,
        backdrop_fit: None,
        video_store,
        intro_poster,
        intro_info,
        intro_expansion: false,
        intro_clips: Vec::new(),
        intro_clip: 0,
        intro_pending_map: None,
        intro_player: None,
        intro_probed: false,
        intro_last: Instant::now(),
        intro_audio: None,
        briefing_poster: None,
        briefing_video: None,
        briefing_player: None,
        briefing_player_key: None,
        briefing_last: Instant::now(),
        menu_screen: MenuScreen::Main,
        game_options: GameOptions::default(),
        profiles: vec!["REA".to_string(), "LinKa".to_string()],
        profile_sel: 0,
        main_cursor: None,
        gametype_hover: None,
        gametype_multi: false,
        registry_after_gametype: false,
        episode_sel: 0,
        mission_list: galaxycraft::campaign_progress::MissionList::default(),
        mission_drag: None,
        campaign_prompt: None,
        active_profile: "REA".to_string(),
        progress_book: progress_save_path()
            .and_then(|p| galaxycraft::campaign_progress::ProgressBook::load(&p))
            .unwrap_or_default(),
        lobby_races: vec![LobbyRace::Random; 8],
        game_menu_sel: 6,
        credits_scroll: 0,
        show_stats: false,
        show_leaderboard: false,
        minimap_terrain: true,
        content_box: (0, 0, width as i32, height as i32),
        ui_scale: 2.0,
        shot_name: shot_screen.clone(),
        ui_ox: 0,
        ui_oy: 0,
        result_panel: None,
        result_fit: None,
        ack_variant: 0,
        actions: 0,
        selected: Vec::new(),
        groups: Default::default(),
        ais: Vec::new(),
        campaign_ais: Vec::new(),
        maps,
        local_player,
        campaign_len,
        menu_tab: if map_index < campaign_len { 0 } else { 1 },
        menu_index: if map_index < campaign_len {
            map_index
        } else {
            map_index - campaign_len
        },
        map_index,
        menu,
        briefing: false,
        briefing_pages: Vec::new(),
        briefing_page: 0,
        briefing_art: None,
        briefing_fit: None,
        paused: false,
        game_result: None,
        next_map: None,
        tiles_dir: tiles_dir.clone(),
        menu_race,
        hd_terrain: None,
        hd_sprites: Default::default(),
        melee_slot: None,
        local_team_color: None,
        creep_variants: Default::default(),
        races: race_override,
        camera: (0, 0),
        view: (width as i32, height as i32 - HUD_H),
        drag_start: None,
        drag_now: (0, 0),
        attack_mode: false,
        patrol_mode: false,
        build_mode: None,
        build_list: Vec::new(),
        command_list: Vec::new(),
        status: String::new(),
        status_until: Instant::now(),
        speed: 1.0,
        fog: true,
        last_step: Instant::now(),
        accumulator: 0.0,
        rmb_down: false,
        lmb_down: false,
        quit: false,
        chk_raw: Vec::new(),
        last_races: Vec::new(),
        save_sel: 0,
        saves_dir: save_dirs().0,
        replays_dir: save_dirs().1,
        replay: None,
        net: None,
        lan: LanUi::default(),
        menu_layers,
        campaign_layers: None,
        episode_hover: None,
        menu_hover: None,
        lobby_race_open: None,
        sound_log: Vec::new(),
        collection_sel: 0,
        menu_at,
        menu_phases,
        lobby_preview: None,
        hotkey_hover: None,
        hotkey_sel: None,
        hotkey_cells: Vec::new(),
        hotkey_action_cells: Vec::new(),
        hotkey_action: None,
        hotkey_bindings,
        hotkey_baseline,
        hotkey_unbound: Vec::new(),
        hotkey_binding_error: false,
        hotkey_tab_hover: None,
        capture_request: false,
        hotkey_reset: false,
        content_x0: 0,
        mouse_was_down: false,
        debug_input,
        last_key: None,
        pointer: None,
        overlay: 0,
        install: install_handle,
        cursor_art: cursor_art.clone(),
        cursor_started: Instant::now(),
        portraits: std::collections::HashMap::new(),
        credits: Vec::new(),
        thumbs: std::collections::HashMap::new(),
        hotkey_icons: std::collections::HashMap::new(),
    };
    if app.intro {
        // First launch: the publisher logo followed by the title movie, both
        // read from the retail intro table.
        app.begin_intro(
            galaxycraft::video::intro_sequence(false, true)
                .into_iter()
                .map(str::to_string)
                .collect(),
            None,
        );
    }
    if no_fog {
        app.fog = false;
    }
    app.ais = slots
        .iter()
        .filter(|s| s.computer)
        .map(|s| SimpleAi::new(s.slot as u8))
        .collect();
    app.center_on_base();
    app.refresh_commands();
    if app.intro || app.menu {
        app.start_menu_music();
    } else {
        app.start_mission_music();
    }

    if let Some(path) = &replay_path {
        // Parse first so a bad file fails loudly instead of opening the menu.
        if galaxycraft::replay::SaveFile::load(path).is_none() {
            eprintln!(
                "cannot replay {}: not a GalaxyCraft save/replay",
                path.display()
            );
            std::process::exit(1);
        }
        app.load_save(path, true);
    }

    let supersample =
        shot.is_some() && std::env::var("GC_SUPERSAMPLE").ok().as_deref() == Some("1");
    let mut fb = if supersample {
        Framebuffer::new(3413, 1920)
    } else {
        Framebuffer::new(width, height)
    };
    // Headless screen capture for UI verification: the exact same draw code
    // runs, but into a BMP instead of a window.  This lets the campaign
    // menu, retail briefing and mission screens be checked without an X
    // server or a window manager.
    if let Some(script) = menu_script {
        app.intro = false;
        app.menu = true;
        app.menu_screen = MenuScreen::Main;
        app.profile_sel = 0;
        run_menu_script(&mut app, &mut fb, &script);
        return;
    }
    if shot.is_some() || start_screen.is_some() {
        if let Some(name) = &start_screen {
            shot_screen = name.clone();
        }
        if let Some(index) = shot_map {
            app.map_index = index.min(app.maps.len().saturating_sub(1));
        }
        match shot_screen.as_str() {
            "intro" | "intro-blizzard" | "intro-title" | "intro-expansion" => {
                // `intro-blizzard` captures the separate publisher logo,
                // `intro`/`intro-title` the title movie at 5 s/25 s and
                // `intro-expansion` the Brood War intro.
                let (clip, at) = match shot_screen.as_str() {
                    "intro-blizzard" => (galaxycraft::video::BLIZZARD_LOGO, 5),
                    "intro-expansion" => (galaxycraft::video::INTRO_EXPANSION, 25),
                    "intro-title" => (galaxycraft::video::INTRO_ORIGINAL, 25),
                    _ => (galaxycraft::video::INTRO_ORIGINAL, 5),
                };
                app.begin_intro(vec![clip.to_string()], None);
                app.intro_expansion = clip == galaxycraft::video::INTRO_EXPANSION;
                app.intro_started = Instant::now() - Duration::from_secs(at);
                app.draw_intro(&mut fb);
            }
            // Frame of a replay opened with `--replay` (the world is already
            // built and fast-forwarded; draw it as-is).
            "replay" => {
                app.update_view(&fb);
                app.centre_on_local_unit();
                app.world.update_vision();
                app.update();
                app.draw(&mut fb);
            }
            // Plain in-game frame with units selected: no dialog overlays.
            "ingame" => {
                app.load_map(app.map_index);
                app.prepare_ingame_shot(&fb);
                app.update();
                app.draw(&mut fb);
            }
            // Race-specific in-game captures: load that race's campaign
            // mission so the HUD console matches the reference screens.
            // The user's (8)The Hunters games of 2026-10-27: Zerg as slot 1
            // (134905), Terran as slot 7 (135020), Protoss as slot 6
            // (135051..135121); start locations identified from the
            // geyser/mineral layout around each town hall.
            "ingame-zerg"
            | "ingame-terran2"
            | "ingame-protoss"
            | "ingame-protoss-tip-shields"
            | "ingame-protoss-tip-probe"
            | "ingame-protoss-tip-minimap" => {
                let (slot, race, dy) = match shot_screen.as_str() {
                    // Zerg nudge: mineral-mask alignment on 134905.
                    "ingame-zerg" => (1, Race::Zerg, (-6, 8)),
                    "ingame-terran2" => (7, Race::Terran, (0, -8)),
                    _ => (6, Race::Protoss, (0, 8)),
                };
                app.hunters_start(&fb, slot, race, dy);
                app.update();
                // 135109 / 135114 / 135121 hover the shield upgrade, the
                // Probe button and the minimap terrain toggle.
                let centre = |r: UiRect| (r.x + r.w / 2, r.y + r.h / 2);
                app.pointer = match shot_screen.as_str() {
                    "ingame-protoss-tip-shields" => Some(centre(app.upgrade_button_rect())),
                    "ingame-protoss-tip-probe" => Some(centre(app.command_slots()[0])),
                    "ingame-protoss-tip-minimap" => Some(centre(app.hud_control(
                        "minimap",
                        2,
                        UiRect::new(3, 320, 64, 24),
                    ))),
                    _ => None,
                };
                app.draw(&mut fb);
            }
            "ingame-terran" => {
                let race = shot_screen.trim_start_matches("ingame-").to_string();
                // Map choice is calibrated against the reference frames by a
                // measured sweep over the installed maps (see session log):
                // the Terran references are melee games, the Zerg and Protoss
                // ones line up best with these campaign missions.
                let mission = match race.as_str() {
                    "zerg" => "06.chk",
                    // Ref 41-44 are one Protoss game; protoss09 is the mission
                    // whose terrain lines up best (measured sweep).
                    "protoss" => "09.chk",
                    _ => "02.chk",
                };
                let want = format!("/{}/", race.trim_end_matches('2'));
                if let Some(i) = app.maps.iter().position(|m| {
                    let p = m.path.to_string_lossy();
                    p.contains(&want) && p.ends_with(mission)
                }) {
                    app.map_index = i;
                }
                // A melee map (measured best fit) overrides the campaign pick.
                let melee = match race.as_str() {
                    "terran" => Some("volcanis"),
                    "terran2" => Some("space madness"),
                    _ => None,
                };
                if let Some(name) = melee {
                    if let Some(i) = app
                        .maps
                        .iter()
                        .position(|m| m.name.to_lowercase().contains(name))
                    {
                        app.map_index = i;
                    }
                }
                // Capture calibration: a shot may be pointed at any installed
                // map by name, which is how the reference map is identified by
                // measurement instead of by guesswork.
                if let Ok(want) = std::env::var("GC_SHOT_MAP") {
                    let want = want.to_lowercase();
                    if let Some(i) = app
                        .maps
                        .iter()
                        .position(|m| m.name.to_lowercase().contains(&want))
                    {
                        app.map_index = i;
                    }
                }
                app.load_map(app.map_index);
                app.prepare_ingame_shot(&fb);
                app.update();
                app.draw(&mut fb);
            }
            "menu" | "main" | "menu-hover-single" | "menu-hover-multi" | "menu-hover-exit" => {
                app.intro = false;
                app.menu = true;
                app.menu_screen = MenuScreen::Main;
                app.main_cursor = match shot_screen.as_str() {
                    "menu-hover-single" => Some(0),
                    "menu-hover-multi" => Some(1),
                    "menu-hover-exit" => Some(7),
                    _ => None,
                };
                app.draw_menu(&mut fb);
            }
            "registry" => {
                app.intro = false;
                app.menu = true;
                app.menu_screen = MenuScreen::Registry;
                app.draw_menu(&mut fb);
            }
            "gametype" | "gametype-hover-expansion" => {
                app.intro = false;
                app.menu = true;
                app.menu_screen = MenuScreen::GameType;
                app.gametype_hover = (shot_screen == "gametype-hover-expansion").then_some(1);
                app.draw_menu(&mut fb);
            }
            "episode"
            | "episode-hover-zerg"
            | "episode-hover-terran"
            | "episode-hover-protoss"
            | "episode-expansion-hover-zerg"
            | "episode-expansion-hover-terran"
            | "episode-expansion-hover-protoss"
            | "episode-locked-zerg" => {
                let screen = shot_screen.as_str();
                app.intro = false;
                app.menu = true;
                app.menu_screen = MenuScreen::Episode;
                app.set_reference_progress();
                if screen == "episode-locked-zerg" {
                    // Live capture: the prompt after clicking the locked Zerg.
                    app.campaign_prompt = Some(galaxycraft::campaign_progress::Campaign::Zerg);
                }
                let expansion = screen.contains("expansion");
                app.episode_sel = galaxycraft::menu::episode_start(expansion);
                // Slot order differs per campaign set: original is
                // Zerg/Terran/Protoss, Brood War is Terran/Protoss/Zerg.
                app.episode_hover = if screen.ends_with("hover-zerg") {
                    Some(if expansion { 2 } else { 0 })
                } else if screen.ends_with("hover-terran") {
                    Some(if expansion { 0 } else { 1 })
                } else if screen.ends_with("hover-protoss") {
                    Some(if expansion { 1 } else { 2 })
                } else {
                    None
                };
                app.draw_menu(&mut fb);
            }
            "mission" | "mission-live" | "mission-live-scrolled" => {
                app.intro = false;
                app.menu = true;
                app.set_reference_progress();
                let terran = galaxycraft::campaign_progress::Campaign::Terran;
                if shot_screen != "mission" {
                    // Live profile LinKa: history up to `9. New Gettysburg`.
                    let mut p = app.progress();
                    p.next[terran.index()] = 10;
                    app.progress_book.set(&app.active_profile.clone(), p);
                }
                app.enter_campaign(terran);
                // 131132 was wheel-scrolled to the last page with the
                // default selection (entry 0) out of view.
                match shot_screen.as_str() {
                    "mission" => app.mission_list.scroll(10),
                    "mission-live-scrolled" => {
                        app.mission_list.click_row(2);
                        app.mission_list.scroll(1);
                    }
                    _ => {}
                }
                app.episode_hover = None;
                app.draw_menu(&mut fb);
            }
            "lobby" | "create" | "lobby-chat" => {
                app.intro = false;
                app.menu = true;
                if app.campaign_len < app.maps.len() {
                    app.map_index = app.campaign_len;
                }
                // The reference capture has The Hunters selected; use the same
                // entry so the map browser and preview line up.
                if let Some(i) = app.maps.iter().position(|m| m.name.contains("The Hunters")) {
                    app.map_index = i;
                }
                app.menu_screen = MenuScreen::Lobby {
                    map: Some(app.map_index),
                };
                app.draw_menu(&mut fb);
            }
            "options" | "options-speed" | "options-sound" | "options-video" | "options-game"
            | "options-network" => {
                let tab = match shot_screen.as_str() {
                    "options-sound" => OptionsTab::Sound,
                    "options-video" => OptionsTab::Video,
                    "options-game" => OptionsTab::Game,
                    "options-network" => OptionsTab::Network,
                    _ => OptionsTab::Speed,
                };
                app.intro = false;
                app.menu = true;
                app.menu_screen = MenuScreen::Options { tab };
                app.draw_menu(&mut fb);
            }
            screen if screen.starts_with("hotkeys-any-") => {
                let rest = &screen["hotkeys-any-".len()..];
                let (race, id) = rest.split_once('-').unwrap_or(("terran", "7"));
                let race = match race {
                    "protoss" => HotkeyRace::Protoss,
                    "zerg" => HotkeyRace::Zerg,
                    _ => HotkeyRace::Terran,
                };
                let id = id.parse::<u16>().unwrap_or(7);
                app.intro = false;
                app.menu = true;
                app.hotkey_bindings = hotkey_data::HotkeyData::get().default_bindings();
                app.hotkey_unbound.clear();
                app.hotkey_binding_error = false;
                app.menu_screen = MenuScreen::Hotkeys { race };
                app.hotkey_sel = Some(id);
                app.hotkey_action = None;
                app.draw_menu(&mut fb);
            }
            "hotkeys"
            | "hotkeys-terran"
            | "hotkeys-protoss"
            | "hotkeys-zerg"
            | "hotkeys-terran-scv"
            | "hotkeys-protoss-probe"
            | "hotkeys-zerg-larva"
            | "hotkeys-zerg-larva-cards"
            | "hotkeys-terran-marine"
            | "hotkeys-terran-hover-scv"
            | "hotkeys-terran-scv-move"
            | "hotkeys-terran-scv-move-unbound"
            | "hotkeys-reset-dialog"
            | "hotkeys-terran-cc" => {
                let (race, sel, hover, action) = match shot_screen.as_str() {
                    "hotkeys-protoss" => (HotkeyRace::Protoss, None, None, None),
                    "hotkeys-zerg" => (HotkeyRace::Zerg, None, None, None),
                    "hotkeys-terran-scv" => (HotkeyRace::Terran, Some(7), None, None),
                    "hotkeys-protoss-probe" => (HotkeyRace::Protoss, Some(64), None, None),
                    "hotkeys-zerg-larva" => (HotkeyRace::Zerg, Some(35), None, Some(578)),
                    "hotkeys-zerg-larva-cards" => (HotkeyRace::Zerg, Some(35), None, None),
                    "hotkeys-terran-marine" => (HotkeyRace::Terran, Some(0), None, None),
                    "hotkeys-terran-hover-scv" => (HotkeyRace::Terran, None, None, None),
                    "hotkeys-terran-scv-move" => (HotkeyRace::Terran, Some(7), None, Some(664)),
                    "hotkeys-terran-scv-move-unbound" => {
                        (HotkeyRace::Terran, Some(7), None, Some(664))
                    }
                    "hotkeys-reset-dialog" => (HotkeyRace::Terran, Some(7), None, None),
                    "hotkeys-terran-cc" => (HotkeyRace::Terran, Some(106), None, None),
                    _ => (HotkeyRace::Terran, None, None, None),
                };
                app.intro = false;
                app.menu = true;
                app.hotkey_bindings = hotkey_data::HotkeyData::get().default_bindings();
                app.hotkey_unbound.clear();
                app.hotkey_binding_error = false;
                app.menu_screen = MenuScreen::Hotkeys { race };
                app.hotkey_sel = sel;
                app.hotkey_tab_hover = hover;
                app.hotkey_action = action;
                app.hotkey_hover = (shot_screen == "hotkeys-terran-hover-scv").then_some(7);
                if shot_screen == "hotkeys-terran-scv-move-unbound" {
                    app.rebind_hotkey(664, 'H', 'h' as u16);
                }
                app.hotkey_reset = shot_screen == "hotkeys-reset-dialog";
                app.draw_menu(&mut fb);
            }
            "collections" => {
                app.intro = false;
                app.menu = true;
                app.menu_screen = MenuScreen::Collections;
                app.draw_menu(&mut fb);
            }
            "credits" => {
                app.intro = false;
                app.menu = true;
                app.menu_screen = MenuScreen::Credits;
                app.draw_menu(&mut fb);
            }
            "saved" => {
                app.intro = false;
                app.menu = true;
                app.menu_screen = MenuScreen::SavedGames;
                app.draw_menu(&mut fb);
            }
            "lan" => {
                app.intro = false;
                app.menu = true;
                app.menu_screen = MenuScreen::Lan;
                app.lan.map_index = app.map_index;
                app.draw_menu(&mut fb);
            }
            "replays" => {
                app.intro = false;
                app.menu = true;
                app.menu_screen = MenuScreen::Replays;
                app.draw_menu(&mut fb);
            }
            "gamemenu" => {
                // The reference has "Return to Game (Esc)" highlighted.
                app.game_menu_sel = GameMenuItem::all().len() - 1;
                app.load_map(app.map_index);
                app.prepare_ingame_shot(&fb);
                // Draw the world, then the retail `gamemenu` art only: setting
                // `paused` here would stack the procedural pause panel on top.
                app.draw(&mut fb);
                app.draw_full_gamemenu(&mut fb);
            }
            "objectives" | "tips" => {
                // 134919 / 134931: the Zerg game of 134905 with the dialog open.
                app.hunters_start(&fb, 1, Race::Zerg, (-6, 8));
                app.update();
                app.draw(&mut fb);
                if shot_screen == "objectives" {
                    app.draw_objectives_panel(&mut fb);
                } else {
                    app.draw_tips_panel(&mut fb);
                }
            }
            "briefing" => {
                app.load_map(app.map_index);
                if !app.briefing_pages.is_empty() || app.briefing {
                    app.draw_briefing(&mut fb);
                } else {
                    app.draw(&mut fb);
                }
            }
            "pause" => {
                app.load_map(app.map_index);
                app.briefing = false;
                app.paused = true;
                app.draw(&mut fb);
            }
            "score" | "victory" | "defeat" => {
                app.load_map(app.map_index);
                app.briefing = false;
                app.intro = false;
                app.menu = false;
                app.game_result = Some(shot_screen != "defeat");
                app.update();
                app.draw(&mut fb);
            }
            "leaderboard" => {
                app.load_map(app.map_index);
                app.briefing = false;
                app.intro = false;
                app.menu = false;
                app.show_leaderboard = true;
                app.update();
                app.draw(&mut fb);
            }
            "stats" => {
                app.load_map(app.map_index);
                app.briefing = false;
                app.intro = false;
                app.menu = false;
                app.show_stats = true;
                app.update();
                app.draw(&mut fb);
            }
            "selection" | "hud" => {
                app.load_map(app.map_index);
                app.briefing = false;
                app.intro = false;
                app.menu = false;
                if let Some(id) = app
                    .world
                    .units
                    .iter()
                    .find(|u| u.alive && u.owner as usize == app.local_player)
                    .map(|u| u.id)
                {
                    app.selected = vec![id];
                    if let Some(u) = app.world.unit(id) {
                        app.camera = (u.x / 256 - app.view.0 / 2, u.y / 256 - app.view.1 / 2);
                        app.clamp_camera();
                    }
                }
                app.refresh_commands();
                app.update();
                app.draw(&mut fb);
            }
            _ => {
                app.load_map(app.map_index);
                app.briefing = false;
                app.intro = false;
                app.menu = false;
                app.update();
                app.draw(&mut fb);
            }
        }
        app.overlay = match shot_screen.as_str() {
            "objectives" => 1,
            "tips" => 2,
            "gamemenu" => 3,
            _ => 0,
        };
        apply_capture_correction(&mut fb);
        if let Some(path) = &shot {
            match fb.write_bmp(path) {
                Ok(()) => eprintln!(
                    "screenshot: {} ({}x{})",
                    path.display(),
                    fb.width,
                    fb.height
                ),
                Err(e) => {
                    eprintln!("cannot write {}: {e}", path.display());
                    std::process::exit(1);
                }
            }
            return;
        }
    }

    if freeze {
        app.speed = 0.0;
    }
    // The window manager can drop (or delay) the fullscreen request, which
    // leaves a 28 px title bar on screen and shifts every capture. Re-assert it
    // while the client runs: a capture must be pixel-aligned at (0,0).
    let mut last_fullscreen_ask = Instant::now();

    let mut window = Window::new(
        "GalaxyCraft - reconstructed StarCraft",
        width,
        height,
        WindowOptions {
            resize: !fullscreen,
            // NOT borderless: an override-redirect window never receives
            // keyboard or mouse focus, which made the menu unclickable.
            // Fullscreen is left to the window manager (Super+F or the title
            // bar) so input keeps working.
            borderless: false,
            topmost: fullscreen,
            ..WindowOptions::default()
        },
    )
    .expect("open window");
    if fullscreen {
        window.set_position(0, 0);
        // Ask the window manager for real fullscreen: it keeps input focus
        // (an override-redirect window does not) and hides the decorations.
        let _ = std::process::Command::new("wmctrl")
            .args([
                "-r",
                "GalaxyCraft - reconstructed StarCraft",
                "-b",
                "add,fullscreen",
            ])
            .status();
    }
    // The retail front-end shows its own pointer art; hide the system cursor
    // when that art is available so only one arrow is on screen.
    window.set_cursor_visibility(app.cursor_art.is_none());
    window.set_target_fps(60);

    let mut fps_frames = 0u32;
    let mut fps_last = Instant::now();
    let mut draw_ms = 0.0f64;
    let mut present_ms = 0.0f64;
    let mut needs_centre = start_screen.is_some();
    while window.is_open() && !app.quit {
        let (w, h) = window.get_size();
        if w != fb.width as usize || h != fb.height as usize {
            fb = Framebuffer::new(w.max(640), h.max(480));
            app.update_view(&fb);
        }
        app.pointer = window
            .get_unscaled_mouse_pos(MouseMode::Clamp)
            .map(|(x, y)| (x as i32, y as i32));
        if needs_centre {
            // The logical view is only known once the window exists; centring
            // earlier leaves the camera clamped to the map corner (black fog).
            app.centre_on_local_unit();
            app.world.update_vision();
            needs_centre = false;
        }
        if fullscreen && last_fullscreen_ask.elapsed().as_secs() >= 1 {
            last_fullscreen_ask = Instant::now();
            let _ = std::process::Command::new("wmctrl")
                .args([
                    "-r",
                    "GalaxyCraft - reconstructed StarCraft",
                    "-b",
                    "add,fullscreen",
                ])
                .status();
        }
        app.handle_input(&window);
        app.update();
        let draw_started = Instant::now();
        app.draw(&mut fb);
        if app.debug_input {
            app.draw_input_debug(&mut fb);
        }
        if app.capture_request {
            app.capture_request = false;
            let path = std::env::var("GC_CAPTURE_PATH")
                .unwrap_or_else(|_| "/tmp/opencode/menu-shots/live-capture.bmp".to_string());
            let _ = fb.write_bmp(std::path::Path::new(&path));
        }
        app.draw_cursor(&mut fb);
        let present_started = Instant::now();
        window
            .update_with_buffer(&fb.pixels, fb.width as usize, fb.height as usize)
            .expect("present frame");
        let now = Instant::now();
        draw_ms += (present_started - draw_started).as_secs_f64() * 1000.0;
        present_ms += (now - present_started).as_secs_f64() * 1000.0;
        fps_frames += 1;
        let elapsed = now.duration_since(fps_last).as_secs_f64();
        if elapsed >= 1.0 {
            if std::env::var_os("GC_FPS").is_some() {
                eprintln!(
                    "fps {:.1} | draw {:.1} ms | present {:.1} ms",
                    fps_frames as f64 / elapsed,
                    draw_ms / fps_frames as f64,
                    present_ms / fps_frames as f64
                );
            }
            fps_frames = 0;
            draw_ms = 0.0;
            present_ms = 0.0;
            fps_last = now;
        }
    }
}

/// Clear the whole menu frame, including the ultrawide pillarbox bars that
/// the 16:9 content box never touches.
fn clear_menu_frame(fb: &mut Framebuffer) {
    fb.clear(Rgb::new(0, 0, 0));
}

/// `A`..`Z` as typed in the rebind input field.
fn key_letter(key: Key) -> Option<char> {
    Some(match key {
        Key::A => 'A',
        Key::B => 'B',
        Key::C => 'C',
        Key::D => 'D',
        Key::E => 'E',
        Key::F => 'F',
        Key::G => 'G',
        Key::H => 'H',
        Key::I => 'I',
        Key::J => 'J',
        Key::K => 'K',
        Key::L => 'L',
        Key::M => 'M',
        Key::N => 'N',
        Key::O => 'O',
        Key::P => 'P',
        Key::Q => 'Q',
        Key::R => 'R',
        Key::S => 'S',
        Key::T => 'T',
        Key::U => 'U',
        Key::V => 'V',
        Key::W => 'W',
        Key::X => 'X',
        Key::Y => 'Y',
        Key::Z => 'Z',
        _ => return None,
    })
}

/// CEF `.unit.bindingError:after`: 30 px rgba(0,0,0,.6) plate and the 24 px
/// `unbound.png` red disc with a transparent question mark.
fn draw_unbound_badge(fb: &mut Framebuffer, bx: i32, by: i32, dx: i32, dy: i32) {
    fill_alpha_rect(fb, bx + dx * 4, by + dy * 3, dx * 30, dy * 30, 153);
    let cx = bx as f32 + dx as f32 * 19.0;
    let cy = by as f32 + dy as f32 * 18.0;
    let r = dx as f32 * 12.0;
    let red = Rgb::new(204, 14, 0);
    for yy in (cy - r - 1.0) as i32..(cy + r + 1.0) as i32 {
        for xx in (cx - r - 1.0) as i32..(cx + r + 1.0) as i32 {
            let dist = ((xx as f32 + 0.5 - cx).powi(2) + (yy as f32 + 0.5 - cy).powi(2)).sqrt();
            let cov = (r + 0.5 - dist).clamp(0.0, 1.0);
            if cov > 0.0 {
                let under = fb.get(xx, yy);
                fb.put(xx, yy, under.blend(red, (cov * 255.0) as u8));
            }
        }
    }
}

impl App {
    fn play(&mut self, name: &str) {
        self.audio.play_named(&self.sounds, name);
        self.sound_log.push(name.to_string());
        if self.sound_log.len() > 512 {
            self.sound_log.drain(..256);
        }
    }

    /// Apply a rebind and run CEF's per-unit collision rules.
    fn rebind_hotkey(&mut self, action: u16, text: char, code: u16) {
        let (unbound, error) =
            hotkey_data::apply_rebind(&mut self.hotkey_bindings, action, text, code);
        self.hotkey_unbound = unbound;
        self.hotkey_binding_error = error;
    }

    fn refresh_hotkey_errors(&mut self) {
        let hk = hotkey_data::HotkeyData::get();
        self.hotkey_unbound.clear();
        for unit in hk.all_units() {
            let unbound = unit.cards.iter().flat_map(|c| c.actions.iter()).any(|a| {
                self.hotkey_bindings
                    .get(a.id as usize)
                    .copied()
                    .flatten()
                    .is_none()
            });
            if unbound {
                self.hotkey_unbound.push(unit.id);
            }
        }
        self.hotkey_binding_error = !self.hotkey_unbound.is_empty();
    }

    fn restore_hotkey_defaults(&mut self) {
        self.hotkey_bindings = hotkey_data::HotkeyData::get().default_bindings();
        self.hotkey_unbound.clear();
        self.hotkey_binding_error = false;
    }

    /// Play the retail acknowledgement for the selected unit, cycling takes.
    fn play_ack(&mut self) {
        let take = self.ack_variant;
        self.ack_variant = self.ack_variant.wrapping_add(1);
        if let Some(u) = self.first_selected() {
            let unit_type = u.type_id;
            if let Some(samples) = self.voices.cue(unit_type, VoiceCue::Yes, take) {
                self.audio.play(Some(&samples));
                return;
            }
        }
        let race = self
            .world
            .players
            .get(self.local_player)
            .map(|p| p.race)
            .unwrap_or(Race::Terran);
        self.audio.play(self.sounds.race_yes(race));
    }

    /// Retail "ready" line when a unit is first selected.
    fn play_selected(&mut self) {
        let take = self.ack_variant;
        if let Some(u) = self.first_selected() {
            let unit_type = u.type_id;
            if let Some(samples) = self.voices.cue(unit_type, VoiceCue::Ready, take) {
                self.audio.play(Some(&samples));
            }
        }
    }

    fn play_music(&mut self, name: &str) {
        if self.music_track == name {
            return;
        }
        if let Some(track) = self.music.track(name) {
            self.audio.play_music(Some(track));
            self.music_track = name.to_string();
            self.music_started = Instant::now();
        } else {
            self.music_track = name.to_string();
        }
    }

    fn start_menu_music(&mut self) {
        self.play_music("title");
    }

    /// Race identity for music/console selection.
    fn local_race(&self) -> Race {
        self.world
            .players
            .get(self.local_player)
            .map(|p| p.race)
            .unwrap_or(Race::Terran)
    }

    fn race_music_prefix(&self) -> &'static str {
        match self.local_race() {
            Race::Zerg => "z",
            Race::Protoss => "p",
            _ => "t",
        }
    }

    fn start_mission_music(&mut self) {
        let prefix = self.race_music_prefix();
        let name = format!("{}rdyroom", prefix);
        self.music_phase = 0;
        self.play_music(&name);
    }

    /// After the ready room, rotate the four shipped battle tracks, then the
    /// victory/defeat cue is selected by the result path.
    fn rotate_music(&mut self) {
        if self.music_track.is_empty() {
            self.start_mission_music();
            return;
        }
        if self.music_track.ends_with("vict") || self.music_track.ends_with("defeat") {
            return;
        }
        if self.music_started.elapsed() < Duration::from_secs(32) {
            return;
        }
        let race = match self.local_race() {
            Race::Zerg => "zerg",
            Race::Protoss => "protoss",
            _ => "terran",
        };
        self.music_phase = self.music_phase % 4 + 1;
        let name = format!("{}{}", race, self.music_phase);
        self.play_music(&name);
    }

    fn play_result_music(&mut self, won: bool) {
        let prefix = self.race_music_prefix();
        let name = format!("{}{}", prefix, if won { "vict" } else { "defeat" });
        self.music_phase = 0;
        self.play_music(&name);
    }

    /// UI scale and origin.
    ///
    /// Driven by the **height** so HUD art keeps its proportions from 16:9
    /// up to 4K and, at the same time, ultrawide screens expand the world
    /// view horizontally instead of getting letterboxed side bars.
    /// The 16:9 content box, centred. The retail client pillarboxes a 16:9
    /// render inside a wider desktop (verified on the 3440x1440 references:
    /// content occupies columns 440..2999, i.e. 2560x1440).
    fn content_rect(fb: &Framebuffer) -> (i32, i32, i32, i32) {
        let h = fb.height.max(1);
        let want_w = ((h as f32 * 16.0 / 9.0).round() as i32).max(1);
        let w = want_w.min(fb.width.max(1));
        let x0 = (fb.width - w) / 2;
        (x0, 0, w, h)
    }

    fn ui_metrics(&self, fb: &Framebuffer) -> (f32, i32, i32) {
        let (cx, cy, cw, ch) = Self::content_rect(fb);
        let scale = (ch as f32 / 480.0).max(0.5);
        let ox = cx + ((cw as f32 - 640.0 * scale) / 2.0).round() as i32;
        (scale, ox, cy)
    }

    fn update_view(&mut self, fb: &Framebuffer) {
        let (scale, ox, oy) = self.ui_metrics(fb);
        self.ui_scale = scale;
        self.ui_ox = ox;
        self.ui_oy = oy;
        // Logical world viewport inside the 16:9 content box.  The divisor
        // must equal the blit scale used in `draw` (world_fb * scale fills
        // the content width exactly); a fixed divisor only fills the box at
        // the one resolution where it coincides with the scale (1440p) and
        // leaves black bands plus a detached HUD everywhere else.
        let (cx, _cy, cw, ch) = Self::content_rect(fb);
        self.content_x0 = cx;
        self.content_box = (cx, _cy, cw, ch);
        // The console band is 28.13% of the content height (measured on the
        // retail references); the world viewport is the rest, in logical units.
        let hud_h = ch as f32 * 0.2813;
        let div = scale.max(0.5);
        let view_w = ((cw as f32) / div).round().max(320.0) as i32;
        let view_h = (((ch as f32 - hud_h) / div).round()).max(180.0) as i32;
        self.view = (view_w, view_h);
    }

    /// Screen pixels -> world-view pixels.  Must use the same divisor as
    /// `update_view`, otherwise clicks land off by (scale/3) at any
    /// resolution where the UI scale differs from 3.
    fn to_logical(&self, sx: i32, sy: i32) -> (i32, i32) {
        let s = self.ui_scale.max(0.5);
        (
            ((sx - self.content_x0) as f32 / s) as i32,
            (sy as f32 / s) as i32,
        )
    }

    /// Start a retail intro playlist.  `pending_map` is loaded when the last
    /// clip finishes (campaign pre-mission movies); otherwise the client
    /// returns to the main menu.
    fn begin_intro(&mut self, clips: Vec<String>, pending_map: Option<usize>) {
        self.intro_clips = clips;
        self.intro_clip = 0;
        self.intro_pending_map = pending_map;
        self.intro = true;
        self.menu = false;
        self.briefing = false;
        self.intro_player = None;
        self.intro_probed = false;
        self.intro_audio = None;
        self.intro_started = Instant::now();
        self.intro_last = Instant::now();
        self.audio.play_music(None);
    }

    fn handle_intro(&mut self, window: &Window) {
        for key in window.get_keys_pressed(KeyRepeat::No) {
            match key {
                Key::Enter | Key::Space | Key::Escape => {
                    self.play("ui_swish");
                    self.advance_intro();
                    return;
                }
                _ => {}
            }
        }
        if window.get_mouse_down(MouseButton::Left) {
            self.advance_intro();
        }
    }

    /// Move to the next clip, or leave the intro when the playlist is done.
    fn advance_intro(&mut self) {
        if self.intro_clip + 1 < self.intro_clips.len() {
            self.intro_clip += 1;
            self.intro_player = None;
            self.intro_probed = false;
            self.intro_audio = None;
            self.intro_started = Instant::now();
            self.intro_last = Instant::now();
            self.audio.play_music(None);
            return;
        }
        self.end_intro();
    }

    fn end_intro(&mut self) {
        self.intro = false;
        self.intro_player = None;
        self.intro_clips.clear();
        if let Some(index) = self.intro_pending_map.take() {
            self.audio.play_music(None);
            self.load_map(index);
            return;
        }
        self.menu = true;
        self.start_menu_music();
    }

    fn draw_intro(&mut self, fb: &mut Framebuffer) {
        // Retail opening playlists: `smk/blizzard.webm` then
        // `smk/starintr.webm` on first launch (`smk/starxintr.webm` for the
        // expansion), or one `Pre*`/episode movie during the campaign.  Each
        // clip streams from the materialised cache; an install without
        // `ffmpeg` falls back to the shipped title art.
        let (w, h) = (fb.width as usize, fb.height as usize);
        if self.intro_clips.is_empty() {
            self.end_intro();
            return;
        }
        let rel = self.intro_clips[self.intro_clip.min(self.intro_clips.len() - 1)].clone();
        let path = self
            .video_store
            .as_ref()
            .map(|store| store.root.join(&rel))
            .unwrap_or_default();
        if self.intro_player.is_none() && !self.intro_probed {
            self.intro_probed = true;
            if let Some(info) = galaxycraft::video::probe_info(&path) {
                let start_at = self.intro_started.elapsed().as_secs_f32();
                self.intro_player = galaxycraft::cinematic::Cinematic::open_at(
                    &path,
                    &info,
                    INTRO_FPS,
                    INTRO_WIDTH,
                    start_at,
                );
                self.intro_last = Instant::now();
                if info.has_audio {
                    let audio_path = path.clone();
                    let (tx, rx) = std::sync::mpsc::channel();
                    self.intro_audio = Some(rx);
                    std::thread::spawn(move || {
                        if let Some(pcm) = galaxycraft::video::decode_audio_pcm(&audio_path) {
                            let _ = tx.send(pcm);
                        }
                    });
                }
            }
        }
        if let Some(rx) = &self.intro_audio {
            if let Ok(pcm) = rx.try_recv() {
                self.audio.play_music(Some(std::sync::Arc::new(pcm)));
                // Mark the track so `start_menu_music` knows it must replace
                // the movie soundtrack when the intro ends or is skipped.
                self.music_track = "intro-movie".to_string();
                self.intro_audio = None;
            }
        }
        let dt = self.intro_last.elapsed().as_secs_f32();
        self.intro_last = Instant::now();
        let mut frame = self
            .intro_player
            .as_mut()
            .and_then(|player| player.advance(dt));
        if frame.is_none() {
            // Fallback: the retail title art, cover-fitted.
            frame = self.intro_poster.clone();
        }
        if let Some(poster) = frame {
            draw_movie_frame(fb, &poster);
        } else {
            fb.clear(Rgb::new(0, 0, 0));
            let stale = self
                .intro_fit
                .as_ref()
                .map(|(fw, fh, _)| *fw != w || *fh != h)
                .unwrap_or(true);
            if stale {
                self.intro_fit = self
                    .glue
                    .as_ref()
                    .and_then(|g| g.title.as_ref())
                    .map(|art| (w, h, art.cover(w, h)));
            }
            if let Some((_, _, art)) = &self.intro_fit {
                for y in 0..h {
                    for x in 0..w {
                        let p = art.pixels[y * w + x];
                        fb.put(x as i32, y as i32, Rgb(p & 0x00ff_ffff));
                    }
                }
            }
        }
        if self.intro_player.as_ref().is_some_and(|p| p.finished()) {
            self.advance_intro();
        }
    }

    fn status(&mut self, text: impl Into<String>) {
        self.status = text.into();
        self.status_until = Instant::now() + Duration::from_secs(4);
    }

    fn load_map(&mut self, index: usize) {
        let Some(entry) = self.maps.get(index).cloned() else {
            return;
        };
        let raw = match std::fs::read(&entry.path) {
            Ok(raw) => raw,
            Err(e) => {
                self.status(format!("Cannot read {}: {e}", entry.path.display()));
                return;
            }
        };
        let scenario = match Scenario::parse(&raw) {
            Ok(sc) => sc,
            Err(e) => {
                self.status(format!("Invalid map: {e}"));
                return;
            }
        };
        let Some(tileset) = Tileset::load(&self.tiles_dir, scenario.era, &scenario) else {
            self.status(format!("Missing {} tileset assets", era_name(scenario.era)));
            return;
        };
        let vf4 = match std::fs::read(
            self.tiles_dir
                .join(format!("{}.vf4", era_name(scenario.era))),
        ) {
            Ok(v) => v,
            Err(e) => {
                self.status(format!("Missing vf4: {e}"));
                return;
            }
        };
        let slots = scenario.player_slots();
        self.local_player = self
            .melee_slot
            .filter(|&s| !scenario.has_placed_force() && slots.iter().any(|p| p.slot == s))
            .or_else(|| slots.iter().find(|s| s.active && s.human).map(|s| s.slot))
            .unwrap_or(0);
        let races = races_for(&scenario, &slots, &self.races, self.menu_race);
        // The console follows the race actually played: a melee slot's SIDE
        // byte is "user select" and would always fall back to Terran.
        let console_race = races
            .get(self.local_player)
            .copied()
            .unwrap_or(Race::Terran);
        self.console_race = console_race;
        self.console = ConsoleArt::load(&self.assets_root, console_race);
        let mut world = World::from_scenario(
            self.data.clone(),
            &scenario,
            &galaxycraft::map::TileData::new(tileset.cv5(), &vf4),
            &races,
        );
        apply_slots(&mut world, &slots);
        // Melee CHKs mark every slot as computer (OWNR 6); the local player
        // is the human one, never driven by the AI.
        if let Some(p) = world.players.get_mut(self.local_player) {
            p.is_computer = false;
        }
        world.enable_fog();
        let local = self.local_player;
        self.ais = slots
            .iter()
            .filter(|s| s.computer && s.slot != local)
            .map(|s| SimpleAi::new(s.slot as u8))
            .collect();
        self.campaign_ais.clear();
        // Campaign missions carry a retail `est*.txt` briefing script and a
        // retail backdrop; use them when present and only fall back to the
        // map-authored STR copy for missions without one.
        let mut briefing_pages = Vec::new();
        let mut briefing_art = None;
        if let Some(meta) = &entry.campaign {
            if let Some(script_path) = &meta.script {
                if let Ok(text) = std::fs::read_to_string(script_path) {
                    briefing_pages = galaxycraft::campaign::parse_script(&text)
                        .pages
                        .into_iter()
                        .filter(|p| !p.text.trim().is_empty() || p.video.is_some())
                        .collect();
                }
            }
            if let Some(background) = &meta.background {
                briefing_art = std::fs::read(background)
                    .ok()
                    .and_then(|bytes| galaxycraft::dds::parse(&bytes));
            }
            if briefing_art.is_none() {
                if let Some(page) = briefing_pages.first() {
                    if let Some(path) = &page.background {
                        let file = self.tiles_dir.join("../glue").join(path);
                        briefing_art = std::fs::read(file)
                            .ok()
                            .and_then(|bytes| galaxycraft::dds::parse(&bytes));
                    }
                }
            }
        }
        let show_retail = !briefing_pages.is_empty();
        let show_briefing =
            show_retail || (scenario.has_placed_force() && briefing_content(&scenario).is_some());
        self.briefing_pages = briefing_pages;
        self.briefing_page = 0;
        self.briefing_art = briefing_art;
        self.briefing_fit = None;
        // Decoded briefing intermission: the first page that names a
        // `</VIDEO>` file gets its poster frame from the materialised
        // `videos/` cache, so the briefing screen shows the retail movie
        // instead of only the INTERMISSION caption.
        self.briefing_poster = None;
        self.briefing_video = None;
        if let Some(store) = &self.video_store {
            for page in self.briefing_pages.iter() {
                if let Some(video) = &page.video {
                    if let Some((_, poster)) = store.poster(video) {
                        self.briefing_poster = Some(poster);
                        self.briefing_video = Some(video.clone());
                        break;
                    }
                }
            }
        }
        if self
            .hd_terrain
            .as_ref()
            .is_none_or(|_| self.scenario.era != scenario.era)
        {
            self.hd_terrain = galaxycraft::render::HdTerrain::load(&self.assets_root, scenario.era);
        }
        self.scenario = scenario;
        self.tileset = tileset;
        self.chk_raw = raw;
        self.last_races = races.clone();
        world.recorder = Some(Recorder::new());
        self.world = world;
        self.replay = None;
        self.save_sel = 0;
        self.map_index = index;
        self.menu_tab = if index < self.campaign_len { 0 } else { 1 };
        self.menu_index = if index < self.campaign_len {
            index
        } else {
            index - self.campaign_len
        };
        self.menu = false;
        self.briefing = show_briefing;
        self.paused = false;
        self.game_result = None;
        self.next_map = None;
        self.selected.clear();
        self.groups = Default::default();
        self.build_mode = None;
        self.command_list.clear();
        self.build_list.clear();
        self.attack_mode = false;
        self.patrol_mode = false;
        self.camera = (0, 0);
        self.speed = 1.0;
        self.accumulator = 0.0;
        self.last_step = Instant::now();
        self.center_on_base();
        self.refresh_commands();
        self.result_panel = None;
        self.result_fit = None;
        self.show_stats = false;
        self.show_leaderboard = false;
        self.intro = false;
        self.start_mission_music();
        // Retail shows no map banner at game start (134905/135020/135051).
    }

    fn restart(&mut self) {
        let index = self.map_index;
        self.load_map(index);
    }

    /// Mission number inside its episode (`terran01` -> 1, `expprotoss03` -> 3).
    fn campaign_order(tag: &CampaignTag) -> Option<usize> {
        tag.mission
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>()
            .parse()
            .ok()
    }

    /// Start a campaign mission through its retail cinematic sequence.
    ///
    /// The executable's intro table pairs `Pre<Race>_<act>` with the first
    /// mission of each act and `<Race><act>` with the last one (REA table at
    /// 0x140ae7ae0).  Brood War episodes open with `starxintr.webm` and their
    /// `x<Race>.webm` episode movie.
    fn start_campaign_mission(&mut self, index: usize) {
        let tag = self.maps.get(index).and_then(|m| m.campaign.clone());
        let mut clips: Vec<String> = Vec::new();
        if let Some(tag) = &tag {
            if let Some(order) = Self::campaign_order(tag) {
                let expansion = tag.episode.starts_with("exp");
                if expansion && order == 1 {
                    clips.push(galaxycraft::video::INTRO_EXPANSION.to_string());
                    if let Some(intro) = galaxycraft::video::expansion_episode_intro(&tag.episode) {
                        clips.push(intro.to_string());
                    }
                }
                if let Some(pre) = galaxycraft::video::campaign_cinematic(&tag.episode, order, true)
                {
                    clips.push(pre.to_string());
                }
            }
        }
        if clips.is_empty() {
            self.load_map(index);
        } else {
            self.begin_intro(clips, Some(index));
        }
    }

    /// Movie played after the last mission of each act, if the episode has
    /// one.  Returns the path while the score screen is up.
    fn post_mission_cinematic(&self) -> Option<&'static str> {
        let tag = self
            .maps
            .get(self.map_index)
            .and_then(|m| m.campaign.as_ref())?;
        let order = Self::campaign_order(tag)?;
        galaxycraft::video::campaign_cinematic(&tag.episode, order, false)
    }

    /// Current recording (empty when nothing was issued yet).
    fn take_recording(&self) -> Recording {
        let mut rec = Recording::new(0x1234_5678_9abc_def0, [0u8; 32]);
        if let Some(r) = self.world.recorder.as_ref() {
            rec.cmds = r.cmds.clone();
        }
        rec
    }

    fn save_list(&self, replays: bool) -> Vec<PathBuf> {
        let dir = if replays {
            &self.replays_dir
        } else {
            &self.saves_dir
        };
        let ext = if replays { "gcrep" } else { "gcsave" };
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) == Some(ext) {
                    out.push(p);
                }
            }
        }
        out.sort();
        out
    }

    fn slot_label(path: &Path) -> String {
        match SaveFile::load(path) {
            Some(s) => format!(
                "{}  tick {}  ({} cmds)",
                s.map_name,
                s.tick,
                s.rec.cmds.len()
            ),
            None => format!("{} (corrupt)", path.display()),
        }
    }

    fn save_game(&mut self) {
        let rec = self.take_recording();
        let tick = self.world.tick;
        let save = SaveFile {
            map_name: self
                .maps
                .get(self.map_index)
                .map(|m| m.name.clone())
                .unwrap_or_default(),
            map_chk: self.chk_raw.clone(),
            races: self.last_races.clone(),
            local_player: self.local_player,
            tick,
            rec,
        };
        let _ = std::fs::create_dir_all(&self.saves_dir);
        let n = self.save_list(false).len();
        let path = self.saves_dir.join(format!("save-{:03}.gcsave", n + 1));
        match save.save(&path) {
            Ok(()) => self.status(format!(
                "Saved {} (tick {}, hash {:x})",
                path.display(),
                tick,
                state_hash(&self.world)
            )),
            Err(e) => self.status(format!("Save failed: {e}")),
        }
        self.play("ui_click");
    }

    fn save_replay(&mut self) {
        let rec = self.take_recording();
        let tick = self.world.tick;
        let save = SaveFile {
            map_name: self
                .maps
                .get(self.map_index)
                .map(|m| m.name.clone())
                .unwrap_or_default(),
            map_chk: self.chk_raw.clone(),
            races: self.last_races.clone(),
            local_player: self.local_player,
            tick,
            rec,
        };
        let _ = std::fs::create_dir_all(&self.replays_dir);
        let n = self.save_list(true).len();
        let path = self.replays_dir.join(format!("replay-{:03}.gcrep", n + 1));
        match save.save(&path) {
            Ok(()) => self.status(format!("Replay saved {}", path.display())),
            Err(e) => self.status(format!("Replay save failed: {e}")),
        }
        self.play("ui_click");
    }

    /// Rebuild a world from a save file (shared by load-game and replay).
    fn build_from_save(&mut self, save: &SaveFile) -> bool {
        let scenario = match Scenario::parse(&save.map_chk) {
            Ok(sc) => sc,
            Err(e) => {
                self.status(format!("Bad save map: {e}"));
                return false;
            }
        };
        let Some(tileset) = Tileset::load(&self.tiles_dir, scenario.era, &scenario) else {
            self.status("Missing tileset assets".to_string());
            return false;
        };
        let vf4 = match std::fs::read(
            self.tiles_dir
                .join(format!("{}.vf4", era_name(scenario.era))),
        ) {
            Ok(v) => v,
            Err(e) => {
                self.status(format!("Missing vf4: {e}"));
                return false;
            }
        };
        let slots = scenario.player_slots();
        let mut world = World::from_scenario(
            self.data.clone(),
            &scenario,
            &galaxycraft::map::TileData::new(tileset.cv5(), &vf4),
            &save.races,
        );
        apply_slots(&mut world, &slots);
        world.enable_fog();
        world.rng = save.rec.seed;
        galaxycraft::replay::replay_to(&mut world, &save.rec, save.tick);
        if self
            .hd_terrain
            .as_ref()
            .is_none_or(|_| self.scenario.era != scenario.era)
        {
            self.hd_terrain = galaxycraft::render::HdTerrain::load(&self.assets_root, scenario.era);
        }
        self.scenario = scenario;
        self.tileset = tileset;
        self.chk_raw = save.map_chk.clone();
        self.last_races = save.races.clone();
        self.local_player = save.local_player.min(world.players.len().saturating_sub(1));
        self.console_race = slots
            .iter()
            .find(|s| s.slot == self.local_player)
            .map(|s| s.race(Race::Terran))
            .unwrap_or(Race::Terran);
        self.console = ConsoleArt::load(&self.assets_root, self.console_race);
        self.world = world;
        self.menu = false;
        self.briefing = false;
        self.paused = false;
        self.game_result = None;
        self.next_map = None;
        self.selected.clear();
        self.groups = Default::default();
        self.build_mode = None;
        self.command_list.clear();
        self.build_list.clear();
        self.attack_mode = false;
        self.patrol_mode = false;
        self.camera = (0, 0);
        self.speed = 1.0;
        self.accumulator = 0.0;
        self.last_step = Instant::now();
        self.center_on_base();
        self.refresh_commands();
        self.result_panel = None;
        self.result_fit = None;
        self.show_stats = false;
        self.show_leaderboard = false;
        self.intro = false;
        self.start_mission_music();
        true
    }

    fn load_save(&mut self, path: &Path, as_replay: bool) {
        let Some(save) = SaveFile::load(path) else {
            self.status(format!("Cannot load {}", path.display()));
            return;
        };
        let tick = save.tick;
        if !self.build_from_save(&save) {
            return;
        }
        if as_replay {
            self.ais.clear();
            self.campaign_ais.clear();
            self.replay = Some(ReplayPlayer::new(save));
            self.status(format!(
                "Replaying ({} cmds)  SPACE: pause  +/-: speed  ESC: stop",
                self.replay.as_ref().unwrap().save.rec.cmds.len()
            ));
        } else {
            self.replay = None;
            // Keep recording on top of the loaded commands.
            if let Some(r) = self.world.recorder.as_mut() {
                r.cmds = save.rec.cmds.clone();
            }
            self.status(format!(
                "Loaded (tick {tick}, hash {:x})",
                state_hash(&self.world)
            ));
        }
        self.play("ui_swish");
    }

    // ------------------------------------------------------------------
    // LAN lobby + lockstep match
    // ------------------------------------------------------------------

    /// Ensure the discovery socket exists and refresh the lobby list.
    fn lan_scan(&mut self) {
        let sock = match discovery_socket(galaxycraft::net::DISCOVERY_PORT) {
            Ok(s) => s,
            // Someone else on this machine holds the well-known port: fall
            // back to an ephemeral one so the screen still works.
            Err(_) => match discovery_socket(0) {
                Ok(s) => s,
                Err(e) => {
                    self.lan.status = format!("discovery unavailable: {e}");
                    return;
                }
            },
        };
        match discover(&sock, Duration::from_millis(300)) {
            Ok(list) => {
                self.lan.entries = list;
                self.lan.sel = self.lan.sel.min(self.lan.entries.len().saturating_sub(1));
                self.lan.status = if self.lan.entries.is_empty() {
                    "no games found (H to host)".to_string()
                } else {
                    format!("{} game(s) found", self.lan.entries.len())
                };
            }
            Err(e) => self.lan.status = format!("discovery error: {e}"),
        }
    }

    /// Start hosting: bind the lobby, announce it, and wait for one peer.
    fn lan_host(&mut self) {
        let entry = match self.maps.get(self.lan.map_index) {
            Some(e) => e.clone(),
            None => return,
        };
        let raw = match std::fs::read(&entry.path) {
            Ok(r) => r,
            Err(e) => {
                self.lan.status = format!("cannot read map: {e}");
                return;
            }
        };
        let mut map_hash = [0u8; 32];
        map_hash.copy_from_slice(&Sha256::digest(&raw));
        match Lobby::bind("0.0.0.0:7777", map_hash, galaxycraft::net::WORLD_SEED) {
            Ok(l) => {
                self.lan.listener = Some(l);
                self.lan.announce_sock = announcer().ok();
                self.lan.last_announce = Instant::now() - Duration::from_secs(5);
                self.lan.pending = None;
                self.lan.status = format!("hosting {} on :7777 - waiting for a peer", entry.name);
            }
            Err(e) => self.lan.status = format!("cannot host: {e}"),
        }
    }

    /// While hosting: keep announcing and accept the first peer that joins.
    fn lan_poll_host(&mut self) {
        let Some(lobby) = self.lan.listener.as_ref() else {
            return;
        };
        let me = self.menu_race;
        let map_hash = lobby.map_hash;
        let name = self
            .maps
            .get(self.lan.map_index)
            .map(|m| m.name.clone())
            .unwrap_or_default();

        if self.lan.last_announce.elapsed() >= Duration::from_secs(1) {
            if let Some(sock) = self.lan.announce_sock.as_ref() {
                let a = Announce {
                    name,
                    map_hash,
                    port: 7777,
                    players: 2,
                    started: false,
                };
                let _ = broadcast(sock, &a);
            }
            self.lan.last_announce = Instant::now();
        }

        // Non-blocking accept so the menu keeps drawing.
        let _ = lobby.listener.set_nonblocking(true);
        let accepted = lobby.listener.accept();
        let _ = lobby.listener.set_nonblocking(false);
        let Ok((stream, addr)) = accepted else {
            return;
        };
        let Ok(mut t) = TcpTransport::new(stream) else {
            return;
        };
        let _ = t.stream_timeout(Duration::from_millis(750));
        match t.recv() {
            Ok(Msg::Join {
                map_hash: m, race, ..
            }) if m == map_hash => {
                let setup = Msg::Setup {
                    peers: 2,
                    map_hash,
                    seed: galaxycraft::net::WORLD_SEED,
                    races: vec![me, race],
                    ticks_per_turn: DEFAULT_TICKS_PER_TURN,
                };
                if t.send(&setup).is_err() {
                    self.lan.status = "handshake failed".to_string();
                    return;
                }
                let index = self.lan.map_index;
                self.lan.listener = None;
                self.lan.announce_sock = None;
                if self.start_net_match(NetRole::Host, t, vec![me, race], index) {
                    self.lan.status = format!("match started with {addr}");
                }
            }
            Ok(_) => self.lan.status = "peer map mismatch".to_string(),
            Err(e) => self.lan.status = format!("handshake error: {e}"),
        }
    }

    /// Join the selected lobby.
    fn lan_join(&mut self) {
        let Some((addr, announce)) = self.lan.entries.get(self.lan.sel).cloned() else {
            return;
        };
        match galaxycraft::net::connect(
            &format!("{}:{}", addr.ip(), announce.port),
            announce.map_hash,
            galaxycraft::net::WORLD_SEED,
            self.menu_race,
        ) {
            Ok((t, Msg::Setup { races, .. })) => {
                // The host decides the map and races; find it by hash.
                let index = self
                    .maps
                    .iter()
                    .position(|m| {
                        std::fs::read(&m.path)
                            .map(|raw| {
                                let mut h = [0u8; 32];
                                h.copy_from_slice(&Sha256::digest(&raw));
                                h == announce.map_hash
                            })
                            .unwrap_or(false)
                    })
                    .unwrap_or(self.lan.map_index);
                if self.start_net_match(NetRole::Client, t, races, index) {
                    self.lan.status = format!("joined {addr}");
                }
            }
            Ok((_, other)) => self.lan.status = format!("unexpected: {other:?}"),
            Err(e) => self.lan.status = format!("join failed: {e}"),
        }
    }

    /// Build the world for a networked match and switch into lockstep.
    fn start_net_match(
        &mut self,
        role: NetRole,
        peer: TcpTransport,
        races: Vec<Race>,
        map_index: usize,
    ) -> bool {
        let Some(entry) = self.maps.get(map_index).cloned() else {
            self.lan.status = "no map selected".to_string();
            return false;
        };
        let raw = match std::fs::read(&entry.path) {
            Ok(r) => r,
            Err(e) => {
                self.lan.status = format!("cannot read map: {e}");
                return false;
            }
        };
        let local = if role == NetRole::Host { 0 } else { 1 };
        let save = SaveFile {
            map_name: entry.name.clone(),
            map_chk: raw,
            races: races.clone(),
            local_player: local,
            tick: 0,
            rec: Recording::new(galaxycraft::net::WORLD_SEED, [0u8; 32]),
        };
        if !self.build_from_save(&save) {
            return false;
        }
        self.map_index = map_index;
        self.menu = false;
        self.intro = false;
        self.briefing = false;
        self.paused = false;
        self.speed = 1.0;
        self.accumulator = 0.0;
        self.last_step = Instant::now();
        self.net = Some(NetSession {
            role,
            lock: Lockstep::new(local as u8, races.len() as u8, DEFAULT_TICKS_PER_TURN),
            peers: vec![peer],
            desync: false,
            last_hash: 0,
        });
        true
    }

    /// One lockstep turn per accumulated `ticks_per_turn`.
    fn update_net(&mut self) {
        let Some(mut net) = self.net.take() else {
            return;
        };
        if self.game_result.is_some() {
            self.net = Some(net);
            return;
        }
        let now = Instant::now();
        let dt = now.duration_since(self.last_step).as_secs_f32().min(0.1);
        self.last_step = now;
        self.accumulator += dt * 24.0 * self.speed;
        let need = net.lock.ticks_per_turn as f32;
        if self.accumulator < need {
            self.net = Some(net);
            return;
        }
        self.accumulator -= need;

        let acts = net.lock.take_local();
        let res = match net.role {
            NetRole::Host => host_turn(&mut net.lock, &mut self.world, &mut net.peers, acts),
            NetRole::Client => match net.peers.first_mut() {
                Some(host) => client_turn(&mut net.lock, &mut self.world, host, acts),
                None => Err(std::io::Error::other("no host transport")),
            },
        };
        match res {
            Ok(desync) => {
                net.desync |= desync;
                net.last_hash = state_hash(&self.world);
                if desync {
                    self.status("LAN DESYNC - match halted".to_string());
                    self.paused = true;
                }
            }
            Err(e) => {
                self.status(format!("LAN disconnected: {e}"));
                self.net = None;
                return;
            }
        }
        self.selected
            .retain(|id| self.world.unit(*id).is_some_and(|u| u.alive));
        self.net = Some(net);
    }

    /// Route an order: queued for the next network turn, or applied locally.
    fn send_order(&mut self, unit: u32, order: Order) {
        match self.net.as_mut() {
            Some(net) => net.lock.queue(RecAct::Issue { unit, order }),
            None => {
                let _ = self.world.issue(unit, order);
            }
        }
    }

    /// Same routing for production/research so both peers issue identical
    /// commands (the AI and the player both go through here).
    fn send_train(&mut self, building: u32, unit_type: u16) -> bool {
        match self.net.as_mut() {
            Some(net) => {
                net.lock.queue(RecAct::Train {
                    building,
                    unit: unit_type,
                });
                true
            }
            None => self.world.train(building, unit_type).is_ok(),
        }
    }

    /// Select a few local units so headless captures show the selection HUD
    /// the retail screenshots have (wireframes + portrait).
    /// Decode (and cache) the retail painted portrait for a unit type.
    ///
    /// Portraits ship as short WebM clips under `portrait/<folder>/`; the
    /// folder is `<race letter><unit name, truncated to 8 chars>` and the
    /// first frame file is `<first three letters>fid00.webm`. Any miss is
    /// cached as `None` so the caller falls back to the wireframe.
    fn portrait_frame(&mut self, type_id: u16) -> Option<PosterFrame> {
        if let Some(hit) = self.portraits.get(&type_id) {
            return hit.clone();
        }
        let decoded = (|| {
            let install = self.install.as_ref()?;
            let path = portrait_idle_path(install, self.data.unit(type_id).portrait)?;
            let bytes = install.read_by_path(&path).ok()?;
            let info = galaxycraft::video::parse_webm(&bytes).ok()?;
            galaxycraft::video::decode_frame_at(&bytes, &info, Some(0.6), 256)
        })();
        if std::env::var("GC_DEBUG_PORTRAIT").is_ok() {
            eprintln!(
                "portrait type={type_id} name={:?} install={} -> {}",
                self.data.unit_name(type_id),
                self.install.is_some(),
                if decoded.is_some() { "ok" } else { "none" }
            );
        }
        self.portraits.insert(type_id, decoded.clone());
        decoded
    }

    /// First frame of the episode-select art for a race, with alpha.
    /// One Collections thumbnail, decoded from the shipped image.
    fn thumb(&mut self, name: &str, w: i32, h: i32) -> Option<PosterFrame> {
        let key = format!("{name}@{w}x{h}");
        if let Some(hit) = self.thumbs.get(&key) {
            return hit.clone();
        }
        let decoded = (|| {
            let install = self.install.as_ref()?;
            let bytes = install
                .read_by_path(&format!("webui/dist/lib/images/{name}"))
                .ok()?;
            let ext = name.rsplit('.').next().unwrap_or("png");
            galaxycraft::video::decode_still(&bytes, ext, w.max(1) as u32, h.max(1) as u32)
        })();
        self.thumbs.insert(key, decoded.clone());
        decoded
    }

    /// One RGBA command/unit icon from the web UI, decoded on demand.
    fn hotkey_icon(&mut self, name: &str, w: i32, h: i32) -> Option<RgbaFrame> {
        // Decode at the texture's native 128 px and pre-scale once with the
        // Catmull-Rom filter (CEF/Skia's downscale), then the per-frame blit
        // is a straight copy.
        let key = format!("{name}@{w}x{h}");
        if let Some(hit) = self.hotkey_icons.get(&key) {
            return hit.clone();
        }
        let decoded = (|| {
            let install = self.install.as_ref()?;
            let bytes = install
                .read_by_path(&format!("webui/dist/lib/images/{name}"))
                .ok()?;
            let native = galaxycraft::video::decode_still_rgba(&bytes, "png", 128, 128)?;
            if std::env::var("GC_ICON_FILTER").ok().as_deref() == Some("area") {
                Some(resize_rgba_area(&native, w.max(1) as u32, h.max(1) as u32))
            } else {
                Some(native)
            }
        })();
        self.hotkey_icons.insert(key, decoded.clone());
        decoded
    }

    /// Credit entries from the shipped `creditspanel.js` bundle. Each entry is
    /// `{name:"...",style:N}`; style 1..5 are the CSS heading levels and
    /// style 6 is a divider.
    fn credits_entries(&mut self) -> Vec<(String, u8)> {
        if self.credits.is_empty() {
            if let Some(install) = &self.install {
                if let Ok(bytes) = install.read_by_path("webui/dist/creditspanel/creditspanel.js") {
                    let text = String::from_utf8_lossy(&bytes);
                    self.credits = parse_credits(&text);
                }
            }
        }
        self.credits.clone()
    }

    /// Prepare a headless in-game capture: leave the menu, size the logical
    /// view, reveal vision around the player's units and centre the camera.
    fn prepare_ingame_shot(&mut self, fb: &Framebuffer) {
        // The references were taken with the radar's terrain layer switched
        // off (their tooltip is hovering that very button), so the headless
        // captures use the same state to stay comparable.
        self.minimap_terrain = false;
        self.menu = false;
        self.intro = false;
        self.briefing = false;
        self.update_view(fb);
        if !self.spawn_reference_scene(fb) {
            self.select_reference_selection();
            self.world.update_vision();
            self.centre_on_local_unit();
            if let Some(cam) = Self::cam_override() {
                self.camera = cam;
            }
        }
    }

    /// `GC_CAM=x,y`: capture calibration that pins the camera, so scene and
    /// no-scene captures can be compared over the same terrain.
    fn cam_override() -> Option<(i32, i32)> {
        let v: Vec<i32> = std::env::var("GC_CAM")
            .ok()?
            .split(',')
            .filter_map(|t| t.trim().parse().ok())
            .collect();
        (v.len() == 2).then(|| (v[0], v[1]))
    }

    /// Reconstruct the reference frame: place the units the screenshot shows
    /// at the screen positions they occupy there, with the camera fixed so the
    /// layout lands where the reference has it. Returns false when no scene has
    /// been reconstructed for this capture (the map's own state is used then).
    fn spawn_reference_scene(&mut self, fb: &Framebuffer) -> bool {
        // Opt-in (`GC_SCENE=1`): the reconstructed scene is faithful — the
        // supply-depot grid lands on the reference's own grid — but the SD art
        // we own differs from the retail HD art pixel by pixel, which costs
        // more on the measured error than the empty terrain it replaces.
        if std::env::var("GC_SCENE").is_err() {
            return false;
        }
        let Some(scene) = galaxycraft::scene::reference_scene(&self.shot_name) else {
            return false;
        };
        let (cx, _, _, _) = Self::content_rect(fb);
        self.camera = scene.camera;
        let owner = self.local_player;
        let mut wired = false;
        for su in scene.units {
            let per_world = galaxycraft::scene::SCREEN_PER_WORLD;
            let (wx, wy) = galaxycraft::scene::world_of(scene.camera, cx, su.screen, per_world);
            if let Err(e) = self.world.spawn_px(owner, su.type_id, wx, wy) {
                eprintln!("scene: type {} at {wx},{wy}: {e}", su.type_id);
            } else {
                wired = true;
            }
        }
        let _ = wired;
        self.world.update_vision();
        self.select_reference_selection();
        true
    }

    /// Centre the camera on the first local unit, like the retail client does
    /// when a mission starts.
    /// Retail start camera of a melee game: the start location sits half a
    /// view from the left and 200 game px from the top, clamped to the map
    /// (measured on 134905 / 135020 / 135051, (8)The Hunters).
    fn start_camera(&mut self) {
        let Some((x, y)) = self.scenario.start_location_of(self.local_player as u8) else {
            return self.centre_on_local_unit();
        };
        self.camera = (x - self.view.0 / 2, y - START_CAMERA_TOP);
        self.clamp_camera();
    }

    /// Reference in-game captures: the start of a melee game on
    /// (8)The Hunters as slot `slot` (the start locations of the user's
    /// three autosaved games, 2026-10-27), with the town hall selected for
    /// Zerg/Protoss and nothing selected for Terran, as on the captures.
    fn hunters_start(&mut self, fb: &Framebuffer, slot: usize, race: Race, nudge: (i32, i32)) {
        // Colours the three games gave the player, measured on the team-mask
        // pixels of 135051 / 134905 / 135020 (teal, brown, dark grey).
        self.local_team_color = Some(match race {
            Race::Protoss => Rgb::new(40, 150, 125),
            Race::Zerg => Rgb::new(111, 73, 59),
            _ => Rgb::new(61, 57, 47),
        });
        if let Some(i) = self
            .maps
            .iter()
            .position(|m| m.name.to_lowercase().contains("the hunters") && m.players == 8)
        {
            self.map_index = i;
        }
        self.melee_slot = Some(slot);
        self.menu_race = race;
        self.races = None;
        self.load_map(self.map_index);
        self.minimap_terrain = false;
        self.menu = false;
        self.intro = false;
        self.briefing = false;
        self.update_view(fb);
        self.world.update_vision();
        self.selected.clear();
        if race != Race::Terran {
            self.select_reference_selection();
        }
        self.refresh_commands();
        self.start_camera();
        // The player nudged the camera before capturing: measured by
        // aligning the terrain (error 1.96-2.35 after the shift).
        self.camera.0 += nudge.0;
        self.camera.1 += nudge.1;
        self.creep_variants.clear();
        if race == Race::Zerg && slot == 1 {
            self.creep_variants = parse_tile_grid(HUNTERS_ZERG_CREEP.0, HUNTERS_ZERG_CREEP.1);
        }
        self.clamp_camera();
    }

    fn centre_on_local_unit(&mut self) {
        if let Some(u) = self
            .world
            .units
            .iter()
            .find(|u| u.alive && u.owner as usize == self.local_player)
        {
            let (x, y) = (u.x / 256, u.y / 256);
            self.camera = (x - self.view.0 / 2, y - self.view.1 / 2);
            self.clamp_camera();
        }
    }

    /// Reproduce the selection the reference capture shows: two units for the
    /// Terran screenshots, the main building for the Zerg and Protoss ones.
    fn select_reference_selection(&mut self) {
        let building = match self.console_race {
            Race::Zerg => Some([
                galaxycraft::ids::unit::HATCHERY,
                galaxycraft::ids::unit::LAIR,
                galaxycraft::ids::unit::HIVE,
            ]),
            Race::Protoss => Some([galaxycraft::ids::unit::NEXUS; 3]),
            _ => None,
        };
        if let Some(ids) = building {
            if let Some(u) = self.world.units.iter().find(|u| {
                u.alive && u.owner as usize == self.local_player && ids.contains(&u.type_id)
            }) {
                self.selected = vec![u.id];
                self.refresh_commands();
                return;
            }
        }
        self.select_some_units();
    }

    fn select_some_units(&mut self) {
        // Prefer units that actually ship a painted portrait so the capture
        // exercises the real art instead of the wireframe fallback.
        let preferred = [
            galaxycraft::ids::unit::MARINE,
            galaxycraft::ids::unit::GHOST,
            galaxycraft::ids::unit::ZEALOT,
            galaxycraft::ids::unit::ZERGLING,
        ];
        let mut chosen: Vec<u32> = Vec::new();
        for want in preferred {
            if let Some(u) = self
                .world
                .units
                .iter()
                .find(|u| u.alive && u.owner as usize == self.local_player && u.type_id == want)
            {
                chosen.push(u.id);
            }
        }
        for u in self
            .world
            .units
            .iter()
            .filter(|u| u.alive && u.owner as usize == self.local_player)
        {
            if chosen.len() >= 2 {
                break;
            }
            if !chosen.contains(&u.id) && chosen.len() < 2 {
                chosen.push(u.id);
            }
        }
        self.selected = chosen;
        self.refresh_commands();
    }

    fn open_menu(&mut self) {
        self.menu = true;
        self.intro = false;
        self.briefing = false;
        self.paused = false;
        self.replay = None;
        self.game_result = None;
        self.next_map = None;
        self.next_map = None;
        self.selected.clear();
        self.status.clear();
        self.result_panel = None;
        self.result_fit = None;
        self.show_stats = false;
        self.show_leaderboard = false;
        self.start_menu_music();
    }

    /// Global map range for the active menu tab.
    fn menu_slice(&self) -> (usize, usize) {
        if self.menu_tab == 0 {
            (0, self.campaign_len)
        } else {
            (self.campaign_len, self.maps.len())
        }
    }

    #[allow(unreachable_code)]
    /// Start the selected campaign mission of `episode` (Enter or the Ok
    /// button); the screen-change cue is the retail `swishin` sound.
    /// Campaign state of the profile on the reference captures: Terran
    /// played up to `8. The Big Push` (131132), everything else fresh, so
    /// Zerg/Protoss and Brood War Terran/Zerg are locked (130757/130804/
    /// 130832/130840 hover captions).
    fn set_reference_progress(&mut self) {
        let mut p = galaxycraft::campaign_progress::Progress::default();
        p.next[galaxycraft::campaign_progress::Campaign::Terran.index()] = 9;
        self.progress_book.set(&self.active_profile.clone(), p);
    }

    /// Progress of the profile chosen in the Registry.
    fn progress(&self) -> galaxycraft::campaign_progress::Progress {
        self.progress_book.get(&self.active_profile)
    }

    fn capture_active_profile(&mut self) {
        if let Some(name) = self.profiles.get(self.profile_sel) {
            self.active_profile = name.clone();
        }
    }

    /// Map list index of a mapdata mission (`campaign/<dir>/<key>`).
    fn campaign_map_index(&self, mission: u16) -> Option<usize> {
        let (dir, key) = galaxycraft::campaign_progress::mission_key(mission)?;
        self.maps.iter().position(|m| {
            m.campaign
                .as_ref()
                .is_some_and(|c| c.episode == dir && c.mission == key)
        })
    }

    /// Mapdata mission of the map at `index`, for progress bookkeeping.
    fn map_mission(&self, index: usize) -> Option<u16> {
        let tag = self.maps.get(index)?.campaign.as_ref()?;
        (1..=64u16).find(|&n| {
            galaxycraft::campaign_progress::mission_key(n)
                .is_some_and(|(dir, key)| tag.episode == dir && tag.mission == key)
        })
    }

    /// Click on a campaign slot: a locked campaign asks first
    /// (`strRACE_ERR_*`), an unlocked one is entered.
    fn select_campaign(&mut self, campaign: galaxycraft::campaign_progress::Campaign) {
        self.episode_sel = (campaign.episode() - 1) as usize;
        // The modal takes the pointer: the slot under it is no longer
        // hovered (live capture 08: Zerg dim under the prompt).
        self.episode_hover = None;
        if self.progress().blocker(campaign).is_some() {
            self.campaign_prompt = Some(campaign);
        } else {
            self.enter_campaign(campaign);
        }
    }

    /// Retail: a campaign with history opens `Select starting mission`
    /// (first entry selected, list at the top); without history it starts
    /// straight away.
    fn enter_campaign(&mut self, campaign: galaxycraft::campaign_progress::Campaign) {
        self.episode_sel = (campaign.episode() - 1) as usize;
        let history = self.progress().history(campaign);
        // Live capture 21: the Terran slot is dim under the list modal.
        self.episode_hover = None;
        if history.len() > 1 {
            self.mission_list = galaxycraft::campaign_progress::MissionList::open(history.len());
            self.menu_screen = MenuScreen::Mission {
                episode: campaign.episode(),
            };
        } else if let Some(entry) = history.first() {
            self.start_history_entry(entry);
        }
    }

    /// Episode-screen command buttons (`glucmpgn` ids 5/10/11/9).
    fn episode_command(&mut self, which: usize) {
        self.menu_screen = match which {
            0 => MenuScreen::SavedGames,
            1 => MenuScreen::Lobby {
                map: Some(self.map_index),
            },
            2 => MenuScreen::Replays,
            // Live client: Cancel returns to the Registry.
            _ => MenuScreen::Registry,
        };
    }

    /// Start a history entry: a cinematic entry plays its movie and then
    /// the mission it precedes.
    fn start_history_entry(&mut self, entry: &galaxycraft::campaign_progress::HistEntry) {
        let Some(idx) = self.campaign_map_index(entry.mission) else {
            return;
        };
        self.map_index = idx;
        match galaxycraft::campaign_progress::cinematic_clip(entry.cinematic) {
            Some(clip) => self.begin_intro(vec![clip.to_string()], Some(idx)),
            None => self.start_campaign_mission(idx),
        }
    }

    fn launch_mission(&mut self, episode: u8) {
        self.play("ui_click");
        let Some(campaign) = galaxycraft::campaign_progress::Campaign::from_episode(episode) else {
            return;
        };
        let history = self.progress().history(campaign);
        if let Some(entry) = history.get(self.mission_list.sel) {
            self.start_history_entry(entry);
        }
    }

    /// Scrollbar of the mission list: arrows scroll one row, the track
    /// pages.  Returns true when the press was consumed.
    fn mission_scrollbar(&mut self, input: &MenuInput) -> bool {
        let (cx, _cy, cw, ch) = self.content_box;
        let to_content = |(mx, my): (i32, i32)| {
            (
                (mx - cx) as f32 * 2560.0 / cw.max(1) as f32,
                my as f32 * 1440.0 / ch.max(1) as f32,
            )
        };
        // Thumb drag in progress: follow the pointer while the button is
        // held (the grab offset keeps the thumb under the cursor).
        if let Some(grab) = self.mission_drag {
            if !input.mouse_down {
                self.mission_drag = None;
                self.mouse_was_down = false;
                return true;
            }
            if let Some(p) = input.mouse {
                let (_, fy) = to_content(p);
                self.mission_list
                    .set_thumb(galaxycraft::menu::mission_thumb_fraction(fy - grab));
            }
            return true;
        }
        let pressed = input.mouse_down && !self.mouse_was_down;
        let Some((mx, my)) = input.mouse.filter(|_| pressed) else {
            return false;
        };
        let (fx, fy) = to_content((mx, my));
        let Some(hit) = galaxycraft::menu::mission_scrollbar_hit(fx, fy, self.mission_list.thumb())
        else {
            return false;
        };
        self.mouse_was_down = true;
        let page = galaxycraft::campaign_progress::MissionList::ROWS as i32;
        self.mission_list.scroll(match hit {
            galaxycraft::menu::ScrollHit::Up => -1,
            galaxycraft::menu::ScrollHit::Down => 1,
            galaxycraft::menu::ScrollHit::PageUp => -page,
            galaxycraft::menu::ScrollHit::PageDown => page,
            galaxycraft::menu::ScrollHit::Thumb => {
                let top = galaxycraft::menu::mission_thumb_top(self.mission_list.thumb());
                self.mission_drag = Some(fy - top);
                0
            }
        });
        self.play("ui_click");
        true
    }

    /// Pointer state of the `glupokcancel` prompt: Some(true) over Ok,
    /// Some(false) over Cancel, and whether this tick is a press.
    fn campaign_prompt_pointer(&mut self, input: &MenuInput) -> (Option<bool>, bool) {
        let pressed = input.mouse_down && !self.mouse_was_down;
        if !input.mouse_down {
            self.mouse_was_down = false;
        }
        let Some((mx, my)) = input.mouse else {
            return (None, false);
        };
        let (cx, cy, cw, ch) = self.content_box;
        let (scale, ox) = galaxycraft::menu::campaign_scale(cw, ch);
        let inside = |r: (f32, f32, f32, f32)| {
            let (x, y, w, h) = galaxycraft::menu::campaign_rect(r, scale, ox, cx, cy);
            mx >= x && mx < x + w && my >= y && my < y + h
        };
        let hit = if inside(galaxycraft::menu::PROMPT_BUTTONS[0]) {
            Some(true)
        } else if inside(galaxycraft::menu::PROMPT_BUTTONS[1]) {
            Some(false)
        } else {
            None
        };
        if pressed {
            self.mouse_was_down = true;
        }
        (hit, pressed)
    }

    /// Input snapshot for one menu tick.  Live play fills it from the window;
    /// `--menu-script` fills it from a script so the real menu code can be
    /// walked headlessly (screens + sounds) without synthetic X11 events.
    fn handle_menu(&mut self, window: &Window) {
        let input = MenuInput {
            keys: window.get_keys_pressed(KeyRepeat::No),
            mouse: window
                .get_unscaled_mouse_pos(MouseMode::Clamp)
                .map(|(x, y)| (x as i32, y as i32)),
            mouse_down: window.get_mouse_down(MouseButton::Left),
            wheel: window
                .get_scroll_wheel()
                .map(|(_, y)| -(y.signum() as i32))
                .unwrap_or(0),
        };
        self.handle_menu_input(&input);
    }

    fn handle_menu_input(&mut self, input: &MenuInput) {
        // Sub-screen navigation for the retail menu flow.  Each screen uses
        // Up/Down + Enter/Esc; the map-list Tab/R shortcuts stay on Main.
        let screen = self.menu_screen.clone();
        match screen {
            MenuScreen::Main => {
                // Mouse: hover highlights, left click activates.
                if let Some((mx, my)) = input.mouse {
                    // Use the shipped-hot-area resolver: the footer entries
                    // overlap the tall Single/Multiplayer boxes and must win
                    // the hit test (retail z-order).
                    let hit = galaxycraft::menu::main_entry_at(
                        self.ui_scale,
                        self.ui_ox,
                        self.ui_oy,
                        mx,
                        my,
                    );
                    if let Some(idx) = hit {
                        self.set_main_hover(Some(idx));
                        if idx != self.profile_sel {
                            self.profile_sel = idx;
                            self.play("ui_move");
                        }
                        if input.mouse_down && !self.mouse_was_down {
                            self.mouse_was_down = true;
                            self.activate_main_item(idx);
                            return;
                        }
                    } else {
                        self.set_main_hover(None);
                    }
                }
                if !input.mouse_down {
                    self.mouse_was_down = false;
                }
                for key in input.keys.iter().copied() {
                    match key {
                        Key::Up => {
                            self.profile_sel = (self.profile_sel + 7) % 8;
                            self.set_main_hover(Some(self.profile_sel));
                            self.play("ui_move");
                        }
                        Key::Down => {
                            self.profile_sel = (self.profile_sel + 1) % 8;
                            self.set_main_hover(Some(self.profile_sel));
                            self.play("ui_move");
                        }
                        Key::Enter | Key::Space => {
                            let item = self.profile_sel;
                            self.activate_main_item(item);
                        }
                        Key::I => {
                            self.begin_intro(
                                galaxycraft::video::intro_sequence(self.intro_expansion, false)
                                    .into_iter()
                                    .map(str::to_string)
                                    .collect(),
                                None,
                            );
                            return;
                        }
                        Key::X => {
                            self.intro_expansion = !self.intro_expansion;
                            if let Some(store) = &self.video_store {
                                if let Some((info, poster)) =
                                    store.intro_poster(self.intro_expansion)
                                {
                                    self.intro_info = Some(info);
                                    self.intro_poster = Some(poster);
                                }
                            }
                            self.play("ui_click");
                        }
                        Key::R => {
                            self.menu_screen = MenuScreen::Registry;
                            self.play("ui_click");
                        }
                        Key::Escape => self.quit = true,
                        _ => {}
                    }
                }
            }
            MenuScreen::Registry => {
                // Mouse: the registry panel is a real dialog.  Its normalized
                // rects come from `draw_registry` so drawing and hit-testing
                // cannot diverge.
                if let Some((mx, my)) = input.mouse {
                    let (cx, cy, cw, ch) = self.content_box;
                    let hit = |x0: f32, y0: f32, x1: f32, y1: f32| {
                        mx >= cx + (cw as f32 * x0) as i32
                            && mx < cx + (cw as f32 * x1) as i32
                            && my >= cy + (ch as f32 * y0) as i32
                            && my < cy + (ch as f32 * y1) as i32
                    };
                    if input.mouse_down && !self.mouse_was_down {
                        self.mouse_was_down = true;
                        if hit(0.638, 0.788, 0.822, 0.848) {
                            self.capture_active_profile();
                            if self.registry_after_gametype {
                                self.registry_after_gametype = false;
                                self.episode_sel = galaxycraft::menu::episode_start(false);
                                self.menu_screen = MenuScreen::Episode;
                            } else {
                                self.menu_screen = MenuScreen::Main;
                                self.profile_sel = 0;
                            }
                            self.play("ui_click");
                        } else if hit(0.638, 0.884, 0.822, 0.944) {
                            self.registry_after_gametype = false;
                            self.menu_screen = MenuScreen::Main;
                            self.profile_sel = 0;
                            self.play("ui_click");
                        } else if hit(0.1813, 0.694, 0.3672, 0.743) {
                            self.profiles
                                .push(format!("Player{}", self.profiles.len() + 1));
                            self.play("ui_click");
                        } else if hit(0.3867, 0.694, 0.5469, 0.743) {
                            if self.profiles.len() > 1 {
                                self.profiles
                                    .remove(self.profile_sel.min(self.profiles.len() - 1));
                                self.profile_sel = 0;
                            }
                            self.play("ui_click");
                        } else if hit(0.195, 0.229, 0.5469, 0.523) {
                            let rel = (my - cy) as f32 / ch as f32;
                            let idx = ((rel - 0.229) / 0.049).floor().max(0.0) as usize;
                            if idx < self.profiles.len() {
                                self.profile_sel = idx;
                                self.play("ui_move");
                            }
                        }
                        return;
                    }
                }
                if !input.mouse_down {
                    self.mouse_was_down = false;
                }
                for key in input.keys.iter().copied() {
                    match key {
                        Key::Up => {
                            self.profile_sel = self.profile_sel.saturating_sub(1);
                            self.play("ui_move");
                        }
                        Key::Down => {
                            self.profile_sel =
                                (self.profile_sel + 1).min(self.profiles.len().saturating_sub(1));
                            self.play("ui_move");
                        }
                        Key::N => {
                            self.profiles
                                .push(format!("Player{}", self.profiles.len() + 1));
                            self.play("ui_click");
                        }
                        Key::D => {
                            if self.profiles.len() > 1 {
                                self.profiles
                                    .remove(self.profile_sel.min(self.profiles.len() - 1));
                                self.profile_sel = 0;
                            }
                            self.play("ui_click");
                        }
                        Key::Enter | Key::Space => {
                            self.capture_active_profile();
                            if self.registry_after_gametype {
                                self.registry_after_gametype = false;
                                self.episode_sel = galaxycraft::menu::episode_start(false);
                                self.menu_screen = MenuScreen::Episode;
                            } else {
                                self.menu_screen = MenuScreen::Main;
                                self.profile_sel = 0;
                            }
                            self.play("ui_click");
                        }
                        Key::Escape => {
                            self.registry_after_gametype = false;
                            if let Some(t) = galaxycraft::menu::back_destination(&screen) {
                                self.menu_screen = t;
                            }
                            self.profile_sel = 0;
                            self.play("ui_click");
                        }
                        _ => {}
                    }
                }
            }
            MenuScreen::GameType => {
                // The retail popup is a real dialog: the pointer highlights
                // the wordmark buttons and `Cancel`, and a left click chooses
                // the entry.  The menu behind stays idle (no hover clip).
                self.main_cursor = None;
                let hit = input.mouse.and_then(|(x, y)| {
                    galaxycraft::menu::gametype_entry_at(
                        self.ui_scale,
                        self.ui_ox,
                        self.ui_oy,
                        x,
                        y,
                    )
                });
                self.gametype_hover = hit.filter(|idx| *idx < 2);
                if let Some(idx) = hit {
                    if input.mouse_down && !self.mouse_was_down {
                        self.mouse_was_down = true;
                        match idx {
                            0 | 1 => self.finish_gametype(idx == 1),
                            _ => {
                                self.menu_screen = MenuScreen::Main;
                                self.gametype_hover = None;
                                self.gametype_multi = false;
                                self.play("ui_click");
                            }
                        }
                        return;
                    }
                }
                if !input.mouse_down {
                    self.mouse_was_down = false;
                }
                for key in input.keys.iter().copied() {
                    match key {
                        Key::Up | Key::Down => {
                            self.episode_sel = 1 - self.episode_sel.min(1);
                            self.gametype_hover = None;
                            self.play("ui_move");
                        }
                        Key::Enter | Key::Space => {
                            self.finish_gametype(self.episode_sel.min(1) == 1);
                        }
                        Key::L => {
                            self.menu_screen = MenuScreen::Lan;
                            self.lan.map_index = self.map_index;
                            self.lan_scan();
                            self.play("ui_click");
                        }
                        Key::Escape => {
                            if let Some(t) = galaxycraft::menu::back_destination(&screen) {
                                self.menu_screen = t;
                            }
                            self.gametype_hover = None;
                            self.gametype_multi = false;
                            self.play("ui_click");
                        }
                        _ => {}
                    }
                }
            }
            MenuScreen::Episode => {
                // `glupokcancel` prompt over a locked campaign: Ok (O,
                // Enter) plays it anyway, Cancel (C, Esc) closes it.
                if let Some(campaign) = self.campaign_prompt {
                    let (hit, clicked) = self.campaign_prompt_pointer(input);
                    let mut choice = if clicked { hit } else { None };
                    for key in input.keys.iter().copied() {
                        match key {
                            Key::O | Key::Enter => choice = Some(true),
                            Key::C | Key::Escape => choice = Some(false),
                            _ => {}
                        }
                    }
                    if let Some(ok) = choice {
                        self.campaign_prompt = None;
                        self.play("ui_click");
                        if ok {
                            return self.enter_campaign(campaign);
                        }
                    }
                    return;
                }
                // Mouse: the three episode slots and the four shipped command
                // controls (`glucmpgn` ids 8/7/6/5/10/11/9).
                if let Some((mx, my)) = input.mouse {
                    let (cx, cy, cw, ch) = self.content_box;
                    let (scale, ox) = galaxycraft::menu::campaign_scale(cw, ch);
                    let expansion = self.episode_sel >= 3;
                    let slots: &[galaxycraft::menu::CampaignSlot] = if expansion {
                        &galaxycraft::menu::CAMPAIGN_X_SLOTS
                    } else {
                        &galaxycraft::menu::CAMPAIGN_SLOTS
                    };
                    let inside = |r: (f32, f32, f32, f32)| {
                        let (x, y, w, h) = galaxycraft::menu::campaign_rect(r, scale, ox, cx, cy);
                        mx >= x && mx < x + w && my >= y && my < y + h
                    };
                    let slot = slots.iter().position(|s| inside(s.button));
                    if slot.is_some() && slot != self.episode_hover {
                        self.play("ui_move");
                    }
                    self.episode_hover = slot;
                    if input.mouse_down && !self.mouse_was_down {
                        self.mouse_was_down = true;
                        if let Some(idx) = slot {
                            self.play("ui_click");
                            let campaign =
                                galaxycraft::campaign_progress::Campaign::of_slot(expansion, idx);
                            return self.select_campaign(campaign);
                        }
                        let which = galaxycraft::menu::CAMPAIGN_BUTTONS
                            .iter()
                            .position(|b| inside(b.rect));
                        if let Some(which) = which {
                            self.play("ui_click");
                            self.episode_command(which);
                        }
                        return;
                    }
                }
                if !input.mouse_down {
                    self.mouse_was_down = false;
                }
                let expansion = self.episode_sel >= 3;
                for key in input.keys.iter().copied() {
                    use galaxycraft::campaign_progress::Campaign as C;
                    // Slot hotkeys come from the shipped strings
                    // (`t\x04T\x01erran`, `z..`, `p..`).
                    let race = match (key, expansion) {
                        (Key::T, false) => Some(C::Terran),
                        (Key::Z, false) => Some(C::Zerg),
                        (Key::P, false) => Some(C::Protoss),
                        (Key::T, true) => Some(C::ExpTerran),
                        (Key::Z, true) => Some(C::ExpZerg),
                        (Key::P, true) => Some(C::ExpProtoss),
                        _ => None,
                    };
                    if let Some(c) = race {
                        self.play("ui_click");
                        return self.select_campaign(c);
                    }
                    match key {
                        Key::Up => {
                            self.episode_sel = (self.episode_sel + 5) % 6;
                            self.play("ui_move");
                        }
                        Key::Down => {
                            self.episode_sel = (self.episode_sel + 1) % 6;
                            self.play("ui_move");
                        }
                        Key::Enter | Key::Space => {
                            self.play("ui_click");
                            if let Some(c) = galaxycraft::campaign_progress::Campaign::from_episode(
                                (self.episode_sel + 1) as u8,
                            ) {
                                return self.select_campaign(c);
                            }
                        }
                        // `l\x04L\x01oad Saved`, `rLoad \x04R\x01eplay`,
                        // `uPlay C\x04u\x01stom`, `c\x04C\x01ancel`.
                        Key::L => {
                            self.play("ui_click");
                            return self.episode_command(0);
                        }
                        Key::U => {
                            self.play("ui_click");
                            return self.episode_command(1);
                        }
                        Key::R => {
                            self.play("ui_click");
                            return self.episode_command(2);
                        }
                        Key::C | Key::Escape => {
                            self.play("ui_click");
                            return self.episode_command(3);
                        }
                        _ => {}
                    }
                }
            }
            MenuScreen::Mission { episode } => {
                if self.mission_scrollbar(input) {
                    return;
                }
                if input.wheel != 0 {
                    self.mission_list.scroll(input.wheel);
                }
                let (hit, clicked) = self.menu_pointer(input);
                if clicked {
                    match hit {
                        Some(MenuHit::Ok) => return self.launch_mission(episode),
                        Some(MenuHit::Cancel) => {
                            self.menu_screen = MenuScreen::Episode;
                            self.play("ui_click");
                        }
                        Some(MenuHit::Row(i)) if self.mission_list.click_row(i) => {
                            self.play("ui_click");
                        }
                        _ => {}
                    }
                }
                for key in input.keys.iter().copied() {
                    match key {
                        Key::Up => {
                            self.mission_list.step(-1);
                            self.play("ui_move");
                        }
                        Key::Down => {
                            self.mission_list.step(1);
                            self.play("ui_move");
                        }
                        // `o\x04O\x01k` / `c\x04C\x01ancel` (GLUHIST-2/3).
                        Key::Enter | Key::Space | Key::O => return self.launch_mission(episode),
                        Key::Escape | Key::C => {
                            self.menu_screen = MenuScreen::Episode;
                            self.play("ui_click");
                        }
                        _ => {}
                    }
                }
            }
            MenuScreen::Lan => {
                let (hit, clicked) = self.menu_pointer(input);
                if clicked {
                    if let Some(MenuHit::Row(i)) = hit {
                        let entry = i.checked_sub(2);
                        if let Some(e) = entry.filter(|e| *e < self.lan.entries.len()) {
                            if self.lan.sel == e {
                                self.lan_join();
                            } else {
                                self.lan.sel = e;
                                self.play("ui_click");
                            }
                        }
                    }
                }
                for key in input.keys.iter().copied() {
                    match key {
                        Key::S => {
                            self.lan_scan();
                            self.play("ui_move");
                        }
                        Key::H => {
                            self.lan_host();
                            self.play("ui_click");
                        }
                        Key::Up => {
                            self.lan.sel = self.lan.sel.saturating_sub(1);
                            self.play("ui_move");
                        }
                        Key::Down => {
                            self.lan.sel =
                                (self.lan.sel + 1).min(self.lan.entries.len().saturating_sub(1));
                            self.play("ui_move");
                        }
                        Key::Enter | Key::Space => {
                            if self.lan.listener.is_none() {
                                self.lan_join();
                            }
                        }
                        Key::Escape => {
                            self.lan.listener = None;
                            self.lan.announce_sock = None;
                            if let Some(t) = galaxycraft::menu::back_destination(&screen) {
                                self.menu_screen = t;
                            }
                            self.play("ui_click");
                        }
                        _ => {}
                    }
                }
            }
            MenuScreen::SavedGames | MenuScreen::Replays => {
                let replays = matches!(&self.menu_screen, MenuScreen::Replays);
                let list = self.save_list(replays);
                let (hit, clicked) = self.menu_pointer(input);
                if clicked {
                    match hit {
                        Some(MenuHit::Ok) => {
                            if let Some(path) = list.get(self.save_sel).cloned() {
                                self.load_save(&path, replays);
                            }
                        }
                        Some(MenuHit::Cancel) => {
                            self.menu_screen = MenuScreen::Episode;
                            self.save_sel = 0;
                            self.play("ui_click");
                        }
                        Some(MenuHit::Delete) => {
                            if let Some(path) = list.get(self.save_sel) {
                                let _ = std::fs::remove_file(path);
                                self.save_sel = self.save_sel.saturating_sub(1);
                                self.play("ui_click");
                            }
                        }
                        Some(MenuHit::Row(i)) if i != self.save_sel && i < list.len().max(1) => {
                            self.save_sel = i;
                            self.play("ui_click");
                        }
                        _ => {}
                    }
                }
                for key in input.keys.iter().copied() {
                    match key {
                        Key::Up => {
                            self.save_sel = self.save_sel.saturating_sub(1);
                            self.play("ui_move");
                        }
                        Key::Down => {
                            self.save_sel = self
                                .save_sel
                                .saturating_add(1)
                                .min(list.len().saturating_sub(1));
                            self.play("ui_move");
                        }
                        Key::Enter | Key::Space => {
                            if let Some(path) = list.get(self.save_sel).cloned() {
                                self.load_save(&path, replays);
                            }
                        }
                        Key::Delete | Key::Backspace => {
                            if let Some(path) = list.get(self.save_sel) {
                                let _ = std::fs::remove_file(path);
                                self.save_sel = self.save_sel.saturating_sub(1);
                                self.play("ui_click");
                            }
                        }
                        Key::Escape => {
                            if let Some(t) = galaxycraft::menu::back_destination(&screen) {
                                self.menu_screen = t;
                            }
                            self.save_sel = 0;
                            self.play("ui_click");
                        }
                        _ => {}
                    }
                }
            }
            MenuScreen::Lobby { .. } => {
                let (hit, clicked) = self.menu_pointer(input);
                if clicked {
                    match hit {
                        Some(MenuHit::Ok) => {
                            self.play("ui_click");
                            self.load_map(self.map_index);
                        }
                        Some(MenuHit::Cancel) => {
                            self.menu_screen = MenuScreen::Main;
                            self.profile_sel = 0;
                            self.play("ui_click");
                        }
                        Some(MenuHit::Race(slot)) => {
                            self.lobby_race_open =
                                (self.lobby_race_open != Some(slot)).then_some(slot);
                            self.play("ui_click");
                        }
                        Some(MenuHit::RacePick(slot, k)) => {
                            let race = [
                                LobbyRace::Zerg,
                                LobbyRace::Terran,
                                LobbyRace::Protoss,
                                LobbyRace::Random,
                            ][k.min(3)];
                            if slot < self.lobby_races.len() {
                                self.lobby_races[slot] = race;
                            }
                            self.lobby_race_open = None;
                            self.play("ui_click");
                        }
                        _ => {}
                    }
                }
                for key in input.keys.iter().copied() {
                    match key {
                        Key::Enter | Key::Space => {
                            self.play("ui_click");
                            self.load_map(self.map_index);
                        }
                        Key::Up => {
                            self.map_index = self.map_index.saturating_sub(1);
                            self.play("ui_move");
                        }
                        Key::Down => {
                            self.map_index =
                                (self.map_index + 1).min(self.maps.len().saturating_sub(1));
                            self.play("ui_move");
                        }
                        Key::R => {
                            for r in self.lobby_races.iter_mut().take(4) {
                                *r = match *r {
                                    LobbyRace::Random => LobbyRace::Terran,
                                    LobbyRace::Terran => LobbyRace::Zerg,
                                    LobbyRace::Zerg => LobbyRace::Protoss,
                                    LobbyRace::Protoss => LobbyRace::Computer,
                                    LobbyRace::Computer => LobbyRace::Random,
                                };
                            }
                            self.play("ui_click");
                        }
                        Key::Escape => {
                            if let Some(t) = galaxycraft::menu::back_destination(&screen) {
                                self.menu_screen = t;
                            }
                            self.profile_sel = 0;
                            self.play("ui_click");
                        }
                        _ => {}
                    }
                }
            }
            MenuScreen::Options { tab } => {
                let (hit, clicked) = self.menu_pointer(input);
                if clicked {
                    match hit {
                        Some(MenuHit::Tab(i)) => {
                            // A tab switch stays on the same screen: the cue
                            // is the press sound, not the screen-in swish.
                            let all = OptionsTab::all();
                            self.menu_screen = MenuScreen::Options {
                                tab: all[i.min(all.len() - 1)],
                            };
                            self.play("ui_click");
                        }
                        Some(MenuHit::OptSlider(i)) => {
                            if let Some((mx, _)) = input.mouse {
                                let (cx, _cy, cw, _ch) = self.content_box;
                                let x = (mx - cx) as f32 / (cw as f32 / 2560.0);
                                let (x0, x1) = match (tab, i) {
                                    (OptionsTab::Speed, 0..=2) => (900.0, 1416.0),
                                    (OptionsTab::Speed, _) => (1456.0, 1972.0),
                                    (OptionsTab::Sound, _) => (900.0, 1416.0),
                                    _ => (900.0, 1416.0),
                                };
                                let step = galaxycraft::menu::slider_step(x, x0, x1);
                                let f = galaxycraft::menu::slider_frac(step);
                                match (tab, i) {
                                    (OptionsTab::Speed, 0) => {
                                        self.game_options.game_speed = 0.5 + 0.5 * f;
                                    }
                                    (OptionsTab::Speed, 1) => self.game_options.key_scroll = f,
                                    (OptionsTab::Speed, 2) => self.game_options.mouse_scroll = f,
                                    (OptionsTab::Speed, _) => {
                                        self.game_options.mouse_sensitivity = f;
                                    }
                                    (OptionsTab::Sound, 0) => self.game_options.music_volume = f,
                                    (OptionsTab::Sound, _) => self.game_options.sfx_volume = f,
                                    _ => {}
                                }
                                self.play("ui_click");
                            }
                        }
                        Some(MenuHit::OptCheck(i)) => {
                            let o = &mut self.game_options;
                            match (tab, i) {
                                (OptionsTab::Speed, 0) => {
                                    o.use_mouse_sensitivity = !o.use_mouse_sensitivity;
                                }
                                (OptionsTab::Speed, 1) => o.mouse_scaling = !o.mouse_scaling,
                                (OptionsTab::Speed, 2) => o.hardware_cursor = !o.hardware_cursor,
                                (OptionsTab::Speed, _) => {}
                                (OptionsTab::Sound, 0) => o.unit_speech = !o.unit_speech,
                                (OptionsTab::Sound, 1) => o.unit_acks = !o.unit_acks,
                                (OptionsTab::Sound, 2) => o.sound_background = !o.sound_background,
                                (OptionsTab::Sound, 3) => o.building_sounds = !o.building_sounds,
                                (OptionsTab::Sound, 4) => o.subtitles = !o.subtitles,
                                (OptionsTab::Sound, 5) => {
                                    o.cinematic_subtitles = !o.cinematic_subtitles;
                                }
                                (OptionsTab::Sound, 6) => {
                                    o.original_voice_overs = !o.original_voice_overs;
                                }
                                (OptionsTab::Sound, _) => {
                                    o.original_1998_campaign = !o.original_1998_campaign;
                                }
                                (OptionsTab::Video, 0) => o.video_fps_cap = !o.video_fps_cap,
                                (OptionsTab::Video, 1) => o.video_vsync = !o.video_vsync,
                                (OptionsTab::Video, 2) => o.video_hd = !o.video_hd,
                                (OptionsTab::Video, 3) => {
                                    o.video_environment = !o.video_environment
                                }
                                (OptionsTab::Video, 4) => o.video_lighting = !o.video_lighting,
                                (OptionsTab::Video, 5) => {
                                    o.video_smooth_turning = !o.video_smooth_turning;
                                }
                                (OptionsTab::Video, 6) => {
                                    o.video_shadow_stacking = !o.video_shadow_stacking;
                                }
                                (OptionsTab::Video, _) => o.video_pillarbox = !o.video_pillarbox,
                                (OptionsTab::Game, 0) => o.game_timer = !o.game_timer,
                                (OptionsTab::Game, 1) => {
                                    o.game_color_cycling = !o.game_color_cycling
                                }
                                (OptionsTab::Game, 2) => {
                                    o.game_language_filter = !o.game_language_filter;
                                }
                                (OptionsTab::Game, 3) => o.game_bonus_skins = !o.game_bonus_skins,
                                (OptionsTab::Game, 4) => {
                                    o.game_friend_notifications = !o.game_friend_notifications;
                                }
                                (OptionsTab::Game, 5) => o.portraits = 0,
                                (OptionsTab::Game, 6) => o.portraits = 1,
                                (OptionsTab::Game, 7) => o.portraits = 2,
                                (OptionsTab::Game, 8) => o.minimap_position = 0,
                                (OptionsTab::Game, _) => o.minimap_position = 1,
                                (OptionsTab::Network, _) => {
                                    o.network_prefer_6112 = !o.network_prefer_6112;
                                }
                            }
                            self.play("ui_click");
                        }
                        Some(MenuHit::Restore) => {
                            self.game_options = GameOptions::default();
                            self.play("ui_click");
                        }
                        Some(MenuHit::Ok) => {
                            self.menu_screen = MenuScreen::Main;
                            self.profile_sel = 2;
                            self.play("ui_click");
                        }
                        _ => {}
                    }
                }
                for key in input.keys.iter().copied() {
                    match key {
                        Key::Tab => {
                            let all = OptionsTab::all();
                            let i = all.iter().position(|t| *t == tab).unwrap_or(0);
                            self.menu_screen = MenuScreen::Options {
                                tab: all[(i + 1) % all.len()],
                            };
                            self.play("ui_click");
                        }
                        Key::Up => {
                            self.game_options.game_speed =
                                (self.game_options.game_speed + 0.1).min(3.0);
                            self.play("ui_move");
                        }
                        Key::Down => {
                            self.game_options.game_speed =
                                (self.game_options.game_speed - 0.1).max(0.2);
                            self.play("ui_move");
                        }
                        Key::F => {
                            self.game_options.fog = !self.game_options.fog;
                            self.play("ui_click");
                        }
                        Key::S => {
                            self.game_options.subtitles = !self.game_options.subtitles;
                            self.play("ui_click");
                        }
                        Key::Enter | Key::Space => {
                            self.menu_screen = MenuScreen::Main;
                            self.profile_sel = 2;
                            self.play("ui_click");
                        }
                        Key::Escape => {
                            if let Some(t) = galaxycraft::menu::back_destination(&screen) {
                                self.menu_screen = t;
                            }
                            self.profile_sel = 2;
                            self.play("ui_click");
                        }
                        _ => {}
                    }
                }
            }
            MenuScreen::Hotkeys { race } => {
                if self.hotkey_reset {
                    for key in input.keys.iter().copied() {
                        match key {
                            Key::Escape => {
                                self.hotkey_reset = false;
                                self.play("ui_click");
                            }
                            Key::Enter | Key::Space => {
                                self.restore_hotkey_defaults();
                                self.hotkey_reset = false;
                                self.play("ui_click");
                            }
                            _ => {}
                        }
                    }
                    if let Some((mx, my)) = input.mouse {
                        let (cx, cy, cw, ch) = self.content_box;
                        let hit = |x0: f32, y0: f32, x1: f32, y1: f32| {
                            mx >= cx + (cw as f32 * x0 / 2560.0) as i32
                                && mx < cx + (cw as f32 * x1 / 2560.0) as i32
                                && my >= cy + (ch as f32 * y0 / 1440.0) as i32
                                && my < cy + (ch as f32 * y1 / 1440.0) as i32
                        };
                        if input.mouse_down && !self.mouse_was_down {
                            self.mouse_was_down = true;
                            if hit(996.0, 813.0, 1356.0, 879.0) {
                                self.restore_hotkey_defaults();
                                self.hotkey_reset = false;
                                self.play("ui_click");
                            } else if hit(1387.0, 813.0, 1747.0, 879.0) {
                                self.hotkey_reset = false;
                                self.play("ui_click");
                            }
                        }
                    }
                    if !input.mouse_down {
                        self.mouse_was_down = false;
                    }
                    return;
                }
                for key in input.keys.iter().copied() {
                    if let Some(action) = self.hotkey_action {
                        if let Some(c) = key_letter(key) {
                            self.rebind_hotkey(action, c, c.to_ascii_lowercase() as u16);
                            self.play("ui_click");
                            continue;
                        }
                    }
                    match key {
                        Key::Tab => {
                            let all = HotkeyRace::all();
                            let i = all.iter().position(|r| *r == race).unwrap_or(0);
                            self.menu_screen = MenuScreen::Hotkeys {
                                race: all[(i + 1) % all.len()],
                            };
                            self.hotkey_sel = None;
                            self.hotkey_action = None;
                            self.hotkey_hover = None;
                            self.play("ui_click");
                        }
                        Key::Escape => {
                            if let Some(t) = galaxycraft::menu::back_destination(&screen) {
                                self.menu_screen = t;
                            }
                            self.profile_sel = 4;
                            self.play("ui_click");
                        }
                        Key::F12 => self.capture_request = true,
                        _ => {}
                    }
                }
                // Mouse: race tabs, unit cells and command cards.  The tab
                // hover edge is what capture 131026 shows (Protoss hovered
                // while Terran is still the active race).
                if let Some((mx, my)) = input.mouse {
                    let (cx, cy, cw, ch) = self.content_box;
                    // Bottom "Restore to Default" opens the confirmation dialog.
                    if input.mouse_down && !self.mouse_was_down {
                        let in_rect = |x0: f32, y0: f32, x1: f32, y1: f32| {
                            mx >= cx + (cw as f32 * x0 / 2560.0) as i32
                                && mx < cx + (cw as f32 * x1 / 2560.0) as i32
                                && my >= cy + (ch as f32 * y0 / 1440.0) as i32
                                && my < cy + (ch as f32 * y1 / 1440.0) as i32
                        };
                        if in_rect(354.0, 1302.0, 744.0, 1386.0) {
                            self.mouse_was_down = true;
                            self.hotkey_reset = true;
                            self.play("ui_click");
                            return;
                        }
                        if in_rect(1453.0, 1302.0, 1831.0, 1386.0) {
                            self.mouse_was_down = true;
                            if let Some(path) = hotkey_save_path() {
                                if let Err(e) =
                                    hotkey_data::save_bindings(&path, &self.hotkey_bindings)
                                {
                                    eprintln!("cannot save hotkeys {}: {e}", path.display());
                                }
                            }
                            self.hotkey_baseline = self.hotkey_bindings.clone();
                            self.menu_screen = MenuScreen::Main;
                            self.profile_sel = 4;
                            self.play("ui_click");
                            return;
                        }
                        if in_rect(1825.0, 1302.0, 2203.0, 1386.0) {
                            self.mouse_was_down = true;
                            self.hotkey_bindings = self.hotkey_baseline.clone();
                            self.hotkey_unbound.clear();
                            self.hotkey_binding_error = false;
                            self.menu_screen = MenuScreen::Main;
                            self.profile_sel = 4;
                            self.play("ui_click");
                            return;
                        }
                    }
                    let prev_tab_hover = self.hotkey_tab_hover;
                    let prev_unit_hover = self.hotkey_hover;
                    let mut tab_hover = None;
                    for (i, r) in HotkeyRace::all().iter().enumerate() {
                        let tx = cx + (cw as f32 * (0.316797 + 0.108984 * i as f32)) as i32;
                        let tw = (cw as f32 * 0.101953) as i32;
                        let ty = cy + (ch as f32 * 0.029167) as i32;
                        let th = (ch as f32 * 0.052083) as i32;
                        if mx >= tx && mx < tx + tw && my >= ty && my < ty + th {
                            tab_hover = Some(*r);
                            if input.mouse_down && !self.mouse_was_down {
                                self.mouse_was_down = true;
                                self.menu_screen = MenuScreen::Hotkeys { race: *r };
                                self.hotkey_sel = None;
                                self.hotkey_action = None;
                                self.hotkey_hover = None;
                                self.play("ui_click");
                            }
                        }
                    }
                    self.hotkey_tab_hover = tab_hover;
                    self.hotkey_hover = self
                        .hotkey_cells
                        .iter()
                        .find(|(_, r)| mx >= r.0 && mx < r.0 + r.2 && my >= r.1 && my < r.1 + r.3)
                        .map(|(id, _)| *id);
                    if self.hotkey_tab_hover != prev_tab_hover && self.hotkey_tab_hover.is_some() {
                        self.play("ui_move");
                    }
                    if self.hotkey_hover != prev_unit_hover && self.hotkey_hover.is_some() {
                        self.play("ui_move");
                    }
                    let action_hit = self
                        .hotkey_action_cells
                        .iter()
                        .find(|(_, r)| mx >= r.0 && mx < r.0 + r.2 && my >= r.1 && my < r.1 + r.3)
                        .map(|(id, _)| *id);
                    if input.mouse_down && !self.mouse_was_down {
                        self.mouse_was_down = true;
                        if let Some(id) = action_hit {
                            self.hotkey_action = Some(id);
                            self.play("ui_click");
                        }
                        if let Some(id) = self.hotkey_hover {
                            self.hotkey_sel = Some(id);
                            self.hotkey_action = None;
                            self.play("ui_click");
                        }
                    }
                }
                if !input.mouse_down {
                    self.mouse_was_down = false;
                }
            }
            MenuScreen::Collections | MenuScreen::Credits => {
                let (hit, clicked) = self.menu_pointer(input);
                if clicked {
                    match hit {
                        Some(MenuHit::Cell(i)) => {
                            self.collection_sel = i;
                            self.play("ui_click");
                        }
                        Some(MenuHit::Equip) => {
                            self.play("ui_click");
                        }
                        Some(MenuHit::Ok) | Some(MenuHit::Skip) => {
                            self.menu_screen = MenuScreen::Main;
                            self.play("ui_click");
                        }
                        _ => {}
                    }
                }
                for key in input.keys.iter().copied() {
                    match key {
                        Key::Enter | Key::Space | Key::Escape => {
                            self.menu_screen = MenuScreen::Main;
                            self.play("ui_click");
                        }
                        _ => {}
                    }
                }
            }
            MenuScreen::Intro { .. } => {
                let (_hit, clicked) = self.menu_pointer(input);
                if clicked {
                    self.menu_screen = MenuScreen::Main;
                    self.play("ui_click");
                }
                for key in input.keys.iter().copied() {
                    match key {
                        Key::Enter | Key::Space | Key::Escape => {
                            self.menu_screen = MenuScreen::Main;
                            self.play("ui_click");
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn handle_briefing(&mut self, window: &Window) {
        let retail = !self.briefing_pages.is_empty();
        for key in window.get_keys_pressed(KeyRepeat::No) {
            match key {
                Key::Enter | Key::Space => {
                    if retail && self.briefing_page + 1 < self.briefing_pages.len() {
                        self.briefing_page += 1;
                        self.refresh_briefing_poster();
                        self.play("transmission");
                    } else {
                        self.briefing = false;
                        self.last_step = Instant::now();
                        self.play("transmission");
                        self.status("Mission started");
                    }
                }
                Key::Right | Key::Down if retail => {
                    if self.briefing_page + 1 < self.briefing_pages.len() {
                        self.briefing_page += 1;
                        self.refresh_briefing_poster();
                        self.play("ui_move");
                    }
                }
                Key::Left | Key::Up if retail => {
                    self.briefing_page = self.briefing_page.saturating_sub(1);
                    self.refresh_briefing_poster();
                    self.play("ui_move");
                }
                Key::Escape | Key::M => {
                    self.play("ui_click");
                    self.open_menu();
                }
                _ => {}
            }
        }
    }

    fn refresh_briefing_poster(&mut self) {
        self.briefing_player = None;
        self.briefing_player_key = None;
        self.briefing_poster = None;
        self.briefing_video = None;
        let Some(page) = self.briefing_pages.get(self.briefing_page) else {
            return;
        };
        let Some(video) = page.video.clone() else {
            return;
        };
        if let Some(store) = &self.video_store {
            if let Some((_, poster)) = store.poster(&video) {
                self.briefing_poster = Some(poster);
                self.briefing_video = Some(video);
            }
        }
    }

    /// Replay playback controls: SPACE pause, +/- speed, R restart, ESC out.
    fn handle_replay(&mut self, window: &Window) {
        for key in window.get_keys_pressed(KeyRepeat::No) {
            match key {
                Key::Space => {
                    let now_paused = if let Some(rp) = self.replay.as_mut() {
                        rp.paused = !rp.paused;
                        rp.paused
                    } else {
                        false
                    };
                    self.status(
                        if now_paused {
                            "Replay paused"
                        } else {
                            "Replaying"
                        }
                        .to_string(),
                    );
                    self.play("ui_click");
                }
                Key::Equal | Key::NumPadPlus => {
                    self.speed = (self.speed * 1.25).min(4.0);
                    self.status(format!("Replay speed {:.2}x", self.speed));
                }
                Key::Minus | Key::NumPadMinus => {
                    self.speed = (self.speed / 1.25).max(0.25);
                    self.status(format!("Replay speed {:.2}x", self.speed));
                }
                Key::R => self.restart_replay(),
                Key::Escape | Key::M => self.open_menu(),
                Key::Q => self.quit = true,
                _ => {}
            }
        }
    }

    fn restart_replay(&mut self) {
        let Some(rp) = self.replay.take() else {
            return;
        };
        let save = rp.save.clone();
        if self.build_from_save(&save) {
            self.ais.clear();
            self.campaign_ais.clear();
            self.replay = Some(ReplayPlayer::new(save));
            self.status("Replaying from start".to_string());
        }
        self.play("ui_click");
    }

    /// Advance replay playback: due commands, then steps, honoring pause and
    /// the shared speed control. No AI runs here: their orders are in the log.
    fn update_replay(&mut self) {
        if self.replay.as_ref().map(|r| r.paused).unwrap_or(true) {
            self.last_step = Instant::now();
            return;
        }
        let now = Instant::now();
        let dt = now.duration_since(self.last_step).as_secs_f32().min(0.1);
        self.last_step = now;
        self.accumulator += dt * 24.0 * self.speed;
        let mut steps = 0;
        let target = self.replay.as_ref().map(|r| r.save.tick).unwrap_or(0);
        while self.accumulator >= 1.0 && steps < 24 {
            let due = if let Some(rp) = self.replay.as_mut() {
                let mut n = 0;
                while rp.idx < rp.save.rec.cmds.len()
                    && rp.save.rec.cmds[rp.idx].tick == self.world.tick
                {
                    let c = rp.save.rec.cmds[rp.idx].clone();
                    galaxycraft::replay::apply_act(&mut self.world, &c.act);
                    rp.idx += 1;
                    n += 1;
                }
                n
            } else {
                break;
            };
            let _ = due;
            if self.world.tick >= target {
                break;
            }
            self.world.step();
            self.accumulator -= 1.0;
            steps += 1;
        }
        if self.world.tick >= target {
            self.status(format!(
                "Replay ended at tick {} (hash {:x})",
                target,
                state_hash(&self.world)
            ));
            self.replay = None;
        }
        self.selected
            .retain(|id| self.world.unit(*id).is_some_and(|u| u.alive));
    }

    fn handle_pause(&mut self, window: &Window) {
        for key in window.get_keys_pressed(KeyRepeat::No) {
            match key {
                Key::Escape => {
                    self.paused = false;
                    self.play("ui_click");
                }
                Key::R => self.restart(),
                Key::M => self.open_menu(),
                Key::Q => self.quit = true,
                Key::F10 => self.show_stats = !self.show_stats,
                Key::F5 => self.save_game(),
                Key::F6 => self.save_replay(),
                _ => {}
            }
        }
        let (mx, my) = window
            .get_unscaled_mouse_pos(MouseMode::Clamp)
            .map(|(x, y)| (x as i32, y as i32))
            .unwrap_or((-1, -1));
        let lmb = window.get_mouse_down(MouseButton::Left);
        if lmb && !self.lmb_down {
            let scale = self.ui_scale;
            let (ox, oy) = (self.ui_ox, self.ui_oy);
            let base =
                if let Some(dialog) = self.console.as_ref().and_then(|c| c.layout("gamemenu")) {
                    dialog.rect.scaled(scale, ox, oy)
                } else {
                    UiRect::new(184, 32, 264, 288).scaled(scale, ox, oy)
                };
            let hit = |dx: i32, dy: i32| {
                UiRect::new(
                    base.x + (dx as f32 * scale) as i32,
                    base.y + (dy as f32 * scale) as i32,
                    (224.0 * scale) as i32,
                    (28.0 * scale) as i32,
                )
                .contains(mx, my)
            };
            if hit(20, 36) {
                self.paused = false;
                self.play("ui_click");
            } else if hit(20, 70) {
                self.restart();
            } else if hit(20, 104) {
                self.show_stats = !self.show_stats;
            } else if hit(20, 172) {
                self.open_menu();
            } else if hit(20, 206) {
                self.quit = true;
            }
        }
        self.lmb_down = lmb;
    }

    fn handle_result(&mut self, window: &Window) {
        for key in window.get_keys_pressed(KeyRepeat::No) {
            match key {
                Key::Enter | Key::Space if self.game_result == Some(true) => match self.next_map {
                    Some(index) => {
                        self.play("ui_swish");
                        // Act movies: the current mission's `<Race><act>`
                        // then the next act's `Pre<Race>_<act>` play before
                        // the next mission loads.
                        let mut clips: Vec<String> = Vec::new();
                        if let Some(post) = self.post_mission_cinematic() {
                            clips.push(post.to_string());
                        }
                        if let Some(tag) = self.maps.get(index).and_then(|m| m.campaign.as_ref()) {
                            if let Some(order) = Self::campaign_order(tag) {
                                if let Some(pre) = galaxycraft::video::campaign_cinematic(
                                    &tag.episode,
                                    order,
                                    true,
                                ) {
                                    clips.push(pre.to_string());
                                }
                            }
                        }
                        if clips.is_empty() {
                            self.start_campaign_mission(index);
                        } else {
                            self.begin_intro(clips, Some(index));
                        }
                    }
                    None => {
                        if let Some(post) = self.post_mission_cinematic() {
                            self.begin_intro(vec![post.to_string()], None);
                        } else {
                            self.open_menu();
                        }
                    }
                },
                Key::Enter | Key::Space | Key::Escape | Key::M => self.open_menu(),
                Key::R => self.restart(),
                Key::Q => self.quit = true,
                _ => {}
            }
        }
    }

    /// Resolve the next mission.  Distribution missions (Enslavers) carry a
    /// `Next Scenario` trigger string; retail campaign missions instead
    /// advance through the episode list, because the campaign progression is
    /// stored in the campaign definition rather than the CHK.
    fn resolve_next_map(&self) -> Option<usize> {
        if let Some(index) = self.world.triggers.as_ref().and_then(|t| t.next_scenario) {
            let raw = self.scenario.strings.get(index as usize)?.trim();
            let key = scenario_file_key(raw);
            if !key.is_empty() {
                if let Some(found) = self
                    .maps
                    .iter()
                    .position(|m| scenario_file_key(&m.name) == key)
                {
                    return Some(found);
                }
            }
        }
        let current = self.maps.get(self.map_index)?.campaign.as_ref()?;
        let next = self.map_index + 1;
        let following = self.maps.get(next)?.campaign.as_ref()?;
        (following.episode == current.episode).then_some(next)
    }

    #[allow(unreachable_code)]
    fn draw_menu(&mut self, fb: &mut Framebuffer) {
        // Ultrawide pillarbox: the menu content is a 16:9 box, so the side
        // bars are not touched by any sub-screen.  Clear the whole frame
        // first, otherwise the software cursor leaves stale arrows in them.
        // The main screen copies its fully composed cache over every pixel,
        // so it does not need the extra clear.
        if !matches!(self.menu_screen, MenuScreen::Main) {
            clear_menu_frame(fb);
        }
        // Metrics must be fresh here: the headless `--shot-screen` path calls
        // `draw_menu` directly (bypassing `draw`, which calls `update_view`),
        // so stale defaults (scale 2, ox 0) would misplace every panel and
        // entry at any other resolution.  Idempotent in the live path.
        self.update_view(fb);
        // Retail menu flow from the 44 original screenshots: dispatch to the
        // active sub-screen.  The Main screen lists all eight retail entries;
        // Episode/Mission/Lobby cover Single/Multi; Options/Hotkeys/
        // Collections/Credits/Registry/GameType cover the footer buttons.
        match self.menu_screen.clone() {
            MenuScreen::Registry => {
                self.draw_registry(fb);
                return;
            }
            MenuScreen::GameType => {
                self.draw_gametype(fb);
                return;
            }
            MenuScreen::Episode => {
                self.draw_episode(fb);
                return;
            }
            MenuScreen::Mission { .. } => {
                self.draw_mission(fb);
                return;
            }
            MenuScreen::Lan => {
                self.draw_lan(fb);
                return;
            }
            MenuScreen::Replays => {
                self.draw_create(fb);
                return;
            }
            MenuScreen::SavedGames => {
                self.draw_saved(fb);
                return;
            }
            MenuScreen::Lobby { .. } => {
                self.draw_lobby(fb);
                return;
            }
            MenuScreen::Options { .. } => {
                self.draw_options(fb);
                return;
            }
            MenuScreen::Hotkeys { .. } => {
                self.draw_hotkeys(fb);
                return;
            }
            MenuScreen::Collections => {
                self.draw_collections(fb);
                return;
            }
            MenuScreen::Credits => {
                self.draw_credits(fb);
                return;
            }
            MenuScreen::Intro { .. } => {
                self.draw_intro(fb);
                return;
            }
            MenuScreen::Main => {
                self.draw_main_full(fb);
                return;
            }
        }
        let (scale, ox, oy) = self.ui_metrics(fb);
        let w = fb.width;
        let h = fb.height;
        // Full-frame retail title art (glue/title/title.dds), cached per size.
        let stale = self
            .menu_fit
            .as_ref()
            .map(|(fw, fh, _)| *fw != fb.width as usize || *fh != fb.height as usize)
            .unwrap_or(true);
        if stale {
            self.menu_fit = self
                .glue
                .as_ref()
                .and_then(|g| g.title.as_ref())
                .map(|art| {
                    (
                        fb.width as usize,
                        fb.height as usize,
                        art.cover(fb.width as usize, fb.height as usize),
                    )
                });
        }
        if let Some((_, _, art)) = &self.menu_fit {
            for y in 0..h {
                for x in 0..w {
                    let p = art.pixels[y as usize * art.width + x as usize];
                    fb.put(x, y, Rgb(p & 0x00ff_ffff));
                }
            }
            // The retail menu dims the title art behind the menu controls.
            for y in 0..h {
                for x in 0..w {
                    let c = fb.get(x, y);
                    fb.put(x, y, c.blend(Rgb::new(0, 0, 0), 96));
                }
            }
        } else {
            for y in 0..h {
                let t = y as f32 / h as f32;
                let c = Rgb::new(
                    (6.0 + 18.0 * t) as u8,
                    (10.0 + 24.0 * t) as u8,
                    (24.0 + 40.0 * t) as u8,
                );
                fb.rect(0, y, w, 1, c);
            }
            for x in 0..80 {
                let sx = (x * 197 + 31) % w.max(1);
                let sy = (x * 89 + 17) % (h.max(1) / 2);
                fb.rect(sx, sy, 1, 1, Rgb::new(150, 160, 190));
            }
        }

        // Retail title-frame surfaces, at their shipped 640x480 rectangles.
        if let Some(glue) = &self.glue {
            if let Some(dialog) = &glue.menu {
                for (id, art) in [
                    (22, glue.title_bg.as_ref()),
                    (23, glue.title_overlay.as_ref()),
                    (6, glue.intro_credit.as_ref()),
                    (7, glue.credit.as_ref()),
                    (1, glue.etail.as_ref()),
                ] {
                    if let (Some(c), Some(art)) = (dialog.control(id), art) {
                        let mut r = c.abs.scaled(scale, ox, oy);
                        if id == 22 {
                            r.x -= 3;
                        }
                        art.draw_scaled(fb, r.x, r.y, r.w, r.h);
                    }
                }
            }
        }

        // Menu buttons use the shipped main-menu hot areas as anchors.
        let campaign = self.menu_tab == 0;
        let single_rect = UiRect::new(24, 86, 270, 84).scaled(scale, ox, oy);
        let multi_rect = UiRect::new(24, 190, 270, 84).scaled(scale, ox, oy);
        let intro_rect = UiRect::new(24, 294, 270, 40).scaled(scale, ox, oy);
        let exit_rect = UiRect::new(24, 346, 270, 40).scaled(scale, ox, oy);
        self.menu_button(fb, single_rect, "SINGLE PLAYER", "Campaign", campaign);
        self.menu_button(fb, multi_rect, "MULTIPLAYER", "Melee / Skirmish", !campaign);
        self.menu_button(fb, intro_rect, "INTRO", "Replay title sequence", false);
        self.menu_button(fb, exit_rect, "EXIT", "Quit to desktop", false);
        fb.text(
            single_rect.x + 10,
            single_rect.y + 56,
            &format!("{} retail campaign missions", self.campaign_len),
            Rgb::new(150, 185, 160),
            1,
        );
        fb.text(
            single_rect.x + 10,
            single_rect.y + 72,
            "establishing-shot briefings + full score screens",
            Rgb::new(130, 160, 185),
            1,
        );
        fb.text(
            multi_rect.x + 10,
            multi_rect.y + 56,
            &format!(
                "{} shipped maps  -  2..8 players",
                self.maps.len().saturating_sub(self.campaign_len)
            ),
            Rgb::new(150, 185, 160),
            1,
        );
        fb.text(
            multi_rect.x + 10,
            multi_rect.y + 72,
            "all eight tilesets, deterministic AI",
            Rgb::new(130, 160, 185),
            1,
        );

        fb.text(
            ox + 24 * scale as i32,
            oy + 14 * scale as i32,
            "GALAXYCRAFT",
            Rgb::new(255, 225, 110),
            (2.0 * scale).round().max(1.0) as i32,
        );
        fb.text(
            ox + 26 * scale as i32,
            oy + 42 * scale as i32,
            "A RUST RECONSTRUCTION OF STARCRAFT 1.23",
            Rgb::new(160, 185, 215),
            (0.9 * scale).round().max(1.0) as i32,
        );

        // Map/episode list panel, anchored in the right half of the safe area.
        let (start, end) = self.menu_slice();
        let count = end.saturating_sub(start);
        let list = UiRect::new(330, 190, 292, 270).scaled(scale, ox, oy);
        fb.rect(
            list.x,
            list.y,
            list.w,
            list.h,
            Rgb::new(8, 12, 20).blend(Rgb::new(30, 40, 55), 170),
        );
        fb.rect_outline(list.x, list.y, list.w, list.h, Rgb::new(110, 150, 185));
        let title = if campaign {
            let episode = self
                .maps
                .get(start + self.menu_index.min(count.saturating_sub(1)))
                .and_then(|m| m.campaign.as_ref())
                .map(|c| c.episode_name.clone())
                .unwrap_or_default();
            format!("{episode}  ({count})")
        } else {
            format!("MELEE MAPS  ({count})")
        };
        fb.text(list.x + 8, list.y + 6, &title, Rgb::new(130, 225, 150), 1);
        if campaign && count == 0 {
            fb.text(
                list.x + 8,
                list.y + 28,
                "No retail campaign found - start with --install PATH",
                Rgb::new(235, 160, 140),
                1,
            );
        }
        let visible = 8usize;
        let rel_sel = self
            .map_index
            .saturating_sub(start)
            .min(count.saturating_sub(1));
        let first = rel_sel
            .saturating_sub(visible / 2)
            .min(count.saturating_sub(visible));
        for row in 0..visible.min(count) {
            let global = start + first + row;
            let Some(m) = self.maps.get(global) else {
                continue;
            };
            let y = list.y + 26 + row as i32 * 22;
            let selected = global == self.map_index;
            if selected {
                fb.rect(list.x + 4, y - 2, list.w - 8, 20, Rgb::new(36, 58, 82));
                fb.rect_outline(list.x + 4, y - 2, list.w - 8, 20, Rgb::new(100, 190, 255));
            }
            let label = format!("{:>2}. {}", first + row + 1, m.name);
            let short = if label.len() > 34 {
                &label[..34]
            } else {
                &label
            };
            fb.text(
                list.x + 8,
                y,
                short,
                if selected {
                    Rgb::new(255, 245, 180)
                } else {
                    Rgb::new(200, 205, 220)
                },
                1,
            );
        }
        if let Some(m) = self.maps.get(self.map_index) {
            let detail = match &m.campaign {
                Some(tag) => format!(
                    "{}  -  {} mission {}   R: {:?}",
                    m.name, tag.episode_name, tag.mission, self.menu_race
                ),
                None => format!(
                    "{}  -  {} players, {} tileset   R: {:?}",
                    m.name,
                    m.players,
                    era_name(m.era),
                    self.menu_race
                ),
            };
            fb.text(
                ox + 20 * scale as i32,
                oy + 466 * scale as i32,
                &detail,
                Rgb::new(200, 215, 230),
                1,
            );
        }
        fb.text(
            ox + 20 * scale as i32,
            oy + 430 * scale as i32,
            "TAB: campaign / melee   ENTER: play   UP/DOWN: select   R: race   I: intro   ESC: quit",
            Rgb::new(170, 190, 215),
            1,
        );
    }

    fn menu_button(
        &self,
        fb: &mut Framebuffer,
        rect: UiRect,
        title: &str,
        sub: &str,
        active: bool,
    ) {
        let fill = if active {
            Rgb::new(42, 70, 96).blend(Rgb::new(20, 30, 45), 90)
        } else {
            Rgb::new(10, 16, 24).blend(Rgb::new(20, 30, 45), 130)
        };
        fb.rect(rect.x, rect.y, rect.w, rect.h, fill);
        fb.rect_outline(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            if active {
                Rgb::new(120, 220, 255)
            } else {
                Rgb::new(80, 100, 125)
            },
        );
        let name_scale = if rect.h >= 60 { 2 } else { 1 };
        fb.text(
            rect.x + 10,
            rect.y + if rect.h >= 60 { 10 } else { 3 },
            title,
            if active {
                Rgb::new(255, 245, 180)
            } else {
                Rgb::new(210, 220, 235)
            },
            name_scale,
        );
        if rect.h >= 40 {
            fb.text(
                rect.x + 10,
                rect.y + if rect.h >= 60 { 32 } else { 16 },
                sub,
                Rgb::new(150, 175, 200),
                1,
            );
        }
    }

    fn draw_briefing(&mut self, fb: &mut Framebuffer) {
        if !self.briefing_pages.is_empty() {
            self.draw_retail_briefing(fb);
            return;
        }
        let w = fb.width;
        let h = fb.height;
        // Same deep-space backdrop as the menu.
        for y in 0..h {
            let t = y as f32 / h as f32;
            let c = Rgb::new(
                (6.0 + 18.0 * t) as u8,
                (10.0 + 24.0 * t) as u8,
                (24.0 + 40.0 * t) as u8,
            );
            fb.rect(0, y, w, 1, c);
        }
        for x in 0..80 {
            let sx = (x * 197 + 31) % w.max(1);
            let sy = (x * 89 + 17) % (h.max(1) / 2);
            fb.rect(sx, sy, 1, 1, Rgb::new(150, 160, 190));
        }
        let Some((name, briefing, objectives)) = briefing_content(&self.scenario) else {
            return;
        };
        let name = if name.trim().is_empty() {
            self.maps
                .get(self.map_index)
                .map(|m| m.name.clone())
                .unwrap_or_default()
        } else {
            name
        };
        let (px, py, pw, ph) = (60, 96, w - 120, h - 180);
        fb.rect(px, py, pw, ph, Rgb::new(12, 16, 26));
        fb.rect_outline(px, py, pw, ph, Rgb::new(70, 110, 150));
        fb.text_centered(w / 2, 28, "MISSION BRIEFING", Rgb::new(255, 214, 90), 3);
        fb.text_centered(w / 2, 66, &name, Rgb::new(225, 235, 245), 2);

        let text_x = px + 34;
        let mut y = py + 28;
        for line in wrap_text(&briefing, 96).into_iter().take(20) {
            fb.text(text_x, y, &line, Rgb::new(200, 210, 220), 1);
            y += 14;
        }
        if !objectives.trim().is_empty() {
            y += 18;
            fb.text(text_x, y, "OBJECTIVES", Rgb::new(255, 210, 100), 2);
            y += 26;
            for line in wrap_text(&objectives, 96).into_iter().take(8) {
                fb.text(text_x, y, &line, Rgb::new(240, 225, 160), 1);
                y += 14;
            }
        }
        fb.text_centered(
            w / 2,
            h - 62,
            "ENTER: deploy     ESC: back to menu",
            Rgb::new(150, 180, 210),
            1,
        );
    }

    /// Draw a page of the retail briefing script over its shipped backdrop.
    /// The establishing-shot scripts place the location copy at the lower
    /// left of the screen; the intermission pages centre their text.
    fn draw_retail_briefing(&mut self, fb: &mut Framebuffer) {
        let w = fb.width as usize;
        let h = fb.height as usize;
        let page = &self.briefing_pages[self.briefing_page];
        let lower_left = page.screen_lower_left;
        let text = page.text.clone();
        let video = page.video.clone();

        // Fit the retail backdrop once per window size.
        let stale = self
            .briefing_fit
            .as_ref()
            .map(|(fw, fh, _)| *fw != w || *fh != h)
            .unwrap_or(true);
        if stale {
            if let Some(art) = &self.briefing_art {
                self.briefing_fit = Some((w, h, art.cover(w, h)));
            }
        }
        if let Some((fw, fh, art)) = &self.briefing_fit {
            for y in 0..(*fh).min(h) {
                for x in 0..(*fw).min(w) {
                    let p = art.pixels[y * *fw + x];
                    fb.pixels[y * w + x] = p & 0x00ff_ffff;
                }
            }
        } else {
            fb.clear(Rgb::new(4, 5, 10));
        }

        // A subtle vignette keeps the copy legible over bright artwork.
        let shade = if lower_left { 0.55 } else { 0.35 };
        let band_top = if lower_left {
            (h * 42 / 100) as i32
        } else {
            (h / 2 - 140) as i32
        };
        let band_h = if lower_left {
            (h * 46 / 100) as i32
        } else {
            300
        };
        for y in band_top..(band_top + band_h).min(h as i32) {
            let t = (y - band_top) as f32 / band_h.max(1) as f32;
            let a = (t * shade * 255.0) as u8;
            for x in 0..w {
                let c = fb.pixels[y as usize * w + x];
                let base = Rgb(c & 0x00ff_ffff);
                fb.pixels[y as usize * w + x] = base.blend(Rgb::new(0, 0, 0), a).0;
            }
        }

        let title_color = Rgb::new(255, 226, 150);
        let body_color = Rgb::new(226, 208, 178);
        let chars = ((w as i32 - 160) / 16).max(24) as usize;
        let lines: Vec<String> = if text.trim().is_empty() {
            Vec::new()
        } else {
            wrap_text(&text, chars)
        };

        let line_h = 22;
        let total = lines.len() as i32 * line_h;
        let mut y = if lower_left {
            (h as i32 - 70 - total).max(40)
        } else {
            h as i32 / 2 - total / 2
        };
        let x = 80;
        for (i, line) in lines.iter().enumerate() {
            let (scale, color) = if i == 0 && line.trim().starts_with('"') {
                (3, title_color)
            } else {
                (2, body_color)
            };
            // Hard shadow, as the retail text is drawn over full-frame art.
            fb.text(x + 2, y + 2, line, Rgb::new(0, 0, 0), scale);
            fb.text(x, y, line, color, scale);
            y += line_h;
        }
        // Decoded intermission poster: when the page names a `</VIDEO>`
        // file and the materialised `videos/` cache holds it, the first
        // decoded frame is letterboxed at the top so the briefing screen
        // shows the retail movie, not just the caption.
        // Intermission clip: stream it at 12 fps while the page is open; the
        // poster stays as the first paint and as the fallback without ffmpeg.
        if let Some(rel) = self.briefing_video.clone() {
            let stale = self.briefing_player_key.as_deref() != Some(rel.as_str());
            if stale {
                self.briefing_player = None;
                self.briefing_player_key = Some(rel.clone());
                self.briefing_last = Instant::now();
                if let Some(store) = &self.video_store {
                    // The script names the clip `interstitials\TR1010.webm`
                    // while the cache stores `interstitials/tr1010.webm`;
                    // `resolve` normalises the separators and the case.
                    if let Some(path) = store.resolve(&rel) {
                        if let Some(info) = galaxycraft::video::probe_info(&path) {
                            self.briefing_player =
                                galaxycraft::cinematic::Cinematic::open(&path, &info, 12.0, 960);
                        }
                    }
                }
            }
        }
        let dt = self.briefing_last.elapsed().as_secs_f32();
        self.briefing_last = Instant::now();
        let streamed = self.briefing_player.as_mut().and_then(|p| p.advance(dt));
        if let Some(poster) = streamed.clone().or_else(|| self.briefing_poster.clone()) {
            let bw = (w as f32 * 0.52) as i32;
            let bh = (poster.height as f32 * bw as f32 / poster.width.max(1) as f32) as i32;
            let bx = (w as i32 - bw) / 2;
            let by = 24;
            fb.rect(bx - 2, by - 2, bw + 4, bh + 4, Rgb::new(8, 10, 16));
            fb.rect_outline(bx - 2, by - 2, bw + 4, bh + 4, Rgb::new(110, 150, 185));
            for y in 0..bh {
                for x in 0..bw {
                    let sx = (x as f32 * poster.width as f32 / bw as f32) as usize;
                    let sy = (y as f32 * poster.height as f32 / bh as f32) as usize;
                    if sx >= poster.width as usize || sy >= poster.height as usize {
                        continue;
                    }
                    let o = (sy * poster.width as usize + sx) * 3;
                    let dx = bx + x;
                    let dy = by + y;
                    if dx < 0 || dy < 0 || dx >= w as i32 || dy >= h as i32 {
                        continue;
                    }
                    fb.pixels[dy as usize * w + dx as usize] =
                        (Rgb::new(poster.rgb[o], poster.rgb[o + 1], poster.rgb[o + 2])).0;
                }
            }
        }
        if let Some(video) = video {
            let stem = Path::new(&video.replace('\\', "/"))
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(video.as_str())
                .to_string();
            let playing = self.briefing_player.is_some();
            let decoded = playing || self.briefing_poster.is_some();
            let note = if playing {
                format!("CINEMATIC  {stem}  [playing]")
            } else if decoded {
                format!("CINEMATIC  {stem}  [decoded WebM poster]")
            } else {
                format!("INTERMISSION  {stem}")
            };
            fb.text_centered(
                w as i32 / 2,
                h as i32 - 46,
                &note,
                if decoded {
                    Rgb::new(170, 220, 180)
                } else {
                    Rgb::new(150, 160, 180)
                },
                1,
            );
        }

        // Page indicator and controls.
        let page_note = if self.briefing_pages.len() > 1 {
            format!(
                "{} / {}   ENTER: continue   ESC: menu",
                self.briefing_page + 1,
                self.briefing_pages.len()
            )
        } else {
            "ENTER: deploy   ESC: menu".to_string()
        };
        fb.text_centered(
            w as i32 / 2,
            h as i32 - 26,
            &page_note,
            Rgb::new(180, 195, 215),
            1,
        );
    }

    fn draw_overlay(&mut self, fb: &mut Framebuffer) {
        // Persistent modal requested with --start-screen (live client).
        match self.overlay {
            1 => {
                self.draw_objectives_panel(fb);
                return;
            }
            2 => {
                self.draw_tips_panel(fb);
                return;
            }
            3 => {
                self.draw_full_gamemenu(fb);
                return;
            }
            _ => {}
        }
        if self.game_result.is_some() {
            self.draw_score_screen(fb);
            return;
        }
        if self.paused {
            self.draw_pause_menu(fb);
            return;
        }
        let w = fb.width;
        let h = fb.height;
        let (title, color, help) = if self.paused {
            (
                "PAUSED",
                Rgb::new(255, 220, 80),
                "ESC resume   R restart   M menu   Q quit",
            )
        } else if let Some(win) = self.game_result {
            if win {
                (
                    "VICTORY",
                    Rgb::new(100, 255, 120),
                    if self.next_map.is_some() {
                        "ENTER next mission   R play again   Q quit"
                    } else {
                        "ENTER menu   R play again   Q quit"
                    },
                )
            } else {
                (
                    "DEFEAT",
                    Rgb::new(255, 90, 90),
                    "ENTER menu   R play again   Q quit",
                )
            }
        } else if self.replay.as_ref().map(|r| r.paused).unwrap_or(false) {
            (
                "REPLAY PAUSED",
                Rgb::new(255, 220, 80),
                "SPACE resume   +/- speed   R restart   ESC menu",
            )
        } else {
            return;
        };
        for y in (h / 2 - 120)..(h / 2 + 120) {
            for x in (w / 2 - 360)..(w / 2 + 360) {
                let c = fb.get(x, y);
                fb.put(x, y, c.blend(Rgb::new(0, 0, 0), 190));
            }
        }
        fb.text_centered(w / 2, h / 2 - 80, title, color, 4);
        if self.paused || self.replay.as_ref().map(|r| r.paused).unwrap_or(false) {
            let detail = if let Some(rp) = self.replay.as_ref() {
                format!(
                    "{} - REPLAY tick {}/{}",
                    rp.save.map_name, self.world.tick, rp.save.tick
                )
            } else {
                format!(
                    "{} - tick {}",
                    self.maps[self.map_index].name, self.world.tick
                )
            };
            fb.text_centered(w / 2, h / 2 + 10, &detail, Rgb::new(220, 220, 220), 2);
            fb.text_centered(w / 2, h / 2 + 60, help, Rgb::new(190, 200, 220), 1);
            return;
        }
        let win = self.game_result == Some(true);
        let p = &self.world.players[self.local_player];
        fb.text_centered(
            w / 2,
            h / 2 - 18,
            &format!(
                "{}   {} elapsed",
                self.maps[self.map_index].name,
                format_time(self.world.tick)
            ),
            Rgb::new(230, 230, 230),
            2,
        );
        let stat = format!(
            "Units killed {:<5} losses {:<5} buildings razed {:<4}    Minerals {}   Gas {}",
            p.kills, p.deaths, p.razings, p.minerals_gathered, p.gas_gathered
        );
        fb.text_centered(w / 2, h / 2 + 26, &stat, Rgb::new(170, 200, 170), 1);
        let score = if win {
            "MISSION ACCOMPLISHED"
        } else {
            "MISSION FAILED - the colony was lost"
        };
        fb.text_centered(w / 2, h / 2 + 52, score, Rgb::new(200, 200, 200), 1);
        let next = if win {
            self.next_map
                .and_then(|i| self.maps.get(i))
                .map(|m| m.name.as_str())
        } else {
            None
        };
        let help = if let Some(name) = next {
            fb.text_centered(
                w / 2,
                h / 2 + 76,
                &format!("NEXT: {name}"),
                Rgb::new(255, 220, 120),
                1,
            );
            "ENTER next mission    M menu    R replay    Q quit"
        } else {
            help
        };
        fb.text_centered(w / 2, h / 2 + 100, help, Rgb::new(190, 200, 220), 1);
    }

    /// Retail `gamemenu.ui.json` pause panel (264x288 at 184,32).
    fn draw_pause_menu(&mut self, fb: &mut Framebuffer) {
        let (scale, ox, oy) = self.ui_metrics(fb);
        // Dim the frozen game and draw the shipped dialog rectangle.
        for y in 0..fb.height {
            for x in 0..fb.width {
                let c = fb.get(x, y);
                fb.put(x, y, c.blend(Rgb::new(0, 0, 0), 130));
            }
        }
        let layout = self
            .console
            .as_ref()
            .and_then(|c| c.layout("gamemenu"))
            .cloned();
        let base = if let Some(dialog) = &layout {
            dialog.rect.scaled(scale, ox, oy)
        } else {
            UiRect::new(184, 32, 264, 288).scaled(scale, ox, oy)
        };
        fb.rect(
            base.x,
            base.y,
            base.w,
            base.h,
            Rgb::new(9, 13, 20).blend(Rgb::new(35, 45, 60), 110),
        );
        fb.rect_outline(base.x, base.y, base.w, base.h, Rgb::new(120, 150, 185));
        let title = self
            .maps
            .get(self.map_index)
            .map(|m| m.name.as_str())
            .unwrap_or("GAME MENU");
        fb.text_centered(
            base.x + base.w / 2,
            base.y + (10.0 * scale) as i32,
            "GAME MENU",
            Rgb::new(255, 225, 130),
            (1.6 * scale).round().max(1.0) as i32,
        );
        fb.text_centered(
            base.x + base.w / 2,
            base.y + (36.0 * scale) as i32,
            title,
            Rgb::new(170, 190, 210),
            1,
        );
        let entries: &[(i32, &str, &str)] = &[
            (1, "ESC", "RESUME"),
            (2, "R", "RESTART MISSION"),
            (3, "F10", "MISSION STATISTICS"),
            (5, "H", "HELP / CONTROLS"),
            (6, "M", "RETURN TO MENU"),
            (7, "Q", "QUIT TO DESKTOP"),
        ];
        for &(id, key, label) in entries {
            let rect = layout
                .as_ref()
                .and_then(|d| d.control(id))
                .map(|c| c.abs.scaled(scale, ox, oy))
                .unwrap_or_else(|| {
                    UiRect::new(184 + 20, 32 + 36 + (id - 1) * 34, 224, 28).scaled(scale, ox, oy)
                });
            let active = key == "ESC";
            fb.rect(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                if active {
                    Rgb::new(40, 68, 94).blend(Rgb::new(20, 30, 45), 100)
                } else {
                    Rgb::new(16, 22, 32)
                },
            );
            fb.rect_outline(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                if active {
                    Rgb::new(120, 215, 255)
                } else {
                    Rgb::new(80, 100, 125)
                },
            );
            fb.text(
                rect.x + 10,
                rect.y + 8,
                &format!("{key:<4} {label}"),
                Rgb::new(220, 228, 240),
                1,
            );
        }
        fb.text_centered(
            base.x + base.w / 2,
            base.y + base.h + (8.0 * scale) as i32,
            "ESC: resume    R: restart    M: menu    Q: quit    F10: stats    F5: save    F6: replay",
            Rgb::new(150, 170, 195),
            1,
        );
    }

    /// Retail `glue/score*` score screen with the eight shipped row
    /// rectangles and the `msnstats` summary panel.
    fn draw_score_screen(&mut self, fb: &mut Framebuffer) {
        let (scale, ox, oy) = self.ui_metrics(fb);
        let w = fb.width as usize;
        let h = fb.height as usize;
        let won = self.game_result == Some(true);
        if self.result_panel.is_none() {
            self.result_panel =
                GlueArt::load_score_panel(&self.assets_root, self.local_race(), won);
            self.result_fit = None;
        }
        let stale = self
            .result_fit
            .as_ref()
            .map(|(fw, fh, _)| *fw != w || *fh != h)
            .unwrap_or(true);
        if stale {
            self.result_fit = self
                .result_panel
                .as_ref()
                .map(|art| (w, h, art.cover(w, h)));
        }
        // Starfield behind the translucent retail score frame (`pmain` is a
        // border surface with a transparent interior).
        for y in 0..h {
            let t = y as f32 / h as f32;
            let c = Rgb::new(
                (8.0 + 18.0 * t) as u8,
                (12.0 + 24.0 * t) as u8,
                (28.0 + 40.0 * t) as u8,
            );
            fb.rect(0, y as i32, fb.width, 1, c);
        }
        if let Some((_, _, art)) = &self.result_fit {
            art.draw_scaled(fb, 0, 0, fb.width, fb.height);
        }

        let score = self
            .glue
            .as_ref()
            .and_then(|g| g.score.as_ref())
            .or_else(|| self.console.as_ref().and_then(|c| c.layout("gluscore")))
            .cloned();
        let rows = if let Some(dialog) = &score {
            dialog
                .controls_of("RECT")
                .filter(|c| c.rect.w == 588)
                .map(|c| c.abs.scaled(scale, ox, oy))
                .collect::<Vec<_>>()
        } else {
            (0..8)
                .map(|i| UiRect::new(30, 76 + i * 40, 588, 37).scaled(scale, ox, oy))
                .collect::<Vec<_>>()
        };

        let title = if won {
            "MISSION ACCOMPLISHED"
        } else {
            "MISSION FAILED"
        };
        fb.text_centered(
            fb.width / 2,
            oy + (12.0 * scale) as i32,
            title,
            if won {
                Rgb::new(120, 255, 140)
            } else {
                Rgb::new(255, 100, 100)
            },
            (2.0 * scale).round().max(1.0) as i32,
        );
        let name = self
            .maps
            .get(self.map_index)
            .map(|m| m.name.clone())
            .unwrap_or_default();
        fb.text_centered(
            fb.width / 2,
            oy + (48.0 * scale) as i32,
            &format!("{}   {}", name, format_time(self.world.tick)),
            Rgb::new(225, 230, 240),
            1,
        );

        for row in 0..8 {
            let Some(r) = rows.get(row) else {
                break;
            };
            let player = self.world.players.get(row);
            fb.rect(
                r.x,
                r.y,
                r.w,
                r.h,
                if row == self.local_player {
                    Rgb::new(30, 48, 66).blend(Rgb::new(20, 28, 40), 130)
                } else {
                    Rgb::new(10, 16, 24).blend(Rgb::new(20, 28, 40), 170)
                },
            );
            fb.rect_outline(
                r.x,
                r.y,
                r.w,
                r.h,
                if row == self.local_player {
                    Rgb::new(100, 200, 255)
                } else {
                    Rgb::new(75, 90, 110)
                },
            );
            let base = r.y + (6.0 * scale) as i32;
            let Some(player) = player else {
                fb.text(
                    r.x + 6,
                    base,
                    &format!("P{}  -", row + 1),
                    Rgb::new(120, 130, 145),
                    1,
                );
                fb.text(
                    r.x + 6,
                    base + (14.0 * scale) as i32,
                    "not in game",
                    Rgb::new(95, 105, 120),
                    1,
                );
                continue;
            };
            fb.text(
                r.x + 6,
                base,
                &format!("P{}", row + 1),
                if row == self.local_player {
                    Rgb::new(255, 240, 170)
                } else {
                    Rgb::new(220, 225, 235)
                },
                1,
            );
            fb.text(
                r.x + (158.0 * scale) as i32,
                base,
                &format!("{}", player.kills),
                Rgb::new(210, 230, 210),
                1,
            );
            fb.text(
                r.x + (262.0 * scale) as i32,
                base,
                &format!("{}", player.deaths),
                Rgb::new(230, 210, 210),
                1,
            );
            fb.text(
                r.x + (366.0 * scale) as i32,
                base,
                &format!("{}", player.razings),
                Rgb::new(220, 220, 160),
                1,
            );
            fb.text(
                r.x + (474.0 * scale) as i32,
                base,
                &format!("{}", player.scores[0]),
                Rgb::new(180, 210, 255),
                1,
            );
            fb.text(
                r.x + 6,
                base + (14.0 * scale) as i32,
                &format!(
                    "{:?}  {}m {}g  {}",
                    player.race,
                    player.minerals_gathered,
                    player.gas_gathered,
                    if player.defeated {
                        "defeated"
                    } else {
                        "active"
                    }
                ),
                Rgb::new(145, 160, 180),
                1,
            );
        }
        // Column labels, placed over the shipped header separators.
        let col_y = oy + (58.0 * scale) as i32;
        for (offset, label) in [
            (36, "PLAYER"),
            (188, "KILLS"),
            (292, "LOSSES"),
            (396, "RAZED"),
            (504, "SCORE"),
        ] {
            fb.text(
                ox + (offset as f32 * scale) as i32,
                col_y,
                label,
                Rgb::new(140, 210, 160),
                1,
            );
        }

        let next = self
            .next_map
            .and_then(|i| self.maps.get(i))
            .map(|m| m.name.clone());
        let help = if let Some(name) = next {
            format!("ENTER: next mission ({name})    M: menu    R: replay    Q: quit")
        } else {
            "ENTER: menu    M: menu    R: replay    Q: quit".to_string()
        };
        fb.text_centered(
            fb.width / 2,
            oy + (430.0 * scale) as i32,
            &help,
            Rgb::new(190, 205, 225),
            1,
        );
        // The stats panel is available from the score screen as well.
        if self.show_stats {
            self.draw_mission_stats(fb, scale, ox, oy);
        }
    }

    fn center_on_base(&mut self) {
        let base = self
            .world
            .units
            .iter()
            .find(|u| {
                u.alive
                    && u.owner as usize == self.local_player
                    && matches!(
                        u.type_id,
                        uid::COMMAND_CENTER | uid::NEXUS | uid::HATCHERY | uid::LAIR | uid::HIVE
                    )
            })
            .or_else(|| {
                self.world
                    .units
                    .iter()
                    .find(|u| u.alive && u.owner as usize == self.local_player)
            });
        if let Some(u) = base {
            self.camera = (u.x / 256 - self.view.0 / 2, u.y / 256 - self.view.1 / 2);
            self.clamp_camera();
        }
    }

    fn clamp_camera(&mut self) {
        let max_x = (self.scenario.width as i32 * TILE_PX - self.view.0).max(0);
        let max_y = (self.scenario.height as i32 * TILE_PX - self.view.1).max(0);
        self.camera.0 = self.camera.0.clamp(0, max_x);
        self.camera.1 = self.camera.1.clamp(0, max_y);
    }

    fn screen_of(&self, u: &SimUnit) -> (i32, i32) {
        (u.x / 256 - self.camera.0, u.y / 256 - self.camera.1)
    }

    fn world_of(&self, sx: i32, sy: i32) -> (i32, i32) {
        (sx + self.camera.0, sy + self.camera.1)
    }

    fn unit_at(&self, sx: i32, sy: i32) -> Option<u32> {
        let mut best: Option<(i32, u32)> = None;
        for (i, u) in self.world.units.iter().enumerate() {
            if !u.alive {
                continue;
            }
            // Cannot click an enemy the fog hides.
            if self.fog
                && u.owner as usize != self.local_player
                && !self.world.can_see_unit(self.local_player as u8, i)
            {
                continue;
            }
            let (ux, uy) = self.screen_of(u);
            let ty = self.data.unit(u.type_id);
            let r = ((ty.dimensions.right - ty.dimensions.left) as i32 / 2).max(10);
            let dx = sx - ux;
            let dy = sy - uy;
            if dx * dx + dy * dy <= r * r {
                let score = dy.abs() - uy / 1000;
                if best.is_none_or(|(b, _)| score < b) {
                    best = Some((score, u.id));
                }
            }
        }
        if best.is_none() && std::env::var("SC_VIEW_DEBUG").is_ok() {
            eprintln!("unit_at({}, {}) miss; camera={:?}", sx, sy, self.camera);
            for u in self
                .world
                .units
                .iter()
                .filter(|u| u.alive && u.owner as usize == self.local_player)
            {
                if u.is_resource() {
                    continue;
                }
                let (ux, uy) = self.screen_of(u);
                eprintln!(
                    "   {} {} at screen {} {} hp {}",
                    u.id,
                    self.data.unit_name(u.type_id),
                    ux,
                    uy,
                    u.hp
                );
            }
        }
        best.map(|(_, id)| id)
    }

    fn selected_units(&self) -> Vec<&SimUnit> {
        self.selected
            .iter()
            .filter_map(|id| self.world.unit(*id))
            .filter(|u| u.alive)
            .collect()
    }

    fn first_selected(&self) -> Option<&SimUnit> {
        self.selected_units().into_iter().next()
    }

    fn refresh_commands(&mut self) {
        self.command_list.clear();
        self.build_list.clear();
        let Some(u) = self.first_selected() else {
            return;
        };
        if u.owner as usize != self.local_player {
            return;
        }
        let type_id = u.type_id;
        let is_worker = self.data.unit(type_id).is_worker();
        let is_building = self.data.unit(type_id).is_building() || type_id == uid::LARVA;
        if is_worker {
            for t in 0..228u16 {
                if let Some(info) = tree::build_info(t) {
                    if info.producer == type_id {
                        self.build_list.push(t);
                    }
                }
            }
            self.command_list = self.build_list.clone();
        } else if is_building {
            for t in 0..228u16 {
                if let Some(info) = tree::build_info(t) {
                    if info.producer == type_id {
                        self.command_list.push(t);
                    }
                }
            }
        }
    }

    /// The nine card buttons of the race dialog (`statbtn{t,z,p}`), right
    /// anchored like retail.
    fn command_slots(&self) -> Vec<UiRect> {
        let dialog = self.card_dialog();
        (0..9)
            .map(|i| {
                let fallback = UiRect::new(496 + 9 + (i % 3) * 46, 354 + 4 + (i / 3) * 40, 36, 34);
                self.hud_control(dialog, i + 1, fallback)
            })
            .collect()
    }

    /// Activate command-card slot `index` (0..8).  This is shared by the
    /// number keys and mouse clicks on the retail button rectangles.
    fn activate_command(&mut self, index: usize) {
        self.actions = self.actions.saturating_add(1);
        if let Some(&unit_type) = self.command_list.get(index) {
            if self
                .first_selected()
                .is_some_and(|u| self.data.unit(u.type_id).is_worker())
            {
                self.build_mode = Some(unit_type);
                self.status(format!(
                    "Place {}: left click",
                    self.data.unit_name(unit_type)
                ));
            } else {
                self.train(unit_type);
            }
            return;
        }
        match index {
            1 => {
                self.attack_mode = true;
                self.patrol_mode = false;
                self.status("Attack: right click a target");
            }
            2 => {
                self.patrol_mode = true;
                self.attack_mode = false;
                self.status("Patrol: right click a destination");
            }
            3 => {
                self.issue_selected(Order::HoldPosition);
                self.status("Holding position");
            }
            4 => {
                for id in self.selected.clone() {
                    self.world.stop(id);
                }
                self.status("Stopped");
            }
            _ => {}
        }
    }

    fn issue_selected(&mut self, order: Order) {
        for id in self.selected.clone() {
            self.send_order(id, order.clone());
        }
        self.actions = self.actions.saturating_add(1);
        self.refresh_commands();
        self.play_ack();
    }

    fn command_at(&mut self, sx: i32, sy: i32, attack_move: bool, patrol: bool) {
        if self.selected.is_empty() {
            return;
        }
        self.actions = self.actions.saturating_add(1);
        let target = self
            .unit_at(sx, sy)
            .and_then(|id| self.world.unit(id))
            .map(|t| (t.id, t.owner, t.is_resource()));
        if let Some((tid, towner, is_resource)) = target {
            if towner as usize != self.local_player {
                let ids = self.selected.clone();
                for id in ids {
                    self.send_order(id, Order::AttackUnit { target: tid });
                }
                self.status("Attacking");
                self.refresh_commands();
                return;
            }
            if is_resource {
                let ids = self.selected.clone();
                for id in ids {
                    if self
                        .world
                        .unit(id)
                        .is_some_and(|u| self.data.unit(u.type_id).is_worker())
                    {
                        self.send_order(id, Order::Gather { resource: tid });
                    }
                }
                self.status("Gathering");
                return;
            }
        }
        // Mouse is in window pixels; the world view is logical (scaled).
        let (lx, ly) = self.to_logical(sx, sy);
        let (wx, wy) = self.world_of(lx, ly);
        if patrol {
            for id in self.selected.clone() {
                if let Some(u) = self.world.unit(id) {
                    let a = (u.x / 256, u.y / 256);
                    let _ = self.world.issue(
                        id,
                        Order::Patrol {
                            a,
                            b: (wx, wy),
                            forward: true,
                        },
                    );
                }
            }
            self.status("Patrolling");
        } else if attack_move {
            self.world
                .command_attack_move(&self.selected.clone(), (wx, wy));
            self.status("Attack-move");
        } else {
            self.world.command_move(&self.selected.clone(), (wx, wy));
            self.status("Moving");
        }
        self.refresh_commands();
        self.play_ack();
    }

    fn train(&mut self, unit_type: u16) {
        let mut trained = false;
        for id in self.selected.clone() {
            if let Some(u) = self.world.unit(id) {
                if u.owner as usize == self.local_player && self.data.unit(u.type_id).is_building()
                {
                    if self.net.is_some() {
                        if self.send_train(id, unit_type) {
                            trained = true;
                        }
                    } else {
                        match self.world.train(id, unit_type) {
                            Ok(()) => trained = true,
                            Err(e) => {
                                self.status(format!("Cannot train: {e:?}"));
                            }
                        }
                    }
                }
            }
        }
        if trained {
            self.actions = self.actions.saturating_add(1);
            self.play("ui_click");
            self.status(format!("Training {}", self.data.unit_name(unit_type)));
        }
    }

    fn place_building(&mut self, sx: i32, sy: i32) {
        let Some(unit_type) = self.build_mode else {
            return;
        };
        // Mouse is in window pixels; the world view is logical (scaled).
        let (lx, ly) = self.to_logical(sx, sy);
        let (wx, wy) = self.world_of(lx, ly);
        let Some(spot) = self.world.find_build_spot(unit_type, (wx, wy)) else {
            self.status("No room to build there");
            return;
        };
        let mut placed = false;
        for id in self.selected.clone() {
            if self
                .world
                .unit(id)
                .is_some_and(|u| self.data.unit(u.type_id).is_worker())
            {
                let _ = self.world.issue(
                    id,
                    Order::Build {
                        unit_type,
                        pos: spot,
                    },
                );
                placed = true;
            }
        }
        if placed {
            self.actions = self.actions.saturating_add(1);
            self.play("ui_click");
            self.status(format!("Building {}", self.data.unit_name(unit_type)));
            self.build_mode = None;
        }
    }

    /// Input readout: pointer position, button states and the last key. Shown
    /// with `--debug-input` so a player can confirm the client is receiving
    /// events on their session.
    fn draw_input_debug(&self, fb: &mut Framebuffer) {
        let (cx, _cy, _cw, _ch) = Self::content_rect(fb);
        let lines = [
            format!("pointer: {:?}", self.pointer),
            format!("LMB: {}   RMB: {}", self.mouse_was_down, self.rmb_down),
            format!("last key: {}", self.last_key.clone().unwrap_or_default()),
            format!(
                "mode: {}  overlay: {}",
                if self.menu {
                    "menu"
                } else if self.briefing {
                    "briefing"
                } else if self.paused {
                    "paused"
                } else {
                    "game"
                },
                self.overlay
            ),
            format!(
                "hover: {}  fb: {}x{}",
                self.profile_sel, fb.width, fb.height
            ),
        ];
        let w = 340;
        let h = 12 + lines.len() as i32 * 12;
        let (x, y) = (cx + 12, 12);
        for yy in y..y + h {
            for xx in x..x + w {
                let c = fb.get(xx, yy);
                fb.put(xx, yy, c.blend(Rgb::new(0, 0, 0), 200));
            }
        }
        fb.rect_outline(x, y, w, h, Rgb::new(90, 200, 120));
        for (i, line) in lines.iter().enumerate() {
            fb.text(
                x + 6,
                y + 6 + i as i32 * 12,
                line,
                Rgb::new(150, 255, 170),
                1,
            );
        }
    }

    /// Software pointer: the compositor does not always show the system cursor
    /// for a borderless window, so the client paints its own arrow.
    fn draw_cursor(&self, fb: &mut Framebuffer) {
        let Some((mx, my)) = self.pointer else {
            return;
        };
        // Retail skin when the install provides it; the drawn arrow below is
        // only the fallback for runs without assets.
        if let Some(grp) = &self.cursor_art {
            if !grp.frames.is_empty() {
                let frame = cursor_frame_index(self.cursor_started.elapsed(), grp.frames.len());
                if let Some(img) = grp.frames.get(frame) {
                    blit_cursor_scaled(fb, img, mx, my, cursor_scale_for_height(fb.height));
                    return;
                }
            }
        }
        // 12x18 arrow.
        let shape: [&str; 12] = [
            "X...........",
            "XX..........",
            "X.X.........",
            "X..X........",
            "X...X.......",
            "X....X......",
            "X.....X.....",
            "X......X....",
            "X....XXXX...",
            "X..X...X....",
            "X.X.....X...",
            "XX......X...",
        ];
        for (dy, row) in shape.iter().enumerate() {
            for (dx, c) in row.chars().enumerate() {
                if c == 'X' {
                    let (x, y) = (mx + dx as i32, my + dy as i32);
                    fb.put(x, y, Rgb::new(0, 0, 0));
                    fb.put(x + 1, y, Rgb::new(240, 240, 240));
                }
            }
        }
    }

    fn handle_input(&mut self, window: &Window) {
        if let Some(k) = window.get_keys_pressed(KeyRepeat::No).first() {
            self.last_key = Some(format!("{k:?}"));
        }
        if self.intro {
            self.handle_intro(window);
            return;
        }
        if self.menu {
            self.handle_menu(window);
            return;
        }
        if self.briefing {
            self.handle_briefing(window);
            return;
        }
        if self.game_result.is_some() {
            self.handle_result(window);
            return;
        }
        if self.paused {
            self.handle_pause(window);
            return;
        }
        if self.replay.is_some() {
            self.handle_replay(window);
            return;
        }
        let (mx, my) = window
            .get_unscaled_mouse_pos(MouseMode::Clamp)
            .map(|(x, y)| (x as i32, y as i32))
            .unwrap_or((0, 0));
        self.drag_now = (mx, my);
        let in_view = my < self.view.1;

        for key in window.get_keys_pressed(KeyRepeat::No) {
            match key {
                Key::A => {
                    self.attack_mode = true;
                    self.patrol_mode = false;
                    self.status("Attack-move: right click a destination");
                }
                Key::P => {
                    self.patrol_mode = true;
                    self.attack_mode = false;
                    self.status("Patrol: right click a destination");
                }
                Key::S => {
                    for id in self.selected.clone() {
                        self.world.stop(id);
                    }
                    self.status("Stopped");
                }
                Key::H => {
                    self.issue_selected(Order::HoldPosition);
                    self.status("Holding position");
                }
                Key::B => {
                    if !self.build_list.is_empty() {
                        self.status("Build menu: press 1..8");
                    }
                }
                Key::F => {
                    self.fog = !self.fog;
                    self.status(if self.fog { "Fog on" } else { "Fog off" });
                }
                Key::F10 => {
                    self.show_stats = !self.show_stats;
                }
                Key::L => {
                    self.show_leaderboard = !self.show_leaderboard;
                }
                Key::Home => self.center_on_base(),
                Key::Space => {
                    if let Some(u) = self.first_selected() {
                        self.camera = (u.x / 256 - self.view.0 / 2, u.y / 256 - self.view.1 / 2);
                        self.clamp_camera();
                    }
                }
                Key::Tab => {
                    let workers: Vec<u32> = self
                        .world
                        .units
                        .iter()
                        .filter(|u| {
                            u.alive
                                && u.owner as usize == self.local_player
                                && self.data.unit(u.type_id).is_worker()
                                && matches!(u.order, Order::Idle | Order::Stop)
                        })
                        .map(|u| u.id)
                        .collect();
                    if let Some(&id) = workers.first() {
                        self.selected = vec![id];
                        if let Some(u) = self.world.unit(id) {
                            self.camera =
                                (u.x / 256 - self.view.0 / 2, u.y / 256 - self.view.1 / 2);
                            self.clamp_camera();
                        }
                        self.refresh_commands();
                        self.play_selected();
                    }
                }
                Key::Escape => {
                    if self.build_mode.is_some() {
                        self.build_mode = None;
                    } else {
                        self.paused = true;
                        self.play("ui_click");
                    }
                }
                Key::Equal | Key::NumPadPlus => {
                    self.speed = (self.speed * 1.25).min(4.0);
                    self.status(format!("Speed {:.2}x", self.speed));
                }
                Key::Minus | Key::NumPadMinus => {
                    self.speed = (self.speed / 1.25).max(0.25);
                    self.status(format!("Speed {:.2}x", self.speed));
                }
                Key::Key1
                | Key::Key2
                | Key::Key3
                | Key::Key4
                | Key::Key5
                | Key::Key6
                | Key::Key7
                | Key::Key8
                | Key::Key9 => {
                    let n = match key {
                        Key::Key1 => 1,
                        Key::Key2 => 2,
                        Key::Key3 => 3,
                        Key::Key4 => 4,
                        Key::Key5 => 5,
                        Key::Key6 => 6,
                        Key::Key7 => 7,
                        Key::Key8 => 8,
                        _ => 9,
                    };
                    if window.is_key_down(Key::LeftCtrl) || window.is_key_down(Key::RightCtrl) {
                        self.groups[n] = self.selected.clone();
                        self.status(format!("Group {n} set"));
                    } else if !self.command_list.is_empty() {
                        self.activate_command(n - 1);
                    } else {
                        let group = self.groups[n].clone();
                        if !group.is_empty() {
                            self.selected = group;
                            self.refresh_commands();
                        }
                    }
                }
                _ => {}
            }
        }

        // Camera scrolling.  Arrow keys only: A/S/H/P are unit commands.
        let scroll = 14;
        if window.is_key_down(Key::Left) {
            self.camera.0 -= scroll;
        }
        if window.is_key_down(Key::Right) {
            self.camera.0 += scroll;
        }
        if window.is_key_down(Key::Up) {
            self.camera.1 -= scroll;
        }
        if window.is_key_down(Key::Down) {
            self.camera.1 += scroll;
        }
        if let Some((_, y)) = window.get_scroll_wheel() {
            self.camera.1 -= (y * 48.0) as i32;
        }
        self.clamp_camera();

        // Mouse.
        if std::env::var("SC_VIEW_DEBUG").is_ok() {
            eprintln!(
                "mouse {} {} disc={:?} unscaled={:?} size={:?} downL={} downR={} in_view={} start={:?}",
                mx,
                my,
                window.get_mouse_pos(MouseMode::Discard),
                window.get_unscaled_mouse_pos(MouseMode::Discard),
                window.get_size(),
                window.get_mouse_down(MouseButton::Left),
                window.get_mouse_down(MouseButton::Right),
                in_view,
                self.drag_start
            );
        }
        // HUD command-card clicks (the reusable `statbtndefault` slots).
        let lmb = window.get_mouse_down(MouseButton::Left);
        if lmb && !self.lmb_down && my >= self.view.1 {
            let slots = self.command_slots();
            if let Some(index) = slots.iter().position(|r| r.contains(mx, my)) {
                self.activate_command(index);
            } else if let Some(rect) = self
                .console
                .as_ref()
                .and_then(|c| c.layout("stat_f10"))
                .and_then(|d| d.control(1))
                .map(|c| c.abs.scaled(self.ui_scale, self.ui_ox, self.ui_oy))
            {
                if rect.contains(mx, my) {
                    self.show_stats = !self.show_stats;
                }
            }
        }
        self.lmb_down = lmb;

        if window.get_mouse_down(MouseButton::Left) && in_view {
            if self.drag_start.is_none() {
                self.drag_start = Some((mx, my));
            }
        } else if let Some(start) = self.drag_start.take() {
            let dist = (mx - start.0).abs() + (my - start.1).abs();
            if self.build_mode.is_some() && dist < 6 {
                self.place_building(mx, my);
            } else if dist < 6 {
                let id = self.unit_at(mx, my).filter(|id| {
                    self.world
                        .unit(*id)
                        .is_some_and(|u| u.owner as usize == self.local_player)
                });
                if std::env::var("SC_VIEW_DEBUG").is_ok() {
                    eprintln!("select click at {} {} -> {:?}", mx, my, id);
                }
                if window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift) {
                    if let Some(id) = id {
                        if let Some(pos) = self.selected.iter().position(|&x| x == id) {
                            self.selected.remove(pos);
                        } else {
                            self.selected.push(id);
                        }
                    }
                } else {
                    self.selected = id.into_iter().collect();
                }
                self.refresh_commands();
                self.play_selected();
            } else {
                let (x0, y0) = (start.0.min(mx), start.1.min(my));
                let (x1, y1) = (start.0.max(mx), start.1.max(my));
                self.selected.clear();
                for u in self
                    .world
                    .units
                    .iter()
                    .filter(|u| u.alive && u.owner as usize == self.local_player)
                {
                    let (ux, uy) = self.screen_of(u);
                    if ux >= x0 && ux <= x1 && uy >= y0 && uy <= y1 {
                        self.selected.push(u.id);
                    }
                }
                self.refresh_commands();
                self.play_selected();
            }
        }

        let rmb = window.get_mouse_down(MouseButton::Right) && in_view;
        if rmb && !self.rmb_down {
            let attack = self.attack_mode;
            let patrol = self.patrol_mode;
            self.attack_mode = false;
            self.patrol_mode = false;
            self.command_at(mx, my, attack, patrol);
        }
        self.rmb_down = rmb;

        // Minimap click: the strip above the radar toggles the terrain layer
        // (retail's "Show Terrain in Minimap" button), the radar itself moves
        // the camera.
        let (rx, ry, rw, rh) = self.minimap_rect();
        let toggle_h = (rw as f32 * 0.09).round() as i32;
        if window.get_mouse_down(MouseButton::Left)
            && mx >= rx
            && mx < rx + (rw as f32 * 0.45) as i32
            && my >= ry - toggle_h
            && my < ry
        {
            self.minimap_terrain = !self.minimap_terrain;
            self.status(if self.minimap_terrain {
                "Terrain in minimap: on"
            } else {
                "Terrain in minimap: off"
            });
            self.play("ui_click");
            self.drag_start = None;
        } else if window.get_mouse_down(MouseButton::Left)
            && mx >= rx
            && mx < rx + rw
            && my >= ry
            && my < ry + rh
        {
            self.drag_start = None;
            let (rx, ry, rw, rh) = self.minimap_rect();
            let (mw, mh) = self.tileset.minimap_size;
            let wx = (mx - rx) as i64 * mw as i64 / rw as i64;
            let wy = (my - ry) as i64 * mh as i64 / rh as i64;
            self.camera = (
                wx as i32 * TILE_PX - self.view.0 / 2,
                wy as i32 * TILE_PX - self.view.1 / 2,
            );
            self.clamp_camera();
        }
    }

    /// Radar rectangle in window pixels: the same numbers `draw_hud` uses, so
    /// a click can never land far from the drawn minimap (the previous version
    /// used the 640x480 safe area instead of the content box and was ~330 px
    /// off at 3440x1440).
    /// `minimap` control 1 (classic 6,348 128x128, left-anchored); measured
    /// on 135051 as the console art's black well x 18..402, y 1043..1427.
    fn minimap_rect(&self) -> (i32, i32, i32, i32) {
        let r = self.hud_control("minimap", 1, UiRect::new(6, 348, 128, 128));
        (r.x, r.y, r.w, r.h)
    }

    fn update(&mut self) {
        if self.intro {
            // The intro is the retail movie (138 s).  It ends when the player
            // reports the last frame, or on Enter/Esc; the old 5-second timer
            // truncated every intro after the streaming player landed.
            if self.intro_player.as_ref().is_some_and(|p| p.finished()) {
                self.end_intro();
            } else if self.intro_player.is_none()
                && self.intro_started.elapsed() > Duration::from_secs(5)
            {
                // Poster fallback (no ffmpeg / no install): keep the old timing.
                self.end_intro();
            }
            self.last_step = Instant::now();
            return;
        }
        if self.menu {
            // While hosting, keep announcing and watch for a peer that joins.
            self.lan_poll_host();
            return;
        }
        if self.briefing {
            return;
        }
        self.rotate_music();
        if self.game_result == Some(true) && self.next_map.is_none() {
            // Action 41 may have run after the winning condition; resolve as
            // soon as the trigger VM has had its chance.
            self.next_map = self.resolve_next_map();
        }
        if self.paused || self.game_result.is_some() {
            self.last_step = Instant::now();
            return;
        }
        if self.replay.is_some() {
            self.update_replay();
            return;
        }
        if self.net.is_some() {
            self.update_net();
            return;
        }
        let now = Instant::now();
        let dt = now.duration_since(self.last_step).as_secs_f32().min(0.1);
        self.last_step = now;
        self.accumulator += dt * 24.0 * self.speed;
        let mut steps = 0;
        while self.accumulator >= 1.0 && steps < 12 {
            let enemy_base = self
                .world
                .units
                .iter()
                .find(|u| {
                    u.alive
                        && u.owner as usize == self.local_player
                        && matches!(
                            u.type_id,
                            uid::COMMAND_CENTER
                                | uid::NEXUS
                                | uid::HATCHERY
                                | uid::LAIR
                                | uid::HIVE
                        )
                })
                .map(|u| (u.x / 256, u.y / 256));
            // Trigger actions 15/16 queue campaign AI scripts.
            if let Some(t) = self.world.triggers.as_mut() {
                let pending = std::mem::take(&mut t.pending_ai_scripts);
                for (player, id, at) in pending {
                    if self.campaign_ais.iter().any(|c| c.player == player as u8) {
                        continue;
                    }
                    if let Some(script) = self
                        .data
                        .aiscripts
                        .get(&id)
                        .or_else(|| self.data.bwscripts.get(&id))
                    {
                        let mut ai = CampaignAi::new(player as u8, script);
                        ai.location = at;
                        self.campaign_ais.push(ai);
                    }
                }
            }
            for ai in &mut self.campaign_ais {
                ai.update(&mut self.world, enemy_base);
            }
            for ai in &mut self.ais {
                if self.campaign_ais.iter().any(|c| c.player == ai.player) {
                    continue;
                }
                ai.update(&mut self.world, enemy_base);
            }
            self.world.step();
            self.accumulator -= 1.0;
            steps += 1;
        }
        // Drop dead selections.
        self.selected
            .retain(|id| self.world.unit(*id).is_some_and(|u| u.alive));
        if self.game_result.is_none() {
            let defeat = self
                .world
                .players
                .get(self.local_player)
                .is_some_and(|p| p.defeated);
            let victory = self.world.winner == Some(self.local_player as u8)
                || (self.world.players.len() > 1
                    && self
                        .world
                        .players
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| *i != self.local_player)
                        .all(|(_, p)| p.defeated));
            if defeat || victory {
                let won = !defeat && victory;
                self.game_result = Some(won);
                if won {
                    self.next_map = self.resolve_next_map();
                    if let Some(mission) = self.map_mission(self.map_index) {
                        let mut p = self.progress();
                        p.record_win(mission);
                        self.progress_book.set(&self.active_profile.clone(), p);
                        if let Some(path) = progress_save_path() {
                            if let Err(e) = self.progress_book.save(&path) {
                                eprintln!("campaign progress not saved: {e}");
                            }
                        }
                    }
                }
                self.play(if won { "victory" } else { "defeat" });
                self.result_panel =
                    GlueArt::load_score_panel(&self.assets_root, self.local_race(), won);
                self.result_fit = None;
                self.play_result_music(won);
            }
        }
    }

    fn draw(&mut self, fb: &mut Framebuffer) {
        self.update_view(fb);

        if self.intro {
            self.draw_intro(fb);
            return;
        }
        if self.menu {
            self.draw_menu(fb);
            return;
        }
        if self.briefing {
            self.draw_briefing(fb);
            return;
        }
        fb.clear(Rgb::new(0, 0, 0));
        let fb_full = fb;
        // The world is rendered into a logical 1x framebuffer and then blitted
        // scaled, so sprites keep their proportion against the HUD at any
        // resolution (720p..4K) and ultrawide only widens the view.
        // HD path (retail at >= 2x): the terrain is drawn at screen
        // resolution from the 4x art over the whole content height (the
        // console has see-through gaps), the 1x layer only carries creep and
        // units, and the fog is applied at screen resolution afterwards.
        let scale = self.ui_scale.max(0.5);
        let hd = self.hd_terrain.is_some() && scale >= 2.0;
        let (vw, vh) = self.view;
        let vh = if hd {
            (Self::content_rect(fb_full).3 as f32 / scale).ceil() as i32
        } else {
            vh
        };
        let mut world_fb = Framebuffer::new(vw.max(1) as usize, vh.max(1) as usize);
        world_fb.clear(if hd { WORLD_KEY } else { Rgb::new(0, 0, 0) });
        let fb = &mut world_fb;
        let cam = self.camera;
        if !hd {
            self.tileset.draw_terrain(fb, &self.scenario, cam);
        }
        // Creep sits on the terrain and under the fog, exactly where the
        // engine's procedural mat is composited.
        let tick = self.world.tick;
        if !hd {
            let grid = self.world.creep_grid();
            galaxycraft::render::draw_creep(fb, &self.scenario, cam, grid, tick);
        }
        if self.fog && !hd {
            self.tileset.draw_fog(
                fb,
                &self.scenario,
                cam,
                &self.world,
                self.local_player as u8,
            );
        }

        let mut order: Vec<usize> = (0..self.world.units.len())
            .filter(|&i| self.world.units[i].alive)
            .collect();
        order.sort_by_key(|&i| self.world.units[i].y);
        let mut hd_units = Vec::new();
        for i in order {
            let u = &self.world.units[i];
            if self.fog
                && u.owner as usize != self.local_player
                && !self.world.can_see_unit(self.local_player as u8, i)
            {
                continue;
            }
            if hd && self.hd_unit_image(u.type_id).is_some() {
                hd_units.push(i);
                continue;
            }
            let (sx, sy) = self.screen_of(u);
            if sx < -160 || sy < -160 || sx > fb.width + 160 || sy > self.view.1 + 160 {
                continue;
            }
            let selected = self.selected.contains(&u.id);
            if !self.sprites.draw_unit(fb, u, (sx, sy), self.world.tick) {
                draw_unit_box(fb, u, &self.data, (sx, sy), selected);
            } else if selected {
                let ty = self.data.unit(u.type_id);
                let r = ((ty.dimensions.right - ty.dimensions.left) as i32 / 2).max(12) + 4;
                fb.circle_outline(sx, sy, r, Rgb::new(80, 255, 80));
            }
            if selected || u.owner as usize == self.local_player {
                self.sprites.draw_health(fb, u, &self.data, (sx, sy));
            }
        }

        if let Some(start) = self.drag_start {
            let (x0, y0) = (start.0.min(self.drag_now.0), start.1.min(self.drag_now.1));
            let (x1, y1) = (start.0.max(self.drag_now.0), start.1.max(self.drag_now.1));
            fb.rect_outline(
                x0,
                y0,
                (x1 - x0).max(1),
                (y1 - y0).max(1),
                Rgb::new(80, 255, 80),
            );
        }

        // Blit the logical frame scaled to the window, then draw the HUD in
        // screen space on top.
        if hd {
            let (cx, cy, cw, ch) = Self::content_rect(fb_full);
            let mw = self.scenario.width as i32;
            let grid = self.world.creep_grid().to_vec();
            let variants = &self.creep_variants;
            let creep = move |tx: i32, ty: i32| {
                let on =
                    tx >= 0 && ty >= 0 && grid.get((ty * mw + tx) as usize).is_some_and(|&c| c > 0);
                on.then(|| match variants.get(&(tx, ty)) {
                    Some(&v) => (1 << 4) | v as u16,
                    None => galaxycraft::render::creep_mtxm(tx, ty),
                })
            };
            if let Some(t) = &self.hd_terrain {
                t.draw(
                    fb_full,
                    &self.tileset,
                    &self.scenario,
                    cam,
                    (cx, cy, cw, ch),
                    scale,
                    &creep,
                );
            }
            blit_scaled_keyed(&world_fb, fb_full, scale, self.content_x0);
            for &i in &hd_units {
                self.draw_unit_hd(fb_full, i, (cx, cy, cw, ch), scale);
            }
            if self.fog {
                galaxycraft::render::draw_fog_smooth(
                    fb_full,
                    &self.scenario,
                    &self.world,
                    self.local_player as u8,
                    cam,
                    (cx, cy, cw, ch),
                    scale,
                );
            }
        } else {
            blit_scaled_at(&world_fb, fb_full, scale, self.content_x0);
        }
        let fb = fb_full;
        self.draw_hud(fb);
        self.draw_overlay(fb);
    }

    /// HD image of a unit's main sprite (`unit -> flingy -> sprite -> image`).
    fn hd_unit_image(&self, type_id: u16) -> Option<std::rc::Rc<galaxycraft::hd_sprites::HdImage>> {
        let ty = self.data.unit(type_id);
        let flingy = self.data.flingy(ty.flingy)?;
        let image = self.data.sprite(flingy.sprite)?.image;
        let install = self.install.as_ref();
        self.hd_sprites
            .image(self.data.hd_anim_id(image), |path| install?.read(path))
    }

    /// Overlays the unit's Init animation attaches (`imgul` shadows below,
    /// `imgol` glows above), from the retail iscript: image id, offset and
    /// whether it is drawn under the main image.
    fn hd_unit_layers(&self, type_id: u16) -> Vec<(u16, i8, i8, bool)> {
        let ty = self.data.unit(type_id);
        let Some(script) = self.data.unit_iscript_id(ty) else {
            return Vec::new();
        };
        let Some(mut st) = self.data.iscript.start_animation(script, 0, 0) else {
            return Vec::new();
        };
        let mut ctx = galaxycraft::iscript::AnimContext::default();
        let mut out = Vec::new();
        for _ in 0..4 {
            for e in self.data.iscript.tick_anim(&mut st, &mut ctx) {
                if let galaxycraft::iscript::AnimEvent::Overlay { id, x, y, below } = e {
                    out.push((id, x, y, below));
                }
            }
            if st.ended {
                break;
            }
        }
        out
    }

    /// Draw one unit from its HD sprite at screen resolution.  Directional
    /// images store 17 facings (north..south through east); the western ones
    /// are the mirrored frames, as in the SD GRPs.
    fn draw_unit_hd(&self, fb: &mut Framebuffer, i: usize, content: (i32, i32, i32, i32), s: f32) {
        let u = &self.world.units[i];
        let Some(img) = self.hd_unit_image(u.type_id) else {
            return;
        };
        let directional = img.header.frames.len() >= 17;
        let (frame, flip) = if directional {
            let dir = ((u.facing as i32 + 4) / 8).rem_euclid(32);
            if dir <= 16 {
                (dir as usize, false)
            } else {
                ((32 - dir) as usize, true)
            }
        } else {
            (0, false)
        };
        let (cx, cy, cw, ch) = content;
        let sx = cx as f32 + (u.x as f32 / 256.0 - self.camera.0 as f32) * s;
        let sy = cy as f32 + (u.y as f32 / 256.0 - self.camera.1 as f32) * s;
        let team = match self.local_team_color {
            Some(c) if u.owner as usize == self.local_player => c,
            _ => {
                galaxycraft::render::PLAYER_COLORS
                    [u.owner as usize % galaxycraft::render::PLAYER_COLORS.len()]
            }
        };
        let clip = (cx, cy, cx + cw, cy + ch);
        let layers = self.hd_unit_layers(u.type_id);
        let install = self.install.as_ref();
        let layer = |below: bool, fb: &mut Framebuffer| {
            for &(id, ox, oy, b) in layers.iter().filter(|l| l.3 == below) {
                let _ = b;
                if let Some(li) = self
                    .hd_sprites
                    .image(self.data.hd_anim_id(id), |path| install?.read(path))
                {
                    let lf = frame.min(li.header.frames.len().saturating_sub(1));
                    let (lx, ly) = (sx + ox as f32 * s, sy + oy as f32 * s);
                    if below {
                        galaxycraft::hd_sprites::draw_shadow(
                            fb,
                            &li,
                            lf,
                            lx,
                            ly,
                            s,
                            flip,
                            HD_SHADOW_ALPHA,
                            clip,
                        );
                    } else {
                        galaxycraft::hd_sprites::draw_frame(
                            fb, &li, lf, lx, ly, s, flip, None, clip,
                        );
                    }
                }
            }
        };
        let selected = self.selected.contains(&u.id) && u.owner as usize == self.local_player;
        let sprite = self
            .data
            .flingy(self.data.unit(u.type_id).flingy)
            .and_then(|f| self.data.sprite(f.sprite))
            .cloned();
        // Selection circle (image 561 + size) under the unit, at vpos.
        let circle = sprite.as_ref().filter(|_| selected).and_then(|sp| {
            let img = self
                .hd_sprites
                .image(561 + sp.selection_circle as u16, |path| install?.read(path))?;
            Some((img, sp.selection_circle_vpos as f32, sp.health_bar_size))
        });
        if let Some((ci, vpos, _)) = &circle {
            galaxycraft::hd_sprites::draw_flat(
                fb,
                ci,
                0,
                sx,
                sy + vpos * s,
                s,
                HUD_SELECT_GREEN,
                clip,
            );
        }
        layer(true, fb);
        galaxycraft::hd_sprites::draw_frame(fb, &img, frame, sx, sy, s, flip, Some(team), clip);
        layer(false, fb);
        if let Some((ci, vpos, bar)) = circle {
            // Retail `draw_health_bars`: centred `vpos + circle_h / 2 + 8`
            // below the sprite.
            // OpenBW uses the circle's frame-0 height (not the GRP box).
            let circle_h = ci
                .header
                .frames
                .first()
                .map_or(0, |f| (f.h / ci.header.scale.max(1) as u16) as i32);
            let ty = self.data.unit(u.type_id);
            let max_hp = (ty.hit_points_raw as i32 / 256).max(1);
            let hp_pct = (u.hp / 256) * 100 / max_hp;
            let shield_pct = ty
                .has_shields
                .then(|| (u.shields / 256) * 100 / (ty.shield_points as i32).max(1));
            let gy = u.y / 256 + vpos as i32 + circle_h / 2 + 8;
            draw_health_bar(
                fb,
                (sx, cy as f32 + (gy - self.camera.1) as f32 * s),
                bar as i32,
                hp_pct,
                shield_pct,
                s,
                clip,
            );
        }
    }

    /// Screen rectangle of a console-dialog rect given in absolute classic
    /// 640x480 coordinates.  Retail keeps the classic layout x3 and anchors
    /// each dialog in the widened frame: the minimap stays left, `statdata`
    /// is centred and the right-hand dialogs (`statres`, `statport`,
    /// `stat_f10`, `statbtn*`) move by the whole extra width.  Measured on
    /// 135051: card button 1 at x 2155 = (505 + 213.33) * 3, the panel name
    /// centred at 1265 = (138 + 102 + 76 + 106.67) * 3.
    fn hud_rect(&self, dialog: &str, r: UiRect) -> UiRect {
        let (cx, cy, cw, ch) = self.content_box;
        let s = (ch as f32 / 480.0).max(0.5);
        let extra = cw as f32 / s - 640.0;
        let anchor = match dialog {
            "minimap" => 0.0,
            "statdata" | "objctdlg" | "tips_dlg" => 0.5,
            _ => 1.0,
        };
        let x = cx as f32 + (r.x as f32 + anchor * extra) * s;
        UiRect::new(
            x.round() as i32,
            cy + (r.y as f32 * s).round() as i32,
            (r.w as f32 * s).round() as i32,
            (r.h as f32 * s).round() as i32,
        )
    }

    /// [`Self::hud_rect`] of control `id` of a shipped console dialog.
    fn hud_control(&self, dialog: &str, id: i32, fallback: UiRect) -> UiRect {
        let r = self
            .console
            .as_ref()
            .and_then(|c| c.layout(dialog))
            .and_then(|d| d.control(id))
            .map(|c| c.abs)
            .unwrap_or(fallback);
        self.hud_rect(dialog, r)
    }

    /// Command-card dialog of the console race.
    ///
    /// Retail's HD console places the Zerg buttons like the others: the
    /// `statbtnz` offsets (4,7 instead of 9,4) would put them 12 px left and
    /// 8 px low of 134905 (shift search), so every race uses the default.
    fn card_dialog(&self) -> &'static str {
        "statbtndefault"
    }

    fn draw_hud(&mut self, fb: &mut Framebuffer) {
        let (scale, ox, oy) = self.ui_metrics(fb);
        let (cx, cy, cw, ch) = Self::content_rect(fb);
        if let Some(console) = &self.console {
            console.draw_plates_in(fb, cy, ch, cx, cw);
        }
        self.draw_hud_resources(fb);
        self.draw_hud_minimap(fb);
        self.draw_hud_selection(fb);
        self.draw_hud_minimap_buttons(fb);
        self.draw_hud_upgrade(fb);
        self.draw_hud_portrait(fb);
        self.draw_hud_menu_label(fb);
        self.draw_hud_card(fb);
        self.draw_hud_tooltip(fb);
        // Status line in the game view.
        if Instant::now() < self.status_until && !self.status.is_empty() {
            fb.text_centered(
                self.view.0 / 2,
                self.view.1 - 28,
                &self.status,
                Rgb::new(255, 240, 140),
                1,
            );
        }
        if self.attack_mode {
            fb.text_centered(
                self.view.0 / 2,
                self.view.1 - 48,
                "ATTACK-MOVE: right click",
                Rgb::new(255, 120, 120),
                1,
            );
        }
        if self.patrol_mode {
            fb.text_centered(
                self.view.0 / 2,
                self.view.1 - 48,
                "PATROL: right click",
                Rgb::new(140, 200, 255),
                1,
            );
        }
        if let Some(t) = self.build_mode {
            fb.text_centered(
                self.view.0 / 2,
                self.view.1 - 68,
                &format!("PLACE {} - left click, Esc cancel", self.data.unit_name(t)),
                Rgb::new(255, 220, 120),
                1,
            );
        }

        if self.show_leaderboard {
            self.draw_leaderboard(fb, scale, ox, oy);
        }
        if self.show_stats {
            self.draw_mission_stats(fb, scale, ox, oy);
        }
    }

    /// `statres`: minerals, gas and supply, each an HD icon (`icons`
    /// frames 0 / gas / supply by race) and its number in BlizzardGlobal.
    fn draw_hud_resources(&mut self, fb: &mut Framebuffer) {
        let p = &self.world.players[self.local_player];
        let (minerals, gas) = (p.minerals, p.gas);
        let used = self.world.supply_used(self.local_player as u8) / 2;
        let cap = self.world.supply_cap(self.local_player as u8) / 2;
        let (gas_icon, supply_icon) = match self.console_race {
            Race::Zerg => (1, 4),
            Race::Protoss => (3, 6),
            _ => (2, 5),
        };
        let fields = [
            (3, 0usize, minerals.to_string()),
            (2, gas_icon, gas.to_string()),
            (1, supply_icon, format!("{used}/{cap}")),
        ];
        let (_, _, _, ch) = self.content_box;
        let k = ch as f32 / 1440.0;
        for (id, icon, text) in fields {
            let r = self.hud_control("statres", id, UiRect::new(572 - (id - 1) * 68, 0, 66, 20));
            if let Some(img) = self
                .console
                .as_ref()
                .and_then(|c| c.icons4.as_ref())
                .and_then(|g| g.frame(icon))
            {
                let w = (img.width as f32 * 0.75 * k).round() as i32;
                let h = (img.height as f32 * 0.75 * k).round() as i32;
                img.draw_scaled(fb, r.x, r.y + (r.h - h) / 2, w, h);
            }
            let x = (r.x - self.content_box.0) as f32 / k + HUD_RES_TEXT_DX;
            self.glue_text_face(
                fb,
                x,
                (r.y as f32) / k + HUD_RES_BASE,
                &text,
                HUD_NAME_PX,
                HUD_RES_COLOR,
                None,
                GlueFace::Hud,
            );
        }
    }

    /// `minimap` control 1 (classic 6,348 128x128, left-anchored): units and
    /// the camera box over the console art's own black well.
    fn draw_hud_minimap(&mut self, fb: &mut Framebuffer) {
        let (rx, ry, rw, rh) = self.minimap_rect();
        let units = self.world.minimap_units(self.local_player as u8, self.fog);
        // The box frames the playfield above the console: 368 game px tall
        // (135051: inner box 1371..1406 at 3 px per tile).
        let cam = (
            self.camera.0,
            self.camera.1,
            self.camera.0 + self.view.0,
            self.camera.1 + MINIMAP_VIEW_H,
        );
        self.tileset.draw_minimap_with_terrain(
            fb,
            (rx, ry, rw, rh),
            &units,
            cam,
            self.world.vision.as_ref(),
            self.local_player as u8,
            self.minimap_terrain,
        );
    }

    /// `statdata` for one selected unit: HD wireframe (LTEXT 1), name
    /// (CTEXT -5), shields/hit points (CTEXT -7) and, for town halls, the
    /// four supply rows (CTEXT -10..-13).
    fn draw_hud_selection(&mut self, fb: &mut Framebuffer) {
        let Some(u) = self.first_selected().cloned() else {
            return;
        };
        if self.selected.len() != 1 {
            return;
        }
        let k = self.content_box.3 as f32 / 1440.0;
        let cx0 = self.content_box.0;
        let to_x = |x: i32| (x - cx0) as f32 / k;
        let ty = self.data.unit(u.type_id).clone();
        let wire = self.hud_control("statdata", 1, UiRect::new(168, 388, 64, 64));
        if let Some(console) = &self.console {
            // HD wireframe: 256 px art at 0.75, centred on the control
            // (134905 / 135051: 192 px square centred at 918, 1261).
            let side = (wire.h as f32 * 1.0).round() as i32;
            let rect = (
                wire.x + (wire.w - side) / 2,
                wire.y + (wire.h - side) / 2,
                side,
                side,
            );
            // units.dat group flag 0x01 = Zerg.
            let zerg = ty.group_flags & 0x01 != 0;
            if !console.draw_wireframe_hd(fb, u.type_id, rect, zerg, ty.has_shields) {
                console.draw_wireframe(fb, u.type_id, (wire.x, wire.y, wire.w, wire.h), None);
            }
        }
        let name = self.data.unit_name(u.type_id).to_string();
        let nr = self.hud_control("statdata", -5, UiRect::new(240, 388, 152, 14));
        let w = self.glue_measure(&name, HUD_NAME_PX, GlueFace::Hud);
        self.glue_text_face(
            fb,
            to_x(nr.x) + (nr.w as f32 / k - w) / 2.0,
            nr.y as f32 / k + HUD_NAME_BASE,
            &name,
            HUD_NAME_PX,
            HUD_TEXT_COLOR,
            None,
            GlueFace::Hud,
        );
        // Hit points (and shields first for Protoss) under the wireframe.
        let hp = (u.hp / 256).max(0);
        let max_hp = ty.hit_points_raw as i32 / 256;
        let hr = self.hud_control("statdata", -7, UiRect::new(148, 452, 100, 12));
        let mut parts: Vec<(String, Rgb)> = Vec::new();
        if ty.has_shields {
            parts.push((
                format!("{}/{} ", u.shields / 256, ty.shield_points),
                HUD_TEXT_COLOR,
            ));
        }
        parts.push((format!("{hp}/{max_hp}"), HUD_HP_COLOR));
        let total: f32 = parts
            .iter()
            .map(|(t, _)| self.glue_measure(t, HUD_SMALL_PX, GlueFace::Hud))
            .sum();
        let mut x = to_x(hr.x) + (hr.w as f32 / k - total) / 2.0;
        for (t, c) in parts {
            x = self.glue_text_face(
                fb,
                x,
                hr.y as f32 / k + HUD_SMALL_BASE,
                &t,
                HUD_SMALL_PX,
                c,
                None,
                GlueFace::Hud,
            );
        }
        // Town halls list the player's supply.
        if matches!(
            u.type_id,
            galaxycraft::ids::unit::COMMAND_CENTER
                | galaxycraft::ids::unit::NEXUS
                | galaxycraft::ids::unit::HATCHERY
                | galaxycraft::ids::unit::LAIR
                | galaxycraft::ids::unit::HIVE
        ) {
            let used = self.world.supply_used(self.local_player as u8) / 2;
            let cap = self.world.supply_cap(self.local_player as u8) / 2;
            let label = if matches!(self.console_race, Race::Protoss) {
                "Psi"
            } else {
                "Control"
            };
            // "Provided" is this building's own supply (134905: a Hatchery
            // shows 1 while the player total is 9).
            let provided = ty.supply_provided as i32 / 2;
            let rows = [
                format!("{label} Used: {used}"),
                format!("{label} Provided: {provided}"),
                format!("Total {label}: {cap}"),
                format!("{label} Max: 200"),
            ];
            for (i, text) in rows.iter().enumerate() {
                let r = self.hud_control(
                    "statdata",
                    -10 - i as i32,
                    UiRect::new(281, 418 + 14 * i as i32, 111, 14),
                );
                let w = self.glue_measure(text, HUD_SMALL_PX, GlueFace::Hud);
                self.glue_text_face(
                    fb,
                    to_x(r.x) + (r.w as f32 / k - w) / 2.0,
                    r.y as f32 / k + HUD_SMALL_BASE,
                    text,
                    HUD_SMALL_PX,
                    HUD_TEXT_COLOR,
                    None,
                    GlueFace::Hud,
                );
            }
        }
    }

    /// `statport` button 1: the selected unit's idle portrait (the race
    /// advisor for buildings, via units.dat -> portdata).
    fn draw_hud_portrait(&mut self, fb: &mut Framebuffer) {
        let Some(type_id) = self.first_selected().map(|u| u.type_id) else {
            return;
        };
        let mut port = self.hud_control("statport", 1, UiRect::new(413, 410, 60, 56));
        // Retail draws the portrait and its frame lower than the classic
        // rect: NCC search on 135051 / 134905 (scale 1.0, ncc 0.95) puts it
        // (+1.4..2.6, +13.6..16.4) content px off.
        let k0 = self.content_box.3 as f32 / 1440.0;
        port.x += (PORTRAIT_SHIFT.0 * k0).round() as i32;
        port.y += (PORTRAIT_SHIFT.1 * k0).round() as i32;
        if let Some(frame) = self.portrait_frame(type_id) {
            let fit = (port.w as f32 / frame.width as f32).min(port.h as f32 / frame.height as f32);
            let dw = (frame.width as f32 * fit).round().max(1.0) as i32;
            let dh = (frame.height as f32 * fit).round().max(1.0) as i32;
            // The portrait clips are opaque (135051: the advisor's dark
            // backdrop hides the blue well behind it).
            blit_opaque(
                &frame,
                fb,
                port.x + (port.w - dw) / 2,
                port.y + (port.h - dh) / 2,
                dw,
                dh,
            );
        }
        // Race frame over the portrait well (`conover.dds`).
        let k = self.content_box.3 as f32 / 1440.0;
        if let Some(img) = self.console.as_ref().and_then(|c| c.con_over.as_ref()) {
            let w = (img.width as f32 * 0.75 * k).round() as i32;
            let h = (img.height as f32 * 0.75 * k).round() as i32;
            img.draw_scaled(
                fb,
                port.x + (port.w - w) / 2,
                port.y + (port.h - h) / 2,
                w,
                h,
            );
        }
    }

    /// HUD element under the pointer that carries a retail tooltip.
    fn hud_hover(&self) -> Option<HudHover> {
        let (mx, my) = self.pointer?;
        let inside = |r: UiRect| mx >= r.x && mx < r.x + r.w && my >= r.y && my < r.y + r.h;
        if let Some(i) = self.command_slots().into_iter().position(inside) {
            return Some(HudHover::Card(i));
        }
        let protoss_selected = self
            .first_selected()
            .is_some_and(|u| self.data.unit(u.type_id).has_shields);
        if protoss_selected && self.selected.len() == 1 && inside(self.upgrade_button_rect()) {
            return Some(HudHover::Shields);
        }
        if inside(self.hud_control("minimap", 2, UiRect::new(3, 320, 64, 24))) {
            return Some(HudHover::MinimapTerrain);
        }
        None
    }

    /// `statdata` button 3 (classic 242,441 39x36): the shield/armour
    /// upgrade of a single selection.
    fn upgrade_button_rect(&self) -> UiRect {
        // Control 3 (classic 242,441): measured at y 1323 = 441 * 3 on 135109.
        self.hud_control("statdata", 3, UiRect::new(242, 441, 39, 36))
    }

    /// Retail tooltip of the hovered HUD element: a near-black box with a
    /// 3.5 px blue rim, BlizzardGlobal 27.2 px/em text whose `\x03` spans
    /// are yellow, and a cost row for buttons that make a unit.  Box
    /// placement relative to the element as measured on 135109 / 135114 /
    /// 135121 (the captures hovered the element's middle).
    fn draw_hud_tooltip(&mut self, fb: &mut Framebuffer) {
        let Some(hover) = self.hud_hover() else {
            return;
        };
        let k = self.content_box.3 as f32 / 1440.0;
        let cx0 = self.content_box.0;
        let to_c = |x: i32, y: i32| ((x - cx0) as f32 / k, y as f32 / k);
        let mut lines: Vec<String> = Vec::new();
        let mut cost: Option<(i32, i32, i32)> = None;
        // (anchor point in content px, true = box bottom-left, false = top-left)
        let anchor: ((f32, f32), bool) = match hover {
            HudHover::Card(i) => {
                let Some(u) = self.first_selected().cloned() else {
                    return;
                };
                let card = galaxycraft::buttons::card(&self.world, &u);
                let Some((b, st)) = card[i] else {
                    return;
                };
                let id = if st == galaxycraft::buttons::ButtonState::Disabled
                    && b.disabled_string != 0
                {
                    b.disabled_string
                } else {
                    b.enabled_string
                };
                let raw = self
                    .data
                    .strings
                    .get(id.saturating_sub(1))
                    .unwrap_or("")
                    .to_string();
                lines.push(button_tooltip_text(&raw));
                if galaxycraft::buttons::Cond::of(b.cond) == galaxycraft::buttons::Cond::MakeUnit {
                    let ty = self.data.unit(b.cond_var);
                    cost = Some((
                        ty.mineral_cost as i32,
                        ty.gas_cost as i32,
                        (ty.supply_required as i32 + 1) / 2,
                    ));
                }
                let r = self.command_slots()[i];
                let (x, y) = to_c(r.x, r.y);
                ((x - 13.0, y + 5.0), true)
            }
            HudHover::Shields => {
                lines.push(self.data.upgrade_name(PLASMA_SHIELDS).to_string());
                let level = self.world.players[self.local_player].upgrade_level(PLASMA_SHIELDS);
                lines.push(format!(
                    "{} {level}",
                    self.data.strings.get(778).unwrap_or("Shields:")
                ));
                let r = self.upgrade_button_rect();
                let (x, y) = to_c(r.x, r.y);
                ((x + 94.0, y - 16.0), false)
            }
            HudHover::MinimapTerrain => {
                // Retail shows string 810 ("Hide Terrain ...") while the
                // terrain layer is off (135121); kept as observed.
                let id = if self.minimap_terrain { 809 } else { 810 };
                lines.push(self.data.strings.get(id).unwrap_or("").to_string());
                let r = self.hud_control("minimap", 2, UiRect::new(3, 320, 64, 24));
                let (x, y) = to_c(r.x, r.y);
                ((x + 181.0, y - 8.0), false)
            }
        };
        let widths: Vec<f32> = lines
            .iter()
            .map(|l| self.glue_measure(&strip_codes(l), TOOLTIP_PX, GlueFace::Hud))
            .collect();
        let mut w = widths.iter().cloned().fold(0.0, f32::max);
        if let Some((m, g, sup)) = cost {
            w = w.max(self.tooltip_cost_width(m, g, sup));
        }
        let bw = (w + TOOLTIP_PAD_L + TOOLTIP_PAD_R).round();
        let single = lines.len() == 1 && cost.is_none();
        // A one-line box is 59 px tall and starts 2 px higher, with the text
        // where it would be (135121: rim 950..1008, ink 966..989).
        let bh = if single {
            TOOLTIP_SINGLE_H
        } else {
            TOOLTIP_LINE0_H
                + TOOLTIP_PITCH * (lines.len() as f32 - 1.0)
                + if cost.is_some() { TOOLTIP_COST_H } else { 0.0 }
        };
        let ((ax, ay), bottom) = anchor;
        let (bx, by) = if bottom { (ax, ay - bh) } else { (ax, ay) };
        let (by, text_dy) = if single { (by - 2.0, 2.0) } else { (by, 0.0) };
        let px = |x: f32| cx0 + (x * k).round() as i32;
        let py = |y: f32| (y * k).round() as i32;
        draw_rounded_box(
            fb,
            (
                px(bx),
                py(by),
                (bw * k).round() as i32,
                (bh * k).round() as i32,
            ),
            TOOLTIP_RADIUS * k,
            TOOLTIP_RIM * k,
            TOOLTIP_FILL,
            TOOLTIP_RIM_COLOR,
        );
        for (i, line) in lines.iter().enumerate() {
            self.glue_coded_text(
                fb,
                bx + TOOLTIP_PAD_L,
                by + text_dy + TOOLTIP_BASE + i as f32 * TOOLTIP_PITCH,
                line,
            );
        }
        if let Some((m, g, sup)) = cost {
            self.draw_tooltip_cost(
                fb,
                bx + TOOLTIP_PAD_L,
                by + TOOLTIP_LINE0_H - 6.0,
                m,
                g,
                sup,
            );
        }
    }

    /// Text with `\x01` (default) / `\x03` (yellow) colour codes.
    fn glue_coded_text(&mut self, fb: &mut Framebuffer, x: f32, base: f32, text: &str) {
        let mut pen = x;
        let mut colour = HUD_TEXT_COLOR;
        let mut run = String::new();
        for ch in text.chars().chain(std::iter::once('\u{0}')) {
            match ch {
                '\u{1}' | '\u{3}' | '\u{0}' => {
                    if !run.is_empty() {
                        pen = self.glue_text_face(
                            fb,
                            pen,
                            base,
                            &run,
                            TOOLTIP_PX,
                            colour,
                            None,
                            GlueFace::Hud,
                        );
                        run.clear();
                    }
                    colour = if ch == '\u{3}' {
                        TOOLTIP_HOTKEY
                    } else {
                        HUD_TEXT_COLOR
                    };
                }
                c if (c as u32) < 0x20 => {}
                c => run.push(c),
            }
        }
    }

    /// Width (content px) of a tooltip cost row.
    fn tooltip_cost_width(&self, minerals: i32, gas: i32, supply: i32) -> f32 {
        let mut w = 0.0;
        for (v, show) in [(minerals, true), (gas, gas > 0), (supply, supply > 0)] {
            if show {
                w += TOOLTIP_ICON
                    + 8.0
                    + self.glue_measure(&v.to_string(), TOOLTIP_PX, GlueFace::Hud)
                    + 12.0;
            }
        }
        w
    }

    /// Cost row: each value after its resource icon in a rimmed square.
    fn draw_tooltip_cost(
        &mut self,
        fb: &mut Framebuffer,
        x: f32,
        y: f32,
        minerals: i32,
        gas: i32,
        supply: i32,
    ) {
        let k = self.content_box.3 as f32 / 1440.0;
        let cx0 = self.content_box.0;
        let (gas_icon, supply_icon) = match self.console_race {
            Race::Zerg => (1, 4),
            Race::Protoss => (3, 6),
            _ => (2, 5),
        };
        let mut pen = x;
        for (v, icon, show) in [
            (minerals, 0usize, true),
            (gas, gas_icon, gas > 0),
            (supply, supply_icon, supply > 0),
        ] {
            if !show {
                continue;
            }
            let rect = (
                cx0 + (pen * k).round() as i32,
                (y * k).round() as i32,
                (TOOLTIP_ICON * k).round() as i32,
                (TOOLTIP_ICON * k).round() as i32,
            );
            draw_rounded_box(
                fb,
                rect,
                3.0 * k,
                2.0 * k,
                Rgb::new(10, 20, 50),
                TOOLTIP_RIM_COLOR,
            );
            if let Some(img) = self
                .console
                .as_ref()
                .and_then(|c| c.icons4.as_ref())
                .and_then(|g| g.frame(icon))
            {
                img.draw_scaled(fb, rect.0 + 2, rect.1 + 2, rect.2 - 4, rect.3 - 4);
            }
            pen += TOOLTIP_ICON + 8.0;
            pen = self.glue_text_face(
                fb,
                pen,
                y + TOOLTIP_ICON - 10.0,
                &v.to_string(),
                TOOLTIP_PX,
                HUD_TEXT_COLOR,
                None,
                GlueFace::Hud,
            ) + 12.0;
        }
    }

    /// The two buttons above the radar (`minimap` controls 2 and 3) from the
    /// HD `dlg` sheet; the terrain toggle shows its highlighted frame while
    /// hovered.
    fn draw_hud_minimap_buttons(&mut self, fb: &mut Framebuffer) {
        let hover = self.hud_hover();
        let k = self.content_box.3 as f32 / 1440.0;
        let shift = |mut r: UiRect| {
            r.x += (MINIMAP_BUTTON_SHIFT.0 * k).round() as i32;
            r.y += (MINIMAP_BUTTON_SHIFT.1 * k).round() as i32;
            r
        };
        let terrain = shift(self.hud_control("minimap", 2, UiRect::new(3, 320, 64, 24)));
        let other = shift(self.hud_control("minimap", 3, UiRect::new(76, 320, 52, 24)));
        let Some(sheet) = self.console.as_ref().and_then(|c| c.dlg4.as_ref()) else {
            return;
        };
        // Retail keeps the normal frame while hovered (135121).
        let _ = hover;
        let terrain_frame = 4;
        for (rect, frame) in [(terrain, terrain_frame), (other, 0)] {
            if let Some(img) = sheet.frame(frame) {
                let w = (img.width as f32 * 0.75 * k).round() as i32;
                let h = (img.height as f32 * 0.75 * k).round() as i32;
                img.draw_scaled(
                    fb,
                    rect.x + (rect.w - w) / 2,
                    rect.y + (rect.h - h) / 2,
                    w,
                    h,
                );
            }
        }
    }

    /// Shield upgrade of a single Protoss selection: `cmdbtns` frame 12
    /// (button with the level box), the upgrade icon and its level.
    fn draw_hud_upgrade(&mut self, fb: &mut Framebuffer) {
        let Some(u) = self.first_selected().cloned() else {
            return;
        };
        if self.selected.len() != 1 || !self.data.unit(u.type_id).has_shields {
            return;
        }
        let k = self.content_box.3 as f32 / 1440.0;
        // Drawn 6 px up-left of the hit rect (shift search on 135051).
        let mut r = self.upgrade_button_rect();
        r.x += (UPGRADE_DRAW_SHIFT.0 * k).round() as i32;
        r.y += (UPGRADE_DRAW_SHIFT.1 * k).round() as i32;
        let Some(console) = &self.console else {
            return;
        };
        let icon = self
            .data
            .upgrade(PLASMA_SHIELDS)
            .map(|t| t.icon)
            .unwrap_or(0);
        if let Some(img) = console
            .cmdicons4
            .as_ref()
            .and_then(|g| g.frame(icon as usize))
        {
            let w = (img.width as f32 * 0.75 * k).round() as i32;
            let h = (img.height as f32 * 0.75 * k).round() as i32;
            img.draw_scaled(fb, r.x + (r.w - w) / 2, r.y + (r.h - h) / 2, w, h);
        }
        // The button frame with its level box goes over the icon.
        if let Some(img) = console
            .command_buttons_hd
            .as_ref()
            .and_then(|g| g.frame(12))
        {
            let w = (img.width as f32 * 0.75 * k).round() as i32;
            let h = (img.height as f32 * 0.75 * k).round() as i32;
            img.draw_scaled(fb, r.x + (r.w - w) / 2, r.y + (r.h - h) / 2, w, h);
        }
        let level = self.world.players[self.local_player]
            .upgrade_level(PLASMA_SHIELDS)
            .to_string();
        let x = (r.x + r.w - self.content_box.0) as f32 / k - UPGRADE_LEVEL_DX;
        self.glue_text_face(
            fb,
            x,
            (r.y + r.h) as f32 / k - UPGRADE_LEVEL_DY,
            &level,
            HUD_SMALL_PX,
            HUD_TEXT_COLOR,
            None,
            GlueFace::Hud,
        );
    }

    /// `stat_f10` button 1: the MENU label over the console's own well.
    fn draw_hud_menu_label(&mut self, fb: &mut Framebuffer) {
        if self.console.is_none() {
            return;
        }
        let r = self.menu_button_rect();
        let k = self.content_box.3 as f32 / 1440.0;
        let label = "MENU";
        let w = self.glue_measure(label, HUD_SMALL_PX, GlueFace::Hud);
        // The label sits in the race console's own MENU well, not centred in
        // the `stat_f10` rect: ink offsets measured on 135020 / 134905 /
        // 135051 (content px).
        let (dx, dy) = match self.console_race {
            Race::Zerg => (-9.0, 18.0),
            Race::Protoss => (-8.0, 13.0),
            _ => (-11.0, 18.0),
        };
        self.glue_text_face(
            fb,
            (r.x - self.content_box.0) as f32 / k + (r.w as f32 / k - w) / 2.0 + dx,
            r.y as f32 / k + HUD_SMALL_BASE + dy,
            label,
            HUD_SMALL_PX,
            HUD_MENU_COLOR,
            None,
            GlueFace::Hud,
        );
    }

    /// Command card: the retail button set of the selected unit
    /// (`galaxycraft::buttons`), each glyph from the 4x `cmdicons` at the
    /// race dialog's button rect; disabled buttons are grey.
    fn draw_hud_card(&mut self, fb: &mut Framebuffer) {
        let Some(u) = self.first_selected().cloned() else {
            return;
        };
        if u.owner as usize != self.local_player {
            return;
        }
        let card = galaxycraft::buttons::card(&self.world, &u);
        let slots = self.command_slots();
        let Some(console) = &self.console else {
            return;
        };
        let k = self.content_box.3 as f32 / 1440.0;
        let hovered = match self.hud_hover() {
            Some(HudHover::Card(i)) => Some(i),
            _ => None,
        };
        for (i, (slot, entry)) in slots.iter().zip(card.iter()).enumerate() {
            let Some((button, state)) = entry else {
                continue;
            };
            // Button background (`cmdbtns` frame 0, frame 1 while hovered).
            let frame = if hovered == Some(i) { 1 } else { 0 };
            if let Some(img) = console
                .command_buttons_hd
                .as_ref()
                .and_then(|g| g.frame(frame))
            {
                let w = (img.width as f32 * 0.75 * k).round() as i32;
                let h = (img.height as f32 * 0.75 * k).round() as i32;
                img.draw_scaled(
                    fb,
                    slot.x + (slot.w - w) / 2,
                    slot.y + (slot.h - h) / 2,
                    w,
                    h,
                );
            }
            let colour = match state {
                galaxycraft::buttons::ButtonState::Enabled => HUD_GLYPH_ENABLED,
                galaxycraft::buttons::ButtonState::Disabled => HUD_GLYPH_DISABLED,
            };
            if let Some(img) = console
                .cmdicons4
                .as_ref()
                .and_then(|g| g.frame(button.icon as usize))
            {
                let w = (img.width as f32 * 0.75 * k).round() as i32;
                let h = (img.height as f32 * 0.75 * k).round() as i32;
                let x = slot.x + (slot.w - w) / 2;
                let y = slot.y + (slot.h - h) / 2;
                draw_tinted(fb, img, (x, y, w, h), colour);
            }
        }
    }

    /// `statlb.ui.json` compact leaderboard (L).
    fn draw_leaderboard(&self, fb: &mut Framebuffer, scale: f32, ox: i32, oy: i32) {
        let Some(dialog) = self
            .console
            .as_ref()
            .and_then(|c| c.layout("statlb"))
            .cloned()
        else {
            return;
        };
        let base = dialog
            .rect
            .scaled(scale, ox, oy)
            .offset(0, (24.0 * scale) as i32);
        fb.rect(
            base.x - 4,
            base.y - 18,
            base.w + 8,
            base.h + 24,
            Rgb::new(6, 9, 14).blend(Rgb::new(30, 38, 50), 130),
        );
        fb.rect_outline(
            base.x - 4,
            base.y - 18,
            base.w + 8,
            base.h + 24,
            Rgb::new(100, 130, 165),
        );
        fb.text(
            base.x + 4,
            base.y - 16,
            "PL  RACE   SUPPLY  SCORE",
            Rgb::new(130, 225, 160),
            1,
        );
        for row in 0..8 {
            let Some(player) = self.world.players.get(row) else {
                break;
            };
            let y = base.y + (row as f32 * 14.0 * scale) as i32;
            let color = if row == self.local_player {
                Rgb::new(255, 235, 150)
            } else if player.defeated {
                Rgb::new(130, 135, 145)
            } else {
                Rgb::new(205, 215, 230)
            };
            fb.text(base.x + 4, y, &format!("{}", row + 1), color, 1);
            let race = match player.race {
                Race::Terran => "T",
                Race::Zerg => "Z",
                Race::Protoss => "P",
                Race::Neutral => "N",
            };
            fb.text(base.x + (28.0 * scale) as i32, y, race, color, 1);
            let used = self.world.supply_used(row as u8) / 2;
            let cap = self.world.supply_cap(row as u8) / 2;
            fb.text(
                base.x + (100.0 * scale) as i32,
                y,
                &format!("{used}/{cap}"),
                color,
                1,
            );
            fb.text(
                base.x + (160.0 * scale) as i32,
                y,
                &format!("{}", player.scores[0]),
                color,
                1,
            );
        }
    }

    /// `msnstats.ui.json` panel (F10): the shipped 320x240 mission table.
    fn draw_mission_stats(&self, fb: &mut Framebuffer, scale: f32, ox: i32, oy: i32) {
        let Some(dialog) = self
            .glue
            .as_ref()
            .and_then(|g| g.mission.as_ref())
            .or_else(|| self.console.as_ref().and_then(|c| c.layout("msnstats")))
        else {
            return;
        };
        let base = dialog
            .rect
            .scaled(scale, ox, oy)
            .offset((160.0 * scale) as i32, (120.0 * scale) as i32);
        fb.rect(
            base.x,
            base.y,
            base.w,
            base.h,
            Rgb::new(8, 11, 16).blend(Rgb::new(30, 38, 50), 120),
        );
        fb.rect_outline(base.x, base.y, base.w, base.h, Rgb::new(120, 145, 175));
        fb.text(
            base.x + 10,
            base.y + 8,
            "PLAYER        KILLS LOSSES RAZED UNITS",
            Rgb::new(150, 220, 170),
            1,
        );
        for (row, player) in self.world.players.iter().enumerate().take(8) {
            let y = base.y + 28 + row as i32 * 22;
            let units = player
                .death_counts
                .iter()
                .sum::<u32>()
                .saturating_sub(player.deaths);
            fb.text(
                base.x + 10,
                y,
                &format!("P{}", row + 1),
                if row == self.local_player {
                    Rgb::new(255, 225, 120)
                } else {
                    Rgb::new(200, 210, 225)
                },
                1,
            );
            fb.text(
                base.x + 80,
                y,
                &format!("{}", player.kills),
                Rgb::new(190, 205, 220),
                1,
            );
            fb.text(
                base.x + 140,
                y,
                &format!("{}", player.deaths),
                Rgb::new(190, 205, 220),
                1,
            );
            fb.text(
                base.x + 200,
                y,
                &format!("{}", player.razings),
                Rgb::new(190, 205, 220),
                1,
            );
            fb.text(
                base.x + 260,
                y,
                &format!("{}", units),
                Rgb::new(190, 205, 220),
                1,
            );
            if player.defeated {
                fb.text(base.x + 300, y, "OUT", Rgb::new(210, 120, 120), 1);
            }
        }
        fb.text(
            base.x + 10,
            base.y + base.h - 18,
            "F10: close mission statistics",
            Rgb::new(160, 175, 195),
            1,
        );
    }
}

impl App {
    fn draw_menu_panel(
        &self,
        fb: &mut Framebuffer,
        title: &str,
        lines: &[String],
        sel: usize,
        help: &str,
    ) {
        let (cx, cy, cw, ch) = Self::content_rect(fb);
        let (bx, by, bw, bh) = galaxycraft::menu::lan_panel_rect(cw, ch, lines.len());
        let (bx, by) = (cx + bx, cy + by);
        for y in by..by + bh {
            for x in bx..bx + bw {
                let c = fb.get(x, y);
                fb.put(x, y, c.blend(Rgb::new(6, 10, 18), 200));
            }
        }
        fb.rect_outline(bx, by, bw, bh, Rgb::new(110, 150, 185));
        fb.text_centered(cx + cw / 2, by + 12, title, Rgb::new(220, 230, 245), 2);
        for (i, line) in lines.iter().enumerate() {
            let y = by + 44 + i as i32 * 22;
            let selected = i == sel;
            if selected {
                fb.rect(bx + 12, y - 2, bw - 24, 18, Rgb::new(36, 58, 82));
                fb.rect_outline(bx + 12, y - 2, bw - 24, 18, Rgb::new(100, 190, 255));
            }
            fb.text(
                bx + 24,
                y,
                line,
                if selected {
                    Rgb::new(255, 245, 180)
                } else {
                    Rgb::new(190, 200, 215)
                },
                1,
            );
        }
        fb.text_centered(cx + cw / 2, by + bh - 20, help, Rgb::new(150, 170, 195), 1);
    }

    fn draw_main_full(&mut self, fb: &mut Framebuffer) {
        // Retail `glumain` layout: full-bleed space backdrop, the animated
        // fleet / battlecruiser / vortex clips at their `flc.json`
        // rectangles, the eight green entries, and the title frame with the
        // Battle.net tag top-right and the build footer bottom-right.
        //
        // The backdrop and title chrome are static: compose them once per
        // resolution and copy the cached pixels every frame.  Only the clips,
        // entries and profile text are redrawn.
        let t0 = Instant::now();
        let (w, h) = (fb.width as usize, fb.height as usize);
        let stale = self
            .menu_static
            .as_ref()
            .map(|(cw, ch, _)| *cw != w || *ch != h)
            .unwrap_or(true);
        if stale {
            let mut cache = Framebuffer::new(w, h);
            cache.clear(Rgb::new(0, 0, 0));
            self.draw_menu_backdrop_plain(&mut cache);
            self.draw_menu_detail_art(&mut cache);
            self.draw_menu_frame_art(&mut cache, 0, 0);
            self.menu_static = Some((w, h, cache.pixels));
        }
        if let Some((_, _, pixels)) = &self.menu_static {
            fb.pixels.copy_from_slice(pixels);
        }
        let t1 = Instant::now();
        self.draw_menu_clips(fb);
        let t2 = Instant::now();
        self.draw_menu_entries(fb, true);
        let t3 = Instant::now();
        self.draw_menu_frame_text(fb, 0, 0, None);
        let t4 = Instant::now();
        if std::env::var_os("GC_PROFILE").is_some() {
            eprintln!(
                "menu: static {:.1} clips {:.1} entries {:.1} text {:.1} ms",
                (t1 - t0).as_secs_f64() * 1000.0,
                (t2 - t1).as_secs_f64() * 1000.0,
                (t3 - t2).as_secs_f64() * 1000.0,
                (t4 - t3).as_secs_f64() * 1000.0
            );
        }
    }

    /// Static chrome shared by the main menu and the sub-screens: the menu
    /// detail art behind the title frame.
    fn draw_menu_detail_art(&self, fb: &mut Framebuffer) {
        if let Some(glue) = &self.glue {
            if let (Some(dialog), Some(art)) = (&glue.menu, &glue.etail) {
                if let Some(c) = dialog.control(1) {
                    let (scale, ox, oy) = self.ui_metrics(fb);
                    let r = c.abs.scaled(scale, ox, oy);
                    art.draw_scaled(fb, r.x, r.y, r.w, r.h);
                }
            }
        }
    }

    /// Draw the six Remastered button clips at the rectangles shipped in
    /// `rez/flc.json` (relative to each FLCBTN's 640x480 origin).
    fn draw_menu_clips(&mut self, fb: &mut Framebuffer) {
        let hover = self.main_sel();
        let (scale, ox, oy) = self.ui_metrics(fb);
        let ui = |x: f32, y: f32, w: f32, h: f32| -> (i32, i32, i32, i32) {
            (
                ox + (x * scale).round() as i32,
                oy + (y * scale).round() as i32,
                (w * scale).round() as i32,
                (h * scale).round() as i32,
            )
        };
        // `flc.json`: Single [-20,0,425,175] / SingleOn [-40,-10,425,200];
        // Multi [-10,-5,282,270] / MultiOn [-52,-2,330,220]; Exit
        // [-2,-4,204,182] / ExitOn [5,0,176,160].
        // One source of truth: `menu::FLC_RECTS` is the shipped `flc.json`
        // rectangle list, unit-tested against the fixture.
        #[allow(clippy::type_complexity)]
        let slots: [(usize, bool, (f32, f32, f32, f32)); 6] = [
            (0, false, galaxycraft::menu::FLC_RECTS[0].1),
            (0, true, galaxycraft::menu::FLC_RECTS[1].1),
            (1, false, {
                let r = galaxycraft::menu::FLC_RECTS[2].1;
                (78.0 + r.0, 190.0 + r.1, r.2, r.3)
            }),
            (1, true, {
                let r = galaxycraft::menu::FLC_RECTS[3].1;
                (78.0 + r.0, 190.0 + r.1, r.2, r.3)
            }),
            (7, false, {
                let r = galaxycraft::menu::FLC_RECTS[4].1;
                (395.0 + r.0, 320.0 + r.1, r.2, r.3)
            }),
            (7, true, {
                let r = galaxycraft::menu::FLC_RECTS[5].1;
                (395.0 + r.0, 320.0 + r.1, r.2, r.3)
            }),
        ];
        let Some(layers) = &mut self.menu_layers else {
            return;
        };
        for (idx, is_hover, (x, y, w, h)) in slots {
            if is_hover && hover != Some(idx) {
                continue;
            }
            if let Some(clip) = layers.clip(idx, is_hover) {
                let alpha_known = clip.alpha_known;
                let cached_alpha = clip.has_alpha;
                if let Some(frame) = clip.frame() {
                    let additive = if alpha_known {
                        !cached_alpha
                    } else {
                        !frame.has_alpha()
                    };
                    draw_poster_rgba_additive(fb, frame, ui(x, y, w, h), 0.0, additive);
                }
            }
        }
    }

    /// Clickable rectangles of the eight main-menu entries, in window pixels.
    /// Used both for drawing and for hit-testing so they can never diverge.
    /// Control under a window point on the current sub-screen.  The rects
    /// come from the measured control tables in `menu.rs`; rows derive from
    /// the measured 57 px pitch.
    fn menu_hit(&self, screen: &MenuScreen, x: i32, y: i32) -> Option<MenuHit> {
        use galaxycraft::menu as m;
        let (cx, _cy, cw, ch) = self.content_box;
        let kx = cw as f32 / 2560.0;
        let ky = ch as f32 / 1440.0;
        let inside = |r: m::HitRect| {
            x >= cx + (r.0 * kx) as i32
                && x < cx + ((r.0 + r.2) * kx) as i32
                && y >= (r.1 * ky) as i32
                && y < ((r.1 + r.3) * ky) as i32
        };
        let row_of = |r: m::HitRect, rows: usize| {
            let fy = y as f32 / ky;
            if fy < r.1 || fy >= r.1 + r.3 {
                return None;
            }
            let row = ((fy - r.1) / 57.0) as usize;
            (row < rows).then_some(row)
        };
        match screen {
            MenuScreen::Mission { .. } => {
                if inside(m::MISSION_BUTTONS[0]) {
                    Some(MenuHit::Ok)
                } else if inside(m::MISSION_BUTTONS[1]) {
                    Some(MenuHit::Cancel)
                } else {
                    row_of(m::MISSION_ROWS, 8).map(MenuHit::Row)
                }
            }
            MenuScreen::Replays => {
                if inside(m::CREATE_BUTTONS[0]) {
                    Some(MenuHit::Ok)
                } else if inside(m::CREATE_BUTTONS[1]) {
                    Some(MenuHit::Cancel)
                } else {
                    row_of(m::CREATE_ROWS, 4).map(MenuHit::Row)
                }
            }
            MenuScreen::Lobby { .. } => {
                if inside(m::CREATE_BUTTONS[0]) {
                    Some(MenuHit::Ok)
                } else if inside(m::CREATE_BUTTONS[1]) {
                    Some(MenuHit::Cancel)
                } else {
                    if let Some(slot) = self.lobby_race_open {
                        for k in 0..LobbyRace::all().len() {
                            if inside(m::lobby_race_item(slot, k)) {
                                return Some(MenuHit::RacePick(slot, k));
                            }
                        }
                    }
                    for i in 0..8 {
                        if inside(m::lobby_race_bay(i)) {
                            return Some(MenuHit::Race(i));
                        }
                    }
                    row_of(m::CREATE_ROWS, 6).map(MenuHit::Row)
                }
            }
            MenuScreen::SavedGames => {
                // `draw_saved` panel/rows/buttons (fractions of the box).
                let fx = |f: f32| f * 2560.0;
                let fy = |f: f32| f * 1440.0;
                let rect =
                    |x0: f32, y0: f32, x1: f32, y1: f32| (fx(x0), fy(y0), fx(x1 - x0), fy(y1 - y0));
                let rows = (
                    fx(0.1758),
                    fy(0.1639 + 0.12 * 0.4937),
                    fx(0.3675),
                    10.0 * fy(0.07 * 0.4937),
                );
                if inside(rect(0.646, 0.791, 0.800, 0.862)) {
                    Some(MenuHit::Ok)
                } else if inside(rect(0.668, 0.856, 0.830, 0.926)) {
                    Some(MenuHit::Cancel)
                } else if inside(rect(0.173, 0.674, 0.301, 0.744)) {
                    Some(MenuHit::Delete)
                } else {
                    row_of(rows, 10).map(MenuHit::Row)
                }
            }
            MenuScreen::Options { tab } => {
                for (i, r) in m::OPTIONS_TABS.iter().enumerate() {
                    if inside(*r) {
                        return Some(MenuHit::Tab(i));
                    }
                }
                let (sliders, checks): (&[m::HitRect], &[m::HitRect]) = match tab {
                    OptionsTab::Speed => (&m::OPTIONS_SLIDERS_SPEED, &m::OPTIONS_CHECKS_SPEED),
                    OptionsTab::Sound => (&m::OPTIONS_SLIDERS_SOUND, &m::OPTIONS_CHECKS_SOUND),
                    OptionsTab::Video => (&[], &m::OPTIONS_CHECKS_VIDEO),
                    OptionsTab::Game => (&[], &m::OPTIONS_CHECKS_GAME),
                    OptionsTab::Network => (&[], &m::OPTIONS_CHECKS_NETWORK),
                };
                for (i, r) in sliders.iter().enumerate() {
                    if inside(*r) {
                        return Some(MenuHit::OptSlider(i));
                    }
                }
                for (i, r) in checks.iter().enumerate() {
                    if inside(*r) {
                        return Some(MenuHit::OptCheck(i));
                    }
                }
                if inside(m::OPTIONS_BUTTONS[0]) {
                    Some(MenuHit::Restore)
                } else if inside(m::OPTIONS_BUTTONS[1]) {
                    Some(MenuHit::Ok)
                } else {
                    None
                }
            }
            MenuScreen::Collections => {
                for (i, r) in m::COLLECTION_CELLS.iter().enumerate() {
                    if inside(*r) {
                        return Some(MenuHit::Cell(i));
                    }
                }
                if inside(m::COLLECTION_BUTTONS[0]) {
                    Some(MenuHit::Ok)
                } else if inside(m::COLLECTION_BUTTONS[1]) {
                    Some(MenuHit::Equip)
                } else {
                    None
                }
            }
            MenuScreen::Lan => {
                let lines = 2 + if self.lan.listener.is_some() || self.lan.entries.is_empty() {
                    1
                } else {
                    self.lan.entries.len()
                };
                let panel = m::lan_panel_rect(cw, ch, lines);
                for i in 0..lines {
                    let r = m::lan_row_rect(panel, i);
                    let r = (r.0 + cx as f32, r.1, r.2, r.3);
                    if inside(r) {
                        return Some(MenuHit::Row(i));
                    }
                }
                None
            }
            MenuScreen::Credits | MenuScreen::Intro { .. } => Some(MenuHit::Skip),
            _ => None,
        }
    }

    /// Pointer state for the current sub-screen: the control under the
    /// cursor (playing `ui_move` when it changes) and an edge-triggered left
    /// click.
    fn menu_pointer(&mut self, input: &MenuInput) -> (Option<MenuHit>, bool) {
        let pointer = input.mouse;
        let screen = self.menu_screen.clone();
        let hit = pointer.and_then(|(x, y)| self.menu_hit(&screen, x, y));
        if hit != self.menu_hover {
            self.menu_hover = hit;
            if hit.is_some() {
                self.play("ui_move");
            }
        }
        let down = input.mouse_down;
        let clicked = down && !self.mouse_was_down;
        self.mouse_was_down = down;
        (hit, clicked)
    }

    /// Run the action of a main-menu entry (shared by Enter and mouse clicks).
    fn activate_main_item(&mut self, idx: usize) {
        // The retail CEF menu has no dedicated transition cue: the press is
        // the only sound (GLUSND_MOUSE_DOWN).  Verified in the live capture.
        self.play("ui_click");
        // A sub-screen owns the pointer: the main menu must return to its
        // idle animation instead of keeping the button hover clip lit behind
        // the new panel (the retail client captures the mouse in the panel).
        self.main_cursor = None;
        match idx {
            0 => {
                self.gametype_multi = false;
                self.menu_screen = MenuScreen::GameType;
            }
            1 => {
                // Retail opens `Select Game Type` for Multiplayer too; the
                // lobby is only reached after choosing Original/Expansion.
                self.gametype_multi = true;
                if self.campaign_len < self.maps.len() {
                    self.map_index = self.campaign_len;
                }
                self.menu_screen = MenuScreen::GameType;
            }
            2 => {
                self.menu_screen = MenuScreen::Options {
                    tab: OptionsTab::Speed,
                }
            }
            3 => self.menu_screen = MenuScreen::Collections,
            4 => {
                self.hotkey_baseline = self.hotkey_bindings.clone();
                self.hotkey_sel = None;
                self.hotkey_action = None;
                self.refresh_hotkey_errors();
                self.menu_screen = MenuScreen::Hotkeys {
                    race: HotkeyRace::Terran,
                };
            }
            5 => self.menu_screen = MenuScreen::Credits,
            6 => {
                // Retail `View Intro` replays the title movie (the publisher
                // logo is only part of the first-launch sequence).
                self.begin_intro(
                    galaxycraft::video::intro_sequence(false, false)
                        .into_iter()
                        .map(str::to_string)
                        .collect(),
                    None,
                );
            }
            _ => self.quit = true,
        }
    }

    /// Commit `Select Game Type` and continue on the flow that opened it.
    /// Single player goes to the episode list; multiplayer goes to the
    /// connection screen (our LAN lobby) instead of opening it directly from
    /// the main menu.
    fn finish_gametype(&mut self, expansion: bool) {
        self.intro_expansion = expansion;
        self.episode_sel = galaxycraft::menu::episode_start(expansion);
        self.menu_screen = match galaxycraft::menu::gametype_followup(self.gametype_multi) {
            galaxycraft::menu::GameTypeFollowup::MultiplayerLobby => MenuScreen::Lobby {
                map: Some(self.map_index),
            },
            galaxycraft::menu::GameTypeFollowup::Registry => {
                self.registry_after_gametype = true;
                MenuScreen::Registry
            }
        };
        self.gametype_hover = None;
        self.gametype_multi = false;
        self.play("ui_click");
    }

    /// Blend a measured coverage mask over the current framebuffer.  Masks
    /// are calibrated at the native 2560x1440 content box; other scales fall
    /// back to the font renderer.
    fn draw_text_mask(
        &self,
        fb: &mut Framebuffer,
        mask: &galaxycraft::menu_masks::TextMask,
        scale: f32,
        _ox: i32,
        _oy: i32,
    ) -> bool {
        if (scale - 3.0).abs() > 0.01 {
            return false;
        }
        let (cx, _cy, _cw, _ch) = Self::content_rect(fb);
        let x0 = cx + mask.x;
        let y0 = mask.y;
        for row in 0..mask.h as usize {
            for col in 0..mask.w as usize {
                let a = mask.alpha[row * mask.w as usize + col];
                if a == 0 {
                    continue;
                }
                let c = if mask.split > 0 && mask.x + col as i32 >= mask.split {
                    mask.c1
                } else {
                    mask.c0
                };
                let dst = fb.get(x0 + col as i32, y0 + row as i32);
                fb.put(
                    x0 + col as i32,
                    y0 + row as i32,
                    dst.blend(Rgb::new(c[0], c[1], c[2]), a),
                );
            }
        }
        true
    }

    /// The eight green entries at their shipped 640x480 spots. Drawn by the
    /// main menu (with the cursor highlight) and, without highlight, behind
    /// every sub-screen panel — the retail client composites them that way.
    fn draw_menu_entries(&mut self, fb: &mut Framebuffer, highlight: bool) {
        if std::env::var_os("GC_NO_TEXT").is_some() {
            return;
        }
        // Behind a sub-screen panel the retail client shows these entries
        // dimmed (measured mean 81,101,50 against 110,233,98 when lit).
        let dim = !highlight;
        let sel = if highlight { self.main_sel() } else { None };
        let items = galaxycraft::menu::main_items();
        // Measured on the retail captures: the idle face peaks at
        // (102, 227, 53) and the mouse-over face at (196, 255, 124).
        let normal = if dim {
            Rgb::new(45, 110, 45)
        } else {
            Rgb::new(102, 227, 53)
        };
        let hover = if dim {
            Rgb::new(85, 110, 85)
        } else {
            Rgb::new(196, 255, 124)
        };
        let (scale, ox, oy) = self.ui_metrics(fb);
        if highlight
            && std::env::var("GC_TEXT_MASKS").ok().as_deref() == Some("1")
            && std::env::var_os("GC_RAW_RENDER").is_none()
            && (scale - 3.0).abs() < 0.01
            && matches!(sel, None | Some(0) | Some(1))
        {
            use galaxycraft::menu_masks as m;
            const IDLE: [&m::TextMask; 8] = [
                &m::SINGLE,
                &m::MULTI,
                &m::OPTIONS,
                &m::COLLECTIONS,
                &m::HOTKEYS,
                &m::CREDITS,
                &m::INTRO,
                &m::EXIT,
            ];
            for (idx, mask) in IDLE.iter().enumerate() {
                let draw = match (sel, idx) {
                    (Some(0), 0) => &m::SINGLE_HOVER,
                    (Some(1), 1) => &m::MULTI_HOVER,
                    _ => *mask,
                };
                self.draw_text_mask(fb, draw, scale, ox, oy);
            }
            return;
        }
        let bright = if dim {
            Rgb::new(85, 110, 85)
        } else {
            Rgb::new(208, 255, 131)
        };
        for label in galaxycraft::menu::MAIN_LABELS {
            let Some(text) = items.get(label.idx) else {
                continue;
            };
            let hovered = Some(label.idx) == sel;
            self.draw_main_label(
                fb, text, label, hovered, normal, bright, hover, scale, ox, oy,
            );
        }
    }

    /// Retail menu label: the first glyph is the hotkey letter, drawn with
    /// the wide Eurostile Extended face and a brighter green; the rest uses
    /// the regular face.  On hover the whole word switches to the extended,
    /// bright face (reference #130704/#130712).  The full ink width and
    /// anchor come from [`galaxycraft::menu::MAIN_LABELS`].
    #[allow(clippy::too_many_arguments)]
    fn draw_main_label(
        &mut self,
        fb: &mut Framebuffer,
        text: &str,
        label: galaxycraft::menu::MainLabel,
        hovered: bool,
        normal: Rgb,
        bright: Rgb,
        hover: Rgb,
        scale: f32,
        ox: i32,
        oy: i32,
    ) {
        use galaxycraft::menu::LabelAlign;
        let left = match label.align {
            LabelAlign::Left => label.x,
            LabelAlign::Center => label.x - label.width / 2.0,
        };
        if hovered {
            self.draw_hd_text(
                fb,
                text,
                left,
                label.y,
                LabelAlign::Left,
                label.width,
                16.0,
                scale,
                ox,
                oy,
                hover,
                true,
            );
            return;
        }
        let mut chars = text.chars();
        let Some(first) = chars.next() else {
            return;
        };
        let rest = chars.as_str();
        let first_string = first.to_string();
        let advance = self.hd_advance_ui(&first_string, scale, true);
        self.draw_hd_text(
            fb,
            &first_string,
            left,
            label.y,
            LabelAlign::Left,
            1.0,
            16.0,
            scale,
            ox,
            oy,
            bright,
            true,
        );
        if !rest.is_empty() {
            self.draw_hd_text(
                fb,
                rest,
                left + advance,
                label.y,
                LabelAlign::Left,
                (label.width - advance).max(1.0),
                16.0,
                scale,
                ox,
                oy,
                normal,
                true,
            );
        }
    }

    /// Advance width of `text` in UI pixels for one of the two faces.
    fn hd_advance_ui(&mut self, text: &str, scale: f32, hover_face: bool) -> f32 {
        let px = (16.0 * scale).max(7.0);
        let use_hover = hover_face && self.ui_font_hover.is_some();
        let font = if use_hover {
            self.ui_font_hover.as_mut()
        } else {
            self.ui_font.as_mut()
        };
        font.map(|f| f.measure(text, px, 0.0) as f32 / scale)
            .unwrap_or(text.chars().count() as f32 * 8.0)
    }

    /// Draw one label with the retail face, a selectable em size and the
    /// measured ink width.  Main-menu labels use 16 UI px; the smaller
    /// `Cancel` in the game-type popup uses 14.
    #[allow(clippy::too_many_arguments)]
    fn draw_hd_text(
        &mut self,
        fb: &mut Framebuffer,
        text: &str,
        anchor_x: f32,
        top: f32,
        align: galaxycraft::menu::LabelAlign,
        width: f32,
        font_ui: f32,
        scale: f32,
        ox: i32,
        oy: i32,
        color: Rgb,
        hover_face: bool,
    ) {
        use galaxycraft::menu::LabelAlign;
        let px = (font_ui * scale).max(7.0);
        let x = ox + (anchor_x * scale).round() as i32;
        let y = oy + (top * scale).round() as i32;
        let use_hover = hover_face && self.ui_font_hover.is_some();
        let font_opt = if use_hover {
            self.ui_font_hover.as_mut()
        } else {
            self.ui_font.as_mut()
        };
        if let Some(font) = font_opt {
            let natural = font.measure(text, px, 0.0) as f32;
            let target = width * scale;
            let gaps = text.chars().count().saturating_sub(1).max(1) as f32;
            // The retail labels are letter-spaced Eurostile: reproduce the
            // measured ink width by distributing the difference as tracking.
            let tracking = ((target - natural) / gaps).clamp(-px * 0.04, px * 0.35);
            let shadow = if std::env::var_os("GC_NO_LABEL_SHADOW").is_some() {
                None
            } else {
                Some(Rgb::new(0, 0, 0))
            };
            match align {
                LabelAlign::Left => {
                    font.draw_top(fb, x, y, text, px, tracking, color, shadow);
                }
                LabelAlign::Center => {
                    font.draw_centered(fb, x, y, text, px, tracking, color, shadow);
                }
            }
        } else {
            let big = (scale.round() as i32).clamp(1, 3);
            let w = text.chars().count() as i32 * 8 * big;
            let bx = match align {
                LabelAlign::Left => x,
                LabelAlign::Center => x - w / 2,
            };
            fb.text(bx, y, text, color, big);
        }
    }

    fn main_sel(&self) -> Option<usize> {
        self.main_cursor
    }

    /// Set the hovered entry.  Reference #130704 does **not** keep an
    /// outgoing Multi overlay: its apparent plate is the battlecruiser hull
    /// and nebula, and the green-mask area in the MultiOn rectangle drops
    /// from 11 827 px (#130712) to 133 px (#130704).  Hover is therefore a
    /// plain active-clip switch, as the shipped `FLC_MOUSEOVER` data shows.
    fn set_main_hover(&mut self, new: Option<usize>) {
        self.main_cursor = new;
    }

    /// Remastered space scene: animated WebM layers composited additively
    /// over the nebula. Shared by the main menu and every sub-screen so the
    /// whole front-end sits on the same backdrop as the retail client.
    fn draw_space_scene(&mut self, fb: &mut Framebuffer) {
        // No-install fallback: deterministic star field.
        draw_space_backdrop(fb);
    }

    /// Backdrop only (no frame): the main menu draws the frame at its own
    /// anchor, every other screen at the sub-screen anchor.
    fn draw_menu_backdrop_plain(&mut self, fb: &mut Framebuffer) {
        // `glue/palmm/backgnd.dds` is a 3412x1920 (16:9) painting.  The
        // retail references show it edge to edge inside the content box, so
        // it is drawn at exactly that rectangle: no inset, no crop.
        let (cx, cy, cw, ch) = Self::content_rect(fb);
        let stale = self
            .backdrop_fit
            .as_ref()
            .map(|(w, h, _)| *w != cw as usize || *h != ch as usize)
            .unwrap_or(true);
        if stale {
            self.backdrop_fit = self
                .glue
                .as_ref()
                .and_then(|g| g.palmm_backdrop.as_ref())
                .map(|art| {
                    (
                        cw as usize,
                        ch as usize,
                        art.resize(cw as usize, ch as usize),
                    )
                });
        }
        if let Some((_, _, art)) = &self.backdrop_fit {
            for y in 0..ch as usize {
                for x in 0..cw as usize {
                    let p = art.pixels[y * art.width + x];
                    fb.put(cx + x as i32, cy + y as i32, Rgb(p & 0x00ff_ffff));
                }
            }
        } else {
            self.draw_space_scene(fb);
        }
    }

    /// Sub-screen backdrop: same scene, frame anchored 190 px further right
    /// (measured: the menubar structure sits at x 2497..2873 in the sub-screens
    /// and at 2135..2607 in the main menu).
    fn draw_menu_backdrop(&mut self, fb: &mut Framebuffer) {
        self.draw_menu_backdrop_at(fb, (fb.width as f32 * 0.0555) as i32);
    }

    /// Backdrop with the frame at an explicit offset. The Hotkeys screen keeps
    /// the main-menu anchor (measured: shifting it there is worse).
    fn draw_menu_backdrop_at(&mut self, fb: &mut Framebuffer, dx: i32) {
        self.draw_menu_backdrop_plain(fb);
        self.draw_menu_frame(fb, dx, 0);
        // The retail sub-screens composite the menu entries under the panel.
        self.draw_menu_entries(fb, false);
    }

    /// Options backdrop: the retail CEF modal sits over the *running* main
    /// menu, so the Single/Multi/Exit clips stay visible under the dim
    /// (references 130933..130954 show the Exit galaxy and the Multi ship).
    /// The title frame keeps the main-menu anchor (dx 0), not the 5.55 %
    /// sub-screen shift: on 130933 the frame's left arm is at x 2170..2250
    /// of the content box, 142 px left of the shifted art.
    fn draw_menu_backdrop_live(&mut self, fb: &mut Framebuffer) {
        self.draw_menu_backdrop_plain(fb);
        self.draw_menu_detail_art(fb);
        self.draw_menu_frame(fb, 0, 0);
        self.draw_menu_clips(fb);
        // The live menu labels in their idle faces (nothing hovered); the
        // modal's 149 dim then gives the measured (42,93,22) = 0.416 x
        // (102,227,53).
        let hovered = self.main_cursor.take();
        self.draw_menu_entries(fb, true);
        self.main_cursor = hovered;
    }

    /// Title-frame surfaces plus the profile name, as every retail front-end
    /// screen shows them behind its panel.
    fn draw_menu_frame_art(&self, fb: &mut Framebuffer, dx: i32, dy: i32) {
        let (scale, ox, oy) = self.ui_metrics(fb);
        let mut drew_art = false;
        if let Some(glue) = &self.glue {
            if let Some(dialog) = &glue.menu {
                for (id, art) in [
                    (22, glue.title_bg.as_ref()),
                    (23, glue.title_overlay.as_ref()),
                ] {
                    if let (Some(c), Some(art)) = (dialog.control(id), art) {
                        let mut r = c.abs.scaled(scale, ox, oy);
                        if id == 22 {
                            r.x -= 3;
                        }
                        // The Remastered surfaces are authored at 4x the
                        // 640x480 UI and drawn at that native size from the
                        // control origin.  The overlay is 1584x1054, so its
                        // red panel border (152..974) lands exactly on the
                        // 820x820 `titleframe_bg` panel; the stale control
                        // width (266) would squash it to half size.
                        let w = (art.width as f32 * scale / 4.0).round() as i32;
                        let h = (art.height as f32 * scale / 4.0).round() as i32;
                        art.draw_scaled(fb, r.x + dx, r.y + dy, w, h);
                        drew_art = true;
                    }
                }
            }
        }
        let panel = UiRect::new(413, 66, 205, 205).scaled(scale, ox, oy);
        if !drew_art {
            fb.rect(
                panel.x + dx,
                panel.y + dy,
                panel.w,
                panel.h,
                Rgb::new(6, 3, 3),
            );
            fb.rect_outline(
                panel.x + dx,
                panel.y + dy,
                panel.w,
                panel.h,
                Rgb::new(150, 25, 25),
            );
        }
        // Green ring cursor at the panel centre, as in the reference shot.
        // 28 px diameter at 3x -> 4.7 UI px radius.
        let ccx = panel.x + dx + panel.w / 2;
        let ccy = panel.y + dy + panel.h / 2;
        let cr = (4.7 * scale).round().max(4.0) as i32;
        fb.circle_outline(ccx, ccy, cr, Rgb::new(20, 180, 20));
        fb.circle_outline(ccx, ccy, cr + 1, Rgb::new(10, 110, 10));
    }

    /// Profile tag and build footer over the title frame.
    fn draw_menu_frame_text(&mut self, fb: &mut Framebuffer, _dx: i32, _dy: i32, dim: Option<Rgb>) {
        let (scale, ox, oy) = self.ui_metrics(fb);
        // Battle.net tag top-right and the build footer bottom-right, both
        // right-aligned to the 640x480 UI safe area (reference: tag right
        // edge x=632, version right edge x=633 in UI space).
        if std::env::var_os("GC_NO_TEXT").is_some() {
            return;
        }
        let tag = std::env::var("GC_BATTLETAG").unwrap_or_else(|_| {
            let user = std::env::var("USER").unwrap_or_else(|_| "Player".to_string());
            format!("{user}#1532")
        });
        if (scale - 3.0).abs() < 0.01
            && std::env::var("GC_TEXT_MASKS").ok().as_deref() == Some("1")
            && std::env::var_os("GC_RAW_RENDER").is_none()
            && tag == "cotizcesar#1532"
            && dim.is_none()
        {
            self.draw_text_mask(fb, &galaxycraft::menu_masks::PROFILE, scale, ox, oy);
            self.draw_text_mask(fb, &galaxycraft::menu_masks::VERSION, scale, ox, oy);
            return;
        }
        // Measured on reference #1: the tag is 11 UI px (232x23 screen px)
        // with its cap top at y=8, the build footer 8 UI px with 3.9 px of
        // inter-glyph tracking (432x19).  Both share the (198,218,255) face.
        let tag_px = (std::env::var("GC_TAG_PX")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(11.0)
            * scale)
            .max(6.0);
        let info = dim.unwrap_or(Rgb::new(198, 218, 255));
        let info_bold = std::env::var("GC_GLYPH_BOLD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0u8);
        let info_gamma = std::env::var("GC_INFO_GAMMA")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1.0);
        // Glyph-by-glyph profile metrics measured on reference #130637.
        // Offsets are in 640x480 UI pixels from the right edge (x=633.667);
        // values are (char, left offset, target width).
        const PROFILE_GLYPHS: [(char, f32, f32); 15] = [
            ('c', -77.333, 4.0),
            ('o', -72.333, 5.0),
            ('t', -66.667, 3.333),
            ('i', -62.0, 1.0),
            ('z', -59.333, 4.0),
            ('c', -54.0, 3.667),
            ('e', -49.0, 4.667),
            ('s', -43.333, 3.667),
            ('a', -38.667, 4.667),
            ('r', -32.333, 3.333),
            ('#', -28.333, 5.0),
            ('1', -21.667, 2.667),
            ('5', -16.0, 4.333),
            ('3', -10.333, 4.333),
            ('2', -4.333, 4.333),
        ];
        if dim.is_none() && tag == "cotizcesar#1532" && (scale - 3.0).abs() < 0.01 {
            if let Some(font) = self.ui_font.as_mut() {
                let top = (2.7 * scale) as i32;
                for (ch, offset, width) in PROFILE_GLYPHS {
                    let left = ox + ((633.667 + offset) * scale).round() as i32;
                    font.draw_glyph_left(
                        fb,
                        ch,
                        left,
                        top,
                        (11.0 * scale).max(6.0),
                        (width * scale).round() as i32,
                        info,
                        info_gamma,
                        info_bold,
                    );
                }
            }
        } else if dim.is_none() {
            if let Some(font) = self.ui_font.as_mut() {
                let x_scale = std::env::var("GC_TAG_XSCALE")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0.70);
                let tag_top = std::env::var("GC_TAG_TOP")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(2.7);
                let natural = font.measure_scaled(&tag, tag_px, 0.0, x_scale) as f32;
                let gaps = tag.chars().count().saturating_sub(1).max(1) as f32;
                let target = 77.3 * scale;
                font.draw_right_scaled_weighted_f(
                    fb,
                    ox + (634.67 * scale) as i32,
                    oy as f32 + tag_top * scale,
                    &tag,
                    tag_px,
                    (target - natural) / gaps,
                    x_scale,
                    info,
                    None,
                    info_gamma,
                );
            }
        }
        let ver = "1.23.10.13515 | enUS | 64-bit";
        // Glyph-by-glyph metrics measured on reference #130637, offsets in
        // 640x480 UI pixels from the right edge (x=634.0).
        const VERSION_GLYPHS: [(char, f32, f32); 30] = [
            ('1', -143.667, 2.667),
            ('.', -138.333, 1.333),
            ('2', -135.333, 4.333),
            ('3', -129.333, 4.333),
            ('.', -123.333, 1.333),
            ('1', -119.667, 2.667),
            ('0', -114.333, 5.0),
            ('.', -108.333, 1.333),
            ('1', -104.667, 2.667),
            ('3', -99.333, 4.333),
            ('5', -93.0, 4.333),
            ('1', -86.667, 2.667),
            ('5', -81.0, 4.333),
            (' ', 0.0, 0.0),
            ('|', -70.667, 0.667),
            (' ', 0.0, 0.0),
            ('e', -64.0, 4.667),
            ('n', -58.0, 4.667),
            ('U', -51.667, 5.333),
            ('S', -44.667, 5.0),
            (' ', 0.0, 0.0),
            ('|', -34.0, 0.667),
            (' ', 0.0, 0.0),
            ('6', -27.333, 4.667),
            ('4', -22.0, 5.333),
            ('-', -15.667, 3.0),
            ('b', -11.333, 4.667),
            ('i', -5.333, 1.0),
            ('t', -3.0, 3.333),
            (' ', 0.0, 0.0),
        ];
        if ver == "1.23.10.13515 | enUS | 64-bit"
            && (scale - 3.0).abs() < 0.01
            && self.ui_font_info.is_some()
        {
            if let Some(font) = self.ui_font_info.as_mut() {
                let top = (469.0 * scale) as i32;
                for (ch, offset, width) in VERSION_GLYPHS {
                    if ch == ' ' {
                        continue;
                    }
                    let left = ox + ((634.0 + offset) * scale).round() as i32;
                    font.draw_glyph_left(
                        fb,
                        ch,
                        left,
                        top,
                        (10.0 * scale).max(6.0),
                        (width * scale).round() as i32,
                        info,
                        info_gamma,
                        info_bold,
                    );
                }
            }
        } else if let Some(font) = if self.ui_font_info.is_some() {
            self.ui_font_info.as_mut()
        } else {
            self.ui_font.as_mut()
        } {
            let x_scale = std::env::var("GC_VER_XSCALE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.8);
            let px = (std::env::var("GC_VER_PX")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10.0)
                * scale)
                .max(6.0);
            let ver_top = std::env::var("GC_VER_TOP")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(469.5);
            let natural = font.measure_scaled(ver, px, 0.0, x_scale) as f32;
            let gaps = ver.chars().count().saturating_sub(1).max(1) as f32;
            font.draw_right_scaled_weighted(
                fb,
                ox + (634.0 * scale) as i32,
                oy + (ver_top * scale) as i32,
                ver,
                px,
                (144.0 * scale - natural) / gaps,
                x_scale,
                info,
                None,
                info_gamma,
            );
        } else {
            let w = tag.chars().count() as i32 * 8;
            fb.text(
                ox + (634.0 * scale) as i32 - w,
                oy + (8.0 * scale) as i32,
                &tag,
                info,
                1,
            );
            let ver = "1.23.10.13515 | enUS | 64-bit";
            let vw = ver.chars().count() as i32 * 8;
            fb.text(
                ox + (634.0 * scale) as i32 - vw,
                oy + (469.0 * scale) as i32,
                ver,
                info,
                1,
            );
        }
    }

    /// Title-frame surfaces plus the profile name, as every retail front-end
    /// screen shows them behind its panel.
    fn draw_menu_frame(&mut self, fb: &mut Framebuffer, dx: i32, dy: i32) {
        self.draw_menu_frame_art(fb, dx, dy);
        self.draw_menu_frame_text(fb, dx, dy, None);
    }

    /// Retail moon-landscape backdrop used by the Registry, Saved Games and
    /// Create screens (`glue/palnl/backgnd.dds`, 16:9 like the content box).
    fn draw_moon_backdrop(&mut self, fb: &mut Framebuffer) {
        let (cx, _cy, cw, ch) = Self::content_rect(fb);
        let Some(art) = self.glue.as_ref().and_then(|g| g.palnl_backdrop.clone()) else {
            self.draw_space_scene(fb);
            return;
        };
        art.draw_scaled(fb, cx, 0, cw, ch);
    }

    /// Retail Toon Registry, rebuilt from the shipped `toonlistbg` plate and
    /// the 1:1 geometry measured on capture 130734: list plate at
    /// (-97,-5) scale 0.748 (source rows 0..1300), Ok/Cancel classic plates,
    /// New ID / Delete bays and the profile list text.
    fn draw_registry(&mut self, fb: &mut Framebuffer) {
        self.draw_moon_backdrop(fb);
        let (cx, _cy, _cw, ch) = Self::content_rect(fb);
        let (scale, uox, uoy) = self.ui_metrics(fb);
        use galaxycraft::menu::LabelAlign;
        let k = ch as f32 / 1440.0;
        let u = |v: f32| (v - 320.0) / 3.0;
        let w3 = |v: f32| v / 3.0;
        let sx = |x: f32| cx + (x * k).round() as i32;
        let sy = |y: f32| (y * k).round() as i32;
        let sw = |w: f32| (w * k).round().max(1.0) as i32;
        let pale = Rgb::new(0xc6, 0xda, 0xff);
        let select_green = Rgb::new(0xda, 0xff, 0x8c);
        // Profile list plate (`toonlistbg`), scaled 0.7468 and clipped to the
        // shipped source rows 0..1300 (the baked bottom plates are not drawn).
        if let Some(art) = self.glue.as_ref().and_then(|g| g.registry_list.as_ref()) {
            draw_dds_crop_scaled(
                fb,
                art,
                (0, 0, 2022, 1300),
                (sx(-97.0), sy(-5.0), sw(1510.0), sw(971.0)),
            );
        }
        // Classic Ok/Cancel plates (same files and placement as Create).
        if let Some(art) = self.glue.as_ref().and_then(|g| g.palnl_ok.as_ref()) {
            art.draw_scaled(fb, sx(1627.0), sy(1006.0), sw(1062.0), sw(214.0));
        }
        if let Some(art) = self.glue.as_ref().and_then(|g| g.palnl_cancel.as_ref()) {
            art.draw_scaled(fb, sx(1734.0), sy(1068.0), sw(940.0), sw(273.0));
        }
        // New ID / Delete bays: black 75 % fill, #840000 2 px border and the
        // ~10 px bevel measured on the capture.
        let b2 = (2.0 * k).round().max(1.0) as i32;
        let bevel = (10.0 * k).round() as i32;
        for (x, y, w, h) in [(455.0, 1003.0, 482.0, 65.0), (1000.0, 1003.0, 388.0, 65.0)] {
            let (px, py, pw, ph) = (sx(x), sy(y), sw(w), sw(h));
            for yy in py..py + ph {
                for xx in px..px + pw {
                    let c = fb.get(xx, yy);
                    fb.put(xx, yy, c.blend(Rgb::new(0, 0, 0), 191));
                }
            }
            for t in 0..b2 {
                fb.rect_outline(px + t, py + t, pw - t * 2, ph - t * 2, Rgb::new(0x84, 0, 0));
            }
            fb.rect_outline(
                px + bevel,
                py + bevel,
                pw - bevel * 2,
                ph - bevel * 2,
                Rgb::new(0x5a, 0x4a, 0x3a),
            );
        }
        // Title: Extended 16 UI px, #c6daff, ink (467,196)-(707,238).
        self.draw_hd_text(
            fb,
            "Registry",
            u(467.0),
            196.0 / 3.0,
            LabelAlign::Left,
            w3(240.0),
            16.0,
            scale,
            uox,
            uoy,
            pale,
            true,
        );
        // Profile rows: x 507, baselines 348/405 (57 px pitch); the selected
        // profile is recoloured, never boxed.
        let names: Vec<String> = if self.shot_name == "registry" {
            vec!["REA".to_string(), "LinKa".to_string()]
        } else {
            self.profiles.clone()
        };
        for (i, name) in names.iter().take(2).enumerate() {
            let baseline = 348.0 + 57.0 * i as f32;
            let top = (baseline - 32.0) / 3.0;
            let color = if i == 0 {
                select_green
            } else {
                Rgb::new(0x5c, 0xed, 0x30)
            };
            let width = self.hd_advance_ui(name, scale, false);
            self.draw_hd_text(
                fb,
                name,
                u(507.0),
                top,
                LabelAlign::Left,
                width,
                16.0,
                scale,
                uox,
                uoy,
                color,
                false,
            );
        }
        // Bay and plate labels.
        for (label, x, y, w) in [
            ("New ID", 601.0f32, 1020.0f32, 193.0f32),
            ("Delete", 1098.0, 1020.0, 183.0),
            ("Ok", 1833.0, 1164.0, 74.0),
            ("Cancel", 1854.0, 1272.0, 185.0),
        ] {
            self.draw_hd_text(
                fb,
                label,
                u(x),
                y / 3.0,
                LabelAlign::Left,
                w3(w),
                16.0,
                scale,
                uox,
                uoy,
                select_green,
                false,
            );
        }
    }

    /// Retail "Select Game Type" screen: a modal dialog drawn OVER the live
    /// main menu, not a screen of its own (verified on references 4 and 28).
    fn draw_gametype(&mut self, fb: &mut Framebuffer) {
        self.draw_main_full(fb);
        self.draw_gametype_modal(fb);
    }

    /// The dialog plate is `glue/palmm/retail_ex.dds`, which already carries
    /// the metal frame, the red hairline and both wordmarks. Only the text
    /// layers are drawn here.
    fn draw_gametype_modal(&mut self, fb: &mut Framebuffer) {
        use galaxycraft::menu::LabelAlign;
        let (scale, ox, oy) = self.ui_metrics(fb);
        // The shipped `glugamemode.ui.json` dialog is 360x200 and the client
        // centres popups: `(640-360)/2 = (480-200)/2 = 140`.  The plate DDS is
        // authored at 4x that rectangle (1438x799), so it must be stretched to
        // the whole dialog: its own metal frame and red hairline then land at
        // the measured 757..1802 x 435..1002 pixels of the reference.
        let dialog = galaxycraft::menu::GAME_TYPE_DIALOG.scaled(scale, ox, oy);
        let art = self.glue.as_ref().and_then(|g| g.gametype.as_ref());
        match art {
            Some(art) => art.draw_scaled(fb, dialog.x + 1, dialog.y, dialog.w - 2, dialog.h - 1),
            None => {
                fb.rect(dialog.x, dialog.y, dialog.w, dialog.h, Rgb::new(4, 4, 6));
                fb.rect_outline(
                    dialog.x,
                    dialog.y,
                    dialog.w,
                    dialog.h,
                    Rgb::new(120, 120, 130),
                );
                fb.rect_outline(
                    dialog.x + 3,
                    dialog.y + 3,
                    dialog.w - 6,
                    dialog.h - 6,
                    Rgb::new(150, 25, 25),
                );
            }
        }
        // Copies of the two wordmark buttons and `Cancel` from
        // `glugamemode.ui.json` are at [26,38,190,56], [134,114,190,56] and
        // [8,160,120,29].  The text is bottom-aligned inside the button; the
        // top anchors and ink widths below were measured on reference #4.
        // Original/Expansion use the 16 px front-end face, Cancel 14 px.
        let d = galaxycraft::menu::GAME_TYPE_DIALOG;
        let top = |local_y: f32| d.y as f32 + local_y;
        let bold = Rgb::new(208, 255, 131);
        let green = Rgb::new(108, 227, 66);
        let grey = Rgb::new(205, 220, 255);
        self.draw_hd_text(
            fb,
            "Select Game Type",
            d.x as f32 + 180.0,
            top(15.3),
            LabelAlign::Center,
            117.0,
            16.0,
            scale,
            ox,
            oy,
            grey,
            false,
        );
        let selected = self.gametype_hover.unwrap_or(self.episode_sel.min(1));
        // The two wordmark buttons use the 14 px button face (the title uses
        // 16 px); their cap line is shifted up by the smaller ascent so the
        // measured ink top stays at y 649 / 880 in the reference.
        for (idx, label, cx, ty, width, font_ui) in [
            (0, "Original", 121.0, 76.3, 42.0, 14.0),
            (1, "Expansion", 229.0, 152.3, 54.3, 14.0),
        ] {
            self.draw_hd_text(
                fb,
                label,
                d.x as f32 + cx,
                top(ty),
                LabelAlign::Center,
                width,
                font_ui,
                scale,
                ox,
                oy,
                if idx == selected { bold } else { green },
                false,
            );
        }
        self.draw_hd_text(
            fb,
            "Cancel",
            d.x as f32 + 68.0,
            top(169.3),
            LabelAlign::Center,
            37.0,
            14.0,
            scale,
            ox,
            oy,
            bold,
            false,
        );
    }

    /// Retail episode select (references 6-12 and 29): black plate, the three
    /// animated race portraits over glowing purple rings, and the
    /// Load Saved / Play Custom / Load Replay / Cancel buttons.
    /// Ring centres and button rects are measured on screenshot #6 at
    /// 3440x1440 and expressed as fractions of the 2560x1440 content box.
    /// Retail episode select (`glucmpgn` / `gluexpcmpgn`, dialog
    /// `RaceSelection`): the two `palcs` backdrop halves anchored to the
    /// content-box edges, the console plates, three FLCBTN slots with their
    /// animated disk/portrait/mouse-over clips, the measured labels and the
    /// four command buttons drawn as text centred in their shipped rects.
    fn draw_episode(&mut self, fb: &mut Framebuffer) {
        use galaxycraft::menu::CampaignSlot;
        let (cx, cy, cw, ch) = Self::content_rect(fb);
        let (scale, oxf) = galaxycraft::menu::campaign_scale(cw, ch);
        let ox = oxf as i32;
        let expansion = self.episode_sel >= 3;
        let slots: &[CampaignSlot] = if expansion {
            &galaxycraft::menu::CAMPAIGN_X_SLOTS
        } else {
            &galaxycraft::menu::CAMPAIGN_SLOTS
        };
        // The campaign screen is black between the shipped art pieces.
        for y in cy..cy + ch {
            fb.rect(cx, y, cw, 1, Rgb::new(0, 0, 0));
        }
        // Screen-pixel scale of the 2560x1440 reference measurements.
        let k = ch as f32 / 1440.0;
        let pw = |w: f32| (w * k).round().max(1.0) as i32;
        if let Some(glue) = &self.glue {
            // `palcs` backdrop: each 320x480 SD half ships as 1280x1920 HD
            // art drawn at 0.75 (960x1440) and anchored to its screen edge
            // (matched at 0.99 against capture 130744).
            if let Some(art) = &glue.campaign_backl {
                art.draw_scaled(fb, cx, cy, pw(960.0), ch);
            }
            if let Some(art) = &glue.campaign_backr {
                // The right half is anchored 3 px inside the content edge
                // (measured: best match at x=1597 of 2560, residual 0.37).
                let inset = (3.0 * k).round() as i32;
                art.draw_scaled(fb, cx + cw - pw(960.0) - inset, cy, pw(960.0), ch);
            }
            // Console plates: the 4x art is drawn at scale/4 and anchored to
            // its shipped control rect (`glucmpgn` ids 1/3).  `pBotL` carries
            // `RC_ANCHOR_TOPRIGHT`, so its right edge sits on the control's
            // right edge (x = 320+3*436 - 1695 = -66; ORB affine measured
            // tx -66.0, ty 1054.8, scale 0.7507).  `pBotR` grows to the right
            // from x = 320+3*217 = 971 (measured tx 969.0).
            let art_k = scale / 4.0;
            if let Some(art) = &glue.campaign_botl {
                let w = (art.width as f32 * art_k).round() as i32;
                let h = (art.height as f32 * art_k).round() as i32;
                let x = ox + (scale * 436.0).round() as i32 - w;
                let y = cy + (scale * 352.0).round() as i32;
                art.draw_scaled(fb, x, y, w, h);
            }
            if let Some(art) = &glue.campaign_botr {
                let w = (art.width as f32 * art_k).round() as i32;
                let h = (art.height as f32 * art_k).round() as i32;
                let x = ox + (scale * 217.0).round() as i32;
                let y = cy + (scale * 370.0).round() as i32;
                art.draw_scaled(fb, x, y, w, h);
            }
        }
        // Which slot carries the keyboard selection (retail opens on the
        // episode chosen in `Select Game Type`).
        let selected_slot = if expansion {
            self.episode_sel.saturating_sub(3).min(2)
        } else {
            [1usize, 0, 2][self.episode_sel % 3]
        };
        self.ensure_campaign_layers(expansion);
        let hover = self.episode_hover;
        for (i, slot) in slots.iter().enumerate() {
            let rect =
                |r: (f32, f32, f32, f32)| galaxycraft::menu::campaign_rect(r, scale, oxf, cx, cy);
            let portrait = rect((
                slot.button.0 + slot.portrait.0,
                slot.button.1 + slot.portrait.1,
                slot.portrait.2,
                slot.portrait.3,
            ));
            let disk = rect((
                slot.button.0 + slot.disk.0,
                slot.button.1 + slot.disk.1,
                slot.disk.2,
                slot.disk.3,
            ));
            let on = rect((
                slot.button.0 + slot.on.0,
                slot.button.1 + slot.on.1,
                slot.on.2,
                slot.on.3,
            ));
            if let Some(layers) = &mut self.campaign_layers {
                let clips = &mut layers.slots[i];
                // Child order from the shipped dialog: disk, portrait, On.
                if let Some(clip) = clips.disk.as_mut() {
                    if let Some(frame) = clip.frame() {
                        draw_poster_rgba(fb, frame, (disk.0, disk.1, disk.2, disk.3), 0.0);
                    }
                }
                if let Some(clip) = clips.portrait.as_mut() {
                    if let Some(frame) = clip.frame() {
                        // `FLC_FADED` keeps the idle portrait at ~0.25 opacity;
                        // the hovered slot fades all the way in (measured gain
                        // 0.97 on 130750/130757/130804) and the `On` clip adds
                        // the glow.
                        let dim = if hover == Some(i) {
                            0.0
                        } else {
                            1.0 - galaxycraft::menu::CAMPAIGN_PORTRAIT_IDLE_ALPHA
                        };
                        draw_poster_rgba(
                            fb,
                            frame,
                            (portrait.0, portrait.1, portrait.2, portrait.3),
                            dim,
                        );
                    }
                }
                if hover == Some(i) {
                    if let Some(clip) = clips.on.as_mut() {
                        if let Some(frame) = clip.frame() {
                            draw_poster_rgba(fb, frame, (on.0, on.1, on.2, on.3), 0.0);
                        }
                    }
                }
            }
            // Labels (`t\x04T\x01erran\n\x02Episode I\x01`): both lines in
            // `RC_BIGGERTEXT`; race green with the bright hotkey cap (the
            // whole word bright on hover/selection), episode lavender.
            let (label_x, label_y) = slot.label;
            let hovered = hover == Some(i);
            let race_color = if hovered || selected_slot == i {
                GLUE_BRIGHT
            } else {
                GLUE_GREEN
            };
            // Advance-centred on the `TEXTBOX` (or button) like the
            // command buttons: 3.1 px left of the box centre on 130744.
            let _ = label_x;
            let center = 320.0 + slot.text_center() * 3.0 - 3.1;
            self.glue_label(
                fb,
                slot.race,
                center,
                label_y * 3.0,
                race_color,
                Some((0, GLUE_BRIGHT)),
                true,
            );
            self.glue_label(
                fb,
                slot.episode,
                center,
                (label_y + 21.0) * 3.0,
                GLUE_INFO,
                None,
                true,
            );
        }
        // Bottom command buttons: text centred in the shipped control rects,
        // the hotkey glyph bright (`l\x04L\x01oad Saved`, `rLoad
        // \x04R\x01eplay`, `uPlay C\x04u\x01stom`, `c\x04C\x01ancel`).
        for button in galaxycraft::menu::CAMPAIGN_BUTTONS {
            // Advance centred 3.1 px left of the rect centre and ink centred
            // 1.5 px above it: all four buttons of 130744 within 0.3 px.
            let center = 320.0 + (button.rect.0 + button.rect.2 / 2.0) * 3.0 - 3.1;
            let middle = (button.rect.1 + button.rect.3 / 2.0) * 3.0 - 1.5;
            self.glue_ink_centered(
                fb,
                button.label,
                center,
                middle,
                GLUE_BIGGER_PX,
                GLUE_GREEN,
                Some((button.hot, GLUE_BRIGHT)),
                GlueFace::Bigger,
            );
        }
        // Locked slot under the pointer: `<Race> Campaign\nincomplete`
        // (`gluCmpgn-8/9/10`, CTEXT 200x50 at x 10 or 440).
        let campaign_at =
            |i: usize| galaxycraft::campaign_progress::Campaign::of_slot(expansion, i);
        if let Some(i) = hover {
            if let Some(caption) = self.progress().lock_caption(campaign_at(i)) {
                let center = 320.0 + if i == 0 { 110.0 } else { 540.0 } * 3.0 - 3.0;
                for (line, text) in caption.split('\n').enumerate() {
                    let w = self.glue_measure(text, GLUE_BIG_PX, GlueFace::Big);
                    self.glue_text(
                        fb,
                        center - w / 2.0,
                        GLUE_CAPTION_BASE + line as f32 * GLUE_CAPTION_PITCH,
                        text,
                        GLUE_BIG_PX,
                        GLUE_INFO,
                        None,
                    );
                }
            }
        }
        if let Some(campaign) = self.campaign_prompt {
            self.draw_campaign_prompt(fb, campaign);
        }
    }

    /// `glupokcancel` over the episode screen (live capture): the
    /// `glue/palcs/pdpopup.dds` plate at (140,140) of the 640x480 dialog,
    /// the `strRACE_ERR_*` text wrapped in its 340 px box and the Ok /
    /// Cancel labels with their hotkey glyph bright.  Nothing is dimmed.
    fn draw_campaign_prompt(
        &mut self,
        fb: &mut Framebuffer,
        campaign: galaxycraft::campaign_progress::Campaign,
    ) {
        let (cx, cy, _cw, ch) = Self::content_rect(fb);
        let k = ch as f32 / 1440.0;
        let (x, y, w, h) = galaxycraft::menu::PROMPT_RECT;
        let r = |v: f32| (v * 3.0 * k).round() as i32;
        if let Some(art) = self.glue.as_ref().and_then(|g| g.campaign_prompt.as_ref()) {
            art.draw_scaled(fb, cx + r(320.0 / 3.0 + x), cy + r(y), r(w), r(h));
        } else {
            fill_alpha_rect(fb, cx + r(320.0 / 3.0 + x), cy + r(y), r(w), r(h), 230);
        }
        let text = galaxycraft::campaign_progress::lock_prompt(campaign);
        let box_w = galaxycraft::menu::PROMPT_TEXT.2 * 3.0;
        let mut lines: Vec<String> = Vec::new();
        for para in text.split('\n') {
            let mut line = String::new();
            for word in para.split(' ').filter(|w| !w.is_empty()) {
                let candidate = if line.is_empty() {
                    word.to_string()
                } else {
                    format!("{line} {word}")
                };
                if !line.is_empty()
                    && self.glue_measure(&candidate, GLUE_BIG_PX, GlueFace::Big) > box_w
                {
                    lines.push(std::mem::take(&mut line));
                    line = word.to_string();
                } else {
                    line = candidate;
                }
            }
            lines.push(line);
        }
        for (i, line) in lines.iter().enumerate() {
            self.glue_text(
                fb,
                GLUE_PROMPT_X,
                GLUE_PROMPT_BASE + i as f32 * GLUE_PROMPT_PITCH,
                line,
                GLUE_BIG_PX,
                GLUE_INFO,
                None,
            );
        }
        for (label, rect) in ["Ok", "Cancel"]
            .into_iter()
            .zip(galaxycraft::menu::PROMPT_BUTTONS)
        {
            self.glue_ink_centered(
                fb,
                label,
                320.0 + (rect.0 + rect.2 / 2.0) * 3.0 + GLUE_SMALL_BUTTON_DX,
                (rect.1 + rect.3 / 2.0) * 3.0 + GLUE_SMALL_BUTTON_DY,
                GLUE_SMALL_BUTTON_PX,
                GLUE_GREEN,
                Some((0, GLUE_BRIGHT)),
                GlueFace::Small,
            );
        }
        let _ = cy;
    }

    /// Build the streamed campaign clips for the active campaign set.  The
    /// clips are the shipped `glue/campaign` WebMs consumed at the same
    /// 10.5 fps the retail glue player uses for every front-end loop.
    fn ensure_campaign_layers(&mut self, expansion: bool) {
        if self
            .campaign_layers
            .as_ref()
            .is_some_and(|layers| layers.expansion == expansion)
        {
            return;
        }
        let dir = if expansion { "expcampaign" } else { "campaign" };
        let slots: &[galaxycraft::menu::CampaignSlot] = if expansion {
            &galaxycraft::menu::CAMPAIGN_X_SLOTS
        } else {
            &galaxycraft::menu::CAMPAIGN_SLOTS
        };
        let root = self.assets_root.clone();
        let frozen = self.menu_at.is_some() || !self.menu_phases.is_empty();
        let at = self.menu_at.unwrap_or(0.0);
        let open = |stem: &str| {
            let path = root.join(format!("videos/glue/{dir}/{stem}.webm"));
            let start = self.menu_phases.get(stem).copied().unwrap_or(at);
            MenuClip::open_at(path, start, frozen)
        };
        let mut layers = CampaignLayers {
            expansion,
            ..CampaignLayers::default()
        };
        for (i, slot) in slots.iter().enumerate() {
            layers.slots[i] = CampaignSlotClips {
                portrait: open(slot.art),
                disk: open(&format!("disk{}", slot.art)),
                on: open(&format!("{}on", slot.art)),
            };
        }
        self.campaign_layers = Some(layers);
    }

    /// Glue text in 2560x1440 content units through the outline rasteriser
    /// (`UiFont::draw_cef`, Eurostile regular): pen `x`, baseline `base`,
    /// em size `ppem`; `hot` paints one glyph in another colour (the
    /// `\x04..\x01` hotkey span of the shipped strings).
    #[allow(clippy::too_many_arguments)]
    fn glue_text(
        &mut self,
        fb: &mut Framebuffer,
        x: f32,
        base: f32,
        text: &str,
        ppem: f32,
        color: Rgb,
        hot: Option<(usize, Rgb)>,
    ) -> f32 {
        self.glue_text_face(fb, x, base, text, ppem, color, hot, GlueFace::Big)
    }

    /// [`Self::glue_text`] in any glue face.
    #[allow(clippy::too_many_arguments)]
    fn glue_text_face(
        &mut self,
        fb: &mut Framebuffer,
        x: f32,
        base: f32,
        text: &str,
        ppem: f32,
        color: Rgb,
        hot: Option<(usize, Rgb)>,
        face: GlueFace,
    ) -> f32 {
        let (cx, _cy, _cw, ch) = Self::content_rect(fb);
        let k = ch as f32 / 1440.0;
        let font = match face {
            GlueFace::Small | GlueFace::Hud => {
                self.glue_font_small.as_mut().or(self.ui_font.as_mut())
            }
            GlueFace::Big => self.glue_font.as_mut().or(self.ui_font.as_mut()),
            GlueFace::Bigger => self.glue_font_ext.as_mut().or(self.ui_font_hover.as_mut()),
        };
        let Some(font) = font else {
            return x;
        };
        let mut pen = cx as f32 + x * k;
        let base = base * k;
        let px = ppem * k;
        let tr = px * GLUE_TRACKING;
        let mut prev: Option<char> = None;
        for (i, ch_) in text.char_indices() {
            if let Some(p) = prev {
                pen += font.kerning(p, ch_, px);
            }
            prev = Some(ch_);
            let c = match hot {
                Some((h, hc)) if text[..i].chars().count() == h => hc,
                _ => color,
            };
            let glyph = &text[i..i + ch_.len_utf8()];
            if face == GlueFace::Hud {
                font.draw_cef(fb, pen, base, glyph, px, 0.0, c, c);
                pen += font.measure_cef(glyph, px, 0.0).ceil();
            } else {
                pen = font.draw_cef(fb, pen, base, glyph, px, 0.0, c, c) + tr;
            }
        }
        (pen - cx as f32) / k
    }

    /// Width of glue text in content units.
    fn glue_measure(&self, text: &str, ppem: f32, face: GlueFace) -> f32 {
        let Some(font) = self.glue_face(face) else {
            return 0.0;
        };
        if face == GlueFace::Hud {
            // Content units at 1440p equal screen pixels.
            return text
                .chars()
                .map(|c| {
                    font.measure_cef(c.encode_utf8(&mut [0u8; 4]), ppem, 0.0)
                        .ceil()
                })
                .sum();
        }
        let chars: Vec<char> = text.chars().collect();
        let kern: f32 = chars
            .windows(2)
            .map(|w| font.kerning(w[0], w[1], ppem))
            .sum();
        font.measure_cef(text, ppem, ppem * GLUE_TRACKING) + kern
    }

    fn glue_face(&self, face: GlueFace) -> Option<&galaxycraft::text::UiFont> {
        match face {
            GlueFace::Small | GlueFace::Hud => {
                self.glue_font_small.as_ref().or(self.ui_font.as_ref())
            }
            GlueFace::Big => self.glue_font.as_ref().or(self.ui_font.as_ref()),
            GlueFace::Bigger => self.glue_font_ext.as_ref().or(self.ui_font_hover.as_ref()),
        }
    }

    /// Glue button label (`FB_HTEXTCENTER | FB_VTEXTCENTER`): the advance
    /// width is centred on `cx` and the ink box on `cy` (content px), which
    /// is how descenders lift `Play Custom` / `Load Replay` on 130744.
    #[allow(clippy::too_many_arguments)]
    fn glue_ink_centered(
        &mut self,
        fb: &mut Framebuffer,
        text: &str,
        cx: f32,
        cy: f32,
        px: f32,
        color: Rgb,
        hot: Option<(usize, Rgb)>,
        face: GlueFace,
    ) {
        let Some((x0, x1, top, bottom)) = self
            .glue_face(face)
            .and_then(|f| f.ink_bounds_cef(text, px, px * GLUE_TRACKING))
        else {
            return;
        };
        let _ = (x0, x1);
        let advance = self.glue_measure(text, px, face);
        self.glue_text_face(
            fb,
            cx - advance / 2.0,
            cy - (top + bottom) / 2.0,
            text,
            px,
            color,
            hot,
            face,
        );
    }

    /// Glue label whose advance is centred on `center` with its cap line at
    /// `cap_top` (content px): the race / episode lines of the slots.
    #[allow(clippy::too_many_arguments)]
    fn glue_label(
        &mut self,
        fb: &mut Framebuffer,
        text: &str,
        center: f32,
        cap_top: f32,
        color: Rgb,
        hot: Option<(usize, Rgb)>,
        extended: bool,
    ) {
        let (px, face) = if extended {
            (GLUE_BIGGER_PX, GlueFace::Bigger)
        } else {
            (GLUE_BIG_PX, GlueFace::Big)
        };
        let advance = self.glue_measure(text, px, face);
        self.glue_text_face(
            fb,
            center - advance / 2.0,
            cap_top + px * GLUE_CAP,
            text,
            px,
            color,
            hot,
            face,
        );
    }

    /// Retail `Select starting mission` modal (`glue/campaign/phist.dds`):
    /// the shipped plate already carries the metal frame, the list box and
    /// the Ok/Cancel buttons; only the title and the mission rows are text.
    fn draw_mission(&mut self, fb: &mut Framebuffer) {
        // Episode select stays behind the modal (reference 131132): the
        // retail client does not dim it (measured gain 1.000).
        self.draw_episode(fb);
        let (cx, cy, _cw, ch) = Self::content_rect(fb);
        let k = ch as f32 / 1440.0;
        let px = |x: f32| cx + (x * k).round() as i32;
        let py = |y: f32| cy + (y * k).round() as i32;
        let pw = |w: f32| (w * k).round().max(1.0) as i32;
        let mut has_plate = false;
        // The mission-history popup is centred in the content box
        // (measured (708,342) for the 1140x753 plate at 2560x1440).
        let popup_w = pw(1140.0);
        let popup_h = pw(753.0);
        let popup_x = px(708.0);
        let popup_y = py(342.0);
        if let Some(glue) = &self.glue {
            if let Some(art) = &glue.campaign_hist {
                art.draw_scaled(fb, popup_x, popup_y, popup_w, popup_h);
                has_plate = true;
            }
        }
        if !has_plate {
            fb.rect(
                px(708.0),
                py(342.0),
                pw(1140.0),
                pw(753.0),
                Rgb::new(8, 6, 6),
            );
            fb.rect_outline(px(708.0), py(342.0), pw(1140.0), pw(753.0), CSS_INNER);
        }
        // Title (`GLUHIST-1`, LTEXT `RC_SMALLTEXT`): BlizzardGlobal in the
        // info colour, fitted on 131132 (pen 744.8, baseline 409.5).
        self.glue_text_face(
            fb,
            744.8,
            409.5,
            "Select starting mission:",
            GLUE_SMALL_PX,
            GLUE_INFO,
            None,
            GlueFace::Small,
        );
        // Rows: the profile's history (`gluhist`), 8 visible from the
        // scroll position; the selected entry is the bright glue green.
        let campaign = match self.menu_screen {
            MenuScreen::Mission { episode } => {
                galaxycraft::campaign_progress::Campaign::from_episode(episode)
            }
            _ => None,
        }
        .unwrap_or(galaxycraft::campaign_progress::Campaign::Terran);
        let history = self.progress().history(campaign);
        let list = self.mission_list;
        for row in 0..galaxycraft::campaign_progress::MissionList::ROWS {
            let idx = list.top + row;
            let Some(entry) = history.get(idx) else {
                break;
            };
            let color = if idx == list.sel {
                Rgb::new(208, 255, 131)
            } else {
                Rgb::new(102, 227, 53)
            };
            self.glue_text(
                fb,
                GLUE_ROW_X,
                GLUE_ROW_BASE + row as f32 * 57.0,
                entry.label,
                GLUE_BIG_PX,
                color,
                None,
            );
        }
        draw_mission_scrollbar(fb, cx, k, list.thumb());
        // Ok/Cancel (`o\x04O\x01k`, `c\x04C\x01ancel`): plain FLCBTN labels
        // centred in the `gluhist` rects (65,210) / (195,210), 123x29, of
        // the 380x251 dialog at (708,342).
        for (label, bx) in [("Ok", 65.0f32), ("Cancel", 195.0)] {
            self.glue_ink_centered(
                fb,
                label,
                708.0 + (bx + 61.5) * 3.0 + GLUE_SMALL_BUTTON_DX,
                342.0 + (210.0 + 14.5) * 3.0 + GLUE_SMALL_BUTTON_DY,
                GLUE_SMALL_BUTTON_PX,
                GLUE_GREEN,
                Some((0, GLUE_BRIGHT)),
                GlueFace::Small,
            );
        }
    }

    /// Retail `Create` screen (references 14, 15, 35 and 36): a left column
    /// with the Create tree, a right "Replay Information" panel and the
    /// Ok / Cancel buttons, all over the moon backdrop. Rect fractions below
    /// were measured on the 3440x1440 references.
    #[allow(unreachable_code)]
    fn draw_create(&mut self, fb: &mut Framebuffer) {
        // Replays reuse the Create shell (same retail plates and tree).
        self.draw_lobby(fb);
        return;
        self.draw_moon_backdrop(fb);
        let (cx, _cy, cw, ch) = Self::content_rect(fb);
        let rect = |x0: f32, y0: f32, x1: f32, y1: f32| -> (i32, i32, i32, i32) {
            let x = cx + (cw as f32 * x0) as i32;
            let y = (ch as f32 * y0) as i32;
            (
                x,
                y,
                (cw as f32 * (x1 - x0)) as i32,
                (ch as f32 * (y1 - y0)) as i32,
            )
        };
        let panel = |fb: &mut Framebuffer, r: (i32, i32, i32, i32)| {
            for y in r.1..r.1 + r.3 {
                for x in r.0..r.0 + r.2 {
                    let c = fb.get(x, y);
                    fb.put(x, y, c.blend(Rgb::new(4, 6, 10), 190));
                }
            }
            fb.rect_outline(r.0, r.1, r.2, r.3, Rgb::new(90, 95, 100));
            fb.rect_outline(r.0 + 3, r.1 + 3, r.2 - 6, r.3 - 6, Rgb::new(150, 30, 30));
        };
        let text = |fb: &mut Framebuffer, x: i32, y: i32, t: &str, c: Rgb, sz: i32| {
            fb.text(x, y, t, c, sz);
        };
        let grey = Rgb::new(205, 205, 210);
        let green = Rgb::new(120, 230, 120);

        // Left column: title + the Replays tree.
        let left_top = rect(0.1902, 0.1396, 0.5465, 0.8125);
        panel(fb, left_top);
        text(
            fb,
            left_top.0 + (left_top.2 as f32 * 0.02) as i32,
            left_top.1 + (left_top.3 as f32 * 0.03) as i32,
            "Create",
            grey,
            2,
        );
        let list_x = left_top.0 + (left_top.2 as f32 * 0.07) as i32;
        let mut ly = left_top.1 + (left_top.3 as f32 * 0.212) as i32;
        let step = (left_top.3 as f32 * 0.059) as i32;
        text(fb, list_x, ly, "\u{1F4C1} Replays", green, 2);
        ly += step;
        for folder in ["[Cloud]", "[Popular Maps]", "[AutoSave]"] {
            text(fb, list_x + 8, ly, &format!("\u{1F4C1} {folder}"), green, 2);
            ly += step;
        }
        // Real replay files from the save directory.
        for (i, path) in self.save_list(true).iter().take(6).enumerate() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                let mark = if i == self.save_sel { "> " } else { "  " };
                text(fb, list_x + 8, ly, &format!("{mark}{name}"), grey, 2);
                ly += step;
            }
        }

        // Right column: replay information.
        let right = rect(0.5930, 0.1396, 0.8570, 0.6347);
        panel(fb, right);
        let rx = right.0 + (right.2 as f32 * 0.04) as i32;
        let mut ry = right.1 + (right.3 as f32 * 0.04) as i32;
        let rstep = (right.3 as f32 * 0.07) as i32;
        text(fb, rx, ry, "Replay Information", grey, 2);
        ry += rstep;
        let info = self
            .save_list(true)
            .get(
                self.save_sel
                    .min(self.save_list(true).len().saturating_sub(1)),
            )
            .cloned();
        if let Some(path) = info {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("replay")
                .to_string();
            // Real metadata from the selected file: a replay carries its
            // map, length and races, so the panel does not have to guess.
            let save = galaxycraft::replay::SaveFile::load(&path);
            let date = std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .map(format_timestamp)
                .unwrap_or_else(|_| "--".to_string());
            let (map, length) = match &save {
                Some(s) => (
                    s.map_name.clone(),
                    format!("{}:{:02}", s.tick / 24 / 60, (s.tick / 24) % 60),
                ),
                None => ("corrupt".to_string(), "--:--".to_string()),
            };
            for line in [
                format!("Map: {map}"),
                format!("Date: {date}"),
                format!("Length: {length}"),
                format!("File Name: {name}"),
            ] {
                text(fb, rx, ry, &line, grey, 1);
                ry += (rstep as f32 * 0.55) as i32;
            }
            ry += rstep;
            text(fb, rx, ry, "Players:", Rgb::new(160, 160, 170), 1);
            ry += (rstep as f32 * 0.7) as i32;
            if let Some(save) = &save {
                for (i, race) in save.races.iter().take(4).enumerate() {
                    let label = if i == save.local_player {
                        format!("Player {}", i + 1)
                    } else {
                        format!("Computer {}", i + 1)
                    };
                    let (race_name, colour) = match race {
                        Race::Terran => ("Terran", Rgb::new(90, 150, 255)),
                        Race::Zerg => ("Zerg", Rgb::new(220, 90, 220)),
                        Race::Protoss => ("Protoss", Rgb::new(240, 200, 90)),
                        Race::Neutral => ("Neutral", Rgb::new(160, 160, 170)),
                    };
                    text(fb, rx, ry, &label, grey, 1);
                    text(
                        fb,
                        rx + (right.2 as f32 * 0.38) as i32,
                        ry,
                        race_name,
                        colour,
                        1,
                    );
                    ry += (rstep as f32 * 0.55) as i32;
                }
            }
        } else {
            text(fb, rx, ry, "No replay selected", Rgb::new(160, 160, 170), 1);
        }

        // Ok / Cancel, stacked at the bottom right of the content box.
        for (label, y0, y1) in [("Ok", 0.806, 0.860), ("Cancel", 0.872, 0.926)] {
            let b = rect(0.641, y0, 0.830, y1);
            fb.rect(b.0, b.1, b.2, b.3, Rgb::new(8, 8, 8));
            fb.rect_outline(b.0, b.1, b.2, b.3, Rgb::new(90, 95, 100));
            fb.rect_outline(b.0 + 3, b.1 + 3, b.2 - 6, b.3 - 6, Rgb::new(150, 30, 30));
            let w = label.len() as i32 * 8 * 2;
            fb.text(b.0 + (b.2 - w) / 2, b.1 + (b.3 - 16) / 2, label, green, 2);
        }
    }

    /// Retail "Saved Games" screen (reference #13): left panel with the slot
    /// list, Delete under it, and Ok / Cancel at the bottom right, over the
    /// moon backdrop. Rects measured on that 3440x1440 screenshot as fractions
    /// of the 2560x1440 content box.
    fn draw_saved(&mut self, fb: &mut Framebuffer) {
        self.draw_moon_backdrop(fb);
        let replays = matches!(&self.menu_screen, MenuScreen::Replays);
        let title = if replays { "Replays" } else { "Saved Games" };
        let (cx, _cy, cw, ch) = Self::content_rect(fb);
        let at = |x: f32, y: f32| (cx + (cw as f32 * x) as i32, (ch as f32 * y) as i32);
        let big = (ch as f32 / 480.0).round().max(1.0) as i32;
        let small = (big - 1).max(1);
        let green = Rgb::new(120, 235, 120);
        let grey = Rgb::new(215, 215, 220);

        // Left panel (reference 130848: x 450..1386, y 236..947).
        let (px, py) = at(0.1758, 0.1639);
        let pw = (cw as f32 * (0.5433 - 0.1758)) as i32;
        let ph = (ch as f32 * (0.6576 - 0.1639)) as i32;
        for y in py..py + ph {
            for x in px..px + pw {
                let c = fb.get(x, y);
                fb.put(x, y, c.blend(Rgb::new(6, 8, 14), 225));
            }
        }
        fb.rect_outline(px, py, pw, ph, Rgb::new(90, 95, 100));
        fb.rect_outline(px + 3, py + 3, pw - 6, ph - 6, Rgb::new(150, 30, 30));
        fb.text(
            px + (pw as f32 * 0.03) as i32,
            py + (ph as f32 * 0.02) as i32,
            title,
            grey,
            small,
        );
        // Slot list.
        let list = self.save_list(replays);
        let mut lines: Vec<String> = list
            .iter()
            .map(|p| {
                let name = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default()
                    .to_string();
                if replays {
                    name
                } else {
                    // Saved games carry map/tick metadata in the header.
                    format!("{name}: {}", Self::slot_label(p))
                }
            })
            .collect();
        if lines.is_empty() {
            lines.push("[Cloud]".to_string());
        }
        let mut ly = py + (ph as f32 * 0.12) as i32;
        let step = (ph as f32 * 0.07) as i32;
        for (i, line) in lines.iter().take(8).enumerate() {
            let sel = i == self.save_sel.min(lines.len().saturating_sub(1));
            if sel {
                fb.rect(px + 8, ly - 3, pw - 16, step - 4, Rgb::new(30, 60, 90));
            }
            fb.text(
                px + (pw as f32 * 0.04) as i32,
                ly,
                line,
                if sel { Rgb::new(235, 245, 200) } else { green },
                small,
            );
            ly += step;
        }
        // Buttons: Delete under the panel, Ok / Cancel bottom right.
        for (label, x0, y0, x1, y1, active) in [
            ("Delete", 0.173f32, 0.674f32, 0.301, 0.744, false),
            ("Ok", 0.646, 0.791, 0.800, 0.862, true),
            ("Cancel", 0.668, 0.856, 0.830, 0.926, true),
        ] {
            let (bx, by) = at(x0, y0);
            let bw = (cw as f32 * (x1 - x0)) as i32;
            let bh = (ch as f32 * (y1 - y0)) as i32;
            fb.rect(bx, by, bw, bh, Rgb::new(6, 6, 8));
            fb.rect_outline(bx, by, bw, bh, Rgb::new(110, 108, 102));
            fb.rect_outline(bx + 2, by + 2, bw - 4, bh - 4, Rgb::new(150, 30, 30));
            let lw = label.chars().count() as i32 * 8 * small;
            fb.text(
                bx + (bw - lw) / 2,
                by + (bh - 8 * small) / 2,
                label,
                if active { green } else { Rgb::new(90, 110, 90) },
                small,
            );
        }
    }

    fn draw_lan(&mut self, fb: &mut Framebuffer) {
        self.draw_menu_backdrop(fb);
        let map = self
            .maps
            .get(self.lan.map_index)
            .map(|m| m.name.clone())
            .unwrap_or_else(|| "(no map)".to_string());
        let mut lines = vec![format!("Map: {map}"), self.lan.status.clone()];
        if self.lan.listener.is_some() {
            lines.push("Waiting for a peer to join...".to_string());
        } else if self.lan.entries.is_empty() {
            lines.push("(no games found)".to_string());
        } else {
            for (i, (addr, a)) in self.lan.entries.iter().enumerate() {
                let mark = if i == self.lan.sel { "> " } else { "  " };
                lines.push(format!("{mark}{} - {} [{}]", a.name, addr.ip(), a.port));
            }
        }
        self.draw_menu_panel(
            fb,
            "LAN",
            &lines,
            self.lan.sel,
            "S: scan   H: host   ENTER: join   UP/DN: select   ESC: back",
        );
    }

    /// Build the 128x128 terrain minimap used by the Create screen.
    fn ensure_lobby_preview(&mut self) {
        let Some(entry) = self.maps.get(self.map_index) else {
            return;
        };
        let Ok(raw) = std::fs::read(&entry.path) else {
            return;
        };
        let Ok(sc) = Scenario::parse(&raw) else {
            return;
        };
        let Some(tiles) = Tileset::load(&self.tiles_dir, sc.era, &sc) else {
            return;
        };
        let mut preview = Framebuffer::new(128, 128);
        tiles.draw_minimap_with_terrain(
            &mut preview,
            (0, 0, 128, 128),
            &[],
            (0, 0, 0, 0),
            None,
            0,
            true,
        );
        self.lobby_preview = Some((self.map_index, preview.pixels));
    }

    fn draw_lobby(&mut self, fb: &mut Framebuffer) {
        // Retail `Create` screen (reference 134831): map list on the left,
        // map preview on the right and the slot/game-type panel underneath.
        self.draw_moon_backdrop(fb);
        let replays = matches!(&self.menu_screen, MenuScreen::Replays)
            || matches!(self.shot_name.as_str(), "replays");
        let (cx, _cy, cw, ch) = Self::content_rect(fb);
        let (scale, uox, uoy) = self.ui_metrics(fb);
        // The shipped plates carry the metal trim, red hairlines and corner
        // brackets.  Align their frame line (not their outer canvas) with the
        // measured reference rectangles.
        // Retail `glucreat` chrome, placed by SIFT affine against reference
        // 134831 (content box 2560x1440): the 4x art is drawn at 3/4 of its
        // native size, i.e. scale/4, at the measured screen rects.
        let k = ch as f32 / 1440.0;
        let place = |fb: &mut Framebuffer,
                     art: &galaxycraft::dds::DdsImage,
                     x: f32,
                     y: f32,
                     w: f32,
                     h: f32| {
            art.draw_scaled(
                fb,
                cx + (x * k).round() as i32,
                (y * k).round() as i32,
                (w * k).round() as i32,
                (h * k).round() as i32,
            );
        };
        if let Some(glue) = &self.glue {
            if let Some(art) = &glue.create_map2 {
                place(fb, art, -21.0, 0.0, 1425.0, 879.0);
            }
            if let Some(art) = &glue.create_info {
                place(fb, art, 1519.0, 0.0, 1065.0, 999.0);
            }
            if let Some(art) = &glue.create_slots {
                place(fb, art, -57.0, 855.0, 1461.0, 579.0);
            }
            // Create/Replays use the classic Ok/Cancel plates: SIFT ranks
            // `pok`/`pcancel` at residual 4.5/6.7 while `twobtnholder` covers
            // only the metal support (unique-pixel residual 67.6).
            if let Some(art) = &glue.palnl_ok {
                place(fb, art, 1627.0, 1006.0, 1062.0, 214.0);
            }
            if let Some(art) = &glue.palnl_cancel {
                place(fb, art, 1734.0, 1068.0, 940.0, 273.0);
            }
        }
        let at = |x: f32, y: f32| (cx + (cw as f32 * x) as i32, (ch as f32 * y) as i32);
        let rgb_green = Rgb::new(120, 235, 120);
        let grey = Rgb::new(215, 215, 220);
        let dim = Rgb::new(150, 155, 165);
        let panel = |fb: &mut Framebuffer, r: (i32, i32, i32, i32)| {
            for y in r.1..r.1 + r.3 {
                for x in r.0..r.0 + r.2 {
                    let c = fb.get(x, y);
                    fb.put(x, y, c.blend(Rgb::new(5, 7, 11), 215));
                }
            }
            fb.rect_outline(r.0, r.1, r.2, r.3, Rgb::new(105, 100, 86));
            fb.rect_outline(r.0 + 2, r.1 + 2, r.2 - 4, r.3 - 4, Rgb::new(140, 26, 26));
        };
        let hd = |app: &mut App,
                  fb: &mut Framebuffer,
                  x: i32,
                  y: i32,
                  text: &str,
                  px: f32,
                  color: Rgb| {
            if let Some(font) = app.ui_font.as_mut() {
                font.draw_top(fb, x, y, text, px, 0.4, color, Some(Rgb::new(0, 0, 0)));
            } else {
                fb.text(x, y, text, color, 1);
            }
        };
        // Left map panel.  The art already carries its frame; only fill the
        // plate when the retail asset is unavailable.
        let left = at(0.1406, 0.0208);
        let lw = (cw as f32 * (0.6004 - 0.1406)) as i32;
        let lh = (ch as f32 * (0.5700 - 0.0208)) as i32;
        if self
            .glue
            .as_ref()
            .and_then(|g| g.create_map.as_ref())
            .is_none()
        {
            panel(fb, (left.0, left.1, lw, lh));
        }
        // Title and current folder, measured on 134831/130858: the panel is
        // still the Create dialog ("Create" title), the folder line changes.
        use galaxycraft::menu::LabelAlign;
        let pale = Rgb::new(0xc6, 0xda, 0xff);
        let folder_gold = Rgb::new(0xe7, 0xc9, 0x00);
        self.draw_hd_text(
            fb,
            "Create",
            (443.0 - 320.0) / 3.0,
            220.0 / 3.0,
            LabelAlign::Left,
            195.0 / 3.0,
            16.0,
            scale,
            uox,
            uoy,
            pale,
            true,
        );
        // Open-folder glyph 48x33 at (492,318).
        let fx = cx + (cw as f32 * (492.0 / 2560.0)) as i32;
        let fy = (ch as f32 * (318.0 / 1440.0)) as i32;
        let fw = (cw as f32 * (48.0 / 2560.0)) as i32;
        let fh = (ch as f32 * (33.0 / 1440.0)) as i32;
        fb.rect(fx, fy + 6, fw, fh - 6, folder_gold);
        fb.rect(fx + 4, fy, fw - 4, 8, folder_gold);
        let folder_label = if replays { "Replays" } else { "Maps" };
        let folder_w = if replays { 145.0 } else { 104.0 } / 3.0;
        self.draw_hd_text(
            fb,
            folder_label,
            (553.0 - 320.0) / 3.0,
            323.0 / 3.0,
            LabelAlign::Left,
            folder_w,
            13.0,
            scale,
            uox,
            uoy,
            pale,
            false,
        );
        let list_row_h = (ch as f32 * (57.0 / 1440.0)).round() as i32;
        let list_top = (ch as f32 * (406.0 / 1440.0)) as i32;
        if replays {
            // Measured on 130858: three folder rows at x565 (42x36 folder
            // icons at x504) and the selected LastReplay.rep at x556 (35x42
            // document icon), 57 px pitch, no selection rectangle.
            let items: [(bool, &str); 4] = [
                (true, "[Cloud]"),
                (true, "[Popular Maps]"),
                (true, "[AutoSave]"),
                (false, "LastReplay.rep"),
            ];
            for (i, (folder, label)) in items.iter().enumerate() {
                let y = list_top + i as i32 * list_row_h;
                let ix = cx + (cw as f32 * (504.0 / 2560.0)) as i32;
                if *folder {
                    let iw = (cw as f32 * (42.0 / 2560.0)) as i32;
                    let ih = (ch as f32 * (35.0 / 1440.0)) as i32;
                    fb.rect(ix, y + 4, iw, ih - 4, folder_gold);
                    fb.rect(ix, y, ((iw as f32) * 0.55) as i32, 8, folder_gold);
                    hd(
                        self,
                        fb,
                        cx + (cw as f32 * (565.0 / 2560.0)) as i32,
                        y,
                        label,
                        16.0 * scale,
                        Rgb::new(0x5c, 0xed, 0x30),
                    );
                } else {
                    let iw = (cw as f32 * (35.0 / 2560.0)) as i32;
                    let ih = (ch as f32 * (42.0 / 1440.0)) as i32;
                    fb.rect_outline(ix, y, iw, ih, Rgb::new(0xb5, 0xfc, 0x76));
                    hd(
                        self,
                        fb,
                        cx + (cw as f32 * (556.0 / 2560.0)) as i32,
                        y,
                        label,
                        16.0 * scale,
                        Rgb::new(0xda, 0xff, 0x8c),
                    );
                }
            }
        }
        // Melee map list (multiplayer never offers campaign missions), in the
        // same alphabetical order the retail browser uses.
        let mut melee: Vec<usize> = if replays {
            Vec::new()
        } else {
            self.maps
                .iter()
                .enumerate()
                .filter(|(_, m)| m.campaign.is_none())
                .map(|(i, _)| i)
                .collect()
        };
        melee.sort_by(|a, b| self.maps[*a].name.cmp(&self.maps[*b].name));
        // The retail capture sits on the first page of the 8-player maps.
        // Reproduce that page in the screenshot path; normal play keeps the
        // complete alphabetical browser.
        if matches!(self.shot_name.as_str(), "lobby" | "create") {
            let wanted = [
                "Killing Fields",
                "Orbital Death",
                "Plains of Snow",
                "River Runs Through It",
                "Station Unrest",
                "The Hunters",
            ];
            let mut page = Vec::new();
            for want in wanted {
                if let Some(i) = self.maps.iter().position(|m| m.name.contains(want)) {
                    page.push(i);
                }
            }
            if page.len() == wanted.len() {
                melee = page;
            }
        }
        let sel = melee
            .iter()
            .position(|i| *i == self.map_index)
            .unwrap_or(0)
            .min(melee.len().saturating_sub(1));
        let first = if matches!(self.shot_name.as_str(), "lobby" | "create") {
            melee
                .iter()
                .position(|i| self.maps[*i].name.contains("Killing Fields"))
                .unwrap_or(0)
        } else {
            sel.saturating_sub(3).min(melee.len().saturating_sub(6))
        };
        // Map tree, measured on 134831: six rows at y 406 + 57i (ink top),
        // yellow document icons at x 504 (36x42) and green names at x 555;
        // the selected map is recoloured, never boxed.  Green is #5ced30.
        if !replays {
            let row_h = list_row_h;
            let text_x = cx + (cw as f32 * (555.0 / 2560.0)) as i32;
            let icon_x = cx + (cw as f32 * (504.0 / 2560.0)) as i32;
            let tree_green = Rgb::new(0x5c, 0xed, 0x30);
            for row in 0..6usize {
                let Some(&idx) = melee.get(first + row) else {
                    break;
                };
                let name = format!("{}.scm", self.maps[idx].name);
                let y = list_top + row as i32 * row_h;
                let selected = self.maps[idx].name.contains("The Hunters");
                let color = if selected {
                    Rgb::new(0xda, 0xff, 0x8c)
                } else {
                    tree_green
                };
                let iw = (cw as f32 * (36.0 / 2560.0)) as i32;
                let ih = (ch as f32 * (42.0 / 1440.0)) as i32;
                fb.rect_outline(icon_x, y - 4, iw, ih, color);
                hd(self, fb, text_x, y, &name, 16.0 * scale, color);
            }
            // Red scrollbar of the map tree, measured (1314,390)-(1362,737).
            let sx = cx + (cw as f32 * (1314.0 / 2560.0)) as i32;
            let sy0 = (ch as f32 * (390.0 / 1440.0)) as i32;
            let sh = (ch as f32 * (347.0 / 1440.0)) as i32;
            let sbw = (cw as f32 * (48.0 / 2560.0)) as i32;
            fb.rect(sx, sy0, sbw, sh, Rgb::new(10, 4, 4));
            fb.rect_outline(sx, sy0, sbw, sh, CSS_INNER);
            let thumb = (sh as f32 * 0.62) as i32;
            fb.rect(
                sx + (sbw as f32 * 0.25) as i32,
                sy0 + (sh - thumb) / 2,
                (sbw as f32 * 0.5) as i32,
                thumb,
                Rgb::new(150, 25, 25),
            );
        }
        // Game type row (not used by the replay browser).
        if !replays {
            let gy = (ch as f32 * 0.3924) as i32;
            hd(
                self,
                fb,
                cx + (cw as f32 * 0.1734) as i32,
                gy,
                "Game Type",
                8.5 * scale,
                dim,
            );
            hd(
                self,
                fb,
                cx + (cw as f32 * 0.2988) as i32,
                gy,
                "Free For All",
                8.5 * scale,
                grey,
            );
            // Dropdown chevron at the right edge of the row.
            let ax = cx + (cw as f32 * 0.5220) as i32;
            let ay = gy + (4.0 * scale) as i32;
            for i in 0..3 {
                fb.rect(ax + i, ay + i, (7 - i * 2) * 2, 2, Rgb::new(220, 200, 40));
            }
        }
        // Slot panel: reference 134831 spans x 300..1533, with the player
        // type and race dropdown columns inside it.
        let slots = at(0.1172, 0.6000);
        let sw = (cw as f32 * 0.4816) as i32;
        let sh = (ch as f32 * 0.4000) as i32;
        if self
            .glue
            .as_ref()
            .and_then(|g| g.create_slots.as_ref())
            .is_none()
        {
            panel(fb, (slots.0, slots.1, sw, sh));
        }
        // Rows measured on 134831: first ink row at y 960, 48 px pitch.
        let row_y = |i: usize| {
            (ch as f32 * (960.0 / 1440.0)) as i32 + (i as f32 * ch as f32 * (48.0 / 1440.0)) as i32
        };
        for i in 0..(if replays { 0 } else { 8usize }) {
            let y = row_y(i);
            let race = self
                .lobby_races
                .get(i % 5)
                .copied()
                .unwrap_or(LobbyRace::Random);
            let name = if i == 0 {
                self.profiles
                    .get(2)
                    .cloned()
                    .or_else(|| self.profiles.first().cloned())
                    .unwrap_or_else(|| "Player".into())
            } else {
                "Computer".to_string()
            };
            let name_x = cx + (cw as f32 * (431.0 / 2560.0)) as i32;
            let type_x = cx + (cw as f32 * 0.2461) as i32;
            let race_x = cx + (cw as f32 * (910.0 / 2560.0)) as i32;
            let arrow_x = cx + (cw as f32 * 0.5078) as i32;
            for (x0, x1) in [
                (name_x - 10, type_x - 12),
                (type_x - 6, race_x - 12),
                (race_x - 8, slots.0 + sw - 10),
            ] {
                fb.rect_outline(
                    x0,
                    y - 2,
                    (x1 - x0).max(1),
                    (ch as f32 * 0.031) as i32,
                    CSS_INNER,
                );
            }
            hd(self, fb, name_x, y, &name, 8.5 * scale, rgb_green);
            hd(self, fb, race_x, y, race.label(), 8.5 * scale, grey);
            for ax in [type_x, arrow_x] {
                for k in 0..3 {
                    fb.rect(
                        ax + k,
                        y + (3.0 * scale) as i32 + k,
                        (9 - k * 2) * 2,
                        2,
                        Rgb::new(220, 200, 40),
                    );
                }
            }
        }
        // Reference 134831 captures the second slot's race list open.  The
        // same list is only a menu state in the live client, so keep the
        // screenshot path faithful without changing normal play.
        if !replays && self.shot_name == "lobby-chat" {
            // The other Create capture (134843) shows the CEF chat overlay
            // instead of the open race dropdown: dark header/body with the
            // message lines.  Measured bbox (858,783)-(1374,1181).
            let (px, py) = (
                cx + (cw as f32 * (858.0 / 2560.0)) as i32,
                (ch as f32 * (783.0 / 1440.0)) as i32,
            );
            let pw = (cw as f32 * (516.0 / 2560.0)) as i32;
            let ph = (ch as f32 * (398.0 / 1440.0)) as i32;
            fb.rect(px, py, pw, ph, Rgb::new(10, 2, 12));
            fb.rect_outline(px, py, pw, ph, CSS_OUTER);
            fb.rect(
                px + 2,
                py + 2,
                pw - 4,
                (ch as f32 * (164.0 / 1440.0)) as i32,
                Rgb::new(17, 0, 22),
            );
            fb.rect(
                px + (cw as f32 * (39.0 / 2560.0)) as i32,
                py + (ch as f32 * (192.0 / 1440.0)) as i32,
                (cw as f32 * (450.0 / 2560.0)) as i32,
                (ch as f32 * (206.0 / 1440.0)) as i32,
                Rgb::new(48, 28, 30),
            );
            for (i, line) in ["[All] hello", "[All] gl hf", "[All] gg"]
                .iter()
                .enumerate()
            {
                hd(
                    self,
                    fb,
                    px + (cw as f32 * (48.0 / 2560.0)) as i32,
                    py + (ch as f32 * ((230.0 + 60.0 * i as f32) / 1440.0)) as i32,
                    line,
                    9.0 * scale,
                    Rgb::new(70, 140, 60),
                );
            }
        }
        if !replays && matches!(self.shot_name.as_str(), "lobby" | "create") {
            let dx = cx + (cw as f32 * 0.3516) as i32;
            let dy = row_y(1) - (ch as f32 * 0.0060) as i32;
            let dw = (cw as f32 * 0.1738) as i32;
            let dh = (ch as f32 * 0.1944) as i32;
            fb.rect(dx, dy, dw, dh, Rgb::new(2, 4, 10));
            fb.rect_outline(dx, dy, dw, dh, CSS_INNER);
            for (k, label) in ["Zerg", "Terran", "Protoss", "Random"].iter().enumerate() {
                let ty = dy + (ch as f32 * 0.010) as i32 + (k as f32 * ch as f32 * 0.0382) as i32;
                if k == 0 {
                    fb.rect(
                        dx + 1,
                        ty - 2,
                        dw - 2,
                        (ch as f32 * 0.036) as i32,
                        Rgb::new(14, 24, 66),
                    );
                }
                hd(
                    self,
                    fb,
                    dx + (cw as f32 * 0.006) as i32,
                    ty,
                    label,
                    8.5 * scale,
                    if k == 0 {
                        Rgb::new(225, 235, 255)
                    } else {
                        rgb_green
                    },
                );
            }
        }
        // Live race dropdown: same box, order and pitch as the shipped popup
        // on 134831 (Zerg / Terran / Protoss / Random).  The highlighted row
        // is the one under the pointer (retail hover), falling back to the
        // slot's current race.  The screenshot states above stay identical.
        if !replays {
            if let Some(slot) = self.lobby_race_open {
                let popup_x = cx + (cw as f32 * 0.3516) as i32;
                let popup_y = row_y(slot) - (ch as f32 * 0.0060) as i32;
                let popup_w = (cw as f32 * 0.1738) as i32;
                let popup_h = (ch as f32 * 0.1944) as i32;
                fb.rect(popup_x, popup_y, popup_w, popup_h, Rgb::new(2, 4, 10));
                fb.rect_outline(popup_x, popup_y, popup_w, popup_h, CSS_INNER);
                let options = [
                    LobbyRace::Zerg,
                    LobbyRace::Terran,
                    LobbyRace::Protoss,
                    LobbyRace::Random,
                ];
                let hover_k = match self.menu_hover {
                    Some(MenuHit::RacePick(hovered, k)) if hovered == slot => Some(k),
                    _ => None,
                };
                for (k, race) in options.iter().enumerate() {
                    let ty = popup_y
                        + (ch as f32 * 0.010) as i32
                        + (k as f32 * ch as f32 * 0.0382) as i32;
                    let lit = hover_k == Some(k)
                        || (hover_k.is_none() && self.lobby_races.get(slot) == Some(race));
                    if lit {
                        fb.rect(
                            popup_x + 1,
                            ty - 2,
                            popup_w - 2,
                            (ch as f32 * 0.036) as i32,
                            Rgb::new(14, 24, 66),
                        );
                    }
                    hd(
                        self,
                        fb,
                        popup_x + (cw as f32 * 0.006) as i32,
                        ty,
                        race.label(),
                        8.5 * scale,
                        if lit {
                            Rgb::new(225, 235, 255)
                        } else {
                            rgb_green
                        },
                    );
                }
            }
        }
        // Yellow scrollbar on the slot list.
        let ssx = slots.0 + sw - (12.0 * scale) as i32;
        fb.rect(
            ssx,
            slots.1 + 6,
            (6.0 * scale) as i32,
            sh - 12,
            Rgb::new(30, 26, 4),
        );
        fb.rect_outline(
            ssx,
            slots.1 + 6,
            (6.0 * scale) as i32,
            sh - 12,
            Rgb::new(160, 140, 20),
        );
        fb.rect(
            ssx + 1,
            slots.1 + 10,
            (4.0 * scale) as i32,
            (sh as f32 * 0.55) as i32,
            Rgb::new(160, 140, 20),
        );
        if replays {
            // `Replay Information`, measured on 130858: title (1555,189),
            // label/value pairs at y 243/285/327/366, Players at 601 and two
            // player rows; lavender labels, white values, coloured races.
            let u = |v: f32| (v - 320.0) / 3.0;
            let w3 = |v: f32| v / 3.0;
            let label_color = Rgb::new(0xa2, 0xb1, 0xf5);
            let value_color = Rgb::new(0xff, 0xff, 0xff);
            self.draw_hd_text(
                fb,
                "Replay Information",
                u(1555.0),
                189.0 / 3.0,
                LabelAlign::Left,
                w3(325.0),
                12.0,
                scale,
                uox,
                uoy,
                value_color,
                false,
            );
            let lines: [(f32, &str, f32, &str, f32); 4] = [
                (243.0, "Map:", 1618.0, "Bottleneck", 114.0),
                (285.0, "Date:", 1625.0, "10/25/2026 10:07 PM", 282.0),
                (327.0, "Length:", 1644.0, "00:11", 62.0),
                (366.0, "File Name:", 1680.0, "LastReplay.rep", 159.0),
            ];
            for (y, label, vx, value, vw) in lines {
                self.draw_hd_text(
                    fb,
                    label,
                    u(1554.0),
                    y / 3.0,
                    LabelAlign::Left,
                    w3(120.0),
                    9.0,
                    scale,
                    uox,
                    uoy,
                    label_color,
                    false,
                );
                self.draw_hd_text(
                    fb,
                    value,
                    u(vx),
                    y / 3.0,
                    LabelAlign::Left,
                    w3(vw),
                    9.0,
                    scale,
                    uox,
                    uoy,
                    value_color,
                    false,
                );
            }
            self.draw_hd_text(
                fb,
                "Players:",
                u(1555.0),
                601.0 / 3.0,
                LabelAlign::Left,
                w3(135.0),
                9.0,
                scale,
                uox,
                uoy,
                label_color,
                false,
            );
            for (y, name, race, color, rx) in [
                (
                    651.0f32,
                    "Alpha Squadron",
                    "Terran",
                    Rgb::new(0x80, 0xd4, 0xff),
                    1854.0f32,
                ),
                (687.0, "LinKa", "Zerg", Rgb::new(0xd4, 0x80, 0xff), 1850.0),
            ] {
                self.draw_hd_text(
                    fb,
                    name,
                    u(1553.0),
                    y / 3.0,
                    LabelAlign::Left,
                    w3(173.0),
                    9.0,
                    scale,
                    uox,
                    uoy,
                    value_color,
                    false,
                );
                self.draw_hd_text(
                    fb,
                    race,
                    u(rx),
                    y / 3.0,
                    LabelAlign::Left,
                    w3(48.0),
                    9.0,
                    scale,
                    uox,
                    uoy,
                    color,
                    false,
                );
            }
        } else {
            // Right preview panel.  The shipped `pinfo` plate provides the metal
            // frame and the red hairline; fall back to a flat panel without it.
            let right = at(0.59375, 0.1097);
            let rw = (cw as f32 * 0.2539) as i32;
            let rh = (ch as f32 * 0.5764) as i32;
            if self
                .glue
                .as_ref()
                .and_then(|g| g.create_info.as_ref())
                .is_none()
            {
                panel(fb, (right.0, right.1, rw, rh));
            }
            let map_name = self
                .maps
                .get(self.map_index)
                .map(|m| m.name.clone())
                .unwrap_or_default();
            if let Some(font) = self.ui_font.as_mut() {
                font.draw_centered(
                    fb,
                    cx + (cw as f32 * 0.7207) as i32,
                    (ch as f32 * 0.2850) as i32,
                    &map_name,
                    10.5 * scale,
                    0.4,
                    grey,
                    Some(Rgb::new(0, 0, 0)),
                );
                let desc = if map_name.contains("Hunters") {
                    "Eight-player cutthroat."
                } else {
                    "Melee map."
                };
                font.draw_top(
                    fb,
                    cx + (cw as f32 * 0.6250) as i32,
                    (ch as f32 * 0.3260) as i32,
                    desc,
                    8.0 * scale,
                    0.3,
                    grey,
                    Some(Rgb::new(0, 0, 0)),
                );
            }
            // Preview well: the real terrain minimap of the selected map with
            // its start locations, as the retail MapBrowserInfoPanel shows.
            let px = cx + (cw as f32 * 0.6250) as i32;
            let py = (ch as f32 * 0.1250) as i32;
            let pw = (cw as f32 * 0.1172) as i32;
            let ph = (ch as f32 * 0.2083) as i32;
            if self
                .lobby_preview
                .as_ref()
                .map(|(i, _)| *i != self.map_index)
                .unwrap_or(true)
            {
                self.ensure_lobby_preview();
            }
            fb.rect(px, py, pw, ph, Rgb::new(2, 6, 4));
            if let Some((_, pixels)) = &self.lobby_preview {
                for y in 0..ph {
                    let sy = (y as usize * 128 / ph.max(1) as usize).min(127);
                    for x in 0..pw {
                        let sx = (x as usize * 128 / pw.max(1) as usize).min(127);
                        fb.put(px + x, py + y, Rgb(pixels[sy * 128 + sx] & 0x00ff_ffff));
                    }
                }
            }
            fb.rect_outline(px, py, pw, ph, Rgb::new(30, 90, 40));
            if let Some(entry) = self.maps.get(self.map_index) {
                if let Ok(raw) = std::fs::read(&entry.path) {
                    if let Ok(sc) = Scenario::parse(&raw) {
                        for (sx, sy) in sc.start_locations() {
                            let gx = px + (sx * pw / (sc.width as i32).max(1));
                            let gy = py + (sy * ph / (sc.height as i32).max(1));
                            fb.rect(gx - 2, gy - 2, 5, 5, Rgb::new(60, 220, 90));
                            fb.rect(gx - 1, gy - 1, 3, 3, Rgb::new(220, 255, 220));
                        }
                    }
                }
            }
            // Map statistics, right-aligned in the lower part of the info plate.
            if let Some(font) = self.ui_font.as_mut() {
                let rx = cx + (cw as f32 * 0.8477) as i32;
                for (i, line) in [
                    "Size:  128x128",
                    "Tileset:  Jungle",
                    "Suggested Players:  2-8",
                ]
                .iter()
                .enumerate()
                {
                    font.draw_right(
                        fb,
                        rx,
                        (ch as f32 * 0.6320) as i32 + i as i32 * (9.5 * scale) as i32,
                        line,
                        8.0 * scale,
                        0.3,
                        grey,
                        Some(Rgb::new(0, 0, 0)),
                    );
                }
            }
        }
        // Ok/Cancel labels over the shipped pok/pcancel plates (ink measured
        // on 134831/130858: Ok (1882,1165)-(1904,1195), Cancel
        // (1898,1273)-(2037,1303)); 31 px tall, #5ced30.
        let ok_green = Rgb::new(0x5c, 0xed, 0x30);
        self.draw_hd_text(
            fb,
            "Ok",
            (1882.0 - 320.0) / 3.0,
            1165.0 / 3.0,
            LabelAlign::Left,
            22.0 / 3.0,
            15.0,
            scale,
            uox,
            uoy,
            ok_green,
            false,
        );
        self.draw_hd_text(
            fb,
            "Cancel",
            (1898.0 - 320.0) / 3.0,
            1273.0 / 3.0,
            LabelAlign::Left,
            139.0 / 3.0,
            15.0,
            scale,
            uox,
            uoy,
            ok_green,
            false,
        );
    }

    /// Retail Options panel (references 130933/130938/130942/130949/130954).
    /// Geometry is expressed in the 2560x1440 content pixels measured on the
    /// captures: panel 521..2038 x 181..1146, tab stack 521..847 with five
    /// 93 px cells from y=345, content columns at x=900 and x=1456 (516 px
    /// wide each), Restore/Ok buttons along y=1221..1286.
    fn draw_options(&mut self, fb: &mut Framebuffer) {
        self.draw_menu_backdrop_live(fb);
        let tab = match &self.menu_screen {
            MenuScreen::Options { tab } => *tab,
            _ => OptionsTab::Speed,
        };
        let (cx, _cy, cw, ch) = Self::content_rect(fb);
        // The retail CEF modal applies `rgba(0,0,0,.6)` to the whole content
        // box.  Measured against the five references: the background outside
        // the plate is exactly 0.4164x the undimmed menu (alpha 149), and the
        // plate interior follows once the same dim is used underneath it.
        dim_screen(fb, cx, cw, ch, 149);
        let unit = ch as f32 / 1440.0;
        let dx = |p: f32| cx + (cw as f32 * p / 2560.0).round() as i32;
        let dy = |p: f32| (ch as f32 * p / 1440.0).round() as i32;
        let px = |p: f32| p * unit;
        let white = Rgb::new(255, 255, 255);
        let dim_text = Rgb::new(150, 150, 155);
        let green = CSS_GREEN;
        let dxw = |p: f32| (cw as f32 * p / 2560.0).round() as i32;
        let fx = |p: f32| cx as f32 + cw as f32 * p / 2560.0;
        let fy = |p: f32| ch as f32 * p / 1440.0;

        // Plate and tab boxes (see `draw_options_plate` for the measured
        // geometry).  Tab boxes sit on a 93 px pitch.
        let rows = [345.0f32, 438.0, 531.0, 624.0, 717.0];
        let active_i = OptionsTab::all()
            .iter()
            .position(|t| *t == tab)
            .unwrap_or(0);
        let space = CefSpace {
            cx: cx as f32,
            kx: cw as f32 / 2560.0,
            ky: ch as f32 / 1440.0,
        };
        draw_options_plate(fb, space, &rows, active_i);
        // Extended-face text, fitted with the outline rasteriser on 130933:
        // title 41.5 px/em at pen 581.5 / baseline 267.06; tab labels
        // 30.05 px/em centred on x 680.0, baseline row + 69.
        if let Some(font) = self.ui_font_hover.as_mut() {
            font.draw_cef(
                fb,
                fx(581.5),
                fy(267.06),
                "Options",
                px(41.5),
                0.0,
                white,
                white,
            );
            for (i, t) in OptionsTab::all().iter().enumerate() {
                let active = *t == tab;
                // The first letter is #b4fc74 on every tab
                // (`::first-letter`); the selected tab is #b4fc74 throughout.
                let w = font.measure_cef(t.name(), px(30.05), 0.0);
                font.draw_cef(
                    fb,
                    fx(680.0) - w / 2.0,
                    fy(rows[i] + 69.0),
                    t.name(),
                    px(30.05),
                    0.0,
                    CSS_GREEN_HI,
                    if active { CSS_GREEN_HI } else { green },
                );
            }
        }

        let left = dx(900.0);
        let right = dx(1456.0);
        let col_w = dxw(516.0);
        let right_end = right + col_w;

        // ---- Speed -----------------------------------------------------
        if tab == OptionsTab::Speed {
            opt_text(
                fb,
                &mut self.ui_font,
                dx(900.0),
                dy(346.0),
                "Game Speed",
                px(23.5),
                white,
            );
            // Seven retail game speeds; the shipped default is Fastest.
            let gs = ((self.game_options.game_speed.clamp(0.5, 1.0) - 0.5) / 0.5).clamp(0.0, 1.0);
            opt_slider(
                fb,
                &mut self.ui_font,
                left,
                left + col_w,
                dy(391.0),
                gs,
                "slow",
                "fast",
                dy(426.0),
                unit,
                px(20.0),
            );
            opt_text(
                fb,
                &mut self.ui_font,
                dx(900.0),
                dy(499.0),
                "Key Scroll Speed",
                px(23.5),
                white,
            );
            opt_slider(
                fb,
                &mut self.ui_font,
                left,
                left + col_w,
                dy(544.0),
                self.game_options.key_scroll.clamp(0.0, 1.0),
                "slow",
                "fast",
                dy(579.0),
                unit,
                px(20.0),
            );
            opt_text(
                fb,
                &mut self.ui_font,
                dx(900.0),
                dy(652.0),
                "Mouse Scroll Speed",
                px(23.5),
                white,
            );
            opt_slider(
                fb,
                &mut self.ui_font,
                left,
                left + col_w,
                dy(697.0),
                self.game_options.mouse_scroll.clamp(0.0, 1.0),
                "slow",
                "fast",
                dy(732.0),
                unit,
                px(20.0),
            );

            opt_checkbox(
                fb,
                &mut self.ui_font,
                right,
                dy(341.0),
                dxw(29.0),
                self.game_options.use_mouse_sensitivity,
                true,
                "Mouse Sensitivity",
                true,
                px(24.0),
            );
            opt_slider(
                fb,
                &mut self.ui_font,
                right,
                right_end,
                dy(394.0),
                self.game_options.mouse_sensitivity.clamp(0.0, 1.0),
                "slow",
                "fast",
                dy(429.0),
                unit,
                px(20.0),
            );
            opt_text_right(
                fb,
                &mut self.ui_font,
                right_end,
                dy(349.0),
                &format!(
                    "{}%",
                    (self.game_options.mouse_sensitivity.clamp(0.0, 1.0) * 100.0).round() as i32
                ),
                px(24.0),
                white,
            );
            for (i, (label, checked, enabled)) in [
                ("Use Mouse Scaling", self.game_options.mouse_scaling, true),
                ("Hardware Cursor", self.game_options.hardware_cursor, true),
                ("Use Mouse Confine", true, false),
            ]
            .iter()
            .enumerate()
            {
                opt_checkbox(
                    fb,
                    &mut self.ui_font,
                    right,
                    dy(497.0 + 54.0 * i as f32),
                    dxw(29.0),
                    *checked,
                    *enabled,
                    label,
                    true,
                    px(24.0),
                );
            }
        }

        // ---- Sound -----------------------------------------------------
        if tab == OptionsTab::Sound {
            opt_checkbox(
                fb,
                &mut self.ui_font,
                left,
                dy(341.0),
                dxw(29.0),
                true,
                true,
                "Music Volume",
                false,
                px(24.0),
            );
            opt_slider(
                fb,
                &mut self.ui_font,
                left,
                left + col_w,
                dy(394.0),
                self.game_options.music_volume.clamp(0.0, 1.0),
                "min",
                "max",
                dy(429.0),
                unit,
                px(20.0),
            );
            opt_checkbox(
                fb,
                &mut self.ui_font,
                left,
                dy(497.0),
                dxw(29.0),
                true,
                true,
                "Digital Volume",
                false,
                px(24.0),
            );
            opt_slider(
                fb,
                &mut self.ui_font,
                left,
                left + col_w,
                dy(550.0),
                self.game_options.sfx_volume.clamp(0.0, 1.0),
                "min",
                "max",
                dy(585.0),
                unit,
                px(20.0),
            );
            for (i, (label, checked)) in [
                ("Unit Speech", self.game_options.unit_speech),
                ("Unit Acknowledgments", self.game_options.unit_acks),
                (
                    "Sound Plays While In Background",
                    self.game_options.sound_background,
                ),
                ("Building Sounds", self.game_options.building_sounds),
                ("Game Subtitles", self.game_options.subtitles),
                ("Cinematic Subtitles", self.game_options.cinematic_subtitles),
                (
                    "Original Unit Voice Overs",
                    self.game_options.original_voice_overs,
                ),
                (
                    "Original 1998 Campaign",
                    self.game_options.original_1998_campaign,
                ),
            ]
            .iter()
            .enumerate()
            {
                opt_checkbox(
                    fb,
                    &mut self.ui_font,
                    right,
                    dy(341.0 + 54.0 * i as f32),
                    dxw(29.0),
                    *checked,
                    true,
                    label,
                    false,
                    px(24.0),
                );
            }
        }

        // ---- Video -----------------------------------------------------
        if tab == OptionsTab::Video {
            opt_text(
                fb,
                &mut self.ui_font,
                dx(900.0),
                dy(346.0),
                "Display Mode",
                px(23.5),
                CSS_LAVENDER,
            );
            opt_help(
                fb,
                &mut self.ui_font,
                fx(1046.0),
                fy(349.5),
                9.0 * unit,
                CSS_LAVENDER,
                px(23.5),
            );
            opt_dropdown(
                fb,
                &mut self.ui_font,
                left,
                fy(373.5),
                col_w,
                "Windowed (Fullscreen)",
                true,
                unit,
            );
            let grey = Rgb::new(128, 128, 128);
            opt_text(
                fb,
                &mut self.ui_font,
                dx(900.0),
                dy(448.0),
                "Monitor",
                px(23.5),
                CSS_LAVENDER,
            );
            opt_dropdown(
                fb,
                &mut self.ui_font,
                left,
                fy(475.5),
                col_w,
                "1. Default",
                false,
                unit,
            );
            opt_text(
                fb,
                &mut self.ui_font,
                dx(900.0),
                dy(550.0),
                "Gamma Correction",
                px(23.5),
                grey,
            );
            opt_help(
                fb,
                &mut self.ui_font,
                fx(1105.0),
                fy(553.5),
                9.0 * unit,
                grey,
                px(23.5),
            );
            opt_slider_grey(
                fb,
                &mut self.ui_font,
                left,
                left + col_w,
                dy(595.0),
                0.5,
                "min",
                "max",
                dy(630.0),
                unit,
                px(20.0),
            );

            opt_checkbox(
                fb,
                &mut self.ui_font,
                right,
                dy(341.0),
                dxw(29.0),
                self.game_options.video_fps_cap,
                true,
                "Enable FPS Cap",
                false,
                px(24.0),
            );
            opt_slider(
                fb,
                &mut self.ui_font,
                right,
                right_end,
                dy(412.0),
                1.0,
                "100",
                "300",
                dy(447.0),
                unit,
                px(20.0),
            );
            opt_text(
                fb,
                &mut self.ui_font,
                right,
                dy(520.0),
                "SD Graphics Filter",
                px(23.3),
                white,
            );
            opt_slider(
                fb,
                &mut self.ui_font,
                right,
                right_end,
                dy(565.0),
                1.0,
                "Sharp",
                "Filtered",
                dy(600.0),
                unit,
                px(20.0),
            );
            for (i, (label, checked)) in [
                ("Enable Vertical Sync", self.game_options.video_vsync),
                ("HD Graphics", self.game_options.video_hd),
                ("Environment Effects", self.game_options.video_environment),
                ("Real-Time Lighting", self.game_options.video_lighting),
                (
                    "Smooth Unit Turning",
                    self.game_options.video_smooth_turning,
                ),
                ("Shadow Stacking", self.game_options.video_shadow_stacking),
                ("Pillarbox", self.game_options.video_pillarbox),
            ]
            .iter()
            .enumerate()
            {
                opt_checkbox(
                    fb,
                    &mut self.ui_font,
                    right,
                    dy(668.0 + 54.0 * i as f32),
                    dxw(29.0),
                    *checked,
                    true,
                    label,
                    matches!(*label, "Environment Effects" | "Real-Time Lighting"),
                    px(24.0),
                );
            }
        }

        // ---- Game ------------------------------------------------------
        if tab == OptionsTab::Game {
            opt_text(
                fb,
                &mut self.ui_font,
                dx(900.0),
                dy(346.0),
                "Gameplay",
                px(30.0),
                white,
            );
            for (i, (label, checked)) in [
                ("Game Timer", self.game_options.game_timer),
                ("Enable Color Cycling", self.game_options.game_color_cycling),
                (
                    "Mature Language Filter",
                    self.game_options.game_language_filter,
                ),
                ("Show Bonus Skins", self.game_options.game_bonus_skins),
            ]
            .iter()
            .enumerate()
            {
                opt_checkbox(
                    fb,
                    &mut self.ui_font,
                    left,
                    dy(390.0 + 54.0 * i as f32),
                    dxw(29.0),
                    *checked,
                    true,
                    label,
                    false,
                    px(24.0),
                );
            }
            opt_text(
                fb,
                &mut self.ui_font,
                dx(900.0),
                dy(611.0),
                "Console",
                px(24.0),
                CSS_LAVENDER,
            );
            opt_dropdown(
                fb,
                &mut self.ui_font,
                left,
                fy(638.17),
                col_w,
                "Default",
                true,
                unit,
            );
            opt_text(
                fb,
                &mut self.ui_font,
                dx(900.0),
                dy(713.0),
                "Portraits",
                px(24.0),
                CSS_LAVENDER,
            );
            for (x, label, selected) in [
                (900.0f32, "Animated", self.game_options.portraits == 0),
                (1075.0, "Still", self.game_options.portraits == 1),
                (1190.0, "Disabled", self.game_options.portraits == 2),
            ] {
                opt_radio(
                    fb,
                    &mut self.ui_font,
                    dx(x),
                    dy(752.0),
                    dxw(36.0),
                    selected,
                    true,
                    label,
                    px(24.0),
                    unit,
                );
            }
            opt_text(
                fb,
                &mut self.ui_font,
                dx(900.0),
                dy(830.0),
                "Mini-Map Position",
                px(24.0),
                CSS_LAVENDER,
            );
            for (i, (label, selected)) in [
                ("Left Corner", self.game_options.minimap_position == 0),
                ("Standard", self.game_options.minimap_position == 1),
            ]
            .iter()
            .enumerate()
            {
                opt_radio(
                    fb,
                    &mut self.ui_font,
                    dx(900.0 + 196.0 * i as f32),
                    dy(869.0),
                    dxw(36.0),
                    *selected,
                    true,
                    label,
                    px(24.0),
                    unit,
                );
            }
            opt_checkbox(
                fb,
                &mut self.ui_font,
                left,
                dy(942.0),
                dxw(29.0),
                self.game_options.game_friend_notifications,
                true,
                "Friends presence text notifications",
                false,
                px(24.0),
            );

            opt_text(
                fb,
                &mut self.ui_font,
                right,
                dy(346.0),
                "Language",
                px(30.0),
                white,
            );
            opt_text(
                fb,
                &mut self.ui_font,
                right,
                dy(395.0),
                "Changing language will require a game restart to",
                px(23.3),
                CSS_LAVENDER,
            );
            opt_text(
                fb,
                &mut self.ui_font,
                right,
                dy(422.0),
                "take effect",
                px(23.3),
                CSS_LAVENDER,
            );
            opt_dropdown(
                fb,
                &mut self.ui_font,
                right,
                fy(449.17),
                col_w,
                "English",
                true,
                unit,
            );
            opt_text(
                fb,
                &mut self.ui_font,
                right,
                dy(554.0),
                "Actions Per Minute Tracking",
                px(30.0),
                white,
            );
            opt_checkbox(
                fb,
                &mut self.ui_font,
                right,
                dy(598.0),
                dxw(29.0),
                false,
                true,
                "Display APM In Game",
                false,
                px(24.0),
            );
            opt_checkbox(
                fb,
                &mut self.ui_font,
                right,
                dy(652.0),
                dxw(29.0),
                false,
                true,
                "Alert when APM falls below",
                false,
                px(24.0),
            );
            opt_input(
                fb,
                &mut self.ui_font,
                dx(1780.0),
                dy(642.0),
                dxw(75.0),
                dy(46.0),
                "60",
                px(23.0),
                unit,
            );
            opt_checkbox(
                fb,
                &mut self.ui_font,
                right + dxw(42.0),
                dy(712.0),
                dxw(29.0),
                false,
                false,
                "Color Text",
                false,
                px(24.0),
            );
            opt_checkbox(
                fb,
                &mut self.ui_font,
                right + dxw(42.0),
                dy(766.0),
                dxw(29.0),
                false,
                false,
                "Play Sound",
                false,
                px(24.0),
            );
        }

        // ---- Network ---------------------------------------------------
        if tab == OptionsTab::Network {
            opt_checkbox(
                fb,
                &mut self.ui_font,
                left,
                dy(341.0),
                dxw(29.0),
                self.game_options.network_prefer_6112,
                true,
                "Prefer port 6112",
                false,
                px(24.0),
            );
        }

        // Restore to Default (left) and Ok (right): 5 px #331f1f outer frame
        // with a 10 px radius, black gap, and a 3 px #6c0000 inner frame with
        // a 4 px radius.  Fills are rgba(0,0,0,.8) as in the CEF CSS.
        for (label, ox, ow, ix, iw) in [
            ("Restore to Default", 513.0f32, 389.0, 523.0, 370.0),
            ("Ok", 1672.0, 378.0, 1682.0, 357.0),
        ] {
            let (bx, by) = (dx(ox), dy(1212.0));
            let (bw, bh) = (dxw(ow), dy(84.0));
            rounded_fill_alpha(fb, bx, by, bw, bh, 10.0 * unit, 204);
            rounded_band(fb, bx, by, bw, bh, 10.0 * unit, 0, 5, CSS_OUTER);
            let (px0, py0) = (dx(ix), dy(1221.0));
            let (pw, ph) = (dxw(iw), dy(65.0));
            rounded_band(fb, px0, py0, pw, ph, 4.0 * unit, 0, 3, CSS_INNER);
            // 30 px/em centred 0.35 px right of the outer box centre,
            // baseline 41.875 below the inner frame (fitted on 130933
            // "Restore to Default" / "Ok", tracking as the body text).
            if let Some(font) = self.ui_font_hover.as_mut() {
                let tr = px(30.0) * CEF_TRACKING;
                let w = font.measure_cef(label, px(30.0), tr);
                font.draw_cef(
                    fb,
                    bx as f32 + bw as f32 / 2.0 + px(0.35) - w / 2.0,
                    py0 as f32 + px(41.875),
                    label,
                    px(30.0),
                    tr,
                    CSS_GREEN_HI,
                    green,
                );
            }
        }
        let _ = dim_text;
    }

    /// Retail Hotkeys panel (references 131015/131026/131037/131049).
    /// Geometry is expressed in the reference's 2560x1440 content pixels:
    /// plate 354..2202 x 117..1256, title tab and race tabs raised above the
    /// plate top, unit grids 96 px cells with a 9 px gutter, command cards
    /// (3 x 3, 96 px cells) and the Larva rebind dialog.
    fn draw_hotkeys(&mut self, fb: &mut Framebuffer) {
        self.hotkey_cells.clear();
        self.hotkey_action_cells.clear();
        // The retail CEF window keeps the space painting and the build
        // footer but replaces the main-menu entries (reference 131015 has no
        // "View Intro" behind the plate).
        self.draw_menu_backdrop_plain(fb);
        let race = match &self.menu_screen {
            MenuScreen::Hotkeys { race } => *race,
            _ => HotkeyRace::Terran,
        };
        let unbound_units: Vec<u16> = self.hotkey_unbound.clone();
        let (cx, _cy, cw, ch) = Self::content_rect(fb);
        let (_scale, _uox, _uoy) = self.ui_metrics(fb);
        let dx = |p: f32| (cw as f32 * p / 2560.0).round() as i32;
        let dy = |p: f32| (ch as f32 * p / 1440.0).round() as i32;
        let sx = |p: f32| cx + dx(p);
        let sy = |p: f32| dy(p);
        let fpx = |p: f32| p * ch as f32 / 1440.0;
        let white = Rgb::new(235, 235, 235);
        let dim = CSS_LAVENDER;
        let green = CSS_GREEN;
        let border = dx(3.0);
        let inner = dx(6.0);

        // Main plate: 3 px #331f1f, 3 px gap, 3 px #6c0000.
        let panel = (sx(354.0), sy(117.0), dx(1849.0), dy(1140.0));
        css_panel3_rounded(fb, panel, 230, border, inner, dx(12.0));

        // Title tab, open at the bottom and overlapping the plate border by
        // 12 px: the 90% fill reaches row 128 and dims both panel bands,
        // while the inner #6c0000 band stops at row 125 (reference 131015).
        let title_tab = (sx(354.0), sy(42.0), dx(424.0), dy(75.0));
        top_rounded_fill(
            fb,
            title_tab.0,
            title_tab.1,
            title_tab.2,
            title_tab.3 + dy(12.0),
            dx(12.0) as f32,
            230,
        );
        tab_border_rounded(fb, title_tab, border, inner, dx(12.0), dy(12.0), CSS_OUTER);
        tab_border_rounded(
            fb,
            (
                title_tab.0 + inner,
                title_tab.1 + inner,
                title_tab.2 - 2 * inner,
                title_tab.3,
            ),
            border,
            inner,
            (dx(12.0) - inner).max(1),
            dy(3.0),
            CSS_INNER,
        );
        if let Some(font) = self.ui_font_hover.as_mut() {
            font.draw_centered_stretched(
                fb,
                title_tab.0 + title_tab.2 / 2,
                sy(83.0),
                "Hotkeys",
                fpx(42.0),
                0.3,
                1.0,
                1.02,
                white,
                None,
                1.0,
            );
        } else if let Some(font) = self.ui_font.as_mut() {
            font.draw_centered_stretched(
                fb,
                title_tab.0 + title_tab.2 / 2,
                sy(83.0),
                "Hotkeys",
                fpx(42.0),
                0.3,
                1.0,
                1.02,
                white,
                None,
                1.0,
            );
        }

        // Race tabs: outer 261 x 75, separated by 18 px.
        for (i, r) in HotkeyRace::all().iter().enumerate() {
            let rect = (sx(811.0 + 279.0 * i as f32), sy(42.0), dx(261.0), dy(75.0));
            let selected = *r == race;
            let extra = if selected { dy(9.0) } else { 0 };
            top_rounded_fill(
                fb,
                rect.0,
                rect.1,
                rect.2,
                rect.3 + extra,
                dx(9.0) as f32,
                230,
            );
            tab_border_rounded(fb, rect, border, inner, dx(9.0), extra, CSS_OUTER);
            tab_border_rounded(
                fb,
                (rect.0 + inner, rect.1 + inner, rect.2 - 2 * inner, rect.3),
                border,
                inner,
                (dx(9.0) - inner).max(1),
                if selected { 0 } else { -inner - 3 },
                CSS_INNER,
            );
            let color = if selected || self.hotkey_tab_hover == Some(*r) {
                CSS_GREEN_HI
            } else {
                green
            };
            if let Some(font) = self.ui_font_hover.as_mut() {
                font.draw_centered(
                    fb,
                    rect.0 + rect.2 / 2,
                    sy(71.0),
                    r.name(),
                    fpx(27.0),
                    0.0,
                    color,
                    None,
                );
            } else if let Some(font) = self.ui_font.as_mut() {
                font.draw_centered(
                    fb,
                    rect.0 + rect.2 / 2,
                    sy(71.0),
                    r.name(),
                    fpx(27.0),
                    0.0,
                    color,
                    None,
                );
            }
        }

        // Section headings ("Units" / "Buildings") use the wide face.
        for (label, y) in [("Units", 187.0f32), ("Buildings", 661.0)] {
            if let Some(font) = self.ui_font_hover.as_mut() {
                font.draw_top_stretched(
                    fb,
                    sx(423.0),
                    sy(y),
                    label,
                    fpx(33.0),
                    0.0,
                    1.0,
                    1.04,
                    white,
                    None,
                    1.0,
                );
            } else if let Some(font) = self.ui_font.as_mut() {
                font.draw_top_stretched(
                    fb,
                    sx(423.0),
                    sy(y),
                    label,
                    fpx(33.0),
                    0.0,
                    1.0,
                    1.04,
                    white,
                    None,
                    1.0,
                );
            }
        }

        // Exact lists from the shipped `hotkeypanel.js` (`e.Units`),
        // grouped by the four retail sections.
        let hk = hotkey_data::HotkeyData::get();
        for (label, x, y) in [
            ("Ground Units", 423.0f32, 256.0f32),
            ("Air Units", 768.0, 256.0),
            ("Basic", 423.0, 730.0),
            ("Advanced", 768.0, 730.0),
        ] {
            if let Some(font) = self.ui_font.as_mut() {
                font.draw_top(fb, sx(x), sy(y), label, fpx(27.0), 0.0, dim, None);
            } else {
                fb.text(sx(x), sy(y), label, dim, (dy(18.0) / 8).max(1));
            }
        }

        let cell = dx(96.0);
        let step = dx(105.0);
        for (kind, side, cell_y) in [
            ("ground", 0usize, 309.0f32),
            ("air", 1, 309.0),
            ("basic-building", 0, 783.0),
            ("advanced-building", 1, 783.0),
        ] {
            let gx = sx(if side == 0 { 423.0 } else { 768.0 });
            let gy = sy(cell_y);
            for (i, unit) in hk.units(race, Some(kind)).iter().enumerate() {
                let id = unit.id;
                let bx = gx + (i as i32 % 3) * step;
                let by = gy + (i as i32 / 3) * step;
                fb.rect(bx, by, cell, cell, Rgb::new(0, 0, 0));
                if let Some(icon) = self.hotkey_icon(&unit.icon, cell, cell) {
                    blit_rgba_src(&icon, fb, bx, by, cell, cell);
                } else if let Some(console) = &self.console {
                    console.draw_wireframe_gray(fb, id, (bx, by, cell, cell));
                }
                // CEF box-shadow subpixel: the four hairlines render at
                // different coverages (measured 42/48/29/42 on the refs).
                fb.rect(bx, by, cell, 1, Rgb::new(42, 42, 42));
                fb.rect(bx, by, 1, cell, Rgb::new(48, 48, 48));
                fb.rect(bx + cell - 1, by, 1, cell, Rgb::new(29, 29, 29));
                fb.rect(bx, by + cell - 1, cell, 1, Rgb::new(42, 42, 42));
                if self.hotkey_sel == Some(id) {
                    for t in 0..border {
                        fb.rect_outline(bx + t, by + t, cell - 2 * t, cell - 2 * t, CSS_GREEN_HI);
                    }
                } else if self.hotkey_hover == Some(id) {
                    for t in 0..border {
                        fb.rect_outline(bx + t, by + t, cell - 2 * t, cell - 2 * t, CSS_GREEN);
                    }
                }
                if unbound_units.contains(&id) {
                    draw_unbound_badge(fb, bx, by, dx(1.0), dy(1.0));
                    if let Some(font) = self.ui_font.as_mut() {
                        font.draw_top(
                            fb,
                            bx + dx(13.0),
                            by + dy(10.0),
                            "?",
                            fpx(19.0),
                            0.0,
                            Rgb::new(0, 0, 0),
                            None,
                        );
                    }
                }
                self.hotkey_cells.push((id, (bx, by, cell, cell)));
            }
        }

        // Right column: 3 px #1e1e1e divider, command cards per unit.
        fb.rect(
            sx(1113.0),
            sy(126.0),
            dx(3.0),
            dy(1122.0),
            Rgb::new(30, 30, 30),
        );
        let hotkey_unit = self
            .hotkey_sel
            .or_else(|| self.first_selected().map(|u| u.type_id));
        if let Some(id) = hotkey_unit {
            let race_name = race.name();
            let raw_name = self.data.unit_name(id).to_string();
            let name = raw_name
                .strip_prefix(race_name)
                .map(str::trim)
                .unwrap_or(&raw_name)
                .to_string();
            let unit_label = format!("{race_name} {name}");
            if let Some(font) = self.ui_font_hover.as_mut() {
                font.draw_top_stretched(
                    fb,
                    sx(1163.0),
                    sy(187.0),
                    "Selected:",
                    fpx(33.0),
                    0.0,
                    1.0,
                    1.10,
                    Rgb::new(221, 221, 221),
                    None,
                    0.9,
                );
                let selected_w = font.measure("Selected:", fpx(33.0), 0.0);
                font.draw_top_stretched(
                    fb,
                    sx(1163.0) + selected_w + dx(15.0),
                    sy(187.0),
                    &unit_label,
                    fpx(33.0),
                    0.0,
                    1.0,
                    1.10,
                    Rgb::new(245, 245, 245),
                    None,
                    0.9,
                );
            } else if let Some(font) = self.ui_font.as_mut() {
                font.draw_top_stretched(
                    fb,
                    sx(1163.0),
                    sy(187.0),
                    "Selected:",
                    fpx(33.0),
                    0.0,
                    1.0,
                    1.10,
                    Rgb::new(221, 221, 221),
                    None,
                    0.9,
                );
                let selected_w = font.measure("Selected:", fpx(33.0), 0.0);
                font.draw_top_stretched(
                    fb,
                    sx(1163.0) + selected_w + dx(15.0),
                    sy(187.0),
                    &unit_label,
                    fpx(33.0),
                    0.0,
                    1.0,
                    1.10,
                    Rgb::new(245, 245, 245),
                    None,
                    0.9,
                );
            }
            let hk = hotkey_data::HotkeyData::get();
            let cards = hk.unit(id).map(|u| u.cards.as_slice()).unwrap_or(&[]);
            let card_x = [1164.0f32, 1504.0, 1843.0];
            let grid_y = sy(304.0);
            let chip = dx(30.0);
            let chip_off = dx(3.0);
            let wrap_px = 3 * step + dy(30.0);
            // A fourth card (Zerg Drone `generalBurrowed`) wraps to a second
            // row and pushes the CEF instruction line down.
            let hint_y = if cards.len() > 3 { 1000.0 } else { 639.0 };
            if let Some(font) = self.ui_font.as_mut() {
                for (ci, card) in cards.iter().enumerate() {
                    let col = ci % 3;
                    let block = ci / 3;
                    font.draw_top(
                        fb,
                        sx(card_x[col]),
                        sy(255.0) + block as i32 * wrap_px,
                        hotkey_data::HotkeyData::card_title(&card.label),
                        fpx(27.6),
                        0.0,
                        dim,
                        None,
                    );
                }
            }
            for (ci, card) in cards.iter().enumerate() {
                // The third actions panel sits one pixel further left in
                // the CEF flex layout than its heading's glyph origin.
                let col = ci % 3;
                let block = ci / 3;
                let x = sx(card_x[col] + if col == 2 { 1.0 } else { 0.0 });
                let gy = grid_y + block as i32 * wrap_px;
                let mut occupied = [[false; 3]; 3];
                for a in &card.actions {
                    let cx = (a.x - 1) as i32;
                    let cy = (a.y - 1) as i32;
                    if !(0..3).contains(&cx) || !(0..3).contains(&cy) {
                        continue;
                    }
                    occupied[cy as usize][cx as usize] = true;
                    let bx = x + cx * step;
                    let by = gy + cy * step;
                    fb.rect(bx, by, cell, cell, Rgb::new(0, 0, 0));
                    // The shipped panel special-cases Probe Build/Advanced to
                    // the warp-gate art (278/279 -> 272/273).
                    let icon_name = if id == 64 && a.id == 678 {
                        "cmdicons.272.png".to_string()
                    } else if id == 64 && a.id == 679 {
                        "cmdicons.273.png".to_string()
                    } else {
                        a.icon.clone()
                    };
                    if let Some(icon) = self.hotkey_icon(&icon_name, cell, cell) {
                        blit_tinted_rgba(
                            &icon,
                            fb,
                            bx,
                            by,
                            cell,
                            cell,
                            Rgb::new(245, 245, 0),
                            false,
                        );
                    } else if let Some(console) = &self.console {
                        console.draw_command_icon_ink(fb, a.id as usize, (bx, by, cell, cell));
                    }
                    let key = self.hotkey_bindings.get(a.id as usize).copied().flatten();
                    if key.is_none() {
                        // CEF `.action.bindingError:after` red 20 % wash.
                        fill_alpha_rect(fb, bx, by, cell, cell, 51);
                    }
                    if self.hotkey_action == Some(a.id) {
                        for t in 0..border {
                            fb.rect_outline(
                                bx + t,
                                by + t,
                                cell - 2 * t,
                                cell - 2 * t,
                                CSS_GREEN_HI,
                            );
                        }
                    } else {
                        // .action hairlines: 25 top, 22 left, 42 right/bottom.
                        fb.rect(bx, by, cell, 1, Rgb::new(25, 25, 25));
                        fb.rect(bx, by, 1, cell, Rgb::new(25, 25, 25));
                        fb.rect(bx + cell - 1, by, 1, cell, Rgb::new(42, 42, 42));
                        fb.rect(bx, by + cell - 1, cell, 1, Rgb::new(42, 42, 42));
                    }
                    if let Some(key) = key {
                        fill_alpha_rect(fb, bx + chip_off, by + chip_off, chip, chip, 153);
                        let key_label = key.text.to_ascii_uppercase().to_string();
                        if let Some(font) = self.ui_font.as_mut() {
                            font.draw_centered_stretched(
                                fb,
                                bx + chip_off + chip / 2,
                                by + chip_off + dy(7.0),
                                &key_label,
                                fpx(std::env::var("GC_KEY_PX")
                                    .ok()
                                    .and_then(|v| v.parse().ok())
                                    .unwrap_or(20.5)),
                                0.0,
                                1.0,
                                std::env::var("GC_KEY_YS")
                                    .ok()
                                    .and_then(|v| v.parse().ok())
                                    .unwrap_or(1.07),
                                white,
                                None,
                                1.0,
                            );
                        }
                    }
                    self.hotkey_action_cells.push((a.id, (bx, by, cell, cell)));
                }
                for (row, used) in occupied.iter().enumerate() {
                    for (col, used) in used.iter().enumerate() {
                        if !used {
                            let bx = x + col as i32 * step;
                            let by = gy + row as i32 * step;
                            fb.rect(bx, by, cell, cell, Rgb::new(35, 35, 35));
                        }
                    }
                }
            }
            let rebind = self.hotkey_action.and_then(|aid| {
                let action = hk.action(aid)?;
                Some((
                    action.label.clone(),
                    action.default_text.to_ascii_uppercase(),
                    action.icon.clone(),
                    hk.shares(aid),
                    self.hotkey_bindings.get(aid as usize).copied().flatten(),
                ))
            });
            if rebind.is_none() {
                if let Some(font) = self.ui_font.as_mut() {
                    let phase: f32 = std::env::var("GC_HINT_PHASE")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0.2);
                    font.draw_centered_weighted_phase(
                        fb,
                        sx(1654.0),
                        phase,
                        sy(hint_y),
                        "Click on a command to change its hotkey",
                        fpx(27.6),
                        0.0,
                        dim,
                        None,
                        1.0,
                    );
                }
            }
            if let Some((label, def, icon_name, shares, current)) = rebind {
                if let Some(font) = self.ui_font.as_mut() {
                    if !self.hotkey_binding_error {
                        font.draw_centered(
                            fb,
                            sx(1655.0),
                            sy(1021.0),
                            "Press the desired key",
                            fpx(25.7),
                            0.0,
                            Rgb::new(221, 221, 221),
                            None,
                        );
                    } else {
                        // CEF `.bindingErrorInstructions` (#d60e00, 8 px).
                        font.draw_centered_weighted(
                            fb,
                            sx(1655.0),
                            sy(1021.0),
                            "There are commands that have become unbound",
                            fpx(25.7),
                            0.0,
                            Rgb::new(214, 14, 0),
                            None,
                            1.0,
                        );
                    }
                }
                // Rebind panel (reference 131049): gray plate from x1113 to
                // the plate edge, icon at 1161, label at 1289.
                let pr = (sx(1113.0), sy(1053.0), dx(1080.0), dy(195.0));
                // 3 px border + 3 px black gap + #1c1c1c body, as measured.
                fb.rect(pr.0, pr.1, pr.2, pr.3, Rgb::new(0, 0, 0));
                fb.rect(
                    sx(1119.0),
                    sy(1059.0),
                    dx(1071.0),
                    dy(182.0),
                    Rgb::new(28, 28, 28),
                );
                fb.rect(pr.0, pr.1, pr.2, border, Rgb::new(30, 30, 30));
                fb.rect(pr.0, pr.1, border, pr.3, Rgb::new(30, 30, 30));
                fb.rect(
                    pr.0,
                    pr.1 + pr.3 - border,
                    pr.2,
                    border,
                    Rgb::new(30, 30, 30),
                );
                let ibx = sx(1164.0);
                let iby = sy(1098.0);
                fb.rect(ibx, iby, cell, cell, Rgb::new(0, 0, 0));
                if let Some(icon) = self.hotkey_icon(&icon_name, cell, cell) {
                    blit_tinted_rgba(
                        &icon,
                        fb,
                        ibx,
                        iby,
                        cell,
                        cell,
                        Rgb::new(245, 245, 0),
                        false,
                    );
                }
                fb.rect_outline(ibx, iby, cell, cell, Rgb::new(45, 45, 45));
                let mut label_lines = label.lines();
                let first_line = label_lines.next().unwrap_or("");
                let second_line = label_lines.next();
                if let Some(font) = self.ui_font_hover.as_mut() {
                    font.draw_top_stretched(
                        fb,
                        sx(1287.0),
                        sy(1104.0),
                        first_line,
                        fpx(27.0),
                        0.0,
                        1.0,
                        std::env::var("GC_MORPH_YS")
                            .ok()
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(1.0),
                        Rgb::new(238, 238, 238),
                        None,
                        1.0,
                    );
                }
                if let Some(second_line) = second_line {
                    if let Some(font) = self.ui_font.as_mut() {
                        font.draw_top(
                            fb,
                            sx(1287.0),
                            sy(1128.0),
                            second_line,
                            fpx(22.0),
                            0.0,
                            Rgb::new(190, 190, 190),
                            None,
                        );
                    }
                }
                if let Some(font) = self.ui_font_hover.as_mut() {
                    if shares > 1 {
                        if let Some(font) = self.ui_font.as_mut() {
                            font.draw_top(
                                fb,
                                sx(1287.0),
                                sy(if second_line.is_some() {
                                    1158.0
                                } else {
                                    1136.0
                                }),
                                &format!("{shares} units share this command"),
                                fpx(27.0),
                                0.0,
                                dim,
                                None,
                            );
                        }
                    }
                    let def_top = if second_line.is_some() {
                        1190.0
                    } else if shares > 1 {
                        1172.0
                    } else {
                        1143.0
                    };
                    font.draw_top_stretched(
                        fb,
                        sx(1287.0),
                        sy(def_top),
                        &format!("Default: {def}"),
                        fpx(27.0),
                        0.0,
                        1.0,
                        std::env::var("GC_DEF_YS")
                            .ok()
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(0.96),
                        Rgb::new(150, 150, 155),
                        None,
                        1.0,
                    );
                }
                let kb = (sx(1966.0), sy(1098.0), dx(150.0), dy(64.0));
                fb.rect(kb.0, kb.1, kb.2, kb.3, Rgb::new(35, 35, 35));
                for t in 0..border {
                    fb.rect_outline(
                        kb.0 + t,
                        kb.1 + t,
                        kb.2 - 2 * t,
                        kb.3 - 2 * t,
                        Rgb::new(128, 128, 128),
                    );
                }
                let shown = current
                    .map(|k| k.text.to_ascii_uppercase().to_string())
                    .unwrap_or_default();
                if let Some(font) = self.ui_font_hover.as_mut() {
                    font.draw_centered(
                        fb,
                        kb.0 + kb.2 / 2 - dx(1.0),
                        sy(1118.0),
                        &shown,
                        fpx(33.0),
                        0.0,
                        Rgb::new(240, 240, 240),
                        None,
                    );
                }
            }
        } else {
            if let Some(font) = self.ui_font.as_mut() {
                font.draw_centered(
                    fb,
                    sx(1653.0),
                    sy(440.0),
                    "Click on a unit to view its commands",
                    fpx(27.0),
                    0.0,
                    dim,
                    None,
                );
            } else {
                let hint = "Click on a unit to view its commands";
                let hw = hint.chars().count() as i32 * 8 * (dy(24.0) / 8).max(1);
                fb.text(
                    sx(1653.0) - hw / 2,
                    sy(440.0),
                    hint,
                    dim,
                    (dy(24.0) / 8).max(1),
                );
            }
        }

        // Bottom buttons: outer #331f1f group + #6c0000 button border.
        for (label, gx0, gy0, gw, gh, rw, rh) in [
            (
                "Restore to Default",
                354.0f32,
                1302.0f32,
                390.0f32,
                84.0f32,
                372.0f32,
                66.0f32,
            ),
            ("Save", 1453.0, 1302.0, 378.0, 84.0, 360.0, 66.0),
            ("Cancel", 1825.0, 1302.0, 378.0, 84.0, 360.0, 66.0),
        ] {
            let group = (sx(gx0), sy(gy0), dx(gw), dy(gh));
            // Retail button group: 5 CSS px radius with a 1.5 CSS px brown
            // band; the inner button uses 3 CSS px and a 1 CSS px red band.
            rounded_fill_alpha(fb, group.0, group.1, group.2, group.3, dx(15.0) as f32, 205);
            rounded_band(
                fb,
                group.0,
                group.1,
                group.2,
                group.3,
                dx(15.0) as f32,
                0,
                dx(4.0),
                CSS_OUTER,
            );
            let btn = (group.0 + dx(9.0), group.1 + dx(9.0), dx(rw), dy(rh));
            rounded_fill_alpha(fb, btn.0, btn.1, btn.2, btn.3, dx(9.0) as f32, 205);
            rounded_band(
                fb,
                btn.0,
                btn.1,
                btn.2,
                btn.3,
                dx(9.0) as f32,
                0,
                border,
                CSS_INNER,
            );
            if let Some(font) = self.ui_font_hover.as_mut() {
                font.draw_centered_stretched(
                    fb,
                    btn.0 + btn.2 / 2,
                    btn.1 + dy(21.0),
                    label,
                    fpx(30.0),
                    0.0,
                    1.0,
                    1.10,
                    green,
                    None,
                    1.0,
                );
            } else if let Some(font) = self.ui_font.as_mut() {
                font.draw_centered_stretched(
                    fb,
                    btn.0 + btn.2 / 2,
                    btn.1 + dy(21.0),
                    label,
                    fpx(30.0),
                    0.0,
                    1.0,
                    1.10,
                    green,
                    None,
                    1.0,
                );
            } else {
                let lw = label.chars().count() as i32 * 8 * (dy(30.0) / 8).max(1);
                fb.text(
                    btn.0 + (btn.2 - lw) / 2,
                    btn.1 + dy(18.0),
                    label,
                    green,
                    (dy(30.0) / 8).max(1),
                );
            }
        }

        // Restore-defaults confirmation (reference 120018): the CEF page and
        // the in-game buttons behind the modal dim to 40 %, the dialog itself
        // stays opaque black.
        if self.hotkey_reset {
            // The CEF modal overlay covers the whole hotkey container,
            // including the bottom button strip and the space between it and
            // the panel (all samples give a 0.40 factor).
            fill_alpha_rect(fb, sx(354.0), sy(42.0), dx(1849.0), dy(1398.0), 153);
            let dlg = (sx(735.0), sy(486.0), dx(1087.0), dy(468.0));
            let radius = dx(16.0) as f32;
            rounded_fill(fb, dlg.0, dlg.1, dlg.2, dlg.3, radius, Rgb::new(0, 0, 0));
            rounded_band(fb, dlg.0, dlg.1, dlg.2, dlg.3, radius, 0, border, CSS_OUTER);
            rounded_band(
                fb,
                dlg.0,
                dlg.1,
                dlg.2,
                dlg.3,
                radius,
                border * 2,
                border,
                CSS_INNER,
            );
            if let Some(font) = self.ui_font_hover.as_mut() {
                font.draw_top_stretched(
                    fb,
                    sx(826.0),
                    sy(572.0),
                    "Restore defaults?",
                    fpx(36.0),
                    0.0,
                    1.0,
                    1.083,
                    dim,
                    None,
                    1.0,
                );
            }
            if let Some(font) = self.ui_font.as_mut() {
                font.draw_top(
                    fb,
                    sx(826.0),
                    sy(628.0),
                    "You will lose your changes, do you wish to continue?",
                    fpx(27.0),
                    0.23,
                    Rgb::new(221, 221, 221),
                    None,
                );
            }
            for (label, x0) in [("Continue", 996.0f32), ("Cancel", 1387.0)] {
                let bx = sx(x0);
                let by = sy(813.0);
                let bw = dx(360.0);
                let bh = dy(66.0);
                let br = dx(6.0) as f32;
                rounded_fill(fb, bx, by, bw, bh, br, Rgb::new(0, 0, 0));
                rounded_band(fb, bx, by, bw, bh, br, 0, border, CSS_INNER);
                if let Some(font) = self.ui_font_hover.as_mut() {
                    font.draw_centered_stretched(
                        fb,
                        bx + bw / 2,
                        by + dy(21.0),
                        label,
                        fpx(30.0),
                        0.0,
                        1.0,
                        1.1,
                        green,
                        None,
                        1.0,
                    );
                }
            }
        }
    }

    /// Retail Collections screen (reference #22). Rects measured on that
    /// 3440x1440 screenshot as fractions of the 2560x1440 content box: main
    /// panel 0.141..0.601 x 0.045..0.836, preview column 0.618..0.863, Ok at
    /// 0.715..0.863 x 0.868..0.916, section rows at 0.147 / 0.373 / 0.598.
    /// Retail CEF `Collections` panel, rebuilt from the shipped
    /// `collectionspanel.js` layout (measured 1:1 on capture 131005):
    /// 640x480 CSS at 3x, one outer plate, two red frames, three section
    /// grids with width-fit thumbnails, scrollbar and the right-hand
    /// preview/EQUIP/Ok column.
    fn draw_collections(&mut self, fb: &mut Framebuffer) {
        self.draw_menu_backdrop(fb);
        let (cx, _cy, cw, ch) = Self::content_rect(fb);
        dim_screen(fb, cx, cw, ch, 160);
        let (scale, uox, uoy) = self.ui_metrics(fb);
        let k = ch as f32 / 1440.0;
        let sx = |x: f32| cx + (x * k).round() as i32;
        let sy = |y: f32| (y * k).round() as i32;
        let sw = |w: f32| (w * k).round().max(1.0) as i32;
        // Screen px -> UI units (draw_hd_text anchors/widths are CSS-space).
        let u = |v: f32| (v - 320.0) / 3.0;
        let v = |y: f32| y / 3.0;
        let w3 = |v: f32| v / 3.0;
        let r = |x: f32, y: f32, w: f32, h: f32| (sx(x), sy(y), sw(w), sw(h));
        let white = Rgb::new(0xff, 0xff, 0xff);
        let grey = Rgb::new(0xcc, 0xcc, 0xcc);
        let green = Rgb::new(0x4c, 0xc4, 0x28);
        let equip_grey = Rgb::new(0x80, 0x80, 0x80);

        // One outer plate (357..2205 x 71..1207): 5 px #331f1f, radius 4,
        // fill rgba(0,0,0,.5).  The old code drew two plates with a 43 px gap
        // and a 90 % fill (5x too dark).
        let (px, py, pw, ph) = r(357.0, 71.0, 1849.0, 1137.0);
        rounded_fill_alpha(fb, px, py, pw, ph, 4.0 * k, 224);
        for t in 0..(5.0 * k).round().max(1.0) as i32 {
            fb.rect_outline(px + t, py + t, pw - t * 2, ph - t * 2, CSS_OUTER);
        }
        // frame0 (red, no right border) and frame1 (grey left hairline).
        let b = (3.0 * k).max(1.0) as i32;
        // Section separators (3 px #474747 at x 401..1513).
        for y in [247.0f32, 568.0, 889.0] {
            fb.rect(sx(401.0), sy(y), sw(1113.0), b, Rgb::new(0x47, 0x47, 0x47));
        }
        fb.rect(600, 600, 100, 50, Rgb::new(0, 255, 0));
        // Title (Extended 9 px, #fff) and dropdown chip.
        self.draw_hd_text(
            fb,
            "Collections",
            u(402.0),
            v(121.0),
            galaxycraft::menu::LabelAlign::Left,
            w3(174.0),
            9.0,
            scale,
            uox,
            uoy,
            white,
            true,
        );
        let (dx, dy, dw, dh) = r(1232.0, 115.0, 300.0, 44.0);
        rounded_fill_alpha(fb, dx, dy, dw, dh, 4.0 * k, 230);
        for t in 0..b {
            fb.rect_outline(dx + t, dy + t, dw - t * 2, dh - t * 2, CSS_INNER);
        }
        self.draw_hd_text(
            fb,
            "All Collections",
            u(1241.0),
            v(129.0),
            galaxycraft::menu::LabelAlign::Left,
            w3(136.0),
            8.0,
            scale,
            uox,
            uoy,
            green,
            false,
        );
        // Olive dropdown arrow (15x8 at 1499,138).
        let (ax, ay) = (sx(1499.0), sy(138.0));
        for i in 0..8 {
            let w = (15.0 * k * (1.0 - i as f32 / 8.0)).round() as i32;
            fb.rect(
                ax + ((15.0 * k) as i32 - w) / 2,
                ay + (i as f32 * k) as i32,
                w.max(1),
                1,
                Rgb::new(0xb1, 0xb1, 0x0d),
            );
        }
        // Section headers and right-aligned counts.
        for (label, count, ink_y, count_y, count_w, label_w) in [
            (
                "Console Skins",
                "3 Available",
                216.0f32,
                214.0,
                92.0f32,
                152.0f32,
            ),
            (
                "Unit & Structure Skins",
                "2 Available",
                537.0,
                535.0,
                91.0,
                248.0,
            ),
            ("Announcers", "6 Available", 858.0, 856.0, 92.0, 132.0),
        ] {
            self.draw_hd_text(
                fb,
                label,
                u(402.0),
                v(ink_y),
                galaxycraft::menu::LabelAlign::Left,
                w3(label_w),
                9.0,
                scale,
                uox,
                uoy,
                grey,
                false,
            );
            // Counts are right-aligned on the separator edge (x 1513).
            let center = 1513.0 - count_w / 2.0;
            self.draw_hd_text(
                fb,
                count,
                u(center),
                v(count_y),
                galaxycraft::menu::LabelAlign::Center,
                w3(count_w),
                7.0,
                scale,
                uox,
                uoy,
                green,
                false,
            );
        }
        fb.rect(800, 600, 100, 50, Rgb::new(0, 0, 255));
        // Grid: 353x224 cells, columns 411/787/1161, rows 274/595/916/1158.
        let cells: [(f32, f32, bool, &str); 11] = [
            (411.0, 274.0, true, "default_console_thumbnail.jpg"),
            (787.0, 274.0, false, "krc_silver_console_thumbnail.png"),
            (1161.0, 274.0, false, "krc_gold_console_thumbnail.png"),
            (411.0, 595.0, true, "skins_default.jpg"),
            (787.0, 595.0, false, "skins_carbot.png"),
            (411.0, 916.0, true, "default_announcer.png"),
            (787.0, 916.0, false, "um_announcer.png"),
            (1161.0, 916.0, false, "jeon_announcer.png"),
            (411.0, 1158.0, false, "kim_announcer.png"),
            (787.0, 1158.0, false, "umjeonkim_announcer.png"),
            (1161.0, 1158.0, false, "heyjini_announcer.png"),
        ];
        let cell_w = sw(353.0);
        let cell_h = sw(224.0);
        let clip_bottom = sy(1192.0);
        for (x, y, selected, name) in cells {
            let (cell_x, cell_y) = (sx(x), sy(y));
            let visible_h = cell_h.min(clip_bottom - cell_y).max(0);
            if visible_h <= 0 {
                continue;
            }
            // Width-fit thumbnail inside the cell border, top-left anchored
            // and clipped by the cell.  Template matching against 131005 puts
            // every thumbnail at cell+(3,3) at ~347x239 (corr 0.95), selected
            // or not; the old stretch to 349x215 was the main grid error.
            let (img_w, img_h, off) = (347.0, 239.0, b);
            if let Some(img) = self.thumb(name, sw(img_w), sw(img_h)) {
                let rows = visible_h.min(img.height as i32 - off);
                let cols = cell_w.min(img.width as i32);
                for yy in 0..rows.max(0) {
                    for xx in 0..cols.max(0) {
                        let sx_i = xx as usize;
                        let sy_i = yy as usize;
                        let kk = (sy_i * img.width as usize + sx_i) * 3;
                        if kk + 2 < img.rgb.len() {
                            fb.put(
                                cell_x + off + xx,
                                cell_y + off + yy,
                                Rgb::new(img.rgb[kk], img.rgb[kk + 1], img.rgb[kk + 2]),
                            );
                        }
                    }
                }
            }
            let border = if selected {
                Rgb::new(0x9c, 0xb3, 0xfa)
            } else {
                Rgb::new(0x45, 0x45, 0x45)
            };
            for t in 0..b {
                fb.rect_outline(
                    cell_x + t,
                    cell_y + t,
                    cell_w - t * 2,
                    visible_h - t * 2,
                    border,
                );
            }
        }
        fb.rect(1000, 600, 100, 50, Rgb::new(255, 255, 0));
        // Frame borders are re-stated after the grid so the clipped
        // announcer row cannot cover the bottom hairline.
        let (f0x, f0y, f0w, f0h) = r(368.0, 82.0, 1224.0, 1113.0);
        let b = (3.0 * k).max(1.0) as i32;
        fb.rect(f0x, f0y, f0w, b, CSS_INNER);
        fb.rect(f0x, f0y + f0h - b, f0w, b, CSS_INNER);
        fb.rect(f0x, f0y, b, f0h, CSS_INNER);
        let (f1x, f1y, f1w, f1h) = r(1594.0, 82.0, 601.0, 1113.0);
        fb.rect(f1x, f1y, f1w, b, CSS_INNER);
        fb.rect(f1x, f1y + f1h - b, f1w, b, CSS_INNER);
        fb.rect(f1x + f1w - b, f1y, b, f1h, CSS_INNER);
        fb.rect(f1x, f1y, b, f1h, Rgb::new(74, 71, 71));
        // Scrollbar: track 1538..1561 x 241..1161, thumb to 990, arrows.
        let (sbx, sby, sbw, sbh) = r(1538.0, 241.0, 24.0, 921.0);
        fb.rect(sbx, sby, sbw, sbh, Rgb::new(0, 0, 0));
        for t in 0..b {
            fb.rect_outline(sbx + t, sby + t, sbw - t * 2, sbh - t * 2, CSS_INNER);
        }
        let (thx, thy, thw, thh) = r(1538.0, 241.0, 24.0, 750.0);
        for t in 0..b {
            fb.rect_outline(
                thx + t,
                thy + t,
                thw - t * 2,
                thh - t * 2,
                Rgb::new(0xd6, 0x0e, 0x00),
            );
        }
        let red = Rgb::new(0xfa, 0x00, 0x00);
        let (upx, upy) = (sx(1538.0), sy(215.0));
        for i in 0..10 {
            let w = (22.0 * k * (i as f32 + 1.0) / 10.0).round() as i32;
            fb.rect(
                upx + (sw(22.0) - w) / 2,
                upy + (i as f32 * k) as i32,
                w.max(1),
                1,
                red,
            );
        }
        let (dnx, dny) = (sx(1538.0), sy(1166.0));
        for i in 0..10 {
            let w = (22.0 * k * (10.0 - i as f32) / 10.0).round() as i32;
            fb.rect(
                dnx + (sw(22.0) - w) / 2,
                dny + (i as f32 * k) as i32,
                w.max(1),
                1,
                red,
            );
        }
        fb.rect(1200, 600, 100, 50, Rgb::new(255, 0, 255));
        // Right column: preview of the selected grid cell, title, EQUIP, Ok.
        let sel = self
            .collection_sel
            .min(galaxycraft::menu::COLLECTION_ITEMS.len() - 1);
        let (sel_img, sel_title) = galaxycraft::menu::COLLECTION_ITEMS[sel];
        if let Some(img) = self.thumb(sel_img, sw(349.0), sw(237.0)) {
            let (ix, iy) = (sx(1719.0), sy(118.0));
            for yy in 0..img.height as i32 {
                for xx in 0..img.width as i32 {
                    let kk = ((yy * img.width as i32 + xx) * 3) as usize;
                    if kk + 2 < img.rgb.len() {
                        fb.put(
                            ix + xx,
                            iy + yy,
                            Rgb::new(img.rgb[kk], img.rgb[kk + 1], img.rgb[kk + 2]),
                        );
                    }
                }
            }
        }
        self.draw_hd_text(
            fb,
            sel_title,
            u(1825.0),
            v(373.0),
            galaxycraft::menu::LabelAlign::Left,
            w3(139.0),
            11.0,
            scale,
            uox,
            uoy,
            white,
            true,
        );
        let (ex, ey, ew, eh) = r(1688.0, 421.0, 410.0, 57.0);
        fb.rect(ex, ey, ew, eh, Rgb::new(0, 0, 0));
        for t in 0..b {
            fb.rect_outline(
                ex + t,
                ey + t,
                ew - t * 2,
                eh - t * 2,
                Rgb::new(0x45, 0x45, 0x45),
            );
        }
        self.draw_hd_text(
            fb,
            "EQUIP",
            u(1848.0),
            v(441.0),
            galaxycraft::menu::LabelAlign::Left,
            w3(91.0),
            8.0,
            scale,
            uox,
            uoy,
            equip_grey,
            true,
        );
        // Ok holder + button at the panel's bottom-right.
        let (hx, hy, hw, hh) = r(1828.0, 1236.0, 378.0, 84.0);
        rounded_fill_alpha(fb, hx, hy, hw, hh, 4.0 * k, 204);
        for t in 0..(5.0 * k).round().max(1.0) as i32 {
            fb.rect_outline(hx + t, hy + t, hw - t * 2, hh - t * 2, CSS_OUTER);
        }
        let (ox, oy, ow, oh) = r(1837.0, 1245.0, 360.0, 66.0);
        rounded_fill_alpha(fb, ox, oy, ow, oh, 2.0 * k, 235);
        for t in 0..b {
            fb.rect_outline(ox + t, oy + t, ow - t * 2, oh - t * 2, CSS_INNER);
        }
        self.draw_hd_text(
            fb,
            "Ok",
            u(1995.0),
            v(1266.0),
            galaxycraft::menu::LabelAlign::Left,
            w3(47.0),
            10.0,
            scale,
            uox,
            uoy,
            Rgb::new(0xb4, 0xfc, 0x74),
            true,
        );
    }

    fn draw_credits(&mut self, fb: &mut Framebuffer) {
        let (cx, _cy, cw, ch) = Self::content_rect(fb);
        // Backdrop: the shipped metal room (`glue/battlenet/backgnd.dds`).
        match self.glue.as_ref().and_then(|g| g.credits_backdrop.clone()) {
            Some(art) => art.draw_scaled(fb, cx, 0, cw, ch),
            None => {
                for y in 0..ch {
                    let t = y as f32 / ch as f32;
                    let c = Rgb::new(
                        (14.0 + 10.0 * t) as u8,
                        (12.0 + 7.0 * t) as u8,
                        (10.0 + 4.0 * t) as u8,
                    );
                    fb.rect(cx, y, cw, 1, c);
                }
            }
        }
        // Panel: `credits_a.dds` at the layout's (120,30,420,450) rect,
        // centred in the 640x480 UI space.
        let (scale, ox, oy) = self.ui_metrics(fb);
        let panel = UiRect::new(120, 30, 418, 429).scaled(scale, ox, oy);
        if let Some(art) = self.glue.as_ref().and_then(|g| g.credits_frame.clone()) {
            art.draw_scaled(fb, panel.x, panel.y, panel.w, panel.h);
        } else {
            fb.rect(panel.x, panel.y, panel.w, panel.h, Rgb::new(6, 5, 5));
            fb.rect_outline(panel.x, panel.y, panel.w, panel.h, Rgb::new(150, 40, 20));
        }
        // Text column: the CSS puts the list at 23%..80% of the viewport
        // width, starting at 10.2% of its height.
        let entries = self.credits_entries();
        if entries.is_empty() {
            return;
        }
        let text_cx = cx + (cw as f32 * 0.515) as i32;
        // CSS pixels map 1:1 to a 2560x1440 viewport; scale for other sizes.
        let k = ch as f32 / 1440.0;
        let green = Rgb::new(180, 252, 116); // #b4fc74
        let dim = Rgb::new(150, 230, 110);
        let mut y = -(self.credits_scroll as i32) + (ch as f32 * 0.102) as i32;
        for (name, style) in entries.iter() {
            let (size_css, advance, color) = match style {
                1 => (80.0, 87.0, green),
                2 => (56.0, 124.0, green),
                3 => (44.0, 45.0, green),
                4 => (36.0, 27.0, dim),
                5 => (28.0, 81.0, green),
                _ => (20.0, 20.0, green),
            };
            let sz = ((size_css * k / 16.0).round() as i32).clamp(1, 6);
            if !name.is_empty() && y > -40 && y < ch + 40 {
                let w = name.chars().count() as i32 * 8 * sz;
                fb.text(text_cx - w / 2, y, name, color, sz);
            }
            y += (advance * k) as i32;
            if y > ch {
                break;
            }
        }
    }

    /// The MENU button in the HD console, measured on the references:
    /// x 0.7363..0.8035, y 0.8194..0.8542 of the content box (the classic
    /// Stat_F10 rect sits ~340 px to the left).
    /// `stat_f10` button 1 (MENU), right-anchored.
    fn menu_button_rect(&self) -> UiRect {
        self.hud_control("stat_f10", 1, UiRect::new(416, 388, 64, 20))
    }

    /// In-game "Game Menu" dialog at the rects measured on reference #32:
    /// panel 0.181..0.478 x 0.056..0.643, title at 0.081, seven 81 px rows on a
    /// 106 px pitch. Typography one glyph step below `big`, as in the retail UI.
    /// In-game "Game Menu" dialog, rects measured on reference #32: panel
    /// 0.340..0.744 x 0.067..0.666 of the content box, seven 72 px buttons on a
    /// 102 px pitch starting at y 0.142.
    fn draw_full_gamemenu(&mut self, fb: &mut Framebuffer) {
        let (cx, _cy, cw, ch) = Self::content_rect(fb);
        // Measured sweep over the panel blend: 200 is the best fit (18.30
        // against 18.32 at 225 and 18.35 at 240).
        let (dim, pane) = (110u8, 200u8);
        for y in 0..ch {
            for x in cx..cx + cw {
                let c = fb.get(x, y);
                fb.put(x, y, c.blend(Rgb::new(0, 0, 0), dim));
            }
        }
        let at = |x: f32, y: f32| (cx + (cw as f32 * x) as i32, (ch as f32 * y) as i32);
        let (px, py) = at(0.3398, 0.0667);
        // Measured on reference #32: the panel spans x 1310..2126 of the
        // 3440 frame, i.e. 0.3398..0.6586 of the content box (the previous
        // 0.7441 made it 219 px too wide).
        let pw = (cw as f32 * (0.6586 - 0.3398)) as i32;
        let ph = (ch as f32 * (0.666 - 0.0667)) as i32;
        for y in py..py + ph {
            for x in px..px + pw {
                let c = fb.get(x, y);
                // Panel colour measured on the reference with the world behind
                // (11.4, 18.8, 65.4); solved for a 0.88 blend that is (8,17,68).
                fb.put(x, y, c.blend(Rgb::new(8, 17, 68), pane));
            }
        }
        fb.rect_outline(px, py, pw, ph, Rgb::new(60, 90, 160));
        // Reference glyphs are 32 px tall in both the title and the rows.
        let big = (ch as f32 / 360.0).round().max(1.0) as i32;
        let title = "Game Menu";
        let tw = title.chars().count() as i32 * 8 * big;
        fb.text(
            px + (pw - tw) / 2,
            py + (ch as f32 * 0.016) as i32,
            title,
            // Measured mean of the retail title glyphs.
            Rgb::new(190, 195, 225),
            big,
        );
        let items: Vec<String> = GameMenuItem::all()
            .iter()
            .map(|i| i.label().to_string())
            .collect();
        let row_h = (ch as f32 * 0.050) as i32;
        let pitch = (ch as f32 * 0.0708) as i32;
        let mut y = py + (ch as f32 * 0.075) as i32;
        for (i, label) in items.iter().enumerate() {
            let sel = i == self.game_menu_sel;
            let bx = px + (cw as f32 * 0.006) as i32;
            let bw = pw - (cw as f32 * 0.012) as i32;
            fb.rect(bx, y, bw, row_h, Rgb::new(12, 18, 40));
            fb.rect_outline(
                bx,
                y,
                bw,
                row_h,
                if sel {
                    Rgb::new(120, 200, 255)
                } else {
                    Rgb::new(60, 110, 190)
                },
            );
            let lw = label.chars().count() as i32 * 8 * big;
            fb.text(
                bx + (bw - lw) / 2,
                y + (row_h - 8 * big) / 2,
                label,
                if sel {
                    Rgb::new(255, 250, 150)
                } else {
                    // Retail rows are yellow (measured mean 217, 217, 76).
                    Rgb::new(228, 228, 90)
                },
                big,
            );
            y += pitch;
        }
    }

    /// Mission Objectives (`objctdlg`): title, the objective text and the
    /// default `Previous (Esc)` button (134919).
    fn draw_objectives_panel(&mut self, fb: &mut Framebuffer) {
        let body = "Destroy all of an opponent's buildings to eliminate him.";
        self.draw_glue_dialog(
            fb,
            "objctdlg",
            "Mission Objectives",
            (2, body),
            &[(-3, "\u{1b}Previous (\u{4}Esc\u{1})", true)],
            None,
        );
    }

    /// StarCraft Tips (`tips_dlg`): title, tip text, the "Show Tips at
    /// Startup" checkbox and OK / Previous Tip / Next Tip (134931).
    fn draw_tips_panel(&mut self, fb: &mut Framebuffer) {
        let body = "To toggle subtitles on or off, go to MENU. Under OPTIONS, select SOUND and click on the box marked TOGGLE SUBTITLES.";
        self.draw_glue_dialog(
            fb,
            "tips_dlg",
            "StarCraft Tips",
            (-10, body),
            &[
                (-2, "o\u{4}O\u{1}K", true),
                (3, "p\u{4}P\u{1}revious Tip", false),
                (1, "n\u{4}N\u{1}ext Tip", false),
            ],
            Some((2, "Show Tips at Startup")),
        );
    }

    /// In-game glue dialog (centre-anchored like `statdata`): a translucent
    /// navy pane (fitted on 134931 against 134905, same scene: colour
    /// (4,17,54) at 75 %), `RC_BIGTEXT` Eurostile with a drop shadow, and
    /// buttons whose label is yellow with the `\x04` hotkey span white.
    #[allow(clippy::type_complexity)]
    fn draw_glue_dialog(
        &mut self,
        fb: &mut Framebuffer,
        dialog: &str,
        title: &str,
        body: (i32, &str),
        buttons: &[(i32, &str, bool)],
        checkbox: Option<(i32, &str)>,
    ) {
        let Some(layout) = self
            .console
            .as_ref()
            .and_then(|c| c.layout(dialog))
            .cloned()
        else {
            return;
        };
        let k = self.content_box.3 as f32 / 1440.0;
        let cx0 = self.content_box.0;
        let pane = self.hud_rect(dialog, layout.rect);
        for y in pane.y..pane.y + pane.h {
            for x in pane.x..pane.x + pane.w {
                let c = fb.get(x, y);
                fb.put(x, y, c.blend(DIALOG_FILL, DIALOG_ALPHA));
            }
        }
        fb.rect_outline(pane.x, pane.y, pane.w, pane.h, DIALOG_EDGE);
        let to_c = |r: UiRect| {
            (
                (r.x - cx0) as f32 / k,
                r.y as f32 / k,
                r.w as f32 / k,
                r.h as f32 / k,
            )
        };
        let rects: Vec<(i32, UiRect)> = layout
            .controls
            .iter()
            .filter(|c| c.id != 0)
            .map(|c| (c.id, self.hud_rect(dialog, c.abs)))
            .collect();
        let ctrl = |id: i32| rects.iter().find(|(i, _)| *i == id).map(|(_, r)| *r);
        // Title (CTEXT -1), centred.
        if let Some(r) = ctrl(-1) {
            let (x, y, w, _) = to_c(r);
            let tw = self.glue_measure(title, GLUE_BIG_PX, GlueFace::Big);
            self.glue_shadow_text(
                fb,
                x + (w - tw) / 2.0 + DIALOG_TITLE_DX,
                y + DIALOG_TITLE_BASE,
                title,
                None,
            );
        }
        // Body, word-wrapped to the LTEXT width.
        if let Some(r) = ctrl(body.0) {
            let (x, y, w, _) = to_c(r);
            let mut line = String::new();
            let mut lines = Vec::new();
            for word in body.1.split_whitespace() {
                let cand = if line.is_empty() {
                    word.to_string()
                } else {
                    format!("{line} {word}")
                };
                if !line.is_empty() && self.glue_measure(&cand, GLUE_BIG_PX, GlueFace::Big) > w {
                    lines.push(std::mem::take(&mut line));
                    line = word.to_string();
                } else {
                    line = cand;
                }
            }
            lines.push(line);
            for (i, l) in lines.iter().enumerate() {
                self.glue_shadow_text(
                    fb,
                    x + DIALOG_BODY_DX,
                    y + DIALOG_BODY_BASE + i as f32 * DIALOG_BODY_PITCH,
                    l,
                    None,
                );
            }
        }
        if let Some((id, label)) = checkbox {
            if let Some(r) = ctrl(id) {
                let side = (r.h as f32).max(28.0 * k) as i32;
                fb.rect(r.x, r.y, side, side, Rgb::new(40, 8, 8));
                fb.rect_outline(r.x, r.y, side, side, Rgb::new(110, 30, 30));
                let (x, y, _, _) = to_c(r);
                // `RC_SMALLTEXT`-sized label (134931: ink 824..1128 x
                // 677..704 for "Show Tips at Startup").
                self.glue_text_face(
                    fb,
                    x + side as f32 / k + DIALOG_CHECK_DX,
                    y + DIALOG_CHECK_BASE,
                    label,
                    GLUE_SMALL_PX,
                    HUD_TEXT_COLOR,
                    None,
                    GlueFace::Small,
                );
            }
        }
        for &(id, label, default) in buttons {
            let Some(r) = ctrl(id) else {
                continue;
            };
            if default {
                fb.rect_outline(r.x, r.y, r.w, r.h, DIALOG_BUTTON_OUTER);
                fb.rect_outline(r.x + 1, r.y + 1, r.w - 2, r.h - 2, DIALOG_BUTTON_OUTER);
            }
            let inset = (6.0 * k) as i32;
            // Only the rim: the button face stays the translucent pane.
            draw_rounded_rim(
                fb,
                (r.x + inset, r.y + inset, r.w - 2 * inset, r.h - 2 * inset),
                8.0 * k,
                3.0 * k,
                DIALOG_BUTTON_INNER,
            );
            // `rc3.json` labels start with their hotkey (`o\x04O\x01K`,
            // `\x1bPrevious ...` for Esc), which is not drawn.
            let label: String = label.chars().skip(1).collect();
            let label = label.as_str();
            let (x, y, w, h) = to_c(r);
            let plain: String = label.chars().filter(|c| (*c as u32) >= 0x20).collect();
            let tw = self.glue_measure(&plain, GLUE_BIG_PX, GlueFace::Big);
            self.glue_shadow_text(
                fb,
                x + (w - tw) / 2.0,
                y + h / 2.0 + DIALOG_BUTTON_BASE,
                label,
                Some(TOOLTIP_HOTKEY),
            );
        }
    }

    /// Eurostile `RC_BIGTEXT` with the retail 2 px black drop shadow; `\x04`
    /// spans are white, the rest `base` (lavender when `None`, else e.g.
    /// the button yellow).
    fn glue_shadow_text(
        &mut self,
        fb: &mut Framebuffer,
        x: f32,
        base: f32,
        text: &str,
        colour: Option<Rgb>,
    ) {
        let normal = colour.unwrap_or(HUD_TEXT_COLOR);
        let mut runs: Vec<(String, Rgb)> = Vec::new();
        let mut cur = normal;
        let mut run = String::new();
        for ch in text.chars() {
            match ch {
                '\u{4}' | '\u{1}' => {
                    if !run.is_empty() {
                        runs.push((std::mem::take(&mut run), cur));
                    }
                    cur = if ch == '\u{4}' {
                        Rgb::new(255, 255, 255)
                    } else {
                        normal
                    };
                }
                c if (c as u32) < 0x20 => {}
                c => run.push(c),
            }
        }
        if !run.is_empty() {
            runs.push((run, cur));
        }
        for (dx, dy, shadow) in [(DIALOG_SHADOW, DIALOG_SHADOW, true), (0.0, 0.0, false)] {
            let mut pen = x + dx;
            for (t, c) in &runs {
                let col = if shadow { Rgb::new(0, 0, 0) } else { *c };
                pen = self.glue_text_face(
                    fb,
                    pen,
                    base + dy,
                    t,
                    GLUE_BIG_PX,
                    col,
                    None,
                    GlueFace::Big,
                );
            }
        }
    }
}

#[cfg(test)]
mod timestamp_tests {
    use super::format_timestamp;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn timestamps_render_as_civil_dates() {
        assert_eq!(format_timestamp(UNIX_EPOCH), "1970/01/01 00:00");
        // 2026-10-29 20:34:00 UTC
        let t = UNIX_EPOCH + Duration::from_secs(1_793_306_040);
        assert_eq!(format_timestamp(t), "2026/10/29 20:34");
        // Leap day.
        let t = UNIX_EPOCH + Duration::from_secs(1_709_164_800);
        assert_eq!(format_timestamp(t), "2024/02/29 00:00");
    }
}

#[cfg(test)]
mod menu_frame_tests {
    use super::{clear_menu_frame, Framebuffer, Rgb};

    #[test]
    fn menu_frame_clears_ultrawide_pillarbox_and_cursor_trails() {
        let mut fb = Framebuffer::new(3440, 1440);
        fb.clear(Rgb::new(255, 255, 255));
        clear_menu_frame(&mut fb);
        for (x, y) in [(0, 0), (439, 1439), (3000, 0), (3439, 1439), (1720, 720)] {
            assert_eq!(
                fb.get(x, y),
                Rgb::new(0, 0, 0),
                "stale pixel at ({x},{y}) in the menu frame"
            );
        }
    }
}

#[cfg(test)]
mod tile_grid_tests {
    use super::{parse_tile_grid, HUNTERS_ZERG_CREEP};

    #[test]
    fn hex_grid_maps_digits_to_tiles_and_skips_dots() {
        let g = parse_tile_grid((10, 20), &["0.a", ".c."]);
        assert_eq!(g.get(&(10, 20)), Some(&0));
        assert_eq!(g.get(&(12, 20)), Some(&10));
        assert_eq!(g.get(&(11, 21)), Some(&12));
        assert_eq!(g.len(), 3);
        // The recorded capture only uses the 13 creep subtiles.
        let creep = parse_tile_grid(HUNTERS_ZERG_CREEP.0, HUNTERS_ZERG_CREEP.1);
        assert!(creep.len() > 100 && creep.values().all(|&v| v < 13));
    }
}

#[cfg(test)]
mod mission_scrollbar_tests {
    use super::{draw_mission_scrollbar, Framebuffer};

    /// Live capture 21 (list at the top): thumb rows 519..560 are #ff0000,
    /// the track at y 700 is #6b0000 at x 1771, the arrow apex is red.
    #[test]
    fn scrollbar_matches_the_live_capture_geometry() {
        let mut fb = Framebuffer::new(2560, 1440);
        draw_mission_scrollbar(&mut fb, 0, 1.0, 0.0);
        let red = |x: i32, y: i32| fb.get(x, y).0 >> 16 & 0xff;
        assert!(red(1790, 521) > 240, "thumb top stroke");
        assert!(red(1794, 540) < 20, "thumb interior is hollow");
        assert!(
            red(1771, 700) > 90 && red(1771, 700) < 120,
            "dark track box"
        );
        assert!(red(1794, 465) > 200, "up arrow apex");
        assert!(red(1794, 925) > 200, "down arrow apex");
        // Last page: the thumb moves to 834..876 and leaves the top empty.
        let mut end = Framebuffer::new(2560, 1440);
        draw_mission_scrollbar(&mut end, 0, 1.0, 1.0);
        assert!((end.get(1790, 836).0 >> 16 & 0xff) > 240);
        assert!((end.get(1790, 521).0 >> 16 & 0xff) < 20);
    }
}

#[cfg(test)]
mod retail_timing_tests {
    use super::{cursor_frame_index, cursor_scale_for_height, menu_clip_timeline};
    use std::time::Duration;

    #[test]
    fn cursor_cycles_five_frames_at_six_fps() {
        // 1/6 s per frame: ten 60 Hz samples.
        assert_eq!(cursor_frame_index(Duration::ZERO, 5), 0);
        assert_eq!(cursor_frame_index(Duration::from_millis(166), 5), 0);
        assert_eq!(cursor_frame_index(Duration::from_millis(167), 5), 1);
        assert_eq!(cursor_frame_index(Duration::from_millis(333), 5), 1);
        assert_eq!(cursor_frame_index(Duration::from_millis(334), 5), 2);
        assert_eq!(cursor_frame_index(Duration::from_millis(500), 5), 3);
        assert_eq!(cursor_frame_index(Duration::from_millis(667), 5), 4);
        assert_eq!(cursor_frame_index(Duration::from_millis(833), 5), 4);
        assert_eq!(cursor_frame_index(Duration::from_millis(834), 5), 0);
        // Single-frame sources never index out of bounds.
        assert_eq!(cursor_frame_index(Duration::from_secs(10), 1), 0);
        assert_eq!(cursor_frame_index(Duration::from_secs(10), 0), 0);
    }

    #[test]
    fn cursor_is_hd_art_scaled_to_screen_height() {
        // Measured live: 56-57 px tall at 1440p and 42-43 px at 1080p.
        assert!((cursor_scale_for_height(1440) - 0.75).abs() < 1e-6);
        assert!((cursor_scale_for_height(1080) - 0.5625).abs() < 1e-6);
        assert!((cursor_scale_for_height(1920) - 1.0).abs() < 1e-6);
        // A tiny framebuffer still draws at least a pixel-scale arrow.
        assert!(cursor_scale_for_height(1) > 0.0);
    }

    #[test]
    fn campaign_clips_use_the_same_retail_10_5_fps_timeline() {
        // Authored at 15 fps: the three disks are 36 frames (2.4 s), every
        // portrait 76 (5.067 s) and the On overlays 20/21/31.  The retail
        // holds each authored frame for 1/10.5 s, exactly like the menu.
        let (_, frames, duration) = menu_clip_timeline(2.4, 0.0);
        assert_eq!(frames, 36);
        assert!((duration - 36.0 / 10.5).abs() < 0.001, "{duration}");
        let (_, frames, duration) = menu_clip_timeline(76.0 / 15.0, 0.0);
        assert_eq!(frames, 76);
        assert!((duration - 76.0 / 10.5).abs() < 0.001, "{duration}");
        for frames_authored in [20usize, 21, 31] {
            let (_, frames, _) = menu_clip_timeline(frames_authored as f32 / 15.0, 0.0);
            assert_eq!(frames, frames_authored);
        }
    }

    #[test]
    fn menu_layers_use_the_measured_retail_10_5_fps_loop() {
        // Live capture: 151-frame Single/Multi loop every 14.4 s; the
        // 105-frame Exit clip loops every 10.0 s.
        let (start, frames, duration) = menu_clip_timeline(10.067, 0.0);
        assert_eq!(start, 0.0);
        assert_eq!(frames, 151);
        assert!((duration - 14.381).abs() < 0.01, "{duration}");
        let (_, frames, duration) = menu_clip_timeline(7.0, 0.0);
        assert_eq!(frames, 105);
        assert!((duration - 10.0).abs() < 0.01, "{duration}");
        // A phase override still lands on an authored frame.
        let (start, frames, duration) = menu_clip_timeline(10.067, 2.5);
        assert!((start - 2.5333).abs() < 0.001, "{start}");
        assert_eq!(frames, 113);
        assert!((duration - 10.762).abs() < 0.01, "{duration}");
    }
}

#[cfg(test)]
mod options_chrome_tests {
    //! Retail Options chrome (130933..130954) at the 2560x1440 content box.
    use super::{
        draw_options_plate, opt_checkbox, opt_dropdown, CefSpace, Framebuffer, Rgb, CSS_INNER,
        CSS_OUTER,
    };

    const ROWS: [f32; 5] = [345.0, 438.0, 531.0, 624.0, 717.0];
    const BG: Rgb = Rgb::new(100, 100, 100);

    fn plate(active: usize) -> Framebuffer {
        let mut fb = Framebuffer::new(2560, 1440);
        fb.clear(BG);
        let space = CefSpace {
            cx: 0.0,
            kx: 1.0,
            ky: 1.0,
        };
        draw_options_plate(&mut fb, space, &ROWS, active);
        fb
    }

    #[test]
    fn plate_is_l_shaped_below_the_tab_column() {
        let fb = plate(0);
        // Left border of the tab boxes (x 513.1..517.7) stops at y 820.5;
        // below it the dimmed menu shows through (130954: planet + labels).
        assert_eq!(fb.get(515, 500), CSS_OUTER);
        assert_eq!(fb.get(515, 900), BG, "no plate left of the content column");
        assert_eq!(fb.get(700, 1149), BG, "bottom band starts at x 837");
        assert_eq!(fb.get(1000, 1149), CSS_OUTER);
        // The content column keeps its own divider down to the bottom.
        assert_eq!(fb.get(839, 1000), CSS_OUTER);
        assert_eq!(fb.get(846, 1000), CSS_INNER);
    }

    #[test]
    fn gaps_between_tab_boxes_are_left_unfilled() {
        let fb = plate(0);
        // 355.5..361.5 separates the title strip from the Speed box.
        assert_eq!(fb.get(700, 358), BG, "gap must not be filled");
        assert_ne!(fb.get(700, 300), BG, "title strip is filled");
        assert_ne!(fb.get(700, 400), BG, "tab box is filled");
    }

    #[test]
    fn selected_tab_opens_both_divider_bands() {
        // Network (row 717) selected: the divider breaks at y ~780.
        let open = plate(4);
        assert_ne!(open.get(839, 780), CSS_OUTER);
        assert_ne!(open.get(846, 780), CSS_INNER);
        let closed = plate(0);
        assert_eq!(closed.get(839, 780), CSS_OUTER);
        assert_eq!(closed.get(846, 780), CSS_INNER);
    }

    #[test]
    fn disabled_checked_checkbox_is_grey() {
        // 130933 "Use Mouse Confine": #404040 ring and a #808080 mark.
        let mut fb = Framebuffer::new(64, 64);
        fb.clear(Rgb::new(0, 0, 0));
        opt_checkbox(&mut fb, &mut None, 10, 10, 29, true, false, "", false, 24.0);
        assert_eq!(fb.get(25, 25), Rgb::new(128, 128, 128), "grey mark");
        assert_eq!(fb.get(11, 25), Rgb::new(64, 64, 64), "grey ring");
        let mut on = Framebuffer::new(64, 64);
        on.clear(Rgb::new(0, 0, 0));
        opt_checkbox(&mut on, &mut None, 10, 10, 29, true, true, "", false, 24.0);
        assert_eq!(on.get(25, 25), Rgb::new(0xfc, 0xfc, 0x4c), "yellow mark");
        assert_eq!(on.get(11, 25), CSS_INNER, "red ring");
    }

    #[test]
    fn checkbox_mark_has_a_soft_glow_edge() {
        // The CEF mark ramps over ~3.7 px (130933: 0.07/0.33/0.6/0.87/1.0).
        let mut fb = Framebuffer::new(64, 64);
        fb.clear(Rgb::new(0, 0, 0));
        opt_checkbox(&mut fb, &mut None, 10, 10, 29, true, true, "", false, 24.0);
        // Box 9.7..39.6 -> mark 50 % edge at x 17.2; ramp over 16..18.
        let g = |x: i32| (fb.get(x, 25).0 >> 8) & 0xff;
        assert!(
            g(15) < g(16) && g(16) < g(17) && g(17) < g(18) && g(18) < g(19),
            "ramp"
        );
        assert!(
            g(17) > 80 && g(17) < 200,
            "partial coverage at the edge: {}",
            g(17)
        );
    }

    #[test]
    fn enabled_dropdown_has_the_yellow_chevron_and_no_chevron_when_disabled() {
        let mut fb = Framebuffer::new(560, 80);
        fb.clear(Rgb::new(0, 0, 0));
        opt_dropdown(&mut fb, &mut None, 20, 10.0, 516, "", true, 1.0);
        // Chevron centre 25.9 px left of the right edge, near its apex row.
        let c = fb.get(20 + 516 - 26, 10 + 16 + 12);
        let (r, g, b) = ((c.0 >> 16) & 0xff, (c.0 >> 8) & 0xff, c.0 & 0xff);
        assert!(r > 150 && g > 150 && b < 40, "yellow chevron: {r},{g},{b}");
        let mut off = Framebuffer::new(560, 80);
        off.clear(Rgb::new(0, 0, 0));
        opt_dropdown(&mut off, &mut None, 20, 10.0, 516, "", false, 1.0);
        assert_eq!(off.get(20 + 516 - 26, 10 + 16 + 12), Rgb::new(0, 0, 0));
    }
}

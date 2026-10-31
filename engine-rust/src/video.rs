//! Retail WebM cinematics: intros, campaign disks and briefing intermissions.
//!
//! StarCraft 1.23 ships its movies as WebM (`smk/*.webm`, `sd/smk/*.webm`,
//! `interstitials/*.webm`, `glue/campaign/*.webm`).  The previous pass
//! recorded the `</VIDEO>` tags from the `est*.txt` briefing scripts but left
//! the files on disk: `retail.rs` skipped them because the client had no
//! video decoder.
//!
//! This module is the decoder front-end:
//!
//! * [`parse_webm`] is a dependency-free EBML/WebM header parser.  It reads
//!   the `EBML`/`Segment`/`Info`/`Tracks` elements the retail files use and
//!   returns [`VideoInfo`] (codec, pixel size, duration, audio presence,
//!   cluster count).  No retail bytes are embedded; the parser is exercised
//!   against a synthetic minimal WebM and, when `SC_INSTALL` is set, against
//!   the real `smk/starintr.webm` header.
//! * [`VIDEO_CATALOG`] / [`intro_for_episode`] map retail playback requests
//!   (title intro, expansion intro, briefing intermission, campaign disk) to
//!   the CASC paths the game actually loads.
//! * [`decode_first_frame`] shells out to the system `ffmpeg` (when present)
//!   to decode the poster frame to `RGB24`.  The client composites that frame
//!   on the title intro and on briefing pages whose script names a video,
//!   so the decoded cinematics are used on the retail title/briefing screens.
//!   Full-motion scrubbing outside those screens remains out of scope and is
//!   recorded as the remaining limit in `docs/EVIDENCE.md`.
//!
//! REA provenance: the path inventory comes from `sc_list --suffix .webm`
//! against 1.23.10.13515 (52,498 CASC paths; 2,578 `.webm` hits, 2,016
//! non-Carbot).  The `est*.txt` `</VIDEO>`/`</VIDEOAUTOPROCEED>` tags name
//! `interstitials/*.webm`; the title intro is `smk/starintr.webm` and the
//! expansion intro `smk/starxintr.webm` (122,081,725 and 75,968,887 bytes in
//! this build, both starting `1a 45 df a3`).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The publisher logo the retail client plays before the title movie.
///
/// REA evidence: the 64-bit 1.23.10.13515 executable stores its intro table at
/// `0x140ae7ae0` (`smk\blizzard.webm`, `smk\starintr.webm`,
/// `smk\PreTerran_1.webm` … `smk\xZerg.webm`).  `blizzard.webm` is a separate
/// 11.4 s clip, so playing only `starintr.webm` skipped the modern logo.
pub const BLIZZARD_LOGO: &str = "smk/blizzard.webm";
/// The two title intros the retail `View Intro` button plays.
pub const INTRO_ORIGINAL: &str = "smk/starintr.webm";
/// Brood War expansion intro.
pub const INTRO_EXPANSION: &str = "smk/starxintr.webm";

/// Every campaign cinematic in the executable's intro table, in table order.
/// The table groups them per episode: Terran `PreTerran_1..4`/`Terran1..4`,
/// Zerg `PreZerg_2..4`/`Zerg1..4`, Protoss
/// `PreProtoss_1..3`/`Protoss1..4`, and the Brood War
/// `xProtoss`/`xTerran`/`xZerg` episode intros.
pub const CAMPAIGN_CINEMATICS: &[&str] = &[
    "smk/preterran_1.webm",
    "smk/preterran_2.webm",
    "smk/preterran_3.webm",
    "smk/preterran_4.webm",
    "smk/terran1.webm",
    "smk/terran2.webm",
    "smk/terran3.webm",
    "smk/terran4.webm",
    "smk/prezerg_2.webm",
    "smk/prezerg_3.webm",
    "smk/prezerg_4.webm",
    "smk/zerg1.webm",
    "smk/zerg2.webm",
    "smk/zerg3.webm",
    "smk/zerg4.webm",
    "smk/preprotoss_1.webm",
    "smk/preprotoss_2.webm",
    "smk/preprotoss_3.webm",
    "smk/protoss1.webm",
    "smk/protoss2.webm",
    "smk/protoss3.webm",
    "smk/protoss4.webm",
    "smk/xprotoss.webm",
    "smk/xterran.webm",
    "smk/xzerg.webm",
];

/// The act a campaign mission belongs to.  The shipped campaigns use four
/// three-mission acts in the Terran episode and shorter closing acts in the
/// Zerg/Protoss episodes, which is exactly what the `Pre*`/`*N` cinematic
/// pairs in the intro table encode.
pub fn campaign_act(episode_key: &str, order: usize) -> Option<u8> {
    let boundaries: &[(usize, usize, u8)] = match episode_key {
        "terran" => &[(1, 3, 1), (4, 6, 2), (7, 9, 3), (10, 12, 4)],
        "zerg" => &[(1, 3, 1), (4, 6, 2), (7, 8, 3), (9, 10, 4)],
        "protoss" => &[(1, 3, 1), (4, 6, 2), (7, 8, 3), (9, 10, 4)],
        _ => return None,
    };
    boundaries
        .iter()
        .find(|(lo, hi, _)| order >= *lo && order <= *hi)
        .map(|(_, _, act)| *act)
}

/// True when `order` is the first mission of its act (the `Pre*` movie plays
/// before it).
pub fn is_act_start(episode_key: &str, order: usize) -> bool {
    match episode_key {
        "terran" => matches!(order, 1 | 4 | 7 | 10),
        "zerg" => matches!(order, 4 | 7 | 9),
        "protoss" => matches!(order, 1 | 4 | 7),
        _ => false,
    }
}

/// True when `order` is the last mission of its act (the numbered episode
/// movie plays after it).
pub fn is_act_end(episode_key: &str, order: usize) -> bool {
    match episode_key {
        "terran" => matches!(order, 3 | 6 | 9 | 12),
        "zerg" => matches!(order, 3 | 6 | 8 | 10),
        "protoss" => matches!(order, 3 | 6 | 8 | 10),
        _ => false,
    }
}

/// Campaign cinematic for a mission start/end.
///
/// * `pre`  — `Pre<Race>_<act>.webm` before the first mission of an act
///   (Zerg and Protoss have no act-1/act-4 pre movie, matching the shipped
///   table, which starts at `PreZerg_2` and `PreProtoss_1`).
/// * `post` — `<Race><act>.webm` after the last mission of an act.
pub fn campaign_cinematic(episode_key: &str, order: usize, pre: bool) -> Option<&'static str> {
    let act = campaign_act(episode_key, order)?;
    let at_edge = if pre {
        is_act_start(episode_key, order)
    } else {
        is_act_end(episode_key, order)
    };
    if !at_edge {
        return None;
    }
    let race = match episode_key {
        "terran" => "terran",
        "zerg" => "zerg",
        _ => "protoss",
    };
    let name = if pre {
        format!("smk/pre{race}_{act}.webm")
    } else {
        format!("smk/{race}{act}.webm")
    };
    CAMPAIGN_CINEMATICS
        .iter()
        .find(|cinematic| cinematic.eq_ignore_ascii_case(&name))
        .copied()
}

/// Brood War episode intro movie (`xProtoss`/`xTerran`/`xZerg`).
pub fn expansion_episode_intro(episode_key: &str) -> Option<&'static str> {
    let name = match episode_key {
        "expprotoss" => "smk/xprotoss.webm",
        "expterran" => "smk/xterran.webm",
        "expzerg" => "smk/xzerg.webm",
        _ => return None,
    };
    CAMPAIGN_CINEMATICS
        .iter()
        .find(|cinematic| cinematic.eq_ignore_ascii_case(name))
        .copied()
}

/// The clips `View Intro` / first launch play, in order.
pub fn intro_sequence(expansion: bool, include_logo: bool) -> Vec<&'static str> {
    let mut clips = Vec::with_capacity(2);
    if include_logo {
        clips.push(BLIZZARD_LOGO);
    }
    clips.push(intro_for_episode(expansion));
    clips
}

/// Campaign-select disk loops shown behind the episode picker.
pub const DISK_TERRAN: &str = "glue/campaign/terr.webm";
pub const DISK_ZERG: &str = "glue/campaign/zerg.webm";
pub const DISK_PROTOSS: &str = "glue/campaign/prot.webm";

/// Provenance inventory: every retail WebM family the client decodes posters
/// from.  Counts observed on 1.23.10.13515 via `sc_list`.
#[derive(Clone, Debug)]
pub struct VideoCatalogEntry {
    pub family: &'static str,
    pub prefix: &'static str,
    pub count: usize,
}

pub const VIDEO_CATALOG: &[VideoCatalogEntry] = &[
    VideoCatalogEntry {
        family: "title intros",
        prefix: "smk/starintr.webm + smk/starxintr.webm",
        count: 2,
    },
    VideoCatalogEntry {
        family: "campaign cinematics",
        prefix: "smk/terran*.webm + zerg*.webm + protoss*.webm + x*.webm + pre*.webm",
        count: 26,
    },
    VideoCatalogEntry {
        family: "briefing intermissions",
        prefix: "interstitials/*.webm",
        count: 62,
    },
    VideoCatalogEntry {
        family: "campaign disk loops",
        prefix: "glue/campaign/*.webm + glue/expcampaign/*.webm",
        count: 18,
    },
    VideoCatalogEntry {
        family: "menu loops",
        prefix: "glue/mainmenu/*.webm + glue/ready*/ *.webm",
        count: 16,
    },
];

/// Decoded header for one WebM file.
#[derive(Clone, Debug, PartialEq)]
pub struct VideoInfo {
    pub codec: String,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub display_width: u32,
    pub display_height: u32,
    pub duration_ms: Option<u64>,
    pub timecode_scale: u64,
    pub has_audio: bool,
    pub video_tracks: usize,
    pub audio_tracks: usize,
    pub clusters: usize,
    pub bytes: usize,
}

impl VideoInfo {
    pub fn aspect(&self) -> f32 {
        if self.pixel_height == 0 {
            1.0
        } else {
            self.pixel_width as f32 / self.pixel_height as f32
        }
    }
}

/// Map a playback request to the CASC path retail loads.
///
/// * `intro_expansion == false` -> `smk/starintr.webm` (the `View Intro`
///   default); `true` -> `smk/starxintr.webm`.
pub fn intro_for_episode(intro_expansion: bool) -> &'static str {
    if intro_expansion {
        INTRO_EXPANSION
    } else {
        INTRO_ORIGINAL
    }
}

/// Normalise an authored briefing video path (`interstitials\TR1010.webm`)
/// to the CASC form (`interstitials/tr1010.webm`).
pub fn normalise_video_path(authored: &str) -> String {
    authored
        .trim()
        .trim_matches('"')
        .replace('\\', "/")
        .to_ascii_lowercase()
}

/// True when the path names a known retail cinematic family.
pub fn is_known_cinematic(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".webm")
        && (lower.starts_with("smk/")
            || lower.starts_with("sd/smk/")
            || lower.starts_with("interstitials/")
            || lower.starts_with("glue/campaign/")
            || lower.starts_with("glue/expcampaign/")
            || lower.starts_with("glue/mainmenu/")
            || lower.starts_with("glue/ready"))
}

// ---------------------------------------------------------------------------
// EBML/WebM header parser
// ---------------------------------------------------------------------------

fn read_id(data: &[u8], pos: usize) -> Option<(u64, usize)> {
    if pos >= data.len() {
        return None;
    }
    let b = data[pos];
    let len = if b & 0x80 != 0 {
        1
    } else if b & 0x40 != 0 {
        2
    } else if b & 0x20 != 0 {
        3
    } else if b & 0x10 != 0 {
        4
    } else {
        return None;
    };
    if pos + len > data.len() {
        return None;
    }
    let mut id: u64 = 0;
    for i in 0..len {
        id = (id << 8) | data[pos + i] as u64;
    }
    Some((id, len))
}

fn read_size(data: &[u8], pos: usize) -> Option<(Option<u64>, usize)> {
    if pos >= data.len() {
        return None;
    }
    let b = data[pos];
    let len = if b & 0x80 != 0 {
        1
    } else if b & 0x40 != 0 {
        2
    } else if b & 0x20 != 0 {
        3
    } else if b & 0x10 != 0 {
        4
    } else if b & 0x08 != 0 {
        5
    } else if b & 0x04 != 0 {
        6
    } else if b & 0x02 != 0 {
        7
    } else if b & 0x01 != 0 {
        8
    } else {
        return None;
    };
    if pos + len > data.len() {
        return None;
    }
    let mask: u8 = match len {
        1 => 0x7f,
        2 => 0x3f,
        3 => 0x1f,
        4 => 0x0f,
        5 => 0x07,
        6 => 0x03,
        7 => 0x01,
        _ => 0x00,
    };
    let mut value: u64 = (data[pos] & mask) as u64;
    for i in 1..len {
        value = (value << 8) | data[pos + i] as u64;
    }
    // All-bits-set means "unknown size" (Segment / Cluster with streaming).
    let all_ones: u64 = if len == 8 {
        u64::MAX
    } else {
        (1u64 << (7 * len as u64)) - 1
    };
    if value == all_ones {
        return Some((None, len));
    }
    Some((Some(value), len))
}

fn read_uint(data: &[u8]) -> u64 {
    let mut v = 0u64;
    for &b in data {
        v = (v << 8) | b as u64;
    }
    v
}

fn read_float(data: &[u8]) -> f64 {
    match data.len() {
        4 => f32::from_be_bytes([data[0], data[1], data[2], data[3]]) as f64,
        8 => f64::from_be_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]),
        _ => {
            // Non-standard width: interpret as big-endian integer.
            read_uint(data) as f64
        }
    }
}

const ID_EBML: u64 = 0x1A45DFA3;
const ID_SEGMENT: u64 = 0x18538067;
const ID_INFO: u64 = 0x1549A966;
const ID_TRACKS: u64 = 0x1654AE6B;
const ID_CLUSTER: u64 = 0x1F43B675;
const ID_TIMECODE_SCALE: u64 = 0x2AD7B1;
const ID_DURATION: u64 = 0x4489;
const ID_TRACK_ENTRY: u64 = 0xAE;
#[allow(dead_code)]
const ID_TRACK_NUMBER: u64 = 0xD7;
const ID_TRACK_TYPE: u64 = 0x83;
const ID_CODEC_ID: u64 = 0x86;
const ID_VIDEO: u64 = 0xE0;
const ID_PIXEL_WIDTH: u64 = 0xB0;
const ID_PIXEL_HEIGHT: u64 = 0xBA;
const ID_DISPLAY_WIDTH: u64 = 0x54B0;
const ID_DISPLAY_HEIGHT: u64 = 0x54BB;

/// Parse the header of a WebM file (EBML + Segment Info/Tracks + cluster count).
pub fn parse_webm(bytes: &[u8]) -> Result<VideoInfo, String> {
    if bytes.len() < 4
        || bytes[0] != 0x1A
        || bytes[1] != 0x45
        || bytes[2] != 0xDF
        || bytes[3] != 0xA3
    {
        return Err("not EBML/WebM (missing 0x1A45DFA3)".into());
    }
    let mut pos = 0usize;
    let mut segment_body: Option<(usize, Option<usize>)> = None;
    // Top level: EBML header + Segment.
    while pos < bytes.len() {
        let (id, id_len) = read_id(bytes, pos).ok_or("truncated EBML id")?;
        let (size, size_len) = read_size(bytes, pos + id_len).ok_or("truncated EBML size")?;
        let head = id_len + size_len;
        let body_len: Option<usize> = size.map(|s| s as usize);
        if id == ID_EBML {
            let len = body_len.ok_or("EBML header with unknown size")?;
            pos += head + len;
        } else if id == ID_SEGMENT {
            segment_body = Some((pos + head, body_len));
            break;
        } else {
            match body_len {
                Some(len) => {
                    pos += head + len;
                }
                None => {
                    break;
                }
            }
        }
        if pos >= bytes.len() {
            break;
        }
    }
    let (mut seg_pos, seg_len) = segment_body.ok_or("WebM has no Segment")?;
    let seg_end = match seg_len {
        Some(len) => (seg_pos + len).min(bytes.len()),
        None => bytes.len(),
    };
    let mut timecode_scale: u64 = 1_000_000;
    let mut duration_ms: Option<u64> = None;
    let mut duration_raw: Option<f64> = None;
    let mut codec = String::from("unknown");
    let mut pixel_width = 0u32;
    let mut pixel_height = 0u32;
    let mut display_width = 0u32;
    let mut display_height = 0u32;
    let mut has_audio = false;
    let mut video_tracks = 0usize;
    let mut audio_tracks = 0usize;
    let mut clusters = 0usize;
    // First pass: Info + Tracks + cluster count.
    while seg_pos < seg_end {
        let (id, id_len) = match read_id(bytes, seg_pos) {
            Some(v) => v,
            None => break,
        };
        let (size, size_len) = match read_size(bytes, seg_pos + id_len) {
            Some(v) => v,
            None => break,
        };
        let head = id_len + size_len;
        let body_start = seg_pos + head;
        let body_end = match size {
            Some(len) => (body_start + (len as usize)).min(seg_end).min(bytes.len()),
            None => seg_end,
        };
        if body_start > bytes.len() {
            break;
        }
        match id {
            ID_INFO => {
                let mut p = body_start;
                while p < body_end {
                    let (cid, clen) = match read_id(bytes, p) {
                        Some(v) => v,
                        None => break,
                    };
                    let (csize, slen) = match read_size(bytes, p + clen) {
                        Some(v) => v,
                        None => break,
                    };
                    let chead = clen + slen;
                    let cbody = p + chead;
                    let cend = match csize {
                        Some(l) => (cbody + l as usize).min(body_end),
                        None => body_end,
                    };
                    if cbody > bytes.len() {
                        break;
                    }
                    match cid {
                        ID_TIMECODE_SCALE => {
                            if cbody < cend {
                                timecode_scale = read_uint(&bytes[cbody..cend.min(bytes.len())]);
                                if timecode_scale == 0 {
                                    timecode_scale = 1_000_000;
                                }
                            }
                        }
                        ID_DURATION if cbody < cend => {
                            duration_raw = Some(read_float(&bytes[cbody..cend.min(bytes.len())]));
                        }
                        ID_DURATION => {}
                        _ => {}
                    }
                    match csize {
                        Some(_) => {
                            p = cend;
                        }
                        None => break,
                    }
                }
            }
            ID_TRACKS => {
                let mut p = body_start;
                while p < body_end {
                    let (cid, clen) = match read_id(bytes, p) {
                        Some(v) => v,
                        None => break,
                    };
                    let (csize, slen) = match read_size(bytes, p + clen) {
                        Some(v) => v,
                        None => break,
                    };
                    let chead = clen + slen;
                    let cbody = p + chead;
                    let cend = match csize {
                        Some(l) => (cbody + l as usize).min(body_end),
                        None => body_end,
                    };
                    if cid == ID_TRACK_ENTRY {
                        let (t, a, c, w, h, dw, dh) =
                            parse_track_entry(&bytes[cbody..cend.min(bytes.len())]);
                        match t {
                            1 => {
                                video_tracks += 1;
                                if !c.is_empty()
                                    && (codec == "unknown"
                                        || c.contains("VP9")
                                        || c.contains("AV1"))
                                {
                                    codec = c;
                                }
                                if w != 0 {
                                    pixel_width = w;
                                }
                                if h != 0 {
                                    pixel_height = h;
                                }
                                if dw != 0 {
                                    display_width = dw;
                                }
                                if dh != 0 {
                                    display_height = dh;
                                }
                            }
                            2 => {
                                audio_tracks += 1;
                                has_audio = true;
                                let _ = a;
                            }
                            _ => {}
                        }
                    }
                    match csize {
                        Some(_) => {
                            p = cend;
                        }
                        None => break,
                    }
                }
            }
            ID_CLUSTER => {
                clusters += 1;
                seg_pos = body_end;
                continue;
            }
            _ => {}
        }
        match size {
            Some(_) => {
                seg_pos = body_end;
            }
            None => break,
        }
    }
    if let Some(raw) = duration_raw {
        // Duration is in timecode ticks; convert through the scale.
        let ms = raw * timecode_scale as f64 / 1_000_000.0;
        if ms.is_finite() && (0.0..86_400_000.0 * 10.0).contains(&ms) {
            duration_ms = Some(ms.round() as u64);
        }
    }
    if display_width == 0 {
        display_width = pixel_width;
    }
    if display_height == 0 {
        display_height = pixel_height;
    }
    Ok(VideoInfo {
        codec,
        pixel_width,
        pixel_height,
        display_width,
        display_height,
        duration_ms,
        timecode_scale,
        has_audio,
        video_tracks,
        audio_tracks,
        clusters,
        bytes: bytes.len(),
    })
}

#[allow(clippy::type_complexity)]
fn parse_track_entry(data: &[u8]) -> (u64, bool, String, u32, u32, u32, u32) {
    let mut track_type = 0u64;
    let mut is_audio = false;
    let mut codec = String::new();
    let mut w = 0u32;
    let mut h = 0u32;
    let mut dw = 0u32;
    let mut dh = 0u32;
    let mut pos = 0usize;
    while pos < data.len() {
        let (id, id_len) = match read_id(data, pos) {
            Some(v) => v,
            None => break,
        };
        let (size, size_len) = match read_size(data, pos + id_len) {
            Some(v) => v,
            None => break,
        };
        let head = id_len + size_len;
        let body = pos + head;
        let end = match size {
            Some(len) => (body + len as usize).min(data.len()),
            None => data.len(),
        };
        if body > data.len() {
            break;
        }
        let slice = &data[body..end.min(data.len())];
        match id {
            ID_TRACK_TYPE => {
                track_type = read_uint(slice);
                if track_type == 2 {
                    is_audio = true;
                }
            }
            ID_CODEC_ID => {
                codec = String::from_utf8_lossy(slice)
                    .trim_matches('\0')
                    .to_string();
            }
            ID_VIDEO => {
                let mut p = 0usize;
                while p < slice.len() {
                    let (vid, vlen) = match read_id(slice, p) {
                        Some(v) => v,
                        None => break,
                    };
                    let (vsize, vslen) = match read_size(slice, p + vlen) {
                        Some(v) => v,
                        None => break,
                    };
                    let vhead = vlen + vslen;
                    let vbody = p + vhead;
                    let vend = match vsize {
                        Some(l) => (vbody + l as usize).min(slice.len()),
                        None => slice.len(),
                    };
                    if vbody > slice.len() {
                        break;
                    }
                    let v = read_uint(&slice[vbody..vend]) as u32;
                    match vid {
                        ID_PIXEL_WIDTH => w = v,
                        ID_PIXEL_HEIGHT => h = v,
                        ID_DISPLAY_WIDTH => dw = v,
                        ID_DISPLAY_HEIGHT => dh = v,
                        _ => {}
                    }
                    match vsize {
                        Some(_) => p = vend,
                        None => break,
                    }
                }
            }
            _ => {}
        }
        match size {
            Some(_) => pos = end,
            None => break,
        }
    }
    (track_type, is_audio, codec, w, h, dw, dh)
}

// ---------------------------------------------------------------------------
// Poster decoding via ffmpeg (optional at runtime)
// ---------------------------------------------------------------------------

/// Decoded poster frame: row-major `RGB24`.
#[derive(Clone, Debug)]
pub struct PosterFrame {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
}

/// Decode the first video frame with the system `ffmpeg`.
///
/// Returns `None` when `ffmpeg` is missing, the input is not a video file, or
/// decoding fails.  The client treats `None` as "show the backdrop art with
/// the INTERMISSION caption" so headless/CI runs without `ffmpeg` still work.
/// Decode `count` consecutive frames from `start` seconds at roughly `fps`,
/// in a single ffmpeg run (one process instead of one per frame).
///
/// Used by the animated main-menu layers, which loop a short clip.
pub fn decode_frames(
    webm: &[u8],
    info: &VideoInfo,
    start: f32,
    count: usize,
    fps: f32,
    max_width: u32,
) -> Vec<PosterFrame> {
    if count == 0 || info.pixel_width == 0 || info.pixel_height == 0 {
        return Vec::new();
    }
    let dir = std::env::temp_dir().join(format!("galaxycraft-frames-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let src = dir.join("frames_in.webm");
    if std::fs::write(&src, webm).is_err() {
        return Vec::new();
    }
    let frames = decode_frames_from_path(&src, start, count, fps, max_width);
    let _ = std::fs::remove_file(&src);
    frames
}

/// Same as [`decode_frames`], reading the video straight from disk.
///
/// The title intro is 122 MB: streaming it from the materialised cache avoids
/// keeping a second full copy in memory (and a temp-file write per chunk).
pub fn decode_frames_from_path(
    path: &Path,
    start: f32,
    count: usize,
    fps: f32,
    max_width: u32,
) -> Vec<PosterFrame> {
    let Some(info) = probe_info(path) else {
        return Vec::new();
    };
    decode_frames_from_info(path, &info, start, count, fps, max_width)
}

/// [`decode_frames_from_path`] with an already-parsed header (the streaming
/// player reuses one `VideoInfo` for every chunk instead of re-reading the
/// file's EBML head each time).
pub fn decode_frames_from_info(
    path: &Path,
    info: &VideoInfo,
    start: f32,
    count: usize,
    fps: f32,
    max_width: u32,
) -> Vec<PosterFrame> {
    if count == 0 || info.pixel_width == 0 || info.pixel_height == 0 {
        return Vec::new();
    }
    let w = info.pixel_width.min(max_width).max(1);
    let h = (((info.pixel_height as u64 * w as u64) / info.pixel_width as u64) as u32).max(1);
    let out = Command::new("ffmpeg")
        .args([
            "-y",
            "-v",
            "error",
            "-ss",
            &format!("{start:.2}"),
            "-i",
            &path.to_string_lossy(),
            "-vframes",
            &count.to_string(),
            "-r",
            &format!("{fps}"),
            "-s",
            &format!("{w}x{h}"),
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    let Ok(out) = out else { return Vec::new() };
    if !out.status.success() {
        return Vec::new();
    }
    let frame_bytes = (w as usize) * (h as usize) * 3;
    if frame_bytes == 0 {
        return Vec::new();
    }
    out.stdout
        .chunks_exact(frame_bytes)
        .map(|f| PosterFrame {
            width: w,
            height: h,
            rgb: f.to_vec(),
        })
        .collect()
}

/// Parse a WebM header straight from disk.
///
/// Only the first two megabytes are read: EBML `Info`/`Tracks` live at the
/// start, and the 122 MB title intro must not be slurped just to learn its
/// duration.  Cluster count is therefore 0 for large files; callers that need
/// it use [`parse_webm`] on the full bytes.
pub fn probe_info(path: &Path) -> Option<VideoInfo> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::with_capacity(2 * 1024 * 1024);
    (&mut file)
        .take(2 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .ok()?;
    parse_webm(&bytes).ok()
}

pub fn decode_first_frame(webm: &[u8], info: &VideoInfo) -> Option<PosterFrame> {
    decode_frame_at(webm, info, None, 640)
}

/// Decode one frame at `seek` seconds (or the poster heuristic when `None`),
/// keeping up to `max_width` pixels. Menu layers need their native detail,
/// so they pass a larger cap than the 640px intro poster.
pub fn decode_frame_at(
    webm: &[u8],
    info: &VideoInfo,
    seek: Option<f32>,
    max_width: u32,
) -> Option<PosterFrame> {
    if info.pixel_width == 0 || info.pixel_height == 0 {
        return None;
    }
    let dir = std::env::temp_dir().join(format!("galaxycraft-video-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let src = dir.join("poster_in.webm");
    if std::fs::write(&src, webm).is_err() {
        return None;
    }
    let w = info.pixel_width.min(max_width);
    let h = if info.pixel_width == 0 {
        info.pixel_height.min(max_width)
    } else {
        (((info.pixel_height as u64 * w as u64) / info.pixel_width as u64) as u32).max(1)
    };
    // Seek past the opening fade: 10% of the duration (1..15 s) lands on
    // real content for the 138 s title intro as well as the 3 s
    // intermissions.  The first frame of `starintr.webm` is pure black.
    let seek = seek.unwrap_or_else(|| {
        info.duration_ms
            .map(|ms| ((ms / 10).clamp(1000, 15_000)) as f32 / 1000.0)
            .unwrap_or(1.0)
    });
    let out = Command::new("ffmpeg")
        .args([
            "-y",
            "-v",
            "error",
            "-ss",
            &format!("{seek:.1}"),
            "-i",
            &src.to_string_lossy(),
            "-vframes",
            "1",
            "-s",
            &format!("{w}x{h}"),
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let _ = std::fs::remove_file(&src);
    if !out.status.success() {
        return None;
    }
    let expect = (w as usize) * (h as usize) * 3;
    if out.stdout.len() < expect {
        return None;
    }
    Some(PosterFrame {
        width: w,
        height: h,
        rgb: out.stdout[..expect].to_vec(),
    })
}

/// Decode a movie's audio track to mono `i16` PCM at [`AUDIO_RATE`].
///
/// The retail cinematics carry their soundtrack in the same WebM; the client's
/// mixer only speaks mono `i16`, so `ffmpeg` resamples once and the samples are
/// handed to the existing music path.  Returns `None` when the file has no
/// audio, `ffmpeg` is missing, or the decode fails.
pub const AUDIO_RATE: u32 = 22_050;

pub fn decode_audio_pcm(path: &Path) -> Option<Vec<i16>> {
    if !path.exists() {
        return None;
    }
    let out = Command::new("ffmpeg")
        .args([
            "-y",
            "-v",
            "error",
            "-i",
            &path.to_string_lossy(),
            "-vn",
            "-f",
            "s16le",
            "-ac",
            "1",
            "-ar",
            &AUDIO_RATE.to_string(),
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() || out.stdout.len() < 2 {
        return None;
    }
    let samples: Vec<i16> = out
        .stdout
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| i16::from_le_bytes(*c))
        .collect();
    (!samples.is_empty()).then_some(samples)
}

/// Decode `count` consecutive frames **with their alpha channel** from a
/// retail WebM on disk (VP9 `BlockAdditional` alpha, as used by every
/// Remastered front-end animation).
pub fn decode_rgba_frames_from_path(
    path: &Path,
    info: &VideoInfo,
    start: f32,
    count: usize,
    fps: f32,
    max_width: u32,
) -> Vec<RgbaFrame> {
    if count == 0 || info.pixel_width == 0 || info.pixel_height == 0 || !path.exists() {
        return Vec::new();
    }
    let w = info.pixel_width.min(max_width).clamp(1, 4096);
    let h = (((info.pixel_height as u64 * w as u64) / info.pixel_width as u64) as u32).max(1);
    let out = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-c:v",
            "libvpx-vp9",
            "-ss",
            &format!("{start:.2}"),
            "-i",
            &path.to_string_lossy(),
            "-vframes",
            &count.to_string(),
            "-r",
            &format!("{fps}"),
            "-vf",
            &format!("scale={w}:{h}"),
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    let Ok(out) = out else { return Vec::new() };
    if !out.status.success() {
        return Vec::new();
    }
    let frame_bytes = (w as usize) * (h as usize) * 4;
    if frame_bytes == 0 {
        return Vec::new();
    }
    out.stdout
        .chunks_exact(frame_bytes)
        .map(|f| RgbaFrame {
            width: w,
            height: h,
            rgba: f.to_vec(),
        })
        .collect()
}

/// Decode one frame of a video file on disk at `seek` seconds.
///
/// Same as [`decode_frame_at`] but streams the file in place, which is what
/// the 122 MB title intro needs (a temp copy per seek is wasted I/O).
pub fn decode_frame_at_path(
    path: &Path,
    info: &VideoInfo,
    seek: f32,
    max_width: u32,
) -> Option<PosterFrame> {
    if info.pixel_width == 0 || info.pixel_height == 0 || !path.exists() {
        return None;
    }
    let w = info.pixel_width.min(max_width).max(1);
    let h = (((info.pixel_height as u64 * w as u64) / info.pixel_width as u64) as u32).max(1);
    let out = Command::new("ffmpeg")
        .args([
            "-y",
            "-v",
            "error",
            "-ss",
            &format!("{seek:.2}"),
            "-i",
            &path.to_string_lossy(),
            "-vframes",
            "1",
            "-s",
            &format!("{w}x{h}"),
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let expect = (w as usize) * (h as usize) * 3;
    if out.stdout.len() < expect {
        return None;
    }
    Some(PosterFrame {
        width: w,
        height: h,
        rgb: out.stdout[..expect].to_vec(),
    })
}

/// Decode a still image (JPEG/PNG) to raw RGB through `ffmpeg`.
///
/// The Remastered front-end ships its collection thumbnails as ordinary
/// images; the rest of the client only speaks DDS/GRP, so they are converted
/// on demand by the same tool already used for video frames.
pub fn decode_still(bytes: &[u8], ext: &str, w: u32, h: u32) -> Option<PosterFrame> {
    if bytes.is_empty() || w == 0 || h == 0 {
        return None;
    }
    let dir = std::env::temp_dir().join(format!("galaxycraft-still-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let src = dir.join(format!("still.{ext}"));
    std::fs::write(&src, bytes).ok()?;
    let out = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-i",
            &src.to_string_lossy(),
            "-frames:v",
            "1",
            "-vf",
            &format!("scale={w}:{h}"),
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let _ = std::fs::remove_file(&src);
    if !out.status.success() || out.stdout.len() < (w as usize) * (h as usize) * 3 {
        return None;
    }
    Some(PosterFrame {
        width: w,
        height: h,
        rgb: out.stdout[..(w as usize) * (h as usize) * 3].to_vec(),
    })
}

/// Decode one still keeping its alpha channel.  The web UI command icons
/// (`webui/dist/lib/images/cmdicons.*.png`) are RGBA masks: the RGB carries
/// the highlight ramp and the alpha is the glyph coverage.
pub fn decode_still_rgba(bytes: &[u8], ext: &str, w: u32, h: u32) -> Option<RgbaFrame> {
    if bytes.is_empty() || w == 0 || h == 0 {
        return None;
    }
    let dir = std::env::temp_dir().join(format!("galaxycraft-still-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let src = dir.join(format!("still.{ext}"));
    std::fs::write(&src, bytes).ok()?;
    let out = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-i",
            &src.to_string_lossy(),
            "-frames:v",
            "1",
            "-vf",
            &format!("scale={w}:{h}"),
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let _ = std::fs::remove_file(&src);
    if !out.status.success() || out.stdout.len() < (w as usize) * (h as usize) * 4 {
        return None;
    }
    Some(RgbaFrame {
        width: w,
        height: h,
        rgba: out.stdout[..(w as usize) * (h as usize) * 4].to_vec(),
    })
}

/// A decoded frame that keeps its alpha channel.
#[derive(Clone, Debug)]
pub struct RgbaFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl RgbaFrame {
    /// True when the clip really carries transparency.
    pub fn has_alpha(&self) -> bool {
        self.rgba.as_chunks::<4>().0.iter().any(|p| p[3] < 250)
    }
}

/// Decode one frame keeping the alpha channel.
///
/// The Remastered front-end art (episode portraits, campaign screens) is VP9
/// with alpha stored in BlockAdditional chunks.  A plain decode drops it and
/// shows the alpha plane as garbage backdrop, so the alpha-capable libvpx
/// decoder is requested explicitly.  Returns `None` without `ffmpeg` or when
/// the clip has no alpha sample (callers then use [`decode_frame_at`]).
pub fn decode_rgba_frame_at(
    webm: &[u8],
    info: &VideoInfo,
    seek: f32,
    max_width: u32,
) -> Option<RgbaFrame> {
    if info.pixel_width == 0 || info.pixel_height == 0 {
        return None;
    }
    let dir = std::env::temp_dir().join(format!("galaxycraft-alpha-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let src = dir.join("alpha_in.webm");
    std::fs::write(&src, webm).ok()?;
    let w = info.pixel_width.min(max_width);
    let h = (((info.pixel_height as u64 * w as u64) / info.pixel_width as u64) as u32).max(1);
    let out = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-c:v",
            "libvpx-vp9",
            "-ss",
            &format!("{seek:.2}"),
            "-i",
            &src.to_string_lossy(),
            "-frames:v",
            "1",
            "-vf",
            &format!("scale={w}:-1"),
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let _ = std::fs::remove_file(&src);
    if !out.status.success() || out.stdout.len() < (w as usize) * (h as usize) * 4 {
        return None;
    }
    Some(RgbaFrame {
        width: w,
        height: h,
        rgba: out.stdout[..(w as usize) * (h as usize) * 4].to_vec(),
    })
}

/// Probe a materialised `.webm` file: parse the header and, when `ffmpeg`
/// exists, decode the poster frame.
pub fn probe_file(path: &Path) -> Option<(VideoInfo, Option<PosterFrame>)> {
    let bytes = std::fs::read(path).ok()?;
    let info = parse_webm(&bytes).ok()?;
    let poster = decode_first_frame(&bytes, &info);
    Some((info, poster))
}

/// Where the retail cache keeps videos: `<cache>/videos/<rel>` plus
/// `<cache>/videos/manifest.tsv` (`rel\tsize\tcodec\tWxH\tduration_ms`).
pub fn manifest_path(cache: &Path) -> PathBuf {
    cache.join("videos/manifest.tsv")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_frame_reports_whether_the_clip_has_transparency() {
        // Opaque: every alpha byte 255.
        let opaque = RgbaFrame {
            width: 1,
            height: 1,
            rgba: vec![10, 20, 30, 255],
        };
        assert!(!opaque.has_alpha());
        // Cut-out: some samples are fully transparent.
        let cut = RgbaFrame {
            width: 2,
            height: 1,
            rgba: vec![10, 20, 30, 255, 0, 0, 0, 0],
        };
        assert!(cut.has_alpha());
    }

    fn ebml_id(id: u64) -> Vec<u8> {
        // Retail IDs already carry their EBML marker bits; emit the minimal
        // big-endian form (1 byte for 0x80..=0xFF, 2 bytes up to 0xFFFF, ...).
        if id <= 0xFF {
            vec![id as u8]
        } else if id <= 0xFFFF {
            vec![((id >> 8) & 0xff) as u8, (id & 0xff) as u8]
        } else if id <= 0xFF_FFFF {
            vec![
                ((id >> 16) & 0xff) as u8,
                ((id >> 8) & 0xff) as u8,
                (id & 0xff) as u8,
            ]
        } else {
            vec![
                ((id >> 24) & 0xff) as u8,
                ((id >> 16) & 0xff) as u8,
                ((id >> 8) & 0xff) as u8,
                (id & 0xff) as u8,
            ]
        }
    }

    fn ebml_size(len: usize) -> Vec<u8> {
        assert!(len < 127);
        vec![(len as u8) | 0x80]
    }

    fn element(id: u64, body: &[u8]) -> Vec<u8> {
        let mut out = ebml_id(id);
        out.extend(ebml_size(body.len()));
        out.extend_from_slice(body);
        out
    }

    fn uint_body(v: u64) -> Vec<u8> {
        if v == 0 {
            return vec![0];
        }
        let mut bytes = Vec::new();
        let mut x = v;
        while x > 0 {
            bytes.push((x & 0xff) as u8);
            x >>= 8;
        }
        bytes.reverse();
        bytes
    }

    /// Build the smallest WebM the parser accepts: EBML + Segment(Info,
    /// Tracks with one VP9 640x360 entry, one Cluster).
    pub(crate) fn minimal_webm() -> Vec<u8> {
        let info = element(
            ID_INFO,
            &[
                element(ID_TIMECODE_SCALE, &uint_body(1_000_000)).as_slice(),
                element(ID_DURATION, &3_600_000.0f32.to_be_bytes()).as_slice(),
            ]
            .concat(),
        );
        let video = element(
            ID_VIDEO,
            &[
                element(ID_PIXEL_WIDTH, &uint_body(640)).as_slice(),
                element(ID_PIXEL_HEIGHT, &uint_body(360)).as_slice(),
            ]
            .concat(),
        );
        let entry = element(
            ID_TRACK_ENTRY,
            &[
                element(ID_TRACK_NUMBER, &uint_body(1)).as_slice(),
                element(ID_TRACK_TYPE, &uint_body(1)).as_slice(),
                element(ID_CODEC_ID, b"V_VP9").as_slice(),
                video.as_slice(),
            ]
            .concat(),
        );
        let tracks = element(ID_TRACKS, &entry);
        let cluster = element(ID_CLUSTER, b"\xe7\x81\x00");
        let segment_body = [info, tracks, cluster].concat();
        let mut out = element(ID_EBML, &element(0x4286, b"webm"));
        out.extend(ebml_id(ID_SEGMENT));
        // Unknown segment size is legal WebM; use the 1-byte unknown form.
        out.push(0xff);
        out.extend(segment_body);
        out
    }

    #[test]
    fn minimal_webm_parses_to_vp9_640x360() {
        let bytes = minimal_webm();
        let info = parse_webm(&bytes).expect("synthetic webm parses");
        assert_eq!(info.codec, "V_VP9");
        assert_eq!(info.pixel_width, 640);
        assert_eq!(info.pixel_height, 360);
        assert_eq!(info.video_tracks, 1);
        assert_eq!(info.clusters, 1);
        assert!(!info.has_audio);
    }

    #[test]
    fn non_webm_is_rejected() {
        assert!(parse_webm(b"RIFF....WAVE").is_err());
        assert!(parse_webm(b"").is_err());
    }

    #[test]
    fn briefing_video_paths_normalise() {
        assert_eq!(
            normalise_video_path("interstitials\\TR1010.webm"),
            "interstitials/tr1010.webm"
        );
        assert!(is_known_cinematic("smk/starintr.webm"));
        assert!(is_known_cinematic("interstitials/tr1010.webm"));
        assert!(is_known_cinematic("glue/campaign/terr.webm"));
        assert!(!is_known_cinematic("glue/title/title.dds"));
        assert_eq!(intro_for_episode(false), INTRO_ORIGINAL);
        assert_eq!(intro_for_episode(true), INTRO_EXPANSION);
    }

    #[test]
    fn the_intro_sequence_starts_with_the_retail_blizzard_logo() {
        assert_eq!(
            intro_sequence(false, true),
            vec![BLIZZARD_LOGO, INTRO_ORIGINAL]
        );
        assert_eq!(
            intro_sequence(true, true),
            vec![BLIZZARD_LOGO, INTRO_EXPANSION]
        );
        // `View Intro` replays the title movie without the publisher logo.
        assert_eq!(intro_sequence(false, false), vec![INTRO_ORIGINAL]);
    }

    #[test]
    fn every_campaign_act_has_its_shipped_cinematic() {
        // Terran: four acts of three missions.
        for (order, pre, post) in [
            (1, "smk/preterran_1.webm", "smk/terran1.webm"),
            (4, "smk/preterran_2.webm", "smk/terran2.webm"),
            (7, "smk/preterran_3.webm", "smk/terran3.webm"),
            (10, "smk/preterran_4.webm", "smk/terran4.webm"),
        ] {
            assert_eq!(campaign_cinematic("terran", order, true), Some(pre));
            assert_eq!(campaign_cinematic("terran", order + 2, false), Some(post));
        }
        // Zerg: no act-1 pre movie, matches the shipped table (PreZerg_2..4).
        assert_eq!(campaign_cinematic("zerg", 1, true), None);
        assert_eq!(
            campaign_cinematic("zerg", 4, true),
            Some("smk/prezerg_2.webm")
        );
        assert_eq!(campaign_cinematic("zerg", 3, false), Some("smk/zerg1.webm"));
        // Protoss: PreProtoss_1..3, no act-4 pre.
        assert_eq!(
            campaign_cinematic("protoss", 1, true),
            Some("smk/preprotoss_1.webm")
        );
        assert_eq!(campaign_cinematic("protoss", 9, true), None);
        assert_eq!(
            campaign_cinematic("protoss", 10, false),
            Some("smk/protoss4.webm")
        );
    }

    #[test]
    fn the_campaign_table_covers_all_28_shipped_movies() {
        // 25 campaign movies + the 3 expansion episode intros.
        assert_eq!(CAMPAIGN_CINEMATICS.len(), 25);
        let mut all: Vec<&str> = CAMPAIGN_CINEMATICS.to_vec();
        all.extend([BLIZZARD_LOGO, INTRO_ORIGINAL, INTRO_EXPANSION]);
        assert_eq!(all.len(), 28);
        for rel in all {
            assert!(is_known_cinematic(rel), "{rel} is a retail cinematic");
        }
    }

    #[test]
    fn expansion_episode_intros_are_the_x_movies() {
        assert_eq!(
            expansion_episode_intro("expprotoss"),
            Some("smk/xprotoss.webm")
        );
        assert_eq!(
            expansion_episode_intro("expterran"),
            Some("smk/xterran.webm")
        );
        assert_eq!(expansion_episode_intro("expzerg"), Some("smk/xzerg.webm"));
        assert_eq!(expansion_episode_intro("terran"), None);
    }

    #[test]
    fn every_intro_table_movie_is_readable_and_decodable() {
        let Some(root) = std::env::var("SC_INSTALL").ok() else {
            eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
            return;
        };
        let install = crate::retail::RetailInstall::open(root).expect("open install");
        let mut all: Vec<&str> = CAMPAIGN_CINEMATICS.to_vec();
        all.extend([BLIZZARD_LOGO, INTRO_ORIGINAL, INTRO_EXPANSION]);
        assert_eq!(all.len(), 28, "the executable intro table has 28 movies");
        for rel in all {
            let bytes = install.read(rel).unwrap_or_else(|| panic!("{rel} present"));
            let info = parse_webm(&bytes).unwrap_or_else(|e| panic!("{rel}: {e}"));
            assert!(
                info.duration_ms.unwrap_or(0) > 500,
                "{rel} has a real duration"
            );
            assert!(info.video_tracks >= 1 && info.pixel_width >= 160, "{rel}");
        }
    }

    #[test]
    fn retail_intro_headers_decode_when_the_install_is_present() {
        let path = std::env::var("SC_INSTALL").ok();
        let Some(root) = path else {
            eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
            return;
        };
        let install = crate::retail::RetailInstall::open(root).expect("open install");
        for rel in [
            BLIZZARD_LOGO,
            INTRO_ORIGINAL,
            INTRO_EXPANSION,
            "interstitials/tr1010.webm",
        ] {
            let bytes = install.read(rel).unwrap_or_else(|| panic!("{rel} present"));
            assert!(bytes.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]), "{rel}");
            let info = parse_webm(&bytes).unwrap_or_else(|e| panic!("{rel}: {e}"));
            assert!(
                info.pixel_width >= 320,
                "{rel} {}x{}",
                info.pixel_width,
                info.pixel_height
            );
            assert!(info.pixel_height >= 180, "{rel}");
            assert!(!info.codec.is_empty() && info.codec != "unknown", "{rel}");
            assert!(info.clusters > 0, "{rel} has clusters");
        }
    }
}

// ---------------------------------------------------------------------------
// Materialised video store (cache/videos)
// ---------------------------------------------------------------------------

/// Posters decoded from the materialised retail cache.
///
/// The store reads `<cache>/videos/manifest.tsv` and decodes the first frame
/// of each requested video with the system `ffmpeg` on demand.  Results are
/// cached in memory so the intro and briefing screens each decode once.
#[derive(Clone, Debug, Default)]
pub struct VideoStore {
    pub root: PathBuf,
    pub entries: Vec<VideoManifestEntry>,
}

/// One row of `videos/manifest.tsv`.
#[derive(Clone, Debug)]
pub struct VideoManifestEntry {
    pub rel: String,
    pub bytes: usize,
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub duration_ms: Option<u64>,
    pub clusters: usize,
    pub has_audio: bool,
}

impl VideoStore {
    pub fn load(cache: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(cache.join("videos/manifest.tsv")).ok()?;
        let root = cache.join("videos");
        if !root.is_dir() {
            return None;
        }
        let mut entries = Vec::new();
        for (i, line) in text.lines().enumerate() {
            if i == 0 || line.trim().is_empty() {
                continue;
            }
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < 7 {
                continue;
            }
            entries.push(VideoManifestEntry {
                rel: cols[0].to_string(),
                bytes: cols[1].parse().unwrap_or(0),
                codec: cols[2].to_string(),
                width: cols[3].parse().unwrap_or(0),
                height: cols[4].parse().unwrap_or(0),
                duration_ms: cols[5].parse().ok(),
                clusters: cols[6].parse().unwrap_or(0),
                has_audio: cols.get(7).is_some_and(|v| *v == "true"),
            });
        }
        if entries.is_empty() {
            return None;
        }
        Some(Self { root, entries })
    }

    pub fn contains(&self, rel: &str) -> bool {
        let want = normalise_video_path(rel);
        self.entries.iter().any(|e| e.rel == want || e.rel == rel)
    }

    pub fn info(&self, rel: &str) -> Option<&VideoManifestEntry> {
        let want = normalise_video_path(rel);
        self.entries.iter().find(|e| e.rel == want || e.rel == rel)
    }

    /// Decode the poster frame for `rel` (authored or cache-relative path).
    /// Materialised path for an authored video reference.
    ///
    /// Briefing scripts name clips with the retail capitalisation (`TR1010`)
    /// while the cache stores them lowercased (`interstitials/tr1010.webm`),
    /// and the exact extension can differ, so the lookup is case-insensitive
    /// and also matches on the file stem.
    pub fn resolve(&self, rel: &str) -> Option<PathBuf> {
        let want = normalise_video_path(rel);
        if let Some(entry) = self.entries.iter().find(|e| e.rel == want || e.rel == rel) {
            let p = self.root.join(&entry.rel);
            if p.exists() {
                return Some(p);
            }
        }
        let p = self.root.join(&want);
        if p.exists() {
            return Some(p);
        }
        let stem = Path::new(&want)
            .file_stem()
            .and_then(|s| s.to_str())?
            .to_ascii_lowercase();
        self.entries
            .iter()
            .find(|e| {
                Path::new(&e.rel)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.eq_ignore_ascii_case(&stem))
            })
            .map(|e| self.root.join(&e.rel))
            .filter(|p| p.exists())
    }

    pub fn poster(&self, rel: &str) -> Option<(VideoInfo, PosterFrame)> {
        let want = normalise_video_path(rel);
        // Prefer the manifest, but fall back to the file on disk: the glue
        // menu layers are materialised without a manifest row, and presence
        // on disk is the real source of truth.
        let path = match self.entries.iter().find(|e| e.rel == want || e.rel == rel) {
            Some(entry) => self.root.join(&entry.rel),
            None => self.root.join(&want),
        };
        let bytes = std::fs::read(path).ok()?;
        let info = parse_webm(&bytes).ok()?;
        let frame = decode_first_frame(&bytes, &info)?;
        Some((info, frame))
    }

    /// Poster for the title intro (`smk/starintr.webm` or the expansion).
    pub fn intro_poster(&self, expansion: bool) -> Option<(VideoInfo, PosterFrame)> {
        self.poster(intro_for_episode(expansion))
    }
}

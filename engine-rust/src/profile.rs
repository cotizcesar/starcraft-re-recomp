//! Player profiles: the Registry IDs, the global settings, per-profile
//! statistics and campaign progress, persisted as one `GCPF` file.
//!
//! Retail evidence (2026-11-03, 32-bit menu dump SHA-256 `7c0bd733…fcc5`,
//! headless Ghidra on `research/build/ghidra-proj32`; details in
//! `research/docs/profile-format.md`):
//!
//! * A Registry ID is a file `Characters\<name>.spc` (`.mpc` when the
//!   multiplayer flag `0x0119f7bc` is set): path built at `0x00683080`,
//!   listed by the enumeration callback `0x00682580`, which skips
//!   directories.  The file name is the authority: the loader `0x00682a80`
//!   rewrites the stored display name when it differs (`_stricmp`).
//! * The record is `0xd0` bytes, written as two blocks (`0x70` + `0x60`).
//!   The default record (`0x00683000`) is `magic 0x00112233`, `u8`
//!   multiplayer flag at `+4`, `time(NULL)` at `+8`, the name at `+0x0c`
//!   (copy budget `0x19`, so 24 characters) and again at `+0x70`.
//! * Campaign progress is six `u32` at `+0x28`: original Zerg, Terran,
//!   Protoss then Brood War Zerg, Terran, Protoss (`0x007a3940` keeps
//!   `max(progress[race], mission)`, Brood War at `+12`; `0x006eaac0`
//!   checks the lock rules on the first three).  0 = never started.
//! * A block is `u32 check, u32 chunks, chunks x {u32 size, data}` with
//!   8 KiB chunks, stored raw when `size` equals the chunk length and zlib
//!   otherwise (`0x00688290`).  `check` is `!crc32(raw)`: it matches all six
//!   blocks of the reference `.spc`/`.mpc` files.
//! * Settings are global (`CSettings.json`), not per ID.
//!
//! Retail keeps no statistics in the record (every inspected byte past the
//! progress array is zero); the statistics here are this client's own
//! (games, wins, losses, per race, score totals), not a retail format.

use crate::campaign_progress::{Campaign, Progress, ProgressBook};
use crate::menu::GameOptions;
use crate::sim::{Player, Race};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::Path;

/// Longest Registry ID: the retail record copies the name with a `0x19`
/// byte budget (24 characters + NUL).
pub const MAX_NAME: usize = 24;
pub const MAX_PROFILES: usize = 256;

/// Why a Registry ID was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameError {
    Empty,
    TooLong,
    /// A path separator, a Windows-reserved file character or a control
    /// character: retail uses the ID as a file name.
    BadCharacter,
    /// Another ID differs only in case (retail compares with `_stricmp`).
    Duplicate,
    Full,
}

/// Check a candidate Registry ID (without the uniqueness rule).
pub fn validate_name(name: &str) -> Result<(), NameError> {
    if name.trim().is_empty() {
        return Err(NameError::Empty);
    }
    if name.len() > MAX_NAME {
        return Err(NameError::TooLong);
    }
    if name != name.trim()
        || name.chars().any(|c| {
            c.is_control() || matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
    {
        return Err(NameError::BadCharacter);
    }
    Ok(())
}

/// Lifetime statistics of one profile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileStats {
    pub games: u64,
    pub wins: u64,
    pub losses: u64,
    /// Indexed by [`Race`] (Zerg, Terran, Protoss).
    pub games_by_race: [u64; 3],
    pub wins_by_race: [u64; 3],
    pub kills: u64,
    pub deaths: u64,
    pub razings: u64,
    pub minerals_gathered: u64,
    pub gas_gathered: u64,
    /// Highest end-of-game total score.
    pub best_score: u64,
    /// Simulation ticks played (`World::tick` at the end of each game).
    pub ticks_played: u64,
}

impl ProfileStats {
    const FIELDS: usize = 16;

    fn to_array(self) -> [u64; Self::FIELDS] {
        let g = self.games_by_race;
        let w = self.wins_by_race;
        [
            self.games,
            self.wins,
            self.losses,
            g[0],
            g[1],
            g[2],
            w[0],
            w[1],
            w[2],
            self.kills,
            self.deaths,
            self.razings,
            self.minerals_gathered,
            self.gas_gathered,
            self.best_score,
            self.ticks_played,
        ]
    }

    /// Missing trailing fields stay 0 (older files), extra ones are ignored.
    fn from_slice(v: &[u64]) -> Self {
        let at = |i: usize| v.get(i).copied().unwrap_or(0);
        Self {
            games: at(0),
            wins: at(1),
            losses: at(2),
            games_by_race: [at(3), at(4), at(5)],
            wins_by_race: [at(6), at(7), at(8)],
            kills: at(9),
            deaths: at(10),
            razings: at(11),
            minerals_gathered: at(12),
            gas_gathered: at(13),
            best_score: at(14),
            ticks_played: at(15),
        }
    }

    /// Add one finished game.
    pub fn record(&mut self, game: &GameRecord) {
        self.games += 1;
        if game.won {
            self.wins += 1;
        } else {
            self.losses += 1;
        }
        if let Some(r) = race_slot(game.race) {
            self.games_by_race[r] += 1;
            if game.won {
                self.wins_by_race[r] += 1;
            }
        }
        self.kills += game.kills;
        self.deaths += game.deaths;
        self.razings += game.razings;
        self.minerals_gathered += game.minerals_gathered;
        self.gas_gathered += game.gas_gathered;
        self.best_score = self.best_score.max(game.score);
        self.ticks_played += game.ticks;
    }
}

fn race_slot(race: Race) -> Option<usize> {
    match race {
        Race::Zerg => Some(0),
        Race::Terran => Some(1),
        Race::Protoss => Some(2),
        Race::Neutral => None,
    }
}

/// The local player's result of one finished game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameRecord {
    pub won: bool,
    pub race: Race,
    pub kills: u64,
    pub deaths: u64,
    pub razings: u64,
    pub minerals_gathered: u64,
    pub gas_gathered: u64,
    pub score: u64,
    pub ticks: u64,
}

impl GameRecord {
    /// Read the end-of-game counters of `player` (score = CHK score type 0,
    /// the trigger "total" category).
    pub fn from_player(player: &Player, won: bool, ticks: u64) -> Self {
        let pos = |v: i32| v.max(0) as u64;
        Self {
            won,
            race: player.race,
            kills: player.kills as u64,
            deaths: player.deaths as u64,
            razings: player.razings as u64,
            minerals_gathered: pos(player.minerals_gathered),
            gas_gathered: pos(player.gas_gathered),
            score: pos(player.scores[0]),
            ticks,
        }
    }
}

/// One Registry ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub name: String,
    /// Creation time, Unix seconds (retail record `+8`).
    pub created: u64,
    pub progress: Progress,
    pub stats: ProfileStats,
}

impl Profile {
    pub fn new(name: &str, created: u64) -> Self {
        Self {
            name: name.to_string(),
            created,
            progress: Progress::default(),
            stats: ProfileStats::default(),
        }
    }
}

/// Every Registry ID (in Registry order), the selected one and the global
/// settings, persisted as `GCPF`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProfileStore {
    pub profiles: Vec<Profile>,
    /// Index of the ID chosen in the Registry (`Ok`).
    pub active: Option<usize>,
    pub settings: GameOptions,
}

const MAGIC: &[u8; 4] = b"GCPF";
const VERSION: u16 = 1;
const MAX_BYTES: usize = 256 * 1024;
const MAX_SETTINGS: usize = 256;
const MAX_KEY: usize = 64;

impl ProfileStore {
    /// Index of `name`, compared case-insensitively like retail.
    pub fn find(&self, name: &str) -> Option<usize> {
        self.profiles
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(name))
    }

    /// Add a Registry ID; returns its index.
    pub fn create(&mut self, name: &str, created: u64) -> Result<usize, NameError> {
        validate_name(name)?;
        if self.find(name).is_some() {
            return Err(NameError::Duplicate);
        }
        if self.profiles.len() >= MAX_PROFILES {
            return Err(NameError::Full);
        }
        self.profiles.push(Profile::new(name, created));
        Ok(self.profiles.len() - 1)
    }

    /// First free `<prefix><n>` (n from 1), the name the `New ID` button
    /// proposes.
    pub fn next_free_name(&self, prefix: &str) -> String {
        (1..)
            .map(|n| format!("{prefix}{n}"))
            .find(|name| self.find(name).is_none())
            .unwrap_or_default()
    }

    /// Remove a Registry ID.  The active selection follows the profile it
    /// named, and is cleared when that profile is the one removed.
    pub fn delete(&mut self, index: usize) -> Option<Profile> {
        if index >= self.profiles.len() {
            return None;
        }
        let removed = self.profiles.remove(index);
        self.active = match self.active {
            Some(a) if a == index => None,
            Some(a) if a > index => Some(a - 1),
            other => other,
        };
        Some(removed)
    }

    pub fn active_profile(&self) -> Option<&Profile> {
        self.profiles.get(self.active?)
    }

    pub fn get(&self, name: &str) -> Option<&Profile> {
        self.profiles.get(self.find(name)?)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Profile> {
        let i = self.find(name)?;
        self.profiles.get_mut(i)
    }

    pub fn names(&self) -> Vec<String> {
        self.profiles.iter().map(|p| p.name.clone()).collect()
    }

    /// Campaign progress of `name` (default for an unknown ID).
    pub fn progress(&self, name: &str) -> Progress {
        self.get(name).map(|p| p.progress).unwrap_or_default()
    }

    pub fn set_progress(&mut self, name: &str, progress: Progress) {
        if let Some(p) = self.get_mut(name) {
            p.progress = progress;
        }
    }

    /// Add a finished game to `name`'s statistics; `false` if the ID does
    /// not exist.
    pub fn record_game(&mut self, name: &str, game: &GameRecord) -> bool {
        match self.get_mut(name) {
            Some(p) => {
                p.stats.record(game);
                true
            }
            None => false,
        }
    }

    /// Migrate the previous `GCPR` file: its progress is the authority, so
    /// IDs already present take it and missing (valid) ones are created.
    pub fn import_progress_book(&mut self, book: &ProgressBook, created: u64) {
        for (name, progress) in &book.profiles {
            let index = self.find(name).or_else(|| self.create(name, created).ok());
            if let Some(i) = index {
                self.profiles[i].progress = *progress;
            }
        }
    }

    /// Bring in a retail `.spc` record: a new ID is created with its
    /// progress; an existing one keeps the further of the two per campaign
    /// (an import never takes progress away).  `false` when nothing changed
    /// or the name is not a valid ID.
    pub fn import_retail(&mut self, character: &RetailCharacter) -> bool {
        let retail = character.to_progress();
        if let Some(p) = self.get_mut(&character.name) {
            let before = p.progress;
            for (mine, theirs) in p.progress.next.iter_mut().zip(retail.next) {
                *mine = (*mine).max(theirs);
            }
            return p.progress != before;
        }
        match self.create(&character.name, character.created as u64) {
            Ok(i) => {
                self.profiles[i].progress = retail;
                true
            }
            Err(_) => false,
        }
    }

    /// Import every readable `*.spc` of a retail `Characters` directory, in
    /// file-name order.  Returns the number of IDs added or advanced.
    pub fn import_retail_dir(&mut self, dir: &Path) -> usize {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return 0;
        };
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("spc")))
            .collect();
        paths.sort();
        paths
            .iter()
            .filter_map(|p| RetailCharacter::load(p))
            .filter(|c| self.import_retail(c))
            .count()
    }

    /// Magic, version, profile count, active index (`0xffff` = none),
    /// profiles `{u8 len, name, u64 created, 6 x u16 progress, u8 n, n x u64
    /// stats}`, settings `{u16 n, n x {u8 len, key, f32}}`, SHA-256 trailer.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        let profiles: Vec<&Profile> = self
            .profiles
            .iter()
            .filter(|p| validate_name(&p.name).is_ok())
            .take(MAX_PROFILES)
            .collect();
        out.extend_from_slice(&(profiles.len() as u16).to_le_bytes());
        let active = self
            .active_profile()
            .and_then(|a| profiles.iter().position(|p| p.name == a.name))
            .map_or(0xffff, |i| i as u16);
        out.extend_from_slice(&active.to_le_bytes());
        for p in profiles {
            out.push(p.name.len() as u8);
            out.extend_from_slice(p.name.as_bytes());
            out.extend_from_slice(&p.created.to_le_bytes());
            for n in p.progress.next {
                out.extend_from_slice(&n.to_le_bytes());
            }
            let stats = p.stats.to_array();
            out.push(stats.len() as u8);
            for v in stats {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        let pairs = self.settings.to_pairs();
        out.extend_from_slice(&(pairs.len() as u16).to_le_bytes());
        for (key, value) in pairs {
            out.push(key.len() as u8);
            out.extend_from_slice(key.as_bytes());
            out.extend_from_slice(&value.to_le_bytes());
        }
        let digest = Sha256::digest(&out);
        out.extend_from_slice(&digest);
        out
    }

    /// Parse a store; anything malformed returns `None`.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > MAX_BYTES || bytes.len() < 10 + 32 {
            return None;
        }
        let (payload, digest) = bytes.split_at(bytes.len() - 32);
        if Sha256::digest(payload).as_slice() != digest || payload.get(0..4)? != MAGIC {
            return None;
        }
        let mut r = Reader { b: payload, at: 4 };
        if r.u16()? != VERSION {
            return None;
        }
        let count = r.u16()? as usize;
        if count > MAX_PROFILES {
            return None;
        }
        let active = r.u16()?;
        let mut store = ProfileStore::default();
        for _ in 0..count {
            let len = r.u8()? as usize;
            let name = std::str::from_utf8(r.take(len)?).ok()?;
            validate_name(name).ok()?;
            if store.find(name).is_some() {
                return None;
            }
            let created = r.u64()?;
            let mut next = [0u16; 6];
            for n in next.iter_mut() {
                *n = r.u16()?;
            }
            let n = r.u8()? as usize;
            let mut stats = Vec::with_capacity(n);
            for _ in 0..n {
                stats.push(r.u64()?);
            }
            store.profiles.push(Profile {
                name: name.to_string(),
                created,
                progress: Progress { next },
                stats: ProfileStats::from_slice(&stats),
            });
        }
        store.active = match active {
            0xffff => None,
            a if (a as usize) < store.profiles.len() => Some(a as usize),
            _ => return None,
        };
        let settings = r.u16()? as usize;
        if settings > MAX_SETTINGS {
            return None;
        }
        for _ in 0..settings {
            let len = r.u8()? as usize;
            if len > MAX_KEY {
                return None;
            }
            let key = std::str::from_utf8(r.take(len)?).ok()?;
            let value = f32::from_le_bytes(r.take(4)?.try_into().ok()?);
            // Unknown keys (a newer client's options) are skipped.
            store.settings.set_pair(key, value);
        }
        (r.at == payload.len()).then_some(store)
    }

    pub fn load(path: &Path) -> Option<Self> {
        let meta = std::fs::metadata(path).ok()?;
        if meta.len() as usize > MAX_BYTES {
            return None;
        }
        Self::from_bytes(&std::fs::read(path).ok()?)
    }

    /// Atomic write (temporary file + rename).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, self.to_bytes())?;
        std::fs::rename(&tmp, path)
    }
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.b.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
}

// ---------------------------------------------------------------------------
// Retail `Characters\<name>.spc` / `.mpc` records (read-only interop).
// ---------------------------------------------------------------------------

/// Retail record magic (`0x00683000`, compared at `0x00682b98`).
pub const RETAIL_MAGIC: u32 = 0x0011_2233;
const RETAIL_HEAD: usize = 0x70;
const RETAIL_NAME2: usize = 0x60;
const RETAIL_CHUNK: usize = 0x2000;
/// Retail progress slots in record order (`+0x28`).
const RETAIL_SLOTS: [Campaign; 6] = [
    Campaign::Zerg,
    Campaign::Terran,
    Campaign::Protoss,
    Campaign::ExpZerg,
    Campaign::ExpTerran,
    Campaign::ExpProtoss,
];

/// The fields of a retail `.spc`/`.mpc` record this client uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetailCharacter {
    pub multiplayer: bool,
    /// `time(NULL)` when the ID was created.
    pub created: u32,
    pub name: String,
    /// Highest mission reached per slot, retail order (see
    /// [`RETAIL_SLOTS`]); 0 = never started.
    pub progress: [u32; 6],
}

/// `!crc32`, the block check of the retail record (`check` field).
pub fn retail_check(raw: &[u8]) -> u32 {
    let mut crc = flate2::Crc::new();
    crc.update(raw);
    !crc.sum()
}

fn read_retail_block(r: &mut Reader, len: usize) -> Option<Vec<u8>> {
    let check = r.u32()?;
    let chunks = r.u32()? as usize;
    if chunks != len.div_ceil(RETAIL_CHUNK) {
        return None;
    }
    let mut raw = Vec::with_capacity(len);
    for k in 0..chunks {
        let want = (len - k * RETAIL_CHUNK).min(RETAIL_CHUNK);
        let size = r.u32()? as usize;
        let data = r.take(size)?;
        if size == want {
            raw.extend_from_slice(data);
        } else {
            let mut out = Vec::with_capacity(want);
            flate2::read::ZlibDecoder::new(data)
                .take(want as u64 + 1)
                .read_to_end(&mut out)
                .ok()?;
            if out.len() != want {
                return None;
            }
            raw.extend_from_slice(&out);
        }
    }
    (retail_check(&raw) == check).then_some(raw)
}

fn write_retail_block(out: &mut Vec<u8>, raw: &[u8]) {
    out.extend_from_slice(&retail_check(raw).to_le_bytes());
    let chunks: Vec<&[u8]> = raw.chunks(RETAIL_CHUNK).collect();
    out.extend_from_slice(&(chunks.len() as u32).to_le_bytes());
    for chunk in chunks {
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        let packed = z
            .write_all(chunk)
            .and_then(|_| z.finish())
            .ok()
            .filter(|p| p.len() < chunk.len());
        // Retail reads a chunk whose size equals its length as raw bytes.
        let data = packed.as_deref().unwrap_or(chunk);
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
    }
}

fn c_string(b: &[u8]) -> Option<String> {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8(b[..end].to_vec()).ok()
}

impl RetailCharacter {
    /// Parse a record; the `+0x70` display name wins over the short one,
    /// as the retail loader keeps it in sync with the file name.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > 4096 {
            return None;
        }
        let mut r = Reader { b: bytes, at: 0 };
        let head = read_retail_block(&mut r, RETAIL_HEAD)?;
        let name2 = read_retail_block(&mut r, RETAIL_NAME2)?;
        if r.at != bytes.len() {
            return None;
        }
        let u32_at = |i: usize| u32::from_le_bytes(head[i..i + 4].try_into().unwrap_or([0; 4]));
        if u32_at(0) != RETAIL_MAGIC {
            return None;
        }
        let name = c_string(&name2)
            .filter(|n| !n.is_empty())
            .or_else(|| c_string(&head[0x0c..0x28]))?;
        let mut progress = [0u32; 6];
        for (i, p) in progress.iter_mut().enumerate() {
            *p = u32_at(0x28 + 4 * i);
        }
        Some(Self {
            multiplayer: head[4] != 0,
            created: u32_at(8),
            name,
            progress,
        })
    }

    pub fn load(path: &Path) -> Option<Self> {
        let meta = std::fs::metadata(path).ok()?;
        if meta.len() > 4096 {
            return None;
        }
        Self::parse(&std::fs::read(path).ok()?)
    }

    /// Serialise in the retail layout (both blocks, zlib chunks).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut head = [0u8; RETAIL_HEAD];
        head[0..4].copy_from_slice(&RETAIL_MAGIC.to_le_bytes());
        head[4] = self.multiplayer as u8;
        head[8..12].copy_from_slice(&self.created.to_le_bytes());
        let name = self.name.as_bytes();
        let short = name.len().min(MAX_NAME);
        head[0x0c..0x0c + short].copy_from_slice(&name[..short]);
        for (i, p) in self.progress.iter().enumerate() {
            head[0x28 + 4 * i..0x2c + 4 * i].copy_from_slice(&p.to_le_bytes());
        }
        let mut name2 = [0u8; RETAIL_NAME2];
        let long = name.len().min(RETAIL_NAME2 - 1);
        name2[..long].copy_from_slice(&name[..long]);
        let mut out = Vec::new();
        write_retail_block(&mut out, &head);
        write_retail_block(&mut out, &name2);
        out
    }

    /// This client's progress: the retail "highest mission reached" is the
    /// next playable mission; 0 keeps the campaign's first mission.
    pub fn to_progress(&self) -> Progress {
        let mut p = Progress::default();
        for (slot, campaign) in RETAIL_SLOTS.iter().enumerate() {
            let (first, last) = campaign.mission_range();
            let v = self.progress[slot];
            if v != 0 {
                p.next[campaign.index()] = (v.min(last as u32 + 1) as u16).max(first);
            }
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ProfileStore {
        let mut s = ProfileStore::default();
        s.create("REA", 1_793_115_617).unwrap();
        let l = s.create("LinKa", 1_793_171_565).unwrap();
        s.profiles[l].progress.next[Campaign::Terran.index()] = 10;
        s.active = Some(l);
        s.settings.music_volume = 0.75;
        s.settings.portraits = 2;
        s.settings.fog = false;
        s
    }

    #[test]
    fn store_round_trips_ids_settings_stats_and_progress() {
        let mut s = sample();
        let game = GameRecord {
            won: true,
            race: Race::Protoss,
            kills: 12,
            deaths: 3,
            razings: 2,
            minerals_gathered: 5000,
            gas_gathered: 1200,
            score: 18_500,
            ticks: 14_400,
        };
        assert!(
            s.record_game("linka", &game),
            "IDs match case-insensitively"
        );
        assert!(s.record_game(
            "LinKa",
            &GameRecord {
                won: false,
                score: 900,
                ..game
            }
        ));
        assert!(!s.record_game("nobody", &game));
        let back = ProfileStore::from_bytes(&s.to_bytes()).unwrap();
        assert_eq!(back, s);
        let st = back.get("LinKa").unwrap().stats;
        assert_eq!((st.games, st.wins, st.losses), (2, 1, 1));
        assert_eq!(st.games_by_race, [0, 0, 2]);
        assert_eq!(st.wins_by_race, [0, 0, 1]);
        assert_eq!(
            (st.kills, st.best_score, st.ticks_played),
            (24, 18_500, 28_800)
        );
        assert_eq!(
            back.active_profile().map(|p| p.name.as_str()),
            Some("LinKa")
        );
        assert_eq!(back.progress("LinKa").next[Campaign::Terran.index()], 10);
        assert_eq!(back.settings.music_volume, 0.75);
        assert!(!back.settings.fog);
    }

    #[test]
    fn store_rejects_corruption_and_oversize() {
        let bytes = sample().to_bytes();
        for i in [0, 4, 7, 12, bytes.len() / 2, bytes.len() - 1] {
            let mut bad = bytes.clone();
            bad[i] ^= 0x40;
            assert_eq!(ProfileStore::from_bytes(&bad), None, "flip at {i}");
        }
        assert_eq!(ProfileStore::from_bytes(&bytes[..bytes.len() - 1]), None);
        assert_eq!(ProfileStore::from_bytes(&vec![0u8; MAX_BYTES + 1]), None);
        assert_eq!(ProfileStore::from_bytes(b""), None);
        // A valid checksum over an out-of-range active index is refused.
        let mut payload = bytes[..bytes.len() - 32].to_vec();
        payload[8..10].copy_from_slice(&7u16.to_le_bytes());
        let digest = Sha256::digest(&payload);
        payload.extend_from_slice(&digest);
        assert_eq!(ProfileStore::from_bytes(&payload), None);
    }

    #[test]
    fn registry_ids_follow_the_retail_name_rules() {
        let mut s = ProfileStore::default();
        assert_eq!(s.create("", 0), Err(NameError::Empty));
        assert_eq!(s.create("   ", 0), Err(NameError::Empty));
        assert_eq!(
            s.create(&"x".repeat(MAX_NAME + 1), 0),
            Err(NameError::TooLong)
        );
        assert!(s.create(&"x".repeat(MAX_NAME), 0).is_ok());
        for bad in ["a/b", "a\\b", "c:d", "q?", "tab\t", " lead", "trail "] {
            assert_eq!(s.create(bad, 0), Err(NameError::BadCharacter), "{bad:?}");
        }
        assert!(s.create("REA", 0).is_ok());
        assert_eq!(s.create("rea", 0), Err(NameError::Duplicate));
        assert_eq!(s.next_free_name("Player"), "Player1");
        s.create("Player1", 0).unwrap();
        assert_eq!(s.next_free_name("Player"), "Player2");
    }

    #[test]
    fn deleting_an_id_keeps_the_active_selection_on_its_profile() {
        let mut s = sample(); // REA, LinKa (active)
        s.create("Third", 0).unwrap();
        assert_eq!(s.delete(0).map(|p| p.name), Some("REA".into()));
        assert_eq!(s.active_profile().map(|p| p.name.as_str()), Some("LinKa"));
        s.delete(0);
        assert_eq!(s.active, None, "deleting the active ID clears it");
        assert!(s.delete(5).is_none());
        let back = ProfileStore::from_bytes(&s.to_bytes()).unwrap();
        assert_eq!(back.names(), vec!["Third".to_string()]);
        assert_eq!(back.active, None);
    }

    #[test]
    fn unknown_settings_keys_are_skipped_and_missing_ones_default() {
        let s = sample();
        let mut bytes = s.to_bytes();
        bytes.truncate(bytes.len() - 32);
        // Append one key a newer client might write, then re-seal.
        let n_at = bytes.len()
            - s.settings
                .to_pairs()
                .iter()
                .map(|(k, _)| 1 + k.len() + 4)
                .sum::<usize>()
            - 2;
        let n = u16::from_le_bytes([bytes[n_at], bytes[n_at + 1]]) + 1;
        bytes[n_at..n_at + 2].copy_from_slice(&n.to_le_bytes());
        bytes.push(9);
        bytes.extend_from_slice(b"future_hz");
        bytes.extend_from_slice(&144f32.to_le_bytes());
        let digest = Sha256::digest(&bytes);
        bytes.extend_from_slice(&digest);
        assert_eq!(
            ProfileStore::from_bytes(&bytes).unwrap().settings,
            s.settings
        );
        // Out-of-range values are clamped to what the UI can produce.
        let mut o = GameOptions::default();
        assert!(o.set_pair("portraits", 9.0) && o.portraits == 2);
        assert!(o.set_pair("game_speed", 50.0) && o.game_speed == 3.0);
        assert!(!o.set_pair("music_volume", f32::NAN));
        assert!(!o.set_pair("nope", 1.0));
    }

    #[test]
    fn every_setting_round_trips_through_its_pair() {
        let mut o = GameOptions::default();
        let d = GameOptions::default();
        // Flip every value away from its default.
        for (key, v) in d.to_pairs() {
            let flipped = match key {
                "game_speed" => 2.0,
                "portraits" => 1.0,
                "minimap_position" => 1.0,
                _ if v == 0.0 || v == 1.0 => 1.0 - v,
                _ => 0.125,
            };
            assert!(o.set_pair(key, flipped), "{key}");
        }
        let mut back = GameOptions::default();
        for (key, v) in o.to_pairs() {
            back.set_pair(key, v);
        }
        assert_eq!(back, o);
        assert!(o
            .to_pairs()
            .iter()
            .zip(d.to_pairs())
            .all(|(a, b)| a.1 != b.1));
    }

    #[test]
    fn progress_book_migration_carries_progress_and_adds_missing_ids() {
        let mut book = ProgressBook::default();
        let mut p = Progress::default();
        p.next[Campaign::Zerg.index()] = 15;
        book.set("linka", p);
        book.set("Old", p);
        book.set("bad/name", p);
        let mut s = sample();
        let linka_created = s.get("LinKa").unwrap().created;
        s.import_progress_book(&book, 5);
        assert_eq!(
            s.names(),
            vec!["REA", "LinKa", "Old"],
            "invalid IDs are dropped"
        );
        assert_eq!(s.progress("LinKa"), p, "the book's progress wins");
        assert_eq!(s.get("LinKa").unwrap().created, linka_created);
        assert_eq!(s.progress("Old").next[Campaign::Zerg.index()], 15);
        assert_eq!(s.get("Old").unwrap().created, 5);
    }

    /// Head block of the reference `REA.spc`, rebuilt from the decoded
    /// fields; its check must be the stored `0xc0aa7dbd`.  The name block
    /// (`"REA"` + 93 NULs) must give the stored `0x72555f52`, and the
    /// `LinKa` one `0x32baa24c`.
    #[test]
    fn retail_block_check_is_inverted_crc32() {
        let rea = RetailCharacter {
            multiplayer: false,
            created: 0x6ade_c5a1,
            name: "REA".into(),
            progress: [0, 1, 0, 0, 0, 0],
        };
        let mut head = [0u8; RETAIL_HEAD];
        head[0..4].copy_from_slice(&RETAIL_MAGIC.to_le_bytes());
        head[8..12].copy_from_slice(&0x6ade_c5a1u32.to_le_bytes());
        head[0x0c..0x0f].copy_from_slice(b"REA");
        head[0x2c] = 1;
        assert_eq!(retail_check(&head), 0xc0aa_7dbd);
        let mut name2 = [0u8; RETAIL_NAME2];
        name2[..3].copy_from_slice(b"REA");
        assert_eq!(retail_check(&name2), 0x7255_5f52);
        name2[..5].copy_from_slice(b"LinKa");
        assert_eq!(retail_check(&name2), 0x32ba_a24c);
        // Our writer reproduces the stored check and reads back.
        let bytes = rea.to_bytes();
        assert_eq!(&bytes[0..4], &0xc0aa_7dbdu32.to_le_bytes());
        assert_eq!(RetailCharacter::parse(&bytes), Some(rea));
    }

    #[test]
    fn retail_records_reject_corruption_and_raw_chunks_load() {
        let c = RetailCharacter {
            multiplayer: true,
            created: 7,
            name: "LinKa".into(),
            progress: [0; 6],
        };
        let bytes = c.to_bytes();
        for i in [0, 5, 13, bytes.len() - 1] {
            let mut bad = bytes.clone();
            bad[i] ^= 1;
            assert_eq!(RetailCharacter::parse(&bad), None, "flip at {i}");
        }
        assert_eq!(RetailCharacter::parse(&bytes[..bytes.len() - 2]), None);
        // An uncompressed chunk (size == chunk length) is accepted, as in
        // `0x00688290`.
        let mut raw = Vec::new();
        let mut head = [0u8; RETAIL_HEAD];
        head[0..4].copy_from_slice(&RETAIL_MAGIC.to_le_bytes());
        head[0x0c..0x11].copy_from_slice(b"LinKa");
        for block in [&head[..], &[0u8; RETAIL_NAME2][..]] {
            raw.extend_from_slice(&retail_check(block).to_le_bytes());
            raw.extend_from_slice(&1u32.to_le_bytes());
            raw.extend_from_slice(&(block.len() as u32).to_le_bytes());
            raw.extend_from_slice(block);
        }
        let parsed = RetailCharacter::parse(&raw).unwrap();
        assert_eq!(parsed.name, "LinKa", "empty long name falls back to +0x0c");
        // Wrong magic is refused even with valid checks.
        let mut wrong = raw.clone();
        wrong[12] = 0;
        let fixed = retail_check(&{
            let mut h = head;
            h[0] = 0;
            h
        });
        wrong[0..4].copy_from_slice(&fixed.to_le_bytes());
        assert_eq!(RetailCharacter::parse(&wrong), None);
    }

    #[test]
    fn retail_progress_maps_onto_campaign_slots() {
        // LinKa: Terran slot (+0x2c) = 10, the live `9. New Gettysburg`.
        let c = RetailCharacter {
            multiplayer: false,
            created: 0,
            name: "LinKa".into(),
            progress: [0, 10, 0, 0, 33, 0],
        };
        let p = c.to_progress();
        assert_eq!(p.next[Campaign::Terran.index()], 10);
        assert_eq!(p.next[Campaign::Zerg.index()], 12, "0 = first mission");
        assert_eq!(
            p.history(Campaign::Terran).last().map(|e| e.label),
            Some("9. New Gettysburg")
        );
        // Brood War Terran slot (+0x38) clamps into its campaign range.
        assert_eq!(p.next[Campaign::ExpTerran.index()], 40);
        let mut s = ProfileStore::default();
        assert!(s.import_retail(&c));
        assert!(!s.import_retail(&c), "re-importing changes nothing");
        // An existing ID keeps whichever progress is further per campaign.
        let mut s = ProfileStore::default();
        s.create("linka", 1).unwrap();
        s.profiles[0].progress.next[Campaign::Zerg.index()] = 15;
        assert!(s.import_retail(&c));
        let p = s.progress("LinKa");
        assert_eq!(p.next[Campaign::Terran.index()], 10, "retail is further");
        assert_eq!(p.next[Campaign::Zerg.index()], 15, "ours is further");
        assert_eq!(s.names(), vec!["linka"], "no duplicate ID");
        assert_eq!(s.get("LinKa").unwrap().created, 1);
    }

    /// Reads a real retail `Characters` directory when `SC_CHARACTERS` points
    /// at one (skips otherwise).
    #[test]
    fn retail_characters_dir_imports_when_available() {
        let Some(dir) = std::env::var_os("SC_CHARACTERS") else {
            return;
        };
        let mut s = ProfileStore::default();
        let n = s.import_retail_dir(Path::new(&dir));
        assert!(n > 0, "no .spc imported from {dir:?}");
        for p in &s.profiles {
            assert!(validate_name(&p.name).is_ok());
        }
    }
}

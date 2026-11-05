//! Campaign selection logic of the retail episode screen (`glucmpgn` /
//! `gluexpcmpgn`) and its `Select starting mission` list (`gluhist`).
//!
//! Evidence (2026-11-02, see `research/analysis/investigations/
//! episode-flow-re.md`):
//!
//! * The mission-history list is the retail `MenuEntry` table found in the
//!   menu dump (`research/work/sc_mem_menu_pe.exe`, file offset
//!   `0x952c88`): 8-byte records `{u16 gluHist string (1-based), u16
//!   mission (mapdata index), u16 cinematic, u8 race, u8 hidden}`, one
//!   block per campaign, each closed by a zero string.  The labels are the
//!   `gluhist.json` values those indices name.
//! * `arr/mapdata.tbl` maps the mission index to its map directory.
//! * The live client (profile with Terran history up to `9. New
//!   Gettysburg`) lists every non-hidden entry whose mission is at most the
//!   next playable one, opens with the first entry selected and scrolled to
//!   the top, scrolls one row per arrow click / wheel notch without moving
//!   the selection, and scrolls the selection into view on Up/Down.  A
//!   profile without history skips the list and starts the campaign.
//! * Locked campaigns show `<Race> Campaign\nincomplete` on hover
//!   (`gluCmpgn-8/9/10`) and, when clicked, the `glupokcancel` prompt with
//!   `strRACE_ERR_*`; Ok plays the campaign anyway.

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// The six retail campaigns (the in-memory `MenuEntry` blocks run Terran,
/// Zerg, Protoss, Brood War Terran, Zerg, Protoss; race bytes 1/0/2).  The
/// `.spc` progress slot of a campaign is its race (+3 for Brood War),
/// see `crate::profile`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Campaign {
    Terran,
    Zerg,
    Protoss,
    ExpProtoss,
    ExpTerran,
    ExpZerg,
}

impl Campaign {
    pub const ALL: [Campaign; 6] = [
        Campaign::Terran,
        Campaign::Zerg,
        Campaign::Protoss,
        Campaign::ExpProtoss,
        Campaign::ExpTerran,
        Campaign::ExpZerg,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    /// Campaign shown by episode-screen slot `slot` (0 left, 1 centre,
    /// 2 right): `glucmpgn` places Zerg/Terran/Protoss, `gluexpcmpgn`
    /// Terran/Protoss/Zerg.
    pub fn of_slot(expansion: bool, slot: usize) -> Campaign {
        match (expansion, slot) {
            (false, 0) => Campaign::Zerg,
            (false, 1) => Campaign::Terran,
            (false, _) => Campaign::Protoss,
            (true, 0) => Campaign::ExpTerran,
            (true, 1) => Campaign::ExpProtoss,
            (true, _) => Campaign::ExpZerg,
        }
    }

    /// Episode number (1..=6) shown under the slot.
    pub fn episode(self) -> u8 {
        match self {
            Campaign::Terran => 1,
            Campaign::Zerg => 2,
            Campaign::Protoss => 3,
            Campaign::ExpProtoss => 4,
            Campaign::ExpTerran => 5,
            Campaign::ExpZerg => 6,
        }
    }

    pub fn from_episode(episode: u8) -> Option<Campaign> {
        Campaign::ALL.into_iter().find(|c| c.episode() == episode)
    }

    /// History entries of this campaign, hidden ones included.
    pub fn entries(self) -> &'static [HistEntry] {
        match self {
            Campaign::Terran => TERRAN,
            Campaign::Zerg => ZERG,
            Campaign::Protoss => PROTOSS,
            Campaign::ExpProtoss => EXP_PROTOSS,
            Campaign::ExpTerran => EXP_TERRAN,
            Campaign::ExpZerg => EXP_ZERG,
        }
    }

    /// First and last playable mission of the campaign.  Cinematic entries
    /// are skipped: the closing movie carries the *next* campaign's first
    /// mission number (Terran `The Inauguration` = mission 12 = Zerg 1).
    pub fn mission_range(self) -> (u16, u16) {
        let missions = || {
            self.entries()
                .iter()
                .filter(|e| e.cinematic == 0)
                .map(|e| e.mission)
        };
        (missions().min().unwrap_or(0), missions().max().unwrap_or(0))
    }
}

/// One `MenuEntry` record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistEntry {
    /// `gluhist.json` value (empty for the unnamed hidden entries).
    pub label: &'static str,
    /// `arr/mapdata.tbl` index of the mission this entry starts.
    pub mission: u16,
    /// Cinematic played first (0 = plain mission entry).
    pub cinematic: u16,
    pub hidden: bool,
}

const fn m(label: &'static str, mission: u16) -> HistEntry {
    HistEntry {
        label,
        mission,
        cinematic: 0,
        hidden: false,
    }
}
const fn c(label: &'static str, mission: u16, cinematic: u16) -> HistEntry {
    HistEntry {
        label,
        mission,
        cinematic,
        hidden: false,
    }
}
const fn h(label: &'static str, mission: u16) -> HistEntry {
    HistEntry {
        label,
        mission,
        cinematic: 0,
        hidden: true,
    }
}

const TERRAN: &[HistEntry] = &[
    m("Tutorial: Boot Camp", 1),
    m("1. Wasteland", 2),
    m("2. Backwater Station", 3),
    c("Cinematic: Wasteland Patrol", 4, 6),
    m("3. Desperate Alliance", 4),
    m("4. The Jacobs Installation", 5),
    m("5. Revolution", 6),
    c("Cinematic: The Downing of Norad II", 7, 7),
    m("6. Norad II", 7),
    m("7. The Trump Card", 8),
    m("8. The Big Push", 9),
    c("Cinematic: Open Rebellion", 10, 8),
    m("9. New Gettysburg", 10),
    m("10. The Hammer Falls", 11),
    c("Cinematic: The Inauguration", 12, 9),
];
const ZERG: &[HistEntry] = &[
    m("1. Among the Ruins", 12),
    m("2. Egression", 13),
    m("3. The New Dominion", 14),
    c("Cinematic: The Dream", 15, 13),
    m("4. Agent of the Swarm", 15),
    m("5. The Amerigo", 16),
    c("Cinematic: Battle on the Amerigo", 17, 14),
    m("6. The Dark Templar", 17),
    m("7. The Culling", 18),
    m("8. Eye for an Eye", 19),
    c("Cinematic: The Warp", 20, 15),
    m("9. The Invasion of Aiur", 20),
    m("10. Full Circle", 21),
    c("Cinematic: The Invasion of Aiur", 22, 16),
];
const PROTOSS: &[HistEntry] = &[
    m("1. First Strike", 22),
    m("2. Into the Flames", 23),
    m("3. Higher Ground", 24),
    c("Cinematic: The Fall of Fenix", 25, 20),
    m("4. The Hunt for Tassadar", 25),
    m("5. Choosing Sides", 26),
    c("Cinematic: The Ambush", 27, 21),
    m("6. Into the Darkness", 27),
    m("7. Homeland", 28),
    c("Cinematic: The Return to Aiur", 29, 22),
    m("8. The Trial of Tassadar", 29),
    m("9. Shadow Hunters", 30),
    m("10. Eye of the Storm", 31),
    c("Cinematic: The Death of the Overmind", 32, 23),
];
const EXP_PROTOSS: &[HistEntry] = &[
    m("1. Escape from Aiur", 32),
    m("2. Dunes of Shakuras", 33),
    m("3. Legacy of the Xel'Naga", 34),
    m("4. The Quest for Uraj", 35),
    m("5. The Battle of Braxis", 36),
    m("6. Return to Char", 37),
    m("7. The Insurgent", 38),
    m("8. Countdown", 39),
    c("Cinematic: Fury of the Xel'Naga", 40, 25),
];
const EXP_TERRAN: &[HistEntry] = &[
    m("1. First Strike", 40),
    m("2. The Dylarian Shipyards", 41),
    m("3. Ruins of Tarsonis", 42),
    m("4. Assault on Korhal", 43),
    m("5a. Emperor's Fall (Ground Zero)", 44),
    h("5b. Emperor's Fall (Birds of War)", 45),
    m("6. Emperor's Flight", 46),
    m("7. Patriot's Blood", 47),
    m("8. To Chain the Beast", 48),
    c("Cinematic: UED Victory Report", 49, 26),
];
const EXP_ZERG: &[HistEntry] = &[
    m("1. Vile Disruption", 49),
    m("2. Reign of Fire", 50),
    m("3. The Kel-Morian Combine", 51),
    m("4. The Liberation of Korhal", 52),
    h("", 53),
    h("", 54),
    h("", 55),
    h("", 56),
    h("", 57),
    m("5. True Colors", 58),
    m("6. Fury of the Swarm", 59),
    m("7. Drawing of the Web", 60),
    m("8. To Slay the Beast", 61),
    m("9. The Reckoning", 62),
    h("", 63),
    m("10. Omega", 64),
    c("Cinematic: The Ascension", 65, 27),
];

/// `arr/mapdata.tbl` entries 1..=64 (index 0 is `null`): campaign
/// directory and mission key.
const MAPDATA: [(&str, &str); 64] = [
    ("terran", "tutorial"),
    ("terran", "terran01"),
    ("terran", "terran02"),
    ("terran", "terran03"),
    ("terran", "terran04"),
    ("terran", "terran05"),
    ("terran", "terran06"),
    ("terran", "terran08"),
    ("terran", "terran09"),
    ("terran", "terran11"),
    ("terran", "terran12"),
    ("zerg", "zerg01"),
    ("zerg", "zerg02"),
    ("zerg", "zerg03"),
    ("zerg", "zerg04"),
    ("zerg", "zerg05"),
    ("zerg", "zerg06"),
    ("zerg", "zerg07"),
    ("zerg", "zerg08"),
    ("zerg", "zerg09"),
    ("zerg", "zerg10"),
    ("protoss", "protoss01"),
    ("protoss", "protoss02"),
    ("protoss", "protoss03"),
    ("protoss", "protoss04"),
    ("protoss", "protoss05"),
    ("protoss", "protoss06"),
    ("protoss", "protoss07"),
    ("protoss", "protoss08"),
    ("protoss", "protoss09"),
    ("protoss", "protoss10"),
    ("expprotoss", "protoss01"),
    ("expprotoss", "protoss02"),
    ("expprotoss", "protoss03"),
    ("expprotoss", "protoss04"),
    ("expprotoss", "protoss05"),
    ("expprotoss", "protoss06"),
    ("expprotoss", "protoss07"),
    ("expprotoss", "protoss08"),
    ("expterran", "terran01"),
    ("expterran", "terran02"),
    ("expterran", "terran03"),
    ("expterran", "terran04"),
    ("expterran", "terran05a"),
    ("expterran", "terran05b"),
    ("expterran", "terran06"),
    ("expterran", "terran07"),
    ("expterran", "terran08"),
    ("expzerg", "zerg01"),
    ("expzerg", "zerg02"),
    ("expzerg", "zerg03"),
    ("expzerg", "zerg04a"),
    ("expzerg", "zerg04b"),
    ("expzerg", "zerg04c"),
    ("expzerg", "zerg04d"),
    ("expzerg", "zerg04e"),
    ("expzerg", "zerg04f"),
    ("expzerg", "zerg05"),
    ("expzerg", "zerg06"),
    ("expzerg", "zerg07"),
    ("expzerg", "zerg08"),
    ("expzerg", "zerg09"),
    ("expzerg", "bonus"),
    ("expzerg", "zerg10"),
];

/// Campaign directory and mission key of a mapdata mission index.
pub fn mission_key(mission: u16) -> Option<(&'static str, &'static str)> {
    MAPDATA.get((mission as usize).checked_sub(1)?).copied()
}

/// Mapdata mission of a campaign map (`terran`, `terran11` -> 10).  The
/// shipped storage also carries maps the retail campaign never plays
/// (`terran07` "Biting the Bullet", `terran10` "Operation Silent
/// Scream"): they have no mapdata index.
pub fn mission_of(dir: &str, key: &str) -> Option<u16> {
    MAPDATA
        .iter()
        .position(|&(d, k)| d == dir && k == key)
        .map(|i| i as u16 + 1)
}

impl Campaign {
    /// Campaign whose playable range holds `mission`.
    pub fn of_mission(mission: u16) -> Option<Campaign> {
        Campaign::ALL.into_iter().find(|c| {
            let (lo, hi) = c.mission_range();
            (lo..=hi).contains(&mission)
        })
    }
}

/// Retail mission title: the `gluhist` label of its entry without the list
/// number (`9. New Gettysburg` -> `New Gettysburg`, `Tutorial: Boot Camp`
/// -> `Boot Camp`).  The CHK's first string is not reliable (`terran11`
/// stores a trigger text there, `SPRP` keeps beta names).
pub fn mission_title(mission: u16) -> Option<&'static str> {
    let entry = Campaign::of_mission(mission)?
        .entries()
        .iter()
        .find(|e| e.mission == mission && e.cinematic == 0 && !e.label.is_empty())?;
    let label = entry.label;
    let rest = label
        .strip_prefix("Tutorial: ")
        .or_else(|| {
            let (num, rest) = label.split_once(". ")?;
            num.chars()
                .all(|c| c.is_ascii_alphanumeric())
                .then_some(rest)
        })
        .unwrap_or(label);
    Some(rest)
}

/// Movies of a cinematic entry: the location card `Pre<Race>_<n>` (when the
/// install ships one) then the film `<Race><n>`.  Every card names the
/// place of its film (`PreTerran_1` "Perimeter Patrol ... Mar Sara" before
/// `Terran1` "Wasteland Patrol"; `Zerg1` and `Protoss4` have none).
pub fn cinematic_movies(cinematic: u16) -> Vec<&'static str> {
    let Some(film) = cinematic_clip(cinematic) else {
        return Vec::new();
    };
    let card = film
        .strip_prefix("smk/")
        .and_then(|f| f.strip_suffix(".webm"))
        .and_then(|f| {
            let n = f.find(|c: char| c.is_ascii_digit())?;
            let (race, act) = f.split_at(n);
            let card = format!("smk/pre{race}_{act}.webm");
            crate::video::CAMPAIGN_CINEMATICS
                .iter()
                .find(|c| **c == card)
                .copied()
        });
    card.into_iter().chain(std::iter::once(film)).collect()
}

/// What follows a won campaign mission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Advance {
    /// Movies to play first (card + film of the entry that precedes the
    /// next mission, or the campaign's closing cinematic).
    pub movies: Vec<&'static str>,
    /// Next mapdata mission, `None` when the campaign is complete.
    pub next: Option<u16>,
}

/// The retail progression after a victory in `mission`: the next mapdata
/// index of the same campaign, preceded by the cinematic entry that names
/// it (Backwater Station -> `Wasteland Patrol` -> Desperate Alliance).
/// After the last mission the closing entry plays (`The Inauguration`,
/// whose mission number is already the next campaign's first).
pub fn after_victory(mission: u16) -> Option<Advance> {
    let campaign = Campaign::of_mission(mission)?;
    let next = mission + 1;
    let movies = campaign
        .entries()
        .iter()
        .find(|e| e.mission == next && e.cinematic != 0)
        .map(|e| cinematic_movies(e.cinematic))
        .unwrap_or_default();
    Some(Advance {
        movies,
        next: (next <= campaign.mission_range().1).then_some(next),
    })
}

/// Retail cinematic enum -> movie.  2..=23 are the shipped
/// `Pre<Race>_<act>` / `<Race><act>` movies in `video::CAMPAIGN_CINEMATICS`
/// order (6 = `terran1` "Wasteland Patrol"); 24 is the Brood War intro and
/// 25..=27 the `x<Race>` episode movies.
pub fn cinematic_clip(cinematic: u16) -> Option<&'static str> {
    match cinematic {
        2..=23 => crate::video::CAMPAIGN_CINEMATICS
            .get(cinematic as usize - 2)
            .copied(),
        24 => Some(crate::video::INTRO_EXPANSION),
        25 => Some("smk/xprotoss.webm"),
        26 => Some("smk/xterran.webm"),
        27 => Some("smk/xzerg.webm"),
        _ => None,
    }
}

/// Per-profile campaign progress: the next playable mission of each
/// campaign (`mission_range().1 + 1` once the campaign is complete).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub next: [u16; 6],
}

impl Default for Progress {
    fn default() -> Self {
        let mut next = [0u16; 6];
        for c in Campaign::ALL {
            next[c.index()] = c.mission_range().0;
        }
        Self { next }
    }
}

impl Progress {
    /// Entries shown in `Select starting mission`.
    pub fn history(&self, campaign: Campaign) -> Vec<&'static HistEntry> {
        let next = self.next[campaign.index()];
        campaign
            .entries()
            .iter()
            .filter(|e| !e.hidden && e.mission <= next)
            .collect()
    }

    pub fn complete(&self, campaign: Campaign) -> bool {
        self.next[campaign.index()] > campaign.mission_range().1
    }

    /// Record a won mission: the campaign advances to the next mission of
    /// the table (hidden branches included, as retail plays them in order).
    pub fn record_win(&mut self, mission: u16) {
        let Some(campaign) = Campaign::ALL.into_iter().find(|c| {
            c.entries()
                .iter()
                .any(|e| e.mission == mission && e.cinematic == 0)
        }) else {
            return;
        };
        let next = campaign
            .entries()
            .iter()
            .map(|e| e.mission)
            .filter(|&n| n > mission)
            .min()
            .unwrap_or(campaign.mission_range().1 + 1);
        let slot = &mut self.next[campaign.index()];
        *slot = (*slot).max(next);
    }

    /// The campaign this one asks to finish first, if any (`gluCmpgn-8/9/10`
    /// and `gluExpCmpgn-8/9/10`).
    pub fn blocker(&self, campaign: Campaign) -> Option<Campaign> {
        let needs: &[Campaign] = match campaign {
            Campaign::Terran | Campaign::ExpProtoss => &[],
            Campaign::Zerg => &[Campaign::Terran],
            Campaign::Protoss => &[Campaign::Terran, Campaign::Zerg],
            Campaign::ExpTerran => &[Campaign::ExpProtoss],
            Campaign::ExpZerg => &[Campaign::ExpProtoss, Campaign::ExpTerran],
        };
        needs.iter().copied().find(|c| !self.complete(*c))
    }

    /// Hover caption of a locked slot (`<Race> Campaign\nincomplete`).
    pub fn lock_caption(&self, campaign: Campaign) -> Option<&'static str> {
        Some(match self.blocker(campaign)? {
            Campaign::Terran | Campaign::ExpTerran => "Terran Campaign\nincomplete",
            Campaign::Zerg | Campaign::ExpZerg => "Zerg Campaign\nincomplete",
            Campaign::Protoss | Campaign::ExpProtoss => "Protoss Campaign\nincomplete",
        })
    }
}

/// `strRACE_ERR_*` prompt shown when a locked campaign is clicked.
pub fn lock_prompt(campaign: Campaign) -> &'static str {
    match campaign {
        Campaign::Zerg => {
            "We recommend that you complete the Terran Campaign before playing this campaign.\n\nDo you wish to play the Zerg Campaign anyway?"
        }
        Campaign::Protoss => {
            "We recommend that you complete the Terran and Zerg Campaigns before playing this campaign.\n\nDo you wish to play the Protoss Campaign anyway?"
        }
        Campaign::ExpTerran => {
            "We recommend that you complete the Protoss Campaign before playing this campaign.\n\nDo you wish to play the Terran Campaign anyway?"
        }
        Campaign::ExpZerg => {
            "We recommend that you complete the Protoss and Terran Campaigns before playing this campaign.\n\nDo you wish to play the Zerg Campaign anyway?"
        }
        Campaign::Terran | Campaign::ExpProtoss => "",
    }
}

/// `Select starting mission` list box state (8 visible rows).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MissionList {
    pub len: usize,
    pub sel: usize,
    pub top: usize,
}

impl MissionList {
    pub const ROWS: usize = 8;

    /// Retail opens with the first entry selected and the list at the top.
    pub fn open(len: usize) -> Self {
        Self {
            len,
            sel: 0,
            top: 0,
        }
    }

    pub fn max_top(&self) -> usize {
        self.len.saturating_sub(Self::ROWS)
    }

    /// Arrow buttons and the wheel scroll without moving the selection.
    pub fn scroll(&mut self, delta: i32) {
        let top = self.top as i64 + delta as i64;
        self.top = top.clamp(0, self.max_top() as i64) as usize;
    }

    /// Click on visible row `row`.
    pub fn click_row(&mut self, row: usize) -> bool {
        let idx = self.top + row;
        if row < Self::ROWS && idx < self.len {
            self.sel = idx;
            true
        } else {
            false
        }
    }

    /// Up/Down: move the selection and scroll it into view.
    pub fn step(&mut self, delta: i32) {
        if self.len == 0 {
            return;
        }
        let sel = (self.sel as i64 + delta as i64).clamp(0, self.len as i64 - 1) as usize;
        self.sel = sel;
        if sel < self.top {
            self.top = sel;
        } else if sel >= self.top + Self::ROWS {
            self.top = sel + 1 - Self::ROWS;
        }
    }

    /// Thumb dragged to `fraction` (0 top .. 1 last page) of its travel.
    pub fn set_thumb(&mut self, fraction: f32) {
        let top = (fraction.clamp(0.0, 1.0) * self.max_top() as f32).round();
        self.top = top as usize;
    }

    /// Thumb position in 0..=1 along the track.
    pub fn thumb(&self) -> f32 {
        if self.max_top() == 0 {
            0.0
        } else {
            self.top as f32 / self.max_top() as f32
        }
    }
}

/// Campaign progress of every profile, persisted as `GCPR`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProgressBook {
    pub profiles: BTreeMap<String, Progress>,
}

const BOOK_MAGIC: &[u8; 4] = b"GCPR";
const BOOK_VERSION: u16 = 1;
const BOOK_MAX_BYTES: usize = 64 * 1024;
const BOOK_MAX_PROFILES: usize = 256;

impl ProgressBook {
    pub fn get(&self, profile: &str) -> Progress {
        self.profiles.get(profile).copied().unwrap_or_default()
    }

    pub fn set(&mut self, profile: &str, progress: Progress) {
        self.profiles.insert(profile.to_string(), progress);
    }

    /// Magic, version, profile count, `{u8 len, name, 6 x u16}` records and
    /// a SHA-256 trailer.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(BOOK_MAGIC);
        out.extend_from_slice(&BOOK_VERSION.to_le_bytes());
        let profiles: Vec<_> = self
            .profiles
            .iter()
            .filter(|(name, _)| !name.is_empty() && name.len() <= 255)
            .take(BOOK_MAX_PROFILES)
            .collect();
        out.extend_from_slice(&(profiles.len() as u16).to_le_bytes());
        for (name, progress) in profiles {
            out.push(name.len() as u8);
            out.extend_from_slice(name.as_bytes());
            for n in progress.next {
                out.extend_from_slice(&n.to_le_bytes());
            }
        }
        let digest = Sha256::digest(&out);
        out.extend_from_slice(&digest);
        out
    }

    /// Parse a book; anything malformed returns `None`.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > BOOK_MAX_BYTES || bytes.len() < 8 + 32 {
            return None;
        }
        let (payload, digest) = bytes.split_at(bytes.len() - 32);
        if Sha256::digest(payload).as_slice() != digest || payload.get(0..4)? != BOOK_MAGIC {
            return None;
        }
        let u16_at = |i: usize| Some(u16::from_le_bytes([*payload.get(i)?, *payload.get(i + 1)?]));
        if u16_at(4)? != BOOK_VERSION {
            return None;
        }
        let count = u16_at(6)? as usize;
        if count > BOOK_MAX_PROFILES {
            return None;
        }
        let mut at = 8;
        let mut book = ProgressBook::default();
        for _ in 0..count {
            let len = *payload.get(at)? as usize;
            let name = std::str::from_utf8(payload.get(at + 1..at + 1 + len)?).ok()?;
            at += 1 + len;
            let mut next = [0u16; 6];
            for n in next.iter_mut() {
                *n = u16_at(at)?;
                at += 2;
            }
            book.profiles.insert(name.to_string(), Progress { next });
        }
        (at == payload.len()).then_some(book)
    }

    pub fn load(path: &std::path::Path) -> Option<Self> {
        let meta = std::fs::metadata(path).ok()?;
        if meta.len() as usize > BOOK_MAX_BYTES {
            return None;
        }
        Self::from_bytes(&std::fs::read(path).ok()?)
    }

    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, self.to_bytes())?;
        std::fs::rename(&tmp, path)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn episode_one_follows_the_retail_mission_table() {
        use super::{Advance, after_victory, mission_of, mission_title};
        // Rebel Yell plays tutorial, terran01..06, 08, 09, 11, 12.
        let keys: Vec<&str> = (1..=11)
            .map(|m| super::mission_key(m).expect("terran mission").1)
            .collect();
        assert_eq!(
            keys,
            [
                "tutorial", "terran01", "terran02", "terran03", "terran04", "terran05", "terran06",
                "terran08", "terran09", "terran11", "terran12"
            ]
        );
        assert_eq!(mission_of("terran", "terran07"), None, "unused map");
        assert_eq!(mission_of("terran", "terran10"), None, "unused map");
        assert_eq!(mission_of("terran", "terran11"), Some(10));
        assert_eq!(mission_title(1), Some("Boot Camp"));
        assert_eq!(mission_title(8), Some("The Trump Card"));
        assert_eq!(mission_title(10), Some("New Gettysburg"));
        // Wasteland Patrol (card + film) between Backwater Station and
        // Desperate Alliance; Norad II after Revolution; Open Rebellion
        // after The Big Push; The Inauguration closes the episode.
        let film = |a: &'static str, b: &'static str| vec![a, b];
        assert_eq!(
            after_victory(2).unwrap(),
            Advance {
                movies: vec![],
                next: Some(3)
            }
        );
        assert_eq!(
            after_victory(3).unwrap(),
            Advance {
                movies: film("smk/preterran_1.webm", "smk/terran1.webm"),
                next: Some(4)
            }
        );
        assert_eq!(after_victory(4).unwrap().movies, Vec::<&str>::new());
        assert_eq!(
            after_victory(6).unwrap().movies,
            film("smk/preterran_2.webm", "smk/terran2.webm")
        );
        assert_eq!(
            after_victory(7).unwrap().next,
            Some(8),
            "Norad II -> Trump Card"
        );
        assert_eq!(
            after_victory(9).unwrap().movies,
            film("smk/preterran_3.webm", "smk/terran3.webm")
        );
        assert_eq!(
            after_victory(11).unwrap(),
            Advance {
                movies: film("smk/preterran_4.webm", "smk/terran4.webm"),
                next: None
            }
        );
    }

    #[test]
    fn cinematic_cards_pair_with_their_films() {
        use super::cinematic_movies;
        assert_eq!(cinematic_movies(13), ["smk/zerg1.webm"], "no PreZerg_1");
        assert_eq!(
            cinematic_movies(14),
            ["smk/prezerg_2.webm", "smk/zerg2.webm"]
        );
        assert_eq!(
            cinematic_movies(20),
            ["smk/preprotoss_1.webm", "smk/protoss1.webm"]
        );
        assert_eq!(
            cinematic_movies(23),
            ["smk/protoss4.webm"],
            "no PreProtoss_4"
        );
        assert_eq!(cinematic_movies(26), ["smk/xterran.webm"]);
        assert!(cinematic_movies(0).is_empty());
    }

    use super::*;

    fn labels(p: &Progress, c: Campaign) -> Vec<&'static str> {
        p.history(c).iter().map(|e| e.label).collect()
    }

    #[test]
    fn terran_history_matches_reference_131132() {
        // The capture's profile has played up to `8. The Big Push`
        // (mission 9): 11 entries, the last 8 visible after scrolling.
        let mut p = Progress::default();
        p.next[Campaign::Terran.index()] = 9;
        let l = labels(&p, Campaign::Terran);
        assert_eq!(l.len(), 11);
        assert_eq!(
            &l[3..],
            &[
                "Cinematic: Wasteland Patrol",
                "3. Desperate Alliance",
                "4. The Jacobs Installation",
                "5. Revolution",
                "Cinematic: The Downing of Norad II",
                "6. Norad II",
                "7. The Trump Card",
                "8. The Big Push",
            ]
        );
        let mut list = MissionList::open(l.len());
        list.scroll(10);
        assert_eq!(list.top, 3, "wheel/arrows stop at the last page");
        assert_eq!(list.sel, 0, "scrolling keeps the selection");
    }

    #[test]
    fn live_profile_history_includes_the_next_mission_and_its_cinematic() {
        // Live client, profile LinKa: last rows `Cinematic: Open Rebellion`
        // and `9. New Gettysburg` (mission 10), 13 entries in all.
        let mut p = Progress::default();
        p.next[Campaign::Terran.index()] = 10;
        let l = labels(&p, Campaign::Terran);
        assert_eq!(l.len(), 13);
        assert_eq!(l[11], "Cinematic: Open Rebellion");
        assert_eq!(l[12], "9. New Gettysburg");
        assert_eq!(MissionList::open(13).max_top(), 5);
    }

    #[test]
    fn fresh_profile_has_a_single_entry_so_the_list_is_skipped() {
        let p = Progress::default();
        for c in Campaign::ALL {
            assert_eq!(p.history(c).len(), 1, "{c:?}");
        }
    }

    #[test]
    fn hidden_branches_never_appear_in_the_list() {
        let mut p = Progress::default();
        for c in Campaign::ALL {
            p.next[c.index()] = c.mission_range().1 + 1;
        }
        assert!(!labels(&p, Campaign::ExpTerran).contains(&"5b. Emperor's Fall (Birds of War)"));
        assert!(labels(&p, Campaign::ExpZerg).iter().all(|l| !l.is_empty()));
    }

    #[test]
    fn keyboard_scrolls_the_selection_into_view_like_the_live_list() {
        // Live: selection on row 2 with the list scrolled to top 5; Down
        // selected entry 3 and the list showed it as the first row.
        let mut list = MissionList {
            len: 13,
            sel: 2,
            top: 5,
        };
        list.step(1);
        assert_eq!((list.sel, list.top), (3, 3));
        list.step(20);
        assert_eq!((list.sel, list.top), (12, 5));
        assert!(list.click_row(7) && list.sel == 12);
        assert!(!MissionList::open(3).click_row(5));
    }

    #[test]
    fn dragging_the_thumb_scrolls_without_moving_the_selection() {
        let mut list = MissionList::open(13);
        list.set_thumb(1.0);
        assert_eq!((list.sel, list.top), (0, 5));
        list.set_thumb(0.5);
        assert_eq!(list.top, 3, "5 * 0.5 rounds to 3");
        list.set_thumb(-2.0);
        assert_eq!(list.top, 0);
        let mut short = MissionList::open(4);
        short.set_thumb(1.0);
        assert_eq!(short.top, 0, "no travel when everything fits");
    }

    #[test]
    fn unlock_order_and_captions_follow_the_retail_strings() {
        let mut p = Progress::default();
        assert_eq!(p.blocker(Campaign::Terran), None);
        assert_eq!(
            p.lock_caption(Campaign::Zerg),
            Some("Terran Campaign\nincomplete")
        );
        assert_eq!(
            p.lock_caption(Campaign::Protoss),
            Some("Terran Campaign\nincomplete")
        );
        assert_eq!(
            p.lock_caption(Campaign::ExpTerran),
            Some("Protoss Campaign\nincomplete")
        );
        assert_eq!(
            p.lock_caption(Campaign::ExpZerg),
            Some("Protoss Campaign\nincomplete")
        );
        // Finishing Terran unlocks Zerg; Protoss then waits for Zerg.
        for e in Campaign::Terran.entries() {
            p.record_win(e.mission);
        }
        assert!(p.complete(Campaign::Terran));
        assert_eq!(p.blocker(Campaign::Zerg), None);
        assert_eq!(
            p.lock_caption(Campaign::Protoss),
            Some("Zerg Campaign\nincomplete")
        );
        assert!(lock_prompt(Campaign::Protoss).contains("Terran and Zerg"));
    }

    #[test]
    fn winning_advances_to_the_next_table_mission() {
        let mut p = Progress::default();
        p.record_win(1); // Boot Camp -> 1. Wasteland
        assert_eq!(p.next[0], 2);
        p.record_win(3); // 2. Backwater Station -> mission 4 (cinematic + 3.)
        assert_eq!(p.next[0], 4);
        p.record_win(2); // replaying an old mission never goes back
        assert_eq!(p.next[0], 4);
        p.record_win(44); // 5a -> 5b (hidden branch) in Brood War Terran
        assert_eq!(p.next[Campaign::ExpTerran.index()], 45);
        // The Hammer Falls (11) completes Terran; its closing movie shares
        // Zerg's first mission number and must not touch Zerg.
        p.record_win(11);
        assert!(p.complete(Campaign::Terran));
        assert_eq!(p.next[Campaign::Zerg.index()], 12);
        assert_eq!(
            p.history(Campaign::Terran).last().map(|e| e.label),
            Some("Cinematic: The Inauguration")
        );
    }

    #[test]
    fn mapdata_and_cinematics_resolve() {
        assert_eq!(mission_key(1), Some(("terran", "tutorial")));
        assert_eq!(mission_key(8), Some(("terran", "terran08")));
        assert_eq!(mission_key(64), Some(("expzerg", "zerg10")));
        assert_eq!(mission_key(0), None);
        assert_eq!(cinematic_clip(6), Some("smk/terran1.webm"));
        assert_eq!(cinematic_clip(13), Some("smk/zerg1.webm"));
        assert_eq!(cinematic_clip(23), Some("smk/protoss4.webm"));
        assert_eq!(cinematic_clip(26), Some("smk/xterran.webm"));
        for c in Campaign::ALL {
            for e in c.entries() {
                if e.mission <= 64 {
                    assert!(mission_key(e.mission).is_some(), "{e:?}");
                }
                if e.cinematic != 0 {
                    assert!(cinematic_clip(e.cinematic).is_some(), "{e:?}");
                }
            }
        }
    }

    #[test]
    fn progress_book_round_trips_and_rejects_corruption() {
        let mut book = ProgressBook::default();
        let mut p = Progress::default();
        p.next[0] = 9;
        book.set("REA", p);
        book.set("LinKa", Progress::default());
        let bytes = book.to_bytes();
        assert_eq!(ProgressBook::from_bytes(&bytes), Some(book.clone()));
        let mut bad = bytes.clone();
        bad[10] ^= 1;
        assert_eq!(ProgressBook::from_bytes(&bad), None, "checksum");
        assert_eq!(ProgressBook::from_bytes(&bytes[..bytes.len() - 1]), None);
        assert_eq!(
            ProgressBook::from_bytes(&vec![0u8; BOOK_MAX_BYTES + 1]),
            None
        );
        assert_eq!(book.get("missing"), Progress::default());
    }

    #[test]
    fn slots_map_to_the_shipped_dialog_order() {
        assert_eq!(Campaign::of_slot(false, 0), Campaign::Zerg);
        assert_eq!(Campaign::of_slot(false, 1), Campaign::Terran);
        assert_eq!(Campaign::of_slot(true, 0), Campaign::ExpTerran);
        assert_eq!(Campaign::of_slot(true, 2), Campaign::ExpZerg);
        assert_eq!(Campaign::from_episode(4), Some(Campaign::ExpProtoss));
    }
}

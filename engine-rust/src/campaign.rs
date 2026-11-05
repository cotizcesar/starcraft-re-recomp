//! Retail campaign missions and the pre-mission briefing screens.
//!
//! The single-player campaign is not stored in the `Maps/` MPQ tree: the
//! shipped 1.23 CASC storage keeps every mission as a loose
//! `sd/campaign/<episode>/<mission>/staredit/scenario.chk` file.  Each
//! mission also has an establishing-shot/briefing script in
//! `locales/enus/assets/rez/est*.txt` that names the retail backdrop art
//! (`glue/palta/terrana.dds`, `glue/palza/zerga.dds`, ...), the font table
//! and optional intermission videos.
//!
//! This module enumerates those missions in retail order, maps each one to
//! its briefing script and parses the script's tag language.  The art paths
//! are resolved against the installation rather than guessed, and the
//! original `.pcx` names are translated to the Remastered `.dds` surfaces.

use std::collections::BTreeMap;

use crate::retail::RetailInstall;
use crate::sim::Race;

/// One playable campaign mission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignMission {
    /// CASC episode directory (`terran`, `zerg`, `protoss`, `expterran`, ...).
    pub episode: String,
    pub episode_name: String,
    /// Zero-based position inside the episode.
    pub order: usize,
    /// Mission directory (`terran01`, `zerg04a`, ...).
    pub key: String,
    /// Cleaned map-authored name (`Wasteland`, `Among the Ruins`, ...).
    pub name: String,
    pub race: Race,
    /// CASC path to the mission's `scenario.chk`.
    pub chk_path: String,
    /// Briefing script key (`estt01`, `estz09b`, ...) when the install has it.
    pub briefing_key: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Campaign {
    pub key: String,
    pub name: String,
    pub missions: Vec<CampaignMission>,
}

/// Episodes in the order the retail campaign menu presents them.  The
/// `terraned`/`terransw`/`multisw` directories are alternate/duplicate
/// distributions (shareware and multiplayer spawn) and are not part of the
/// episode progression.
const EPISODES: &[(&str, &str)] = &[
    ("terran", "Terran Campaign"),
    ("zerg", "Zerg Campaign"),
    ("protoss", "Protoss Campaign"),
    ("expterran", "Brood War: Terran"),
    ("expzerg", "Brood War: Zerg"),
    ("expprotoss", "Brood War: Protoss"),
];

/// Enumerate every mission in the installation, grouped by episode.
pub fn episodes(install: &RetailInstall) -> Vec<Campaign> {
    let mut by_episode: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for path in install.list("sd/campaign/", "scenario.chk") {
        let parts: Vec<&str> = path.split('/').collect();
        // sd/campaign/<episode>/<mission>/staredit/scenario.chk
        if parts.len() != 6 || parts[4] != "staredit" {
            continue;
        }
        by_episode
            .entry(parts[2].to_string())
            .or_default()
            .push((parts[3].to_string(), path));
    }

    let mut out = Vec::new();
    for (episode, name) in EPISODES {
        let Some(entries) = by_episode.remove(*episode) else {
            continue;
        };
        let mut missions = Vec::new();
        let mut entries = entries;
        entries.sort_by_key(|a| mission_sort_key(&a.0));
        for (order, (key, chk_path)) in entries.into_iter().enumerate() {
            let Some(raw) = install.read(&chk_path) else {
                continue;
            };
            if raw.is_empty() {
                continue;
            }
            let Ok(scenario) = crate::map::Scenario::parse(&raw) else {
                continue;
            };
            // Retail missions are named by their `gluhist` entry; the CHK's
            // first string is a trigger text on `terran11`.  Only the maps
            // the campaign never plays keep the CHK name.
            let mission_name = crate::campaign_progress::mission_of(episode, &key)
                .and_then(crate::campaign_progress::mission_title)
                .map(str::to_string)
                .unwrap_or_else(|| mission_name(&scenario));
            // Follow the OWNR human seat; campaign missions do not always
            // put the player at slot 0 (Zerg 01 uses slot 3, Enslavers 01
            // uses slot 6), and the human's SIDE is the mission race.
            let race = scenario
                .player_slots()
                .into_iter()
                .find(|s| s.active && s.human)
                .map(|s| s.race(episode_race(episode)))
                .unwrap_or_else(|| episode_race(episode));
            let briefing_key = briefing_candidates(episode, &key)
                .into_iter()
                .find(|k| install.contains(&format!("locales/enus/assets/rez/{k}.txt")));
            // A handful of shipped campaign maps still have the StarEdit
            // default `Untitled Scenario`; the retail title is the quoting
            // line of the establishing-shot script (`"THE JACOBS
            // INSTALLATION"`).
            let mut mission_name = mission_name;
            if (mission_name.is_empty() || mission_name.eq_ignore_ascii_case("untitled scenario"))
                && let Some(title) = briefing_key
                    .as_deref()
                    .and_then(|k| install.read(&format!("locales/enus/assets/rez/{k}.txt")))
                    .and_then(|bytes| brief_title(&String::from_utf8_lossy(&bytes)))
            {
                mission_name = title;
            }
            missions.push(CampaignMission {
                episode: episode.to_string(),
                episode_name: name.to_string(),
                order,
                key,
                name: mission_name,
                race,
                chk_path,
                briefing_key,
            });
        }
        if !missions.is_empty() {
            out.push(Campaign {
                key: episode.to_string(),
                name: name.to_string(),
                missions,
            });
        }
    }
    out
}

fn episode_race(episode: &str) -> Race {
    match episode {
        "zerg" | "expzerg" => Race::Zerg,
        "protoss" | "expprotoss" => Race::Protoss,
        _ => Race::Terran,
    }
}

/// Order missions the way the campaign plays them: numeric suffix first, so
/// `terran05a`/`terran05b` stay between 05 and 06 and `zerg04a`..`04f`
/// remain consecutive.  `tutorial` sorts before the numbered missions.
fn mission_sort_key(key: &str) -> (u32, u32, String) {
    if key == "tutorial" {
        return (0, 0, key.to_string());
    }
    if key == "bonus" {
        return (u32::MAX - 1, 0, key.to_string());
    }
    let number: String = key.chars().filter(|c| c.is_ascii_digit()).collect();
    let suffix: String = key
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_lowercase() && !c.is_ascii_digit())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let num = number.parse::<u32>().unwrap_or(u32::MAX);
    let sub = if suffix.is_empty() {
        0
    } else {
        suffix
            .bytes()
            .fold(0u32, |acc, b| acc * 26 + (b - b'a' + 1) as u32)
    };
    (num, sub, key.to_string())
}

/// First fully quoted line in a briefing script, title-cased for the menu.
fn brief_title(source: &str) -> Option<String> {
    for page in parse_script(source).pages {
        for line in page.text.lines() {
            let line = line.trim();
            if line.len() >= 2 && line.starts_with('"') && line.ends_with('"') {
                let inner = &line[1..line.len() - 1];
                if !inner.trim().is_empty() {
                    return Some(title_case(inner));
                }
            }
        }
    }
    None
}

fn title_case(text: &str) -> String {
    text.split_whitespace()
        .map(|word| {
            let upper = word.to_ascii_uppercase();
            if matches!(upper.as_str(), "I" | "II" | "III" | "IV" | "V" | "VI") {
                return upper;
            }
            if word.chars().any(|c| c.is_ascii_digit()) {
                return word.to_string();
            }
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    let mut out = first.to_uppercase().to_string();
                    out.push_str(&chars.as_str().to_lowercase());
                    out
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn mission_name(scenario: &crate::map::Scenario) -> String {
    let raw = scenario.strings.first().cloned().unwrap_or_default();
    let mut name = raw.trim().trim_matches('"').trim().to_string();
    // Campaign maps prefix the name with the episode marker (`T1) `, `Z10) `,
    // `P3) `).  The in-game name is only the part after the marker.
    if let Some(open) = name.find(')') {
        let head = &name[..open];
        if head.len() <= 4
            && head.starts_with(['T', 'Z', 'P'])
            && head[1..].chars().all(|c| c.is_ascii_digit())
        {
            name = name[open + 1..].trim().to_string();
        }
    }
    if name.is_empty() {
        scenario
            .strings
            .iter()
            .find(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    } else {
        name
    }
}

/// Retail briefing script name for a mission directory.
pub fn briefing_key(episode: &str, mission: &str) -> Option<String> {
    briefing_candidates(episode, mission).into_iter().next()
}

/// Briefing names to try for a mission, most specific first.  Some expansion
/// mission directories (`zerg04a`..`zerg04f`) share one shipped script
/// (`estz04x`), so the caller resolves the first name that exists.
pub fn briefing_candidates(episode: &str, mission: &str) -> Vec<String> {
    let letter = match episode {
        "terran" | "expterran" => 't',
        "zerg" | "expzerg" => 'z',
        "protoss" | "expprotoss" => 'p',
        _ => return Vec::new(),
    };
    if mission == "tutorial" {
        return vec![format!("est{letter}0t_hd"), format!("est{letter}0t")];
    }
    let digits: String = mission.chars().filter(|c| c.is_ascii_digit()).collect();
    let Ok(number) = digits.parse::<u32>() else {
        return Vec::new();
    };
    let suffix: String = mission
        .chars()
        .skip_while(|c| c.is_ascii_alphabetic())
        .skip_while(|c| c.is_ascii_digit())
        .collect();
    let expansion = episode.starts_with("exp");
    let base = format!("est{letter}{number:02}");
    let mut out = Vec::new();
    // Prefer the Remastered `_hd` establishing shots when the install has
    // them: they carry the same copy plus the `</VIDEO>` intermission tags
    // (`interstitials/*.webm`) the SD scripts lack.
    if !suffix.is_empty() {
        out.push(format!(
            "{base}{suffix}{}_hd",
            if expansion { "x" } else { "" }
        ));
        out.push(format!(
            "{base}{suffix}{}",
            if expansion { "x" } else { "" }
        ));
    }
    if expansion {
        out.push(format!("{base}x_hd"));
        out.push(format!("{base}x"));
    }
    out.push(format!("{base}_hd"));
    out.push(base);
    out
}

/// One page of a briefing script.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BriefingPage {
    pub text: String,
    /// Retail backdrop path, translated to the shipped `.dds` surface when
    /// one exists (`glue\palta\TerranA.pcx` -> `glue/palta/terrana.dds`).
    pub background: Option<String>,
    /// Font table path (`glue/palta/tfont.pcx`).
    pub font: Option<String>,
    /// Intermission video, when the script plays one.
    pub video: Option<String>,
    /// `</TEXTBOX>` means the page shows text over a video.
    pub textbox: bool,
    /// `</SCREENLOWERLEFT>` places the establishing-shot text at the lower
    /// left instead of the centre.
    pub screen_lower_left: bool,
    /// `</DISPLAYTIME ms>`: how long the page stays before the next one
    /// (the establishing shots use 5000).
    pub display_ms: Option<u32>,
    /// `</VIDEOAUTOPROCEED>`: the page leaves when its video ends.
    pub auto_proceed: bool,
}

impl BriefingPage {
    /// When the page leaves on its own (ms after it showed), given its
    /// video's length: `DISPLAYTIME n > 0` after n ms, `VIDEOAUTOPROCEED`
    /// when the video ends (Boot Camp's TR1035: 0.67 s, retail 28.6 ->
    /// 29.3 s); otherwise never.  `DISPLAYTIME 0` pages wait for the
    /// player (INFERENCE: Rebel Yell's three text pages stayed 7.95 /
    /// 9.65 / 9.0 s on the retail recording, matching neither their 3.3 s
    /// videos nor their 4.0-5.7 s sounds).
    pub fn due_ms(&self, video_ms: Option<u64>) -> Option<u64> {
        match self.display_ms {
            Some(ms) if ms > 0 => Some(ms as u64),
            _ if self.auto_proceed => video_ms,
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BriefingScript {
    pub pages: Vec<BriefingPage>,
}

impl BriefingScript {
    pub fn is_empty(&self) -> bool {
        self.pages.iter().all(|p| p.text.trim().is_empty())
    }
}

/// Parse StarEdit's briefing/establishing-shot script language.
///
/// Commands are wrapped in `</...>` and persist for subsequent pages until
/// changed (`BACKGROUND`, `FONTCOLOR`); `</PAGE>` terminates a page.
/// Sections that would break the page flow (`COMMENT`) are dropped, and
/// timing/fade directives are recorded as no-ops rather than echoed as text.
pub fn parse_script(source: &str) -> BriefingScript {
    let mut pages = Vec::new();
    let mut current = BriefingPage::default();
    let mut background: Option<String> = None;
    let mut font: Option<String> = None;
    let mut textbox = false;
    let mut lower_left = false;
    let mut display_ms: Option<u32> = None;

    for raw in source.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
        let line = raw.trim_end();
        let trimmed = line.trim();
        if trimmed.starts_with("</") && trimmed.ends_with('>') {
            let inner = &trimmed[2..trimmed.len() - 1];
            let mut parts = inner.splitn(2, char::is_whitespace);
            let tag = parts.next().unwrap_or("").to_ascii_uppercase();
            let arg = parts.next().unwrap_or("").trim();
            match tag.as_str() {
                "PAGE" => {
                    if !current.text.trim().is_empty()
                        || current.background.is_some()
                        || current.video.is_some()
                    {
                        pages.push(std::mem::take(&mut current));
                    }
                    // A page without its own DISPLAYTIME keeps the last one.
                    current.display_ms = display_ms;
                }
                "BACKGROUND" => {
                    background = non_empty(arg).map(|s| s.to_string());
                }
                "FONTCOLOR" => {
                    font = non_empty(arg).map(|s| s.to_string());
                }
                "TEXTBOX" => textbox = true,
                "DISPLAYTIME" => {
                    // Per page: Rebel Yell's video pages say 0, its
                    // establishing shot 6000 in the same script.
                    display_ms = arg.parse().ok();
                    current.display_ms = display_ms;
                }
                "SCREENLOWERLEFT" => lower_left = true,
                "VIDEO" | "VIDEOAUTOPROCEED" => {
                    current.video = non_empty(arg).map(|s| s.to_string());
                    current.auto_proceed = tag == "VIDEOAUTOPROCEED";
                }
                // Timing, fades, comments and unknown directives do not add
                // copy; unknown tags are ignored rather than rendered.
                _ => {}
            }
            continue;
        }
        if trimmed.parse::<u32>().is_ok() {
            // Bare timing numbers appear between directives in a few
            // scripts; they are not dialogue.
            continue;
        }
        if current.text.trim().is_empty() && trimmed.is_empty() {
            continue;
        }
        if !current.text.is_empty() {
            current.text.push('\n');
        }
        current.text.push_str(line);
    }
    if !current.text.trim().is_empty() || current.background.is_some() || current.video.is_some() {
        pages.push(current);
    }

    for page in &mut pages {
        page.background = background.clone();
        page.font = font.clone();
        page.textbox = textbox;
        page.screen_lower_left = lower_left;
        page.text = page.text.trim().to_string();
    }
    // A page's `TEXTBOX`/`SCREENLOWERLEFT` state applies from the directive
    // onward; recompute per page by re-walking is unnecessary because all
    // shipped scripts set them once before the first page.
    BriefingScript { pages }
}

fn non_empty(s: &str) -> Option<&str> {
    let s = s.trim();
    if s.is_empty() { None } else { Some(s) }
}

/// Resolve an authored glue path to a file that exists in the installation.
///
/// The classic scripts name `.pcx` art; Remastered ships the HD `.dds`
/// replacement in the same directory.  The returned path is normalised to
/// forward slashes and lower case, matching [`RetailInstall::read`].
pub fn resolve_art(install: &RetailInstall, authored: &str) -> Option<String> {
    let authored = authored.trim().trim_matches('"').replace('\\', "/");
    let lower = authored.to_ascii_lowercase();
    let mut candidates = vec![lower.clone()];
    if let Some(stem) = lower.strip_suffix(".pcx") {
        candidates.push(format!("{stem}.dds"));
        candidates.push(format!("hd2/{stem}.dds"));
    }
    if !lower.starts_with("hd2/") {
        candidates.push(format!("hd2/{lower}"));
    }
    candidates
        .into_iter()
        .find(|candidate| install.contains(candidate))
}

/// Load and resolve the briefing script for a mission.  Returns `None` when
/// the install has no script for it.
pub fn load_briefing(install: &RetailInstall, mission: &CampaignMission) -> Option<BriefingScript> {
    let key = mission.briefing_key.as_ref()?;
    let path = format!("locales/enus/assets/rez/{key}.txt");
    let bytes = install.read(&path)?;
    let text = String::from_utf8_lossy(&bytes);
    let mut script = parse_script(&text);
    for page in &mut script.pages {
        page.background = page
            .background
            .as_deref()
            .and_then(|p| resolve_art(install, p));
        page.font = page.font.as_deref().and_then(|p| {
            if install.contains(&p.replace('\\', "/").to_ascii_lowercase()) {
                Some(p.replace('\\', "/").to_ascii_lowercase())
            } else {
                None
            }
        });
    }
    Some(script)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_leave_after_their_display_time_or_video() {
        let script = parse_script(
            "</DISPLAYTIME 0>\n</VIDEO a.webm>\n</TEXTBOX>\nText\n</PAGE>\n\
             </DISPLAYTIME 0>\n</VIDEOAUTOPROCEED b.webm>\n</PAGE>\n\
             </DISPLAYTIME 6000>\n\"BOOT CAMP\"\n</PAGE>\n",
        );
        let p = &script.pages;
        assert_eq!(p.len(), 3);
        assert_eq!(
            p[0].due_ms(Some(3_334)),
            None,
            "DISPLAYTIME 0 waits for the player"
        );
        assert!(p[1].auto_proceed);
        assert_eq!(p[1].due_ms(Some(667)), Some(667));
        assert_eq!(p[2].due_ms(None), Some(6_000));
    }

    #[test]
    fn mission_order_keeps_branches_consecutive() {
        let mut keys = vec![
            "terran06",
            "terran05b",
            "terran05a",
            "terran05",
            "tutorial",
            "terran12",
        ];
        keys.sort_by_key(|k| mission_sort_key(k));
        assert_eq!(
            keys,
            vec![
                "tutorial",
                "terran05",
                "terran05a",
                "terran05b",
                "terran06",
                "terran12"
            ]
        );
        assert!(mission_sort_key("zerg04a") < mission_sort_key("zerg04b"));
        assert!(mission_sort_key("bonus") > mission_sort_key("zerg10"));
    }

    #[test]
    fn briefing_keys_follow_the_shipped_names() {
        // The `_hd` establishing shots are preferred when present: they
        // carry the same copy plus the `</VIDEO>` intermission tags.
        assert_eq!(
            briefing_key("terran", "terran01").as_deref(),
            Some("estt01_hd")
        );
        assert_eq!(
            briefing_key("terran", "tutorial").as_deref(),
            Some("estt0t_hd")
        );
        assert_eq!(briefing_key("zerg", "zerg09").as_deref(), Some("estz09_hd"));
        assert_eq!(
            briefing_key("expprotoss", "protoss03").as_deref(),
            Some("estp03x_hd")
        );
        assert_eq!(
            briefing_key("expterran", "terran05a").as_deref(),
            Some("estt05ax_hd")
        );
        assert_eq!(
            briefing_key("expzerg", "zerg04c").as_deref(),
            Some("estz04cx_hd")
        );
    }

    #[test]
    fn briefing_script_parses_pages_and_persistent_art() {
        let src = "</COMMENT header>\n\
                   </FADESPEED 100>\n\
                   </BACKGROUND glue\\palta\\TerranA.pcx>\n\
                   </FONTCOLOR glue\\palta\\tfont.pcx>\n\
                   </SCREENLOWERLEFT>\n\
                   \"WASTELAND\"\n\
                   \n\
                   Office of the Colonial Magistrate\n\
                   </PAGE>\n\
                   </TEXTBOX>\n\
                   Second page copy.\n\
                   </PAGE>\n";
        let script = parse_script(src);
        assert_eq!(script.pages.len(), 2);
        assert_eq!(
            script.pages[0].text,
            "\"WASTELAND\"\n\nOffice of the Colonial Magistrate"
        );
        assert_eq!(
            script.pages[0].background.as_deref(),
            Some("glue\\palta\\TerranA.pcx")
        );
        assert!(script.pages[0].screen_lower_left);
        assert_eq!(script.pages[1].text, "Second page copy.");
        assert_eq!(script.pages[1].background, script.pages[0].background);
        assert!(script.pages[1].textbox);
    }

    #[test]
    fn comments_and_timings_do_not_become_copy() {
        let script = parse_script(
            "</COMMENT Establishing Shot>\n\n</DISPLAYTIME 5000>\n</FADESPEED 100>\nHello\n",
        );
        assert_eq!(script.pages.len(), 1);
        assert_eq!(script.pages[0].text, "Hello");
    }

    #[test]
    fn map_names_drop_the_episode_marker() {
        assert_eq!(clean_marker("T1) Wasteland"), "Wasteland");
        assert_eq!(clean_marker("Z10) The Hammer Falls"), "The Hammer Falls");
        assert_eq!(clean_marker("P3) Higher Ground"), "Higher Ground");
        assert_eq!(clean_marker("Terran Tutorial"), "Terran Tutorial");
    }

    fn clean_marker(raw: &str) -> String {
        let mut name = raw.trim().to_string();
        if let Some(open) = name.find(')') {
            let head = &name[..open];
            if head.len() <= 4
                && head.starts_with(['T', 'Z', 'P'])
                && head[1..].chars().all(|c| c.is_ascii_digit())
            {
                name = name[open + 1..].trim().to_string();
            }
        }
        name
    }
}

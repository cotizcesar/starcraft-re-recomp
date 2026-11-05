//! Trigger text and map audio: what the player reads and hears when a
//! trigger runs Transmission (7), Play WAV (8) or Display Text Message (9).
//!
//! * Text comes from the scenario `STR` section; StarEdit colour / alignment
//!   control bytes (`< 0x20` other than line breaks) are not printable and
//!   are dropped, and `\r\n` breaks a message into several lines.
//! * A WAV is named by a `STR` string such as `staredit\wav\t1m01uci.wav`.
//!   The materialiser copies each map's sounds next to its cached CHK
//!   ([`map_wav_dir`]): the original `.wav` (the `sd/` tree of a campaign
//!   mission, or the file inside an MPQ map) and, for campaign missions, the
//!   Remastered voice-over `.ogg` from `locales/enus/`.

use std::path::{Path, PathBuf};

/// How long trigger text stays: 62 ms per visible character (map padding
/// not counted), at least 4.0 s, and a Transmission at least as long as it
/// talks.  Fitted on nine retail samples (2026-11-05 recordings, Wasteland
/// and Boot Camp; error <= 0.2 s each): Display Text TIP 165 chars 10.1 s,
/// NEW MISSION OBJECTIVE 44 chars 4.0 s, "- Select the Command Center" 60
/// chars 3.8 s; Transmissions T-280 (112 chars, 7447 ms) 7.5 s, TERRAN SCV
/// (113, 4920 ms) 7.2 s, Roger that (14, 1005 ms) 4.0 s, In order (71,
/// 4531 ms) 4.5 s, You can build (56, 3146 ms) 4.0 s, Raynor (61,
/// 3956 ms) 4.0 s.  Falsified: the per-line model (1.0-1.5 s long on the
/// 3-line Boot Camp lines) and the paused-transmission rule it needed.
pub const TEXT_MS_PER_CHAR: u64 = 62;
pub const TEXT_MIN_MS: u64 = 4_000;

/// Characters a trigger text shows (each line trimmed of its padding).
pub fn text_chars(raw: &str) -> usize {
    text_lines(raw)
        .iter()
        .map(|l| l.trim().chars().count())
        .sum()
}

/// On-screen lifetime of a trigger text of `chars` visible characters:
/// Display Text (`transmission_ms` 0) or a Transmission.
pub fn text_life_ms(transmission_ms: u32, chars: usize) -> u64 {
    (TEXT_MS_PER_CHAR * chars as u64)
        .max(TEXT_MIN_MS)
        .max(transmission_ms as u64)
}

/// Lines visible at once above the console (oldest scroll away first).
/// FACT: 10 at once on the Wasteland recording (90.5 s: the 6-line TIP
/// with its blank line and the 4-line SCV transmission); 11 is INFERENCE
/// (the 6 earlier assumed is falsified by that frame).
pub const MAX_LINES: usize = 11;

/// Display lines of a trigger string: as [`clean_lines`], but a blank line
/// between text lines keeps its slot (the TIP's gap line).
pub fn text_lines(raw: &str) -> Vec<String> {
    let lines: Vec<String> = raw
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .map(|line| line.chars().filter(|c| !c.is_control()).collect::<String>())
        .map(|line| line.trim_end().to_string())
        .collect();
    let first = lines.iter().position(|l| !l.trim().is_empty());
    let last = lines.iter().rposition(|l| !l.trim().is_empty());
    match (first, last) {
        (Some(a), Some(b)) => lines[a..=b].to_vec(),
        _ => Vec::new(),
    }
}

/// Printable lines of a trigger string.
pub fn clean_lines(raw: &str) -> Vec<String> {
    raw.replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .map(|line| line.chars().filter(|c| !c.is_control()).collect::<String>())
        .map(|line| line.trim_end().to_string())
        .filter(|line| !line.trim().is_empty())
        .collect()
}

/// File stem of a map sound string (`staredit\wav\t1m01uci.wav` →
/// `t1m01uci`), lower-cased like the CASC and cache names.
pub fn wav_stem(raw: &str) -> Option<String> {
    let name = raw.trim().rsplit(['\\', '/']).next()?;
    let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
    (!stem.is_empty()).then(|| stem.to_ascii_lowercase())
}

/// Where a cached map's sounds live: `maps/(2)Boxer.chk` →
/// `maps/(2)Boxer/wav/`.
pub fn map_wav_dir(chk: &Path) -> PathBuf {
    chk.with_extension("").join("wav")
}

/// The cached file for a sound string, preferring the Remastered `.ogg`
/// unless `original_voice_overs` is set (the Options switch), and falling
/// back to whichever exists.
pub fn find_map_wav(chk: &Path, raw: &str, original_voice_overs: bool) -> Option<PathBuf> {
    let stem = wav_stem(raw)?;
    let dir = map_wav_dir(chk);
    let ogg = dir.join(format!("{stem}.ogg"));
    let wav = dir.join(format!("{stem}.wav"));
    let order = if original_voice_overs {
        [wav, ogg]
    } else {
        [ogg, wav]
    };
    order.into_iter().find(|p| p.is_file())
}

/// On-screen trigger messages, newest last.
#[derive(Clone, Debug, Default)]
pub struct TextLog {
    lines: Vec<(String, u64)>,
}

impl TextLog {
    /// Add every line of `raw` at time `now_ms`, visible for `life_ms`.
    pub fn push(&mut self, raw: &str, now_ms: u64, life_ms: u64) {
        for line in text_lines(raw) {
            self.lines.push((line, now_ms + life_ms));
        }
        let excess = self.lines.len().saturating_sub(MAX_LINES);
        self.lines.drain(..excess);
    }

    /// Lines still visible at `now_ms`, oldest first.
    pub fn visible(&self, now_ms: u64) -> Vec<&str> {
        self.lines
            .iter()
            .filter(|(_, until)| now_ms < *until)
            .map(|(line, _)| line.as_str())
            .collect()
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_text_drops_colour_codes_and_splits_lines() {
        assert_eq!(
            clean_lines("\x03Jim Raynor:\x01 Get moving!\r\n\x04Next\r\n\r\n"),
            vec!["Jim Raynor: Get moving!", "Next"]
        );
        assert!(clean_lines("\x02\r\n  ").is_empty());
    }

    #[test]
    fn sound_strings_name_their_file_stem() {
        assert_eq!(
            wav_stem("staredit\\wav\\T1M01UCI.wav").as_deref(),
            Some("t1m01uci")
        );
        assert_eq!(wav_stem("sound/x.ogg").as_deref(), Some("x"));
        assert_eq!(wav_stem("  "), None);
        assert_eq!(
            map_wav_dir(Path::new("cache/campaign/terran/terran01.chk")),
            PathBuf::from("cache/campaign/terran/terran01/wav")
        );
    }

    #[test]
    fn the_voice_over_switch_picks_ogg_or_wav_and_falls_back() {
        let dir = std::env::temp_dir().join(format!("gc-tx-{}", std::process::id()));
        let chk = dir.join("m.chk");
        let wavs = map_wav_dir(&chk);
        std::fs::create_dir_all(&wavs).unwrap();
        std::fs::write(wavs.join("a.ogg"), b"o").unwrap();
        std::fs::write(wavs.join("a.wav"), b"w").unwrap();
        std::fs::write(wavs.join("b.wav"), b"w").unwrap();
        let raw = "staredit\\wav\\a.wav";
        assert_eq!(find_map_wav(&chk, raw, false), Some(wavs.join("a.ogg")));
        assert_eq!(find_map_wav(&chk, raw, true), Some(wavs.join("a.wav")));
        assert_eq!(
            find_map_wav(&chk, "staredit\\wav\\b.wav", false),
            Some(wavs.join("b.wav"))
        );
        assert_eq!(find_map_wav(&chk, "staredit\\wav\\c.wav", false), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn text_lifetimes_follow_the_retail_measurement() {
        // (visible chars, transmission ms, measured s) on the recordings.
        let samples = [
            (165, 0, 10.1),
            (44, 0, 4.0),
            (60, 0, 3.8),
            (112, 7_447, 7.5),
            (113, 4_920, 7.2),
            (14, 1_005, 4.0),
            (71, 4_531, 4.5),
            (56, 3_146, 4.0),
            (61, 3_956, 4.0),
        ];
        for (chars, ms, seen) in samples {
            let got = text_life_ms(ms, chars) as f64 / 1000.0;
            assert!(
                (got - seen).abs() <= 0.21,
                "{chars} chars, {ms} ms: {got} vs {seen}"
            );
        }
        assert_eq!(
            text_life_ms(20_000, 30),
            20_000,
            "long transmissions keep their length"
        );
        assert_eq!(text_chars("  RAYNOR\r\n     Howdy.\r\n"), 12);
    }

    #[test]
    fn a_blank_line_keeps_its_slot() {
        let tip = "TIP\r\n  You can select\r\n\r\nAlternatively\r\n";
        assert_eq!(
            text_lines(tip),
            vec!["TIP", "  You can select", "", "Alternatively"]
        );
        // Retail Wasteland at 90.5 s: the TIP (with its blank line) and the
        // SCV transmission are on screen together.
        let mut log = TextLog::default();
        log.push(tip, 0, 1_000);
        log.push("TERRAN SCV\r\na\r\nb\r\nc", 10, 1_000);
        assert_eq!(
            log.visible(20),
            vec![
                "TIP",
                "  You can select",
                "",
                "Alternatively",
                "TERRAN SCV",
                "a",
                "b",
                "c"
            ]
        );
    }

    #[test]
    fn the_log_expires_lines_and_keeps_the_newest() {
        let mut log = TextLog::default();
        log.push("one\r\ntwo", 0, 1_000);
        log.push("three", 500, 5_000);
        assert_eq!(log.visible(900), vec!["one", "two", "three"]);
        assert_eq!(log.visible(1_000), vec!["three"]);
        for i in 0..20 {
            log.push(&format!("l{i}"), 2_000, 10_000);
        }
        let v = log.visible(2_001);
        assert_eq!(v.len(), MAX_LINES);
        assert_eq!(v.last(), Some(&"l19"));
    }
}

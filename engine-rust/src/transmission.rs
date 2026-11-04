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

/// How long a Display Text Message stays on screen when the trigger gives
/// no duration.  INFERENCE (classic client behaviour, not measured here);
/// falsifier: time a retail Display Text line from appearance to fade.
pub const TEXT_MS: u64 = 7_000;
/// Lines visible at once above the console (oldest scroll away first).
pub const MAX_LINES: usize = 8;

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
        for line in clean_lines(raw) {
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
    fn the_log_expires_lines_and_keeps_the_newest() {
        let mut log = TextLog::default();
        log.push("one\r\ntwo", 0, 1_000);
        log.push("three", 500, 5_000);
        assert_eq!(log.visible(900), vec!["one", "two", "three"]);
        assert_eq!(log.visible(1_000), vec!["three"]);
        for i in 0..20 {
            log.push(&format!("l{i}"), 2_000, TEXT_MS);
        }
        let v = log.visible(2_001);
        assert_eq!(v.len(), MAX_LINES);
        assert_eq!(v.last(), Some(&"l19"));
    }
}

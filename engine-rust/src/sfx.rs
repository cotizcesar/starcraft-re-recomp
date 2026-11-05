//! `arr\sfxdata.tbl` — the shipped sound-file name table.
//!
//! The first `u16` is the entry count, followed by that many absolute `u16`
//! byte offsets into the same file.  Each offset points at a NUL-terminated
//! DOS path such as `Terran\MARINE\TMaYes00.WAV`; the CASC path is the same
//! path lower-cased with the `sound/` prefix, which is exactly how the
//! materialiser locates the voice set in the player's installation.
//!
//! `arr\sfxdata.dat` ([`SfxData`]) is the sound-id table: its `SoundFile`
//! column names the `.tbl` string a sound id plays (1-based), which is how
//! the CEF `GLUSND_*` ids resolve to their WAVs ([`glusnd_path`]).  Unit
//! voices index the `.tbl` directly.

#[derive(Clone, Debug, Default)]
pub struct SfxTable {
    paths: Vec<String>,
}

impl SfxTable {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let count = u16::from_le_bytes([*bytes.first()?, *bytes.get(1)?]) as usize;
        if count == 0 || count > 8192 {
            return None;
        }
        let mut paths = Vec::with_capacity(count);
        for i in 0..count {
            let o = 2 + i * 2;
            let start = u16::from_le_bytes([*bytes.get(o)?, *bytes.get(o + 1)?]) as usize;
            let rest = bytes.get(start..)?;
            let end = rest.iter().position(|b| *b == 0)?;
            paths.push(String::from_utf8_lossy(&rest[..end]).into_owned());
        }
        Some(Self { paths })
    }

    pub fn bundled() -> Self {
        Self::parse(embedded!("fixtures/sfxdata.tbl")).expect("shipped sfxdata.tbl is well formed")
    }

    pub fn len(&self) -> usize {
        self.paths.len()
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Raw DOS path as stored in the table.
    pub fn dos_path(&self, id: u16) -> Option<&str> {
        self.paths.get(id as usize).map(String::as_str)
    }

    /// CASC path (`sound/terran/marine/tmayes00.wav`).
    pub fn casc_path(&self, id: u16) -> Option<String> {
        let raw = self.dos_path(id)?;
        let mut out = String::with_capacity(raw.len() + 6);
        out.push_str("sound/");
        for ch in raw.chars() {
            match ch {
                '\\' => out.push('/'),
                _ => out.push(ch.to_ascii_lowercase()),
            }
        }
        Some(out)
    }

    /// Relative cache path (`terran/marine/tmayes00.wav`).
    pub fn cache_path(&self, id: u16) -> Option<String> {
        self.casc_path(id)
            .and_then(|p| p.strip_prefix("sound/").map(str::to_string))
    }

    /// Voice folder (`terran/marine`) used by the client's unit voice sets.
    pub fn folder(&self, id: u16) -> Option<String> {
        let path = self.cache_path(id)?;
        let mut parts = path.split('/');
        let race = parts.next()?;
        let unit = parts.next()?;
        // Only the three race voice trees are unit folders; `misc/...`,
        // `bullet/...` and `glue/...` are standalone effects.
        if !matches!(race, "terran" | "zerg" | "protoss") || parts.next().is_none() {
            return None;
        }
        Some(format!("{race}/{unit}"))
    }

    /// All folders, lower-cased and de-duplicated, in table order.
    pub fn folders(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for id in 0..self.paths.len() as u16 {
            if let Some(folder) = self.folder(id)
                && !out.contains(&folder)
            {
                out.push(folder);
            }
        }
        out
    }
}

/// `arr\sfxdata.dat`: 1 144 sounds stored column by column (`u32 SoundFile`,
/// `u8 unknown`, `u8 flags`, `u16 race`, `u8 volume`; 9 bytes per sound).
/// The row is the engine's sound id (`SND_*` / `GLUSND_*`); `SoundFile` is a
/// **1-based** `sfxdata.tbl` string number (0 = no file).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SfxData {
    pub sound_file: Vec<u32>,
    pub flags: Vec<u8>,
    pub race: Vec<u16>,
    pub volume: Vec<u8>,
}

impl SfxData {
    pub const SOUNDS: usize = 1144;

    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let n = Self::SOUNDS;
        if bytes.len() != n * 9 {
            return None;
        }
        let u32_at = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap_or([0; 4]));
        let race_at = 6 * n;
        Some(Self {
            sound_file: (0..n).map(|i| u32_at(4 * i)).collect(),
            flags: bytes[5 * n..6 * n].to_vec(),
            race: (0..n)
                .map(|i| u16::from_le_bytes([bytes[race_at + 2 * i], bytes[race_at + 2 * i + 1]]))
                .collect(),
            volume: bytes[8 * n..9 * n].to_vec(),
        })
    }

    pub fn bundled() -> Self {
        Self::parse(embedded!("fixtures/sfxdata.dat")).expect("shipped sfxdata.dat is well formed")
    }

    /// `sfxdata.tbl` index (0-based, as [`SfxTable`] takes it) of sound `id`.
    pub fn table_index(&self, id: u16) -> Option<u16> {
        let v = *self.sound_file.get(id as usize)?;
        u16::try_from(v.checked_sub(1)?).ok()
    }

    /// CASC path of sound `id` (`sound/glue/mousedown2.wav`).
    pub fn casc_path(&self, table: &SfxTable, id: u16) -> Option<String> {
        table.casc_path(self.table_index(id)?)
    }
}

/// Sound ids of the CEF front-end (`GLUSND_*`), as the shipped
/// `webui/dist/*/*.js` enum numbers them (`{GLUSND_MOUSE_DOWN:165}` …
/// `{GLUSND_SWISH_OUT:173}`, after the `SND_*` entries).  The page sends
/// `PlaySound {id: "GLUSND_…"}`; the name's number is an `sfxdata.dat` row.
pub const GLUSND: [(&str, u16); 9] = [
    ("GLUSND_MOUSE_DOWN", 165),
    ("GLUSND_MOUSE_OVER", 166),
    ("GLUSND_BNET_CLICK", 167),
    ("GLUSND_SCORE_FILL", 168),
    ("GLUSND_SCORE_FILLEND", 169),
    ("GLUSND_CHAT_COUNTDOWN", 170),
    ("GLUSND_SWISH_LOCK", 171),
    ("GLUSND_SWISH_IN", 172),
    ("GLUSND_SWISH_OUT", 173),
];

/// CASC WAV a `GLUSND_*` name plays.
pub fn glusnd_path(data: &SfxData, table: &SfxTable, name: &str) -> Option<String> {
    let id = GLUSND.iter().find(|(n, _)| *n == name)?.1;
    data.casc_path(table, id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_table_maps_ids_to_casc_voice_paths() {
        let table = SfxTable::bundled();
        assert_eq!(table.len(), 1127);
        // Marine ready / what / yes and Ghost ready, recovered from the
        // shipped names.  These are the ids the classic unit-sound fields use.
        assert_eq!(
            table.casc_path(263).as_deref(),
            Some("sound/terran/marine/tmardy00.wav")
        );
        assert_eq!(
            table.casc_path(275).as_deref(),
            Some("sound/terran/marine/tmawht00.wav")
        );
        assert_eq!(
            table.casc_path(279).as_deref(),
            Some("sound/terran/marine/tmayes00.wav")
        );
        assert_eq!(
            table.casc_path(213).as_deref(),
            Some("sound/terran/ghost/tghrdy00.wav")
        );
        assert_eq!(table.folder(263).as_deref(), Some("terran/marine"));
    }

    #[test]
    fn every_table_entry_is_a_sound_path_and_every_folder_is_known() {
        let table = SfxTable::bundled();
        for id in 0..table.len() as u16 {
            let path = table.casc_path(id).expect("path");
            assert!(path.starts_with("sound/"), "{id}: {path}");
            assert!(path.ends_with(".wav"), "{id}: {path}");
        }
        let folders = table.folders();
        assert_eq!(folders.len(), 65, "{folders:?}");
        assert!(folders.contains(&"terran/marine".to_string()));
        assert!(folders.contains(&"protoss/zealot".to_string()));
        assert!(folders.contains(&"zerg/zergling".to_string()));
    }

    #[test]
    fn sound_ids_resolve_through_the_1_based_sound_file_column() {
        let data = SfxData::bundled();
        let table = SfxTable::bundled();
        assert_eq!(data.sound_file.len(), SfxData::SOUNDS);
        assert_eq!(data.table_index(0), None, "sound 0 has no file");
        // The CEF enum's first entries pin the base: SND_Z_ERROR:1,
        // SND_T_ERROR:2, SND_P_ERROR:3 (drone, Terran buzz, Protoss error).
        let path = |id| data.casc_path(&table, id);
        assert_eq!(path(1).as_deref(), Some("sound/zerg/drone/zdrerr00.wav"));
        assert_eq!(path(2).as_deref(), Some("sound/misc/buzz.wav"));
        assert_eq!(path(3).as_deref(), Some("sound/misc/perror.wav"));
        // SND_T_NEED_POWER:154 / SND_P_NEED_POWER:155: the supply advisors.
        assert_eq!(
            path(154).as_deref(),
            Some("sound/terran/advisor/taderr02.wav")
        );
        assert_eq!(
            path(155).as_deref(),
            Some("sound/protoss/advisor/paderr02.wav")
        );
    }

    #[test]
    fn every_glusnd_id_names_its_glue_wav() {
        let data = SfxData::bundled();
        let table = SfxTable::bundled();
        let got: Vec<(&str, String)> = GLUSND
            .iter()
            .map(|(name, _)| (*name, glusnd_path(&data, &table, name).expect(name)))
            .collect();
        let want = [
            ("GLUSND_MOUSE_DOWN", "sound/glue/mousedown2.wav"),
            ("GLUSND_MOUSE_OVER", "sound/glue/mouseover.wav"),
            ("GLUSND_BNET_CLICK", "sound/glue/bnetclick.wav"),
            ("GLUSND_SCORE_FILL", "sound/glue/scorefill.wav"),
            ("GLUSND_SCORE_FILLEND", "sound/glue/scorefillend.wav"),
            ("GLUSND_CHAT_COUNTDOWN", "sound/glue/countdown.wav"),
            ("GLUSND_SWISH_LOCK", "sound/glue/swishlock.wav"),
            ("GLUSND_SWISH_IN", "sound/glue/swishin.wav"),
            ("GLUSND_SWISH_OUT", "sound/glue/swishout.wav"),
        ];
        for ((name, path), (wname, wpath)) in got.iter().zip(want) {
            assert_eq!((*name, path.as_str()), (wname, wpath));
        }
        assert_eq!(glusnd_path(&data, &table, "GLUSND_NOPE"), None);
        // The client's menu cues are exactly the two sounds the CEF pages
        // send: hover = MOUSE_OVER, press = MOUSE_DOWN.
        let cue = |name: &str| {
            crate::retail::SOUND_PATHS
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, c)| c[0])
        };
        assert_eq!(cue("ui_click"), Some(want[0].1));
        assert_eq!(cue("ui_move"), Some(want[1].1));
    }

    #[test]
    fn sfxdata_dat_is_column_major_and_rejects_other_sizes() {
        let bytes = embedded!("fixtures/sfxdata.dat");
        let data = SfxData::parse(bytes).expect("1144 x 9 bytes");
        assert!(data.volume.iter().any(|&v| v > 0));
        assert!(data.race.iter().all(|&r| r <= 4), "race 0..=4");
        assert_eq!(SfxData::parse(&bytes[..bytes.len() - 1]), None);
    }
}

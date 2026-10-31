//! `arr\sfxdata.tbl` — the shipped sound-file name table.
//!
//! The first `u16` is the entry count, followed by that many absolute `u16`
//! byte offsets into the same file.  Each offset points at a NUL-terminated
//! DOS path such as `Terran\MARINE\TMaYes00.WAV`; the CASC path is the same
//! path lower-cased with the `sound/` prefix, which is exactly how the
//! materialiser locates the voice set in the player's installation.
//!
//! `arr\sfxdata.dat` is the parallel 4-byte record table (`u16` sound id,
//! `u8` priority, `u8` flags).  The client does not need it to play a voice —
//! it is parsed and checked so the table pair can be validated against the
//! fixtures.

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
        Self::parse(include_bytes!("../fixtures/sfxdata.tbl"))
            .expect("shipped sfxdata.tbl is well formed")
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
            if let Some(folder) = self.folder(id) {
                if !out.contains(&folder) {
                    out.push(folder);
                }
            }
        }
        out
    }
}

/// Decode the parallel `sfxdata.dat` records.
pub fn parse_dat(bytes: &[u8]) -> Vec<(u16, u8, u8)> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|r| (u16::from_le_bytes([r[0], r[1]]), r[2], r[3]))
        .collect()
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
    fn the_parallel_record_table_has_one_record_per_identity_sound() {
        let records = parse_dat(include_bytes!("../fixtures/sfxdata.dat"));
        assert_eq!(records.len(), 2574);
        // The first 83 records are the identity map (index 83 repeats 82);
        // later records carry the shipped priority/flag variants.
        for (i, record) in records.iter().take(83).enumerate() {
            if i == 83 {
                continue;
            }
            assert_eq!(record, &(i as u16, 0, 0), "record {i}");
        }
        assert_eq!(records[83], (82, 0, 0));
        assert!(records
            .iter()
            .any(|(_, priority, flags)| *priority > 0 || *flags > 0));
    }
}

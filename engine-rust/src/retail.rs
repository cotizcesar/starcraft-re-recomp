//! Read-only access to the retail StarCraft installation.
//!
//! A decompilation project must not redistribute the game's art, audio, maps
//! or tables.  Instead this module opens the files that are already on the
//! player's machine:
//!
//! * `Data/data` + `Data/indices` — Blizzard's CASC storage (1.23 Remastered);
//! * `Maps/**/*.scm|scx` — the shipped MPQ map archives;
//! * classic `*.mpq` archives, when the user has a pre-Remastered install.
//!
//! Nothing in this file contains retail bytes.  `AssetBundle::materialize`
//! copies the subset the client needs into a local cache, which is derived
//! from the user's own installation exactly like a decompiler's extracted
//! resources.
//!
//! The CASC implementation follows the on-disk structures documented by
//! CascLib (MIT): a text `.build.info`/build config names the ENCODING and
//! ROOT manifests, local `%02x%08x.idx` files map encoded keys to
//! `data.NNN` offsets, and each file body is a BLTE stream (`N`, `Z`, `E`
//! frames).  StarCraft's ROOT manifest is the text `path|cKey` list used by
//! CascLib's `TRootHandler_SC1`.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use flate2::read::ZlibDecoder;

/// Errors from opening or decoding a retail installation.
#[derive(Debug)]
pub enum RetailError {
    Io(std::io::Error),
    Missing(String),
    Format(String),
    Unsupported(String),
}

impl std::fmt::Display for RetailError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RetailError::Io(e) => write!(f, "I/O error: {e}"),
            RetailError::Missing(s) => write!(f, "missing retail file: {s}"),
            RetailError::Format(s) => write!(f, "bad retail format: {s}"),
            RetailError::Unsupported(s) => write!(f, "unsupported retail format: {s}"),
        }
    }
}

impl std::error::Error for RetailError {}

impl From<std::io::Error> for RetailError {
    fn from(e: std::io::Error) -> Self {
        RetailError::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, RetailError>;

/// One file in the CASC index.
#[derive(Clone, Copy, Debug)]
struct CascLocation {
    archive: u16,
    offset: u64,
    encoded_size: u32,
}

#[derive(Clone, Debug)]
struct EncodingEntry {
    content_size: u32,
    encoded_keys: Vec<[u8; 16]>,
}

/// A map archive shipped under `Maps/`.
#[derive(Clone, Debug)]
pub struct RetailMap {
    pub path: PathBuf,
    pub relative: PathBuf,
    pub name: String,
}

/// Result of copying retail assets into a local cache.
#[derive(Clone, Debug, Default)]
pub struct AssetReport {
    pub files: usize,
    pub bytes: u64,
    pub maps: usize,
    pub campaign_maps: usize,
    pub briefings: usize,
    pub glue_files: usize,
    pub console_files: usize,
    pub ui_files: usize,
    pub music_files: usize,
    pub voice_files: usize,
    pub video_files: usize,
    pub missing_sounds: Vec<String>,
    pub missing_assets: Vec<String>,
}

/// Read-only view of a StarCraft installation.
pub struct RetailInstall {
    root: PathBuf,
    data_dir: PathBuf,
    index: HashMap<[u8; 9], CascLocation>,
    encoding: HashMap<[u8; 16], EncodingEntry>,
    paths: HashMap<String, [u8; 16]>,
    index_entries: usize,
    encoding_entries: usize,
    build_key: String,
    encoding_key: String,
    root_key: String,
    handles: std::cell::RefCell<HashMap<u16, File>>,
}

impl RetailInstall {
    /// Open `root`, which must contain `StarCraft.exe`'s `Data/` directory.
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        let data_dir = root.join("Data").join("data");
        if !data_dir.is_dir() {
            return Err(RetailError::Missing(data_dir.display().to_string()));
        }

        let build_key = read_build_key(&root)?;
        let config = read_build_config(&root, &build_key)?;
        let encoding_key = config
            .encoding_key
            .clone()
            .ok_or_else(|| RetailError::Format("build config has no encoding key".into()))?;
        let root_key = config
            .root_key
            .clone()
            .ok_or_else(|| RetailError::Format("build config has no root key".into()))?;

        let index = read_local_indexes(&root)?;
        let index_entries = index.len();
        let mut install = Self {
            root,
            data_dir,
            index,
            encoding: HashMap::new(),
            paths: HashMap::new(),
            index_entries,
            encoding_entries: 0,
            build_key,
            encoding_key: encoding_key.clone(),
            root_key: root_key.clone(),
            handles: std::cell::RefCell::new(HashMap::new()),
        };

        // The ENCODING manifest is addressed directly by the encoded key in
        // the build config.  Older configs only carry a content key, in which
        // case the content key doubles as the local encoded key.
        let encoding_bytes = install
            .read_encoded(&encoding_key)
            .or_else(|_| install.read_encoded(&root_key))
            .or_else(|_| install.find_encoding_manifest())?;
        install.parse_encoding(&encoding_bytes)?;
        install.encoding_entries = install.encoding.len();

        let root_bytes = install.read_by_ckey(&root_key).or_else(|_| {
            // Some configs carry only the encoded key for ROOT.
            install.read_encoded(&root_key)
        })?;
        install.parse_root(&root_bytes)?;

        Ok(install)
    }

    /// Open `$SC_INSTALL` or the default Wine/Battle.net location.
    pub fn from_env() -> Option<Self> {
        for var in ["SC_INSTALL", "GALAXYCRAFT_INSTALL", "STARCRAFT_HOME"] {
            if let Ok(path) = std::env::var(var) {
                if let Ok(install) = Self::open(path) {
                    return Some(install);
                }
            }
        }
        let mut candidates = vec![
            PathBuf::from("/opt/StarCraft"),
            PathBuf::from("/usr/local/games/StarCraft"),
        ];
        if let Ok(home) = std::env::var("HOME") {
            let home = PathBuf::from(home);
            candidates.extend([
                home.join("Games/battlenet/drive_c/Program Files (x86)/StarCraft"),
                home.join(".wine/drive_c/Program Files (x86)/StarCraft"),
                home.join(".wine/drive_c/Program Files/StarCraft"),
                home.join("StarCraft"),
            ]);
        }
        for path in candidates {
            if path.join("Data").join("data").is_dir() {
                if let Ok(install) = Self::open(&path) {
                    return Some(install);
                }
            }
        }
        None
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn build_key(&self) -> &str {
        &self.build_key
    }

    pub fn encoding_key(&self) -> &str {
        &self.encoding_key
    }

    pub fn root_key(&self) -> &str {
        &self.root_key
    }

    /// Number of `path|cKey` records in the ROOT manifest.
    pub fn path_count(&self) -> usize {
        self.paths.len()
    }

    /// Number of `data.NNN.idx` entries.
    pub fn index_count(&self) -> usize {
        self.index_entries
    }

    /// Number of CKey→EKey records in the ENCODING manifest.
    pub fn encoding_count(&self) -> usize {
        self.encoding_entries
    }

    /// Return the contents of a CASC path.  Separators are normalised, so
    /// `TileSet/platform.vf4` and `TileSet\\platform.vf4` both work.
    pub fn read(&self, path: &str) -> Option<Vec<u8>> {
        self.read_by_path(path).ok()
    }

    /// Does the installation contain `path`?
    pub fn contains(&self, path: &str) -> bool {
        self.paths.contains_key(&normalise(path))
    }

    pub fn read_by_path(&self, path: &str) -> Result<Vec<u8>> {
        let key = normalise(path);
        let ckey = self
            .paths
            .get(&key)
            .copied()
            .ok_or_else(|| RetailError::Missing(path.to_string()))?;
        self.read_by_ckey_bytes(&ckey)
    }

    fn read_by_ckey(&self, ckey: &str) -> Result<Vec<u8>> {
        let bytes = hex16(ckey).ok_or_else(|| RetailError::Format("bad CKey".into()))?;
        self.read_by_ckey_bytes(&bytes)
    }

    fn read_by_ckey_bytes(&self, ckey: &[u8; 16]) -> Result<Vec<u8>> {
        let entry = self
            .encoding
            .get(ckey)
            .ok_or_else(|| RetailError::Missing(format!("CKey {}", hex(ckey))))?;
        let mut last = None;
        for ekey in &entry.encoded_keys {
            match self.read_encoded_bytes(ekey) {
                Ok(data) => return Ok(data),
                Err(e) => last = Some(e),
            }
        }
        Err(last.unwrap_or_else(|| RetailError::Missing("no encoded key".into())))
    }

    fn read_encoded(&self, ekey: &str) -> Result<Vec<u8>> {
        let bytes = hex16(ekey).ok_or_else(|| RetailError::Format("bad EKey".into()))?;
        self.read_encoded_bytes(&bytes)
    }

    fn read_encoded_bytes(&self, ekey: &[u8; 16]) -> Result<Vec<u8>> {
        let mut prefix = [0u8; 9];
        prefix.copy_from_slice(&ekey[..9]);
        let loc = self
            .index
            .get(&prefix)
            .copied()
            .ok_or_else(|| RetailError::Missing(format!("EKey {}", hex(ekey))))?;
        let mut raw = vec![0u8; loc.encoded_size as usize];
        let archive = self.data_dir.join(format!("data.{:03}", loc.archive));
        let mut handles = self.handles.borrow_mut();
        let file = match handles.get_mut(&loc.archive) {
            Some(f) => f,
            None => {
                let f = File::open(&archive)?;
                handles.insert(loc.archive, f);
                handles.get_mut(&loc.archive).unwrap()
            }
        };
        file.seek(SeekFrom::Start(loc.offset))?;
        file.read_exact(&mut raw)?;
        blte_decode(&raw)
    }

    /// Fallback for old build configs that do not name the ENCODING EKey.
    /// Scan the archives for a BLTE stream whose plaintext starts with `EN`.
    fn find_encoding_manifest(&self) -> Result<Vec<u8>> {
        let mut entries: Vec<_> = fs::read_dir(&self.data_dir)?
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.starts_with("data."))
            })
            .collect();
        entries.sort();
        for path in entries {
            let data = fs::read(&path)?;
            let mut from = 0usize;
            while let Some(pos) = find_bytes(&data[from..], b"BLTE").map(|p| p + from) {
                let end = (pos + 4 * 1024 * 1024).min(data.len());
                if let Ok(decoded) = blte_decode(&data[pos..end]) {
                    if decoded.len() >= 2 && &decoded[..2] == b"EN" {
                        return Ok(decoded);
                    }
                }
                from = pos + 4;
            }
        }
        Err(RetailError::Missing("ENCODING manifest".into()))
    }

    fn parse_encoding(&mut self, data: &[u8]) -> Result<()> {
        if data.len() < 22 || &data[..2] != b"EN" || data[2] != 1 {
            return Err(RetailError::Format("ENCODING header".into()));
        }
        let ckey_len = data[3] as usize;
        let ekey_len = data[4] as usize;
        if ckey_len != 16 || ekey_len != 16 {
            return Err(RetailError::Unsupported(format!(
                "CKey/EKey lengths {ckey_len}/{ekey_len}"
            )));
        }
        let ckey_page_size = be16(&data[5..7]) as usize * 1024;
        let ckey_pages = be32(&data[9..13]) as usize;
        let espec_size = be32(&data[18..22]) as usize;
        let page_table = 22usize + espec_size;
        let bytes: usize = page_table
            .checked_add(ckey_pages * 32)
            .ok_or_else(|| RetailError::Format("ENCODING size".into()))?;
        if bytes + ckey_pages * ckey_page_size > data.len() {
            return Err(RetailError::Format("ENCODING pages out of range".into()));
        }
        let mut offset = bytes;
        for _ in 0..ckey_pages {
            let page = &data[offset..offset + ckey_page_size];
            offset += ckey_page_size;
            let mut p = 0usize;
            while p + 22 <= page.len() {
                let count = u16::from_le_bytes([page[p], page[p + 1]]) as usize;
                p += 2;
                if count == 0 {
                    break;
                }
                let content_size = be32(&page[p..p + 4]);
                p += 4;
                if p + 16 + count * 16 > page.len() {
                    return Err(RetailError::Format("ENCODING entry out of range".into()));
                }
                let mut ckey = [0u8; 16];
                ckey.copy_from_slice(&page[p..p + 16]);
                p += 16;
                let mut encoded_keys = Vec::with_capacity(count);
                for _ in 0..count {
                    let mut ekey = [0u8; 16];
                    ekey.copy_from_slice(&page[p..p + 16]);
                    p += 16;
                    encoded_keys.push(ekey);
                }
                self.encoding.insert(
                    ckey,
                    EncodingEntry {
                        content_size,
                        encoded_keys,
                    },
                );
            }
        }
        Ok(())
    }

    fn parse_root(&mut self, data: &[u8]) -> Result<()> {
        let text = String::from_utf8_lossy(data);
        for line in text.lines() {
            let line = line.trim_end_matches('\r');
            let Some((path, key)) = line.rsplit_once('|') else {
                continue;
            };
            let Some(ckey) = hex16(key.trim()) else {
                continue;
            };
            self.paths.insert(normalise(path), ckey);
        }
        if self.paths.is_empty() {
            return Err(RetailError::Format("empty ROOT manifest".into()));
        }
        Ok(())
    }

    /// Content size recorded in the ENCODING manifest.
    pub fn content_size(&self, path: &str) -> Option<u32> {
        let ckey = self.paths.get(&normalise(path))?;
        self.encoding.get(ckey).map(|e| e.content_size)
    }

    /// Paths under `prefix` with `suffix` (case-insensitive), sorted.
    pub fn list(&self, prefix: &str, suffix: &str) -> Vec<String> {
        let prefix = normalise(prefix);
        let suffix = suffix.to_ascii_lowercase();
        let mut out: Vec<String> = self
            .paths
            .keys()
            .filter(|p| p.starts_with(&prefix) && p.ends_with(&suffix))
            .cloned()
            .collect();
        out.sort();
        out
    }

    /// Every `.scm`/`.scx` archive in the installation's `Maps/` tree.
    pub fn maps(&self) -> Vec<RetailMap> {
        let mut out = Vec::new();
        let maps = self.root.join("Maps");
        walk(&maps, &mut |path| {
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if ext != "scm" && ext != "scx" {
                return;
            }
            let relative = path.strip_prefix(&maps).unwrap_or(path).to_path_buf();
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("map")
                .to_string();
            out.push(RetailMap {
                path: path.to_path_buf(),
                relative,
                name,
            });
        });
        out.sort_by(|a, b| a.relative.cmp(&b.relative));
        out
    }

    /// Extract `staredit\scenario.chk` from a shipped MPQ map.
    pub fn read_map(&self, map: &RetailMap) -> Result<Vec<u8>> {
        let archive = MpqArchive::open(&map.path)?;
        archive
            .read_file("staredit\\scenario.chk")
            .ok_or_else(|| RetailError::Missing(format!("{}: scenario.chk", map.path.display())))
    }

    /// Load the rules tables, scripts and string table from CASC.
    pub fn game_data(&self) -> Result<crate::data::GameData> {
        let read = |p: &str| {
            self.read_by_path(p)
                .map_err(|e| RetailError::Missing(e.to_string()))
        };
        let mut data = crate::data::GameData::parse_all(
            &read("arr/units.dat")?,
            &read("arr/weapons.dat")?,
            &read("arr/flingy.dat")?,
            &read("arr/orders.dat")?,
            &read("arr/techdata.dat")?,
            &read("arr/upgrades.dat")?,
            &read("arr/sprites.dat")?,
            &read("arr/images.dat")?,
            &read("rez/stat_txt.tbl")?,
        )
        .map_err(|e| RetailError::Format(e.to_string()))?;
        data.iscript = crate::iscript::Iscript::parse(&read("scripts/iscript.bin")?)
            .map_err(|e| RetailError::Format(e.to_string()))?;
        data.aiscripts = crate::aiscript::parse_aiscript(&read("scripts/aiscript.bin")?);
        data.bwscripts = crate::aiscript::parse_bwscript(&read("scripts/bwscript.bin")?);
        Ok(data)
    }

    /// Default per-install cache directory.
    pub fn default_cache_dir(&self) -> PathBuf {
        if let Ok(path) = std::env::var("GALAXYCRAFT_CACHE") {
            return PathBuf::from(path);
        }
        let base = std::env::var("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".cache")))
            .unwrap_or_else(|_| PathBuf::from("/tmp"));
        let tag = &self.build_key[..self.build_key.len().min(8)];
        base.join("galaxycraft").join(format!("retail-{tag}"))
    }

    /// Materialise once, guarded by a completion marker.
    pub fn ensure_cache(&self, dest: &Path) -> Result<AssetReport> {
        let marker = dest.join(".complete");
        let want =
            b"galaxycraft retail cache v14 (objectives/tips dialogs, hd console dlg, hd hud art, minimap layout, hd terrain, consoles, menus, cinematics, fonts, ui layouts, campaign console, palcs backdrop, phist popup, campaign prompt, glue small font, create variants, chat + toon registry)\n";
        if fs::read(&marker)
            .map(|current| current == want)
            .unwrap_or(false)
        {
            return Ok(AssetReport::default());
        }
        let report = self.materialize(dest)?;
        fs::write(marker, want)?;
        Ok(report)
    }

    /// Copy the asset subset used by the client into `dest`:
    ///
    /// ```text
    /// dest/TileSet/<era>.{vf4,cv5,vx4ex,vr4,wpe}
    /// dest/unit/<race>/<sprite>.grp   + dest/unit.pal
    /// dest/sound/<effect>.wav
    /// dest/maps/<relative map>.chk
    /// ```
    pub fn materialize(&self, dest: &Path) -> Result<AssetReport> {
        let mut report = AssetReport::default();
        let tiles = dest.join("TileSet");
        fs::create_dir_all(&tiles)?;
        for era in [
            "badlands", "platform", "install", "ashworld", "jungle", "desert", "ice", "twilight",
        ] {
            for ext in ["vf4", "cv5", "vx4ex", "vr4", "wpe"] {
                let name = format!("TileSet/{era}.{ext}");
                if let Ok(bytes) = self.read_by_path(&name) {
                    let out = tiles.join(format!("{era}.{ext}"));
                    write_file(&out, &bytes)?;
                    report.files += 1;
                    report.bytes += bytes.len() as u64;
                }
            }
            // 4x HD megatiles (128 px per tile): what retail draws the
            // terrain from at 1440p.
            let hd = format!("tileset/{era}.dds.vr4");
            if let Ok(bytes) = self.read_by_path(&hd) {
                write_file(&dest.join("hd").join(&hd), &bytes)?;
                report.files += 1;
                report.bytes += bytes.len() as u64;
            }
        }

        let unit = dest.join("unit");
        for rel in SPRITE_PATHS {
            if let Ok(bytes) = self.read_by_path(&format!("unit/{rel}")) {
                let out = unit.join(rel);
                write_file(&out, &bytes)?;
                report.files += 1;
                report.bytes += bytes.len() as u64;
            }
        }

        // The retail unit palette is not a standalone CASC file in the 1.23
        // storage; the Remastered client keeps it in the unit PCX.  Old
        // installs may expose `units.pal` directly.
        let palette = ["units.pal", "unit.pal", "arr/units.pal"]
            .iter()
            .find_map(|p| self.read(p))
            .or_else(|| {
                self.read("game/tunit.pcx")
                    .and_then(|pcx| pcx_palette(&pcx))
            });
        if let Some(bytes) = palette {
            write_file(&dest.join("unit.pal"), &bytes)?;
            report.files += 1;
            report.bytes += bytes.len() as u64;
        }

        let sound_dir = dest.join("sound");
        fs::create_dir_all(&sound_dir)?;
        for (name, candidates) in SOUND_PATHS {
            let mut found = false;
            for path in *candidates {
                if let Ok(bytes) = self.read_by_path(path) {
                    write_file(&sound_dir.join(format!("{name}.wav")), &bytes)?;
                    report.files += 1;
                    report.bytes += bytes.len() as u64;
                    found = true;
                    break;
                }
            }
            if !found {
                report.missing_sounds.push((*name).to_string());
            }
        }

        let map_dir = dest.join("maps");
        for map in self.maps() {
            let rel = map.relative.with_extension("chk");
            let out = map_dir.join(rel);
            match self.read_map(&map) {
                Ok(chk) => {
                    write_file(&out, &chk)?;
                    report.maps += 1;
                    report.bytes += chk.len() as u64;
                }
                Err(e) => return Err(RetailError::Format(format!("{}: {e}", map.path.display()))),
            }
        }

        // Single-player campaign: the missions are loose CASC CHKs under
        // `sd/campaign/`, not MPQ archives.  Copy them with a manifest so the
        // client can present the retail episode order, and copy the briefing
        // scripts plus the backdrop art they name.
        let campaign_dir = dest.join("campaign");
        let mut manifest = String::from(
            "episode\tepisode_name\torder\tkey\tname\trace\tchk\tbriefing\tbackground\n",
        );
        for episode in crate::campaign::episodes(self) {
            for mission in &episode.missions {
                let Ok(raw) = self.read_by_path(&mission.chk_path) else {
                    continue;
                };
                let rel = format!("{}/{}.chk", episode.key, mission.key);
                let out = campaign_dir.join(&rel);
                write_file(&out, &raw)?;
                report.campaign_maps += 1;
                report.bytes += raw.len() as u64;

                let mut briefing_rel = String::new();
                let mut background_rel = String::new();
                if let Some(script) = crate::campaign::load_briefing(self, mission) {
                    if let Some(key) = &mission.briefing_key {
                        let text_path = format!("briefing/{key}.txt");
                        if let Some(bytes) =
                            self.read(&format!("locales/enus/assets/rez/{key}.txt"))
                        {
                            write_file(&dest.join(&text_path), &bytes)?;
                            report.briefings += 1;
                            report.bytes += bytes.len() as u64;
                        }
                        briefing_rel = text_path;
                    }
                    // Every path the script references (backdrop, font) is
                    // copied under `glue/` so the client never needs CASC at
                    // play time.  Video intermissions are recorded but not
                    // copied: the .webm files are large and the client has no
                    // video decoder.
                    for page in &script.pages {
                        for path in [&page.background, &page.font].into_iter().flatten() {
                            // `path` is already the CASC-relative name
                            // (`glue/palta/terrana.dds`), so it lands under
                            // `cache/glue/...` and the manifest can serve it.
                            if page.background.as_deref() == Some(path.as_str()) {
                                background_rel = path.clone();
                            }
                            let out = dest.join(path);
                            if out.is_file() {
                                continue;
                            }
                            if let Ok(bytes) = self.read_by_path(path) {
                                write_file(&out, &bytes)?;
                                report.glue_files += 1;
                                report.bytes += bytes.len() as u64;
                            }
                        }
                    }
                }
                let race = match mission.race {
                    crate::sim::Race::Zerg => "zerg",
                    crate::sim::Race::Protoss => "protoss",
                    _ => "terran",
                };
                let name = mission.name.replace(['\t', '\n'], " ");
                manifest.push_str(&format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    episode.key,
                    episode.name,
                    mission.order,
                    mission.key,
                    name,
                    race,
                    rel,
                    briefing_rel,
                    background_rel
                ));
            }
        }
        write_file(&campaign_dir.join("manifest.tsv"), manifest.as_bytes())?;
        report.files += report.campaign_maps + report.briefings + report.glue_files;

        // Retail WebM cinematics: the title intros plus every briefing
        // intermission (`interstitials/*.webm`) and campaign disk loop the
        // `est*.txt` scripts name.  Headers are parsed with the dependency-
        // free `video::parse_webm` front-end; the client decodes poster
        // frames with the system ffmpeg on the title/briefing screens.
        // Full-motion playback outside those screens stays out of scope.
        {
            let video_dir = dest.join("videos");
            let mut vmanifest =
                String::from("rel\tsize\tcodec\twidth\theight\tduration_ms\tclusters\thas_audio\n");
            let mut videos: Vec<String> = Vec::new();
            for rel in [
                crate::video::BLIZZARD_LOGO,
                crate::video::INTRO_ORIGINAL,
                crate::video::INTRO_EXPANSION,
            ] {
                videos.push(rel.to_string());
            }
            // Every campaign cinematic in the executable's intro table
            // (`smk\blizzard.webm`, `smk\starintr.webm`, `smk\PreTerran_1.webm`
            // … `smk\xZerg.webm`, recovered with REA at 0x140ae7ae0).
            for rel in self.list("smk/", ".webm") {
                let wanted = rel.rsplit('/').next().is_some_and(|name| {
                    name.contains("blizzard")
                        || crate::video::CAMPAIGN_CINEMATICS
                            .iter()
                            .any(|c| c.eq_ignore_ascii_case(&rel))
                });
                if wanted && !videos.iter().any(|v| v.eq_ignore_ascii_case(&rel)) {
                    videos.push(rel);
                }
            }
            for rel in self.list("interstitials/", ".webm") {
                if !videos.contains(&rel) {
                    videos.push(rel);
                }
            }
            for rel in self.list("glue/campaign/", ".webm") {
                if !videos.contains(&rel) {
                    videos.push(rel);
                }
            }
            for rel in self.list("glue/expcampaign/", ".webm") {
                if !videos.contains(&rel) {
                    videos.push(rel);
                }
            }
            // HD menu-button animations (the SD `glue/mainmenu` clips are four
            // times the pixels); drawn with their alpha at the `flc.json`
            // rectangles.
            for rel in self.list("hd2/glue/mainmenu/", ".webm") {
                if let Some(name) = rel.rsplit('/').next() {
                    let dst = format!("hd2/mainmenu/{name}");
                    if let Ok(bytes) = self.read_by_path(&rel) {
                        let out = video_dir.join(&dst);
                        if write_file(&out, &bytes).is_ok() {
                            report.video_files += 1;
                            report.files += 1;
                            report.bytes += bytes.len() as u64;
                            if let Ok(info) = crate::video::parse_webm(&bytes) {
                                vmanifest.push_str(&format!(
                                    "{dst}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                                    bytes.len(),
                                    info.codec,
                                    info.pixel_width,
                                    info.pixel_height,
                                    info.duration_ms.map(|d| d.to_string()).unwrap_or_default(),
                                    info.clusters,
                                    info.has_audio,
                                ));
                            }
                        }
                    }
                }
            }
            videos.sort();
            for rel in &videos {
                let Ok(bytes) = self.read_by_path(rel) else {
                    report.missing_assets.push(rel.clone());
                    continue;
                };
                let out = video_dir.join(rel);
                if write_file(&out, &bytes).is_err() {
                    report.missing_assets.push(rel.clone());
                    continue;
                }
                report.video_files += 1;
                report.files += 1;
                report.bytes += bytes.len() as u64;
                match crate::video::parse_webm(&bytes) {
                    Ok(info) => {
                        vmanifest.push_str(&format!(
                            "{rel}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                            bytes.len(),
                            info.codec,
                            info.pixel_width,
                            info.pixel_height,
                            info.duration_ms.map(|d| d.to_string()).unwrap_or_default(),
                            info.clusters,
                            info.has_audio,
                        ));
                    }
                    Err(e) => {
                        vmanifest.push_str(&format!(
                            "{rel}\t{}\tERR:{e}\t0\t0\t\t0\tfalse\n",
                            bytes.len()
                        ));
                    }
                }
            }
            write_file(&video_dir.join("manifest.tsv"), vmanifest.as_bytes())?;
        }

        // Remastered front-end fonts, the `flc.json` video rectangles and the
        // screen dialogs the menu uses.  All are read from the player's own
        // installation and never committed.
        for src in [
            "webui/dist/lib/fonts/eurostileext-reg.otf",
            "webui/dist/lib/fonts/eurostile-reg.otf",
            "webui/dist/lib/fonts/eurostile-reg.woff",
            "font/eurostileext-reg.ttf",
            "font/eurostile-reg.ttf",
            "font/blizzard-regular.ttf",
            // BlizzardGlobal: the glue `RC_SMALLTEXT` and plain button face.
            "font/bl.ttf",
        ] {
            let dst = dest.join(src);
            if dst.is_file() {
                continue;
            }
            if let Ok(bytes) = self.read_by_path(src) {
                write_file(&dst, &bytes)?;
                report.files += 1;
                report.bytes += bytes.len() as u64;
            }
        }
        if let Ok(bytes) = self.read_by_path("rez/flc.json") {
            write_file(&dest.join("ui/flc.json"), &bytes)?;
            report.files += 1;
            report.bytes += bytes.len() as u64;
        }

        // Table fixtures are useful for offline runs; they are also read
        // from the installation, never copied into the source tree.
        let table_dir = dest.join("tables");
        for path in [
            "arr/units.dat",
            "arr/weapons.dat",
            "arr/flingy.dat",
            "arr/orders.dat",
            "arr/techdata.dat",
            "arr/upgrades.dat",
            "arr/sprites.dat",
            "arr/images.dat",
            "rez/stat_txt.tbl",
            "scripts/iscript.bin",
            "scripts/aiscript.bin",
            "scripts/bwscript.bin",
        ] {
            if let Ok(bytes) = self.read_by_path(path) {
                let out = table_dir.join(path.rsplit('/').next().unwrap());
                write_file(&out, &bytes)?;
                report.files += 1;
                report.bytes += bytes.len() as u64;
            }
        }

        // ------------------------------------------------------------------
        // Retail console, menu, score, music and voice assets.
        // ------------------------------------------------------------------
        let copy = |src: &str, dst: &std::path::Path, report: &mut AssetReport| -> bool {
            let Ok(bytes) = self.read_by_path(src) else {
                report.missing_assets.push(src.to_string());
                return false;
            };
            match write_file(dst, &bytes) {
                Ok(()) => {
                    report.files += 1;
                    report.bytes += bytes.len() as u64;
                    true
                }
                Err(_) => {
                    report.missing_assets.push(src.to_string());
                    false
                }
            }
        };

        for race in ["terran", "zerg", "protoss"] {
            for file in CONSOLE_FILES {
                let src = format!("game/consoles/{race}/{file}");
                let dst = dest.join(&src);
                if copy(&src, &dst, &mut report) {
                    report.console_files += 1;
                }
            }
        }
        for file in ["wirefram.grp", "grpwire.grp", "tranwire.grp"] {
            let src = format!("unit/wirefram/{file}");
            let dst = dest.join(&src);
            if copy(&src, &dst, &mut report) {
                report.console_files += 1;
            }
        }
        let src = "unit/cmdicons/cmdicons.grp";
        let dst = dest.join(src);
        if copy(src, &dst, &mut report) {
            report.console_files += 1;
        }

        for src in GLUE_ART {
            let dst = dest.join(src);
            if copy(src, &dst, &mut report) {
                report.glue_files += 1;
            }
        }

        let ui_dir = dest.join("ui");
        fs::create_dir_all(&ui_dir)?;
        for name in crate::ui::UI_JSON {
            let src = format!("locales/enus/assets/rez/{name}.ui.json");
            let dst = ui_dir.join(format!("{name}.ui.json"));
            if copy(&src, &dst, &mut report) {
                report.ui_files += 1;
            }
        }

        let music_dir = dest.join("music");
        fs::create_dir_all(&music_dir)?;
        for name in MUSIC_TRACKS {
            let src = format!("music/{name}.ogg");
            let dst = music_dir.join(format!("{name}.ogg"));
            if copy(&src, &dst, &mut report) {
                report.music_files += 1;
            }
        }

        // `sfxdata.tbl` names every voice file.  Copy the whole table so the
        // client has the complete voice sets, and keep the parallel record
        // table next to it for the differential test.
        let sfx_bytes = self.read("arr/sfxdata.tbl");
        if let Some(bytes) = &sfx_bytes {
            write_file(&dest.join("tables/sfxdata.tbl"), bytes)?;
            report.files += 1;
            report.bytes += bytes.len() as u64;
        }
        if let Some(bytes) = self.read("arr/sfxdata.dat") {
            write_file(&dest.join("tables/sfxdata.dat"), &bytes)?;
            report.files += 1;
            report.bytes += bytes.len() as u64;
        }
        if let Some(table) = sfx_bytes.as_deref().and_then(crate::sfx::SfxTable::parse) {
            let voices = dest.join("voices");
            let mut index = String::from("id\tpath\n");
            for id in 0..table.len() as u16 {
                let Some(rel) = table.cache_path(id) else {
                    continue;
                };
                let src = format!("sound/{rel}");
                let dst = voices.join(&rel);
                if copy(&src, &dst, &mut report) {
                    report.voice_files += 1;
                    index.push_str(&format!("{id}\t{rel}\n"));
                }
            }
            write_file(&voices.join("index.tsv"), index.as_bytes())?;
        }

        Ok(report)
    }
}

/// Unit GRPs used by the playable client (`render::SpriteBank::load`).
pub const SPRITE_PATHS: &[&str] = &[
    "terran/marine.grp",
    "terran/ghost.grp",
    "terran/vulture.grp",
    "terran/goliath.grp",
    "terran/tank.grp",
    "terran/scv.grp",
    "terran/wessel.grp",
    "terran/dropship.grp",
    "terran/battlecr.grp",
    "terran/spider.grp",
    "terran/firebat.grp",
    "terran/medic.grp",
    "zerg/zergling.grp",
    "zerg/hydra.grp",
    "zerg/ultra.grp",
    "zerg/brood.grp",
    "zerg/drone.grp",
    "zerg/overlord.grp",
    "zerg/mutalid.grp",
    "zerg/guardian.grp",
    "zerg/queen.grp",
    "zerg/defiler.grp",
    "zerg/cocoon.grp",
    "zerg/egg.grp",
    "zerg/larva.grp",
    "zerg/lurker.grp",
    "protoss/corsair.grp",
    "protoss/dtemplar.grp",
    "protoss/darchn.grp",
    "protoss/probe.grp",
    "protoss/zealot.grp",
    "protoss/dragoon.grp",
    "protoss/templar.grp",
    "protoss/archon.grp",
    "protoss/shuttle.grp",
    "protoss/scout.grp",
    "protoss/arbiter.grp",
    "protoss/carrier.grp",
    "protoss/intercep.grp",
    "protoss/trilob.grp",
    "protoss/witness.grp",
    "terran/depot.grp",
    "terran/refinery.grp",
    "terran/tbarrack.grp",
    "terran/academy.grp",
    "terran/factory.grp",
    "terran/starport.grp",
    "terran/missile.grp",
    "terran/pillbox.grp",
    "terran/comsat.grp",
    "terran/nukesilo.grp",
    "terran/control.grp",
    "terran/physics.grp",
    "terran/machines.grp",
    "neutral/starbase.grp",
    "protoss/nexus.grp",
    "protoss/gateway.grp",
    "protoss/robotic.grp",
    "protoss/stargate.grp",
    "protoss/pylon.grp",
    "protoss/assim.grp",
    "protoss/forge.grp",
    "protoss/gencore.grp",
    "protoss/citadel.grp",
    "protoss/archives.grp",
    "protoss/photon.grp",
    "protoss/sbattery.grp",
    "zerg/hatchery.grp",
    "zerg/lair.grp",
    "zerg/hive.grp",
    "zerg/extract.grp",
    "zerg/spire.grp",
    "zerg/nest.grp",
    "zerg/nyduspit.grp",
    "zerg/scolony.grp",
    "zerg/fcolony.grp",
    "neutral/min01.grp",
    "neutral/min02.grp",
    "neutral/min03.grp",
    "neutral/geyser.grp",
];

/// Console plates, command-button GRP and wireframes for one race.
///
/// `game/consoles/<race>` holds the three-part bottom console and its
/// overlays; `unit/wirefram/wirefram.grp` is the 64x64 portrait sheet indexed
/// by unit type.  The client draws these instead of its old custom bar.
pub const CONSOLE_FILES: &[&str] = &[
    "console_center.dds",
    "console_left.dds",
    "console_right.dds",
    "console_time.dds",
    "console_apm.dds",
    "conover.dds",
    "cmdbtns.grp",
    "cmdbtns.dds.grp",
    "dlg.grp",
    // HD console dialog sheet: minimap buttons (frames 0-5) and more.
    "dlg.dds.grp",
    "pbrempt.dds",
    "pbrfull.dds",
];

/// Main-menu, title and score-panel surfaces from the retail glue layer.
pub const GLUE_ART: &[&str] = &[
    // Remastered menu entries: animated WebM buttons (idle + hover) that
    // already carry their glow, so the client draws them instead of text.
    "glue/mainmenu/single.webm",
    "glue/mainmenu/singleon.webm",
    "glue/mainmenu/multi.webm",
    "glue/mainmenu/multion.webm",
    "glue/mainmenu/exit.webm",
    "glue/mainmenu/exiton.webm",
    "glue/mainmenu/editor.webm",
    "glue/mainmenu/editoron.webm",
    "glue/mainmenu/titleframe_bg.dds",
    "glue/mainmenu/titleframe_overlay.dds",
    "glue/mainmenu/pintro.dds",
    "glue/mainmenu/pcredit.dds",
    "glue/mainmenu/etail.dds",
    "glue/mainmenu/lock.dds",
    "glue/title/title.dds",
    // "Select Game Type" modal: metal frame, red hairline and both logos.
    "glue/palmm/retail_ex.dds",
    // Moon-landscape backdrop behind Registry / Saved Games / Create.
    "glue/palnl/backgnd.dds",
    // Front-end space backdrop (planet + red nebula) used behind the menu,
    // options, hotkeys and collections screens.
    "glue/palmm/backgnd.dds",
    // 4x HD in-game HUD art: resource icons, command-card glyphs and the
    // selection wireframes (retail draws them at 0.75 at 1440p).
    "game/icons.dds.grp",
    "unit/cmdicons/cmdicons.dds.grp",
    "unit/wirefram/wirefram.dds.grp",
    // Episode-select console plates (`glucmpgn.ui.json` ids 1/3), the
    // edge-anchored `palcs` backdrop halves and the mission-history popup
    // (`phist.dds` carries the modal frame, list box and Ok/Cancel buttons).
    "glue/campaign/pbotl.dds",
    "glue/campaign/pbotr.dds",
    "glue/campaign/ptopl.dds",
    "glue/campaign/ptopr.dds",
    "glue/campaign/phist.dds",
    // Locked-campaign prompt plate (`glupokcancel`).
    "glue/palcs/pdpopup.dds",
    "glue/palcs/backgndl.dds",
    "glue/palcs/backgndr.dds",
    // Create-game chrome: map-list plate, player slots, map-info column,
    // the shared two-button holder and the chat plate.
    "glue/create/plistmap.dds",
    "glue/create/plistmap2.dds",
    "glue/create/plistmaptall.dds",
    "glue/create/pslots.dds",
    "glue/create/pinfo.dds",
    "glue/create/twobtnholder.dds",
    "glue/chatroom/pchat.dds",
    // Toon Registry: backdrop, profile list plate and its popup.
    "glue/battlenet/toonregistry/backgnd.dds",
    "glue/battlenet/toonregistry/toonlistbg.dds",
    "glue/battlenet/toonregistry/popopup.dds",
    // Shared `palnl` Ok/Cancel plates (classic dialogs).
    "glue/palnl/pok.dds",
    "glue/palnl/pcancel.dds",
    // Credits panel frame (`creditsscreen.ui.json` id 2).
    "glue/battlenet/credits_a.dds",
    // Metal-room backdrop the credits scroll over.
    "glue/battlenet/backgnd.dds",
    "glue/scoretd/pmain.dds",
    "glue/scoretd/pinset.dds",
    "glue/scoretd/scorebox.dds",
    "glue/scoretd/untab.dds",
    "glue/scoretd/iscore.grp",
    "glue/scoretv/pmain.dds",
    "glue/scoretv/pinset.dds",
    "glue/scoretv/scorebox.dds",
    "glue/scoretv/untab.dds",
    "glue/scoretv/iscore.grp",
    "glue/scorepd/pmain.dds",
    "glue/scorepd/pinset.dds",
    "glue/scorepd/scorebox.dds",
    "glue/scorepd/untab.dds",
    "glue/scorepd/iscore.grp",
    "glue/scorepv/pmain.dds",
    "glue/scorepv/pinset.dds",
    "glue/scorepv/scorebox.dds",
    "glue/scorepv/untab.dds",
    "glue/scorepv/iscore.grp",
    "glue/scorezd/pmain.dds",
    "glue/scorezd/pinset.dds",
    "glue/scorezd/scorebox.dds",
    "glue/scorezd/untab.dds",
    "glue/scorezd/iscore.grp",
    "glue/scorezv/pmain.dds",
    "glue/scorezv/pinset.dds",
    "glue/scorezv/scorebox.dds",
    "glue/scorezv/untab.dds",
    "glue/scorezv/iscore.grp",
];

/// Remastered music tracks, shipped as Ogg Vorbis in `music/`.
pub const MUSIC_TRACKS: &[&str] = &[
    "title",
    "trdyroom",
    "terran1",
    "terran2",
    "terran3",
    "terran4",
    "tvict",
    "tdefeat",
    "zrdyroom",
    "zerg1",
    "zerg2",
    "zerg3",
    "zerg4",
    "zvict",
    "zdefeat",
    "prdyroom",
    "protoss1",
    "protoss2",
    "protoss3",
    "protoss4",
    "pvict",
    "pdefeat",
    "radiofreezerg",
];

/// Retail sources for the nine effects mixed by [`crate::audio`].
pub const SOUND_PATHS: &[(&str, &[&str])] = &[
    (
        "ui_click",
        &["sound/glue/mousedown2.wav", "setupdat/audio/mousedown.wav"],
    ),
    ("ui_move", &["sound/glue/mouseover.wav"]),
    ("ui_swish", &["sound/glue/swishin.wav"]),
    ("transmission", &["sound/misc/transmission.wav"]),
    ("terran_yes", &["sound/terran/marine/tmayes00.wav"]),
    ("zerg_yes", &["sound/zerg/drone/zdryes01.wav"]),
    ("protoss_yes", &["sound/protoss/probe/ppryes03.wav"]),
    ("victory", &["sound/misc/youwin.wav"]),
    ("defeat", &["sound/glue/scorefillend.wav"]),
];

// ---------------------------------------------------------------------------
// Build config / local index
// ---------------------------------------------------------------------------

struct BuildConfig {
    encoding_key: Option<String>,
    root_key: Option<String>,
}

fn read_build_key(root: &Path) -> Result<String> {
    let path = root.join(".build.info");
    let text = fs::read_to_string(&path)?;
    let mut lines = text.lines();
    let header = lines.next().unwrap_or_default();
    let columns: Vec<&str> = header.split('|').collect();
    let index = columns
        .iter()
        .position(|c| c.starts_with("Build Key"))
        .ok_or_else(|| RetailError::Format(".build.info has no Build Key column".into()))?;
    let row = lines
        .find(|l| !l.trim().is_empty())
        .ok_or_else(|| RetailError::Format(".build.info has no build row".into()))?;
    let value = row.split('|').nth(index).unwrap_or_default().trim();
    if value.len() != 32 || hex16(value).is_none() {
        return Err(RetailError::Format(".build.info build key".into()));
    }
    Ok(value.to_string())
}

fn read_build_config(root: &Path, build_key: &str) -> Result<BuildConfig> {
    let candidates = [
        root.join("Data/config")
            .join(&build_key[0..2])
            .join(&build_key[2..4])
            .join(build_key),
        root.join("Data/config")
            .join(&build_key[0..2])
            .join(&build_key[2..4])
            .join(build_key.to_ascii_lowercase()),
    ];
    let mut text = None;
    for path in candidates {
        if let Ok(t) = fs::read_to_string(&path) {
            text = Some(t);
            break;
        }
    }
    let text = match text {
        Some(t) => t,
        None => find_build_config_text(&root.join("Data/config"))?,
    };
    let mut encoding_key = None;
    let mut root_key = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        let name = name.trim();
        if name.starts_with("encoding") && !name.ends_with("-size") {
            let hashes = hex_tokens(value);
            encoding_key = hashes.get(1).or_else(|| hashes.first()).cloned();
        } else if name.starts_with("root") && !name.ends_with("-size") {
            let hashes = hex_tokens(value);
            root_key = hashes.first().cloned();
        }
    }
    Ok(BuildConfig {
        encoding_key,
        root_key,
    })
}

fn find_build_config_text(dir: &Path) -> Result<String> {
    let mut found = None;
    walk(dir, &mut |path| {
        if found.is_some() {
            return;
        }
        if let Ok(text) = fs::read_to_string(path) {
            if text.contains("encoding") && text.contains("root") {
                found = Some(text);
            }
        }
    });
    found.ok_or_else(|| RetailError::Missing("build configuration".into()))
}

fn read_local_indexes(root: &Path) -> Result<HashMap<[u8; 9], CascLocation>> {
    let dir = root.join("Data/data");
    let mut versions: HashMap<u8, (u32, PathBuf)> = HashMap::new();
    for entry in fs::read_dir(&dir)?.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !name.ends_with(".idx") || name.len() != 14 {
            continue;
        }
        let bucket = u8::from_str_radix(&name[0..2], 16).unwrap_or(0xff);
        let version = u32::from_str_radix(&name[2..10], 16).unwrap_or(0);
        if bucket > 0x0f {
            continue;
        }
        match versions.get(&bucket) {
            Some((v, _)) if *v >= version => {}
            _ => {
                versions.insert(bucket, (version, path));
            }
        }
    }
    let mut index = HashMap::new();
    for (_, (_, path)) in versions {
        parse_idx(&path, &mut index)?;
    }
    Ok(index)
}

fn parse_idx(path: &Path, out: &mut HashMap<[u8; 9], CascLocation>) -> Result<()> {
    let data = fs::read(path)?;
    if data.len() < 40 {
        return Err(RetailError::Format(format!(
            "{}: short idx",
            path.display()
        )));
    }
    // Guarded header block (8 bytes) followed by FILE_INDEX_HEADER_V2.
    if le32(&data[0..4]) != 16 {
        return Err(RetailError::Unsupported(format!(
            "{}: index header v1",
            path.display()
        )));
    }
    let revision = u16::from_le_bytes([data[8], data[9]]);
    if revision != 7 {
        return Err(RetailError::Unsupported(format!(
            "{}: index revision {revision}",
            path.display()
        )));
    }
    let span_size = data[12] as usize;
    let span_offset = data[13] as usize;
    let key_bytes = data[14] as usize;
    let segment_bits = data[15] as u32;
    if span_size != 4 || span_offset != 5 || key_bytes != 9 {
        return Err(RetailError::Format(format!(
            "{}: unexpected index field sizes",
            path.display()
        )));
    }
    let block_size = le32(&data[32..36]) as usize;
    let entry_len = key_bytes + span_offset + span_size;
    if entry_len != 18 || 40 + block_size > data.len() {
        return Err(RetailError::Format(format!(
            "{}: bad entry block",
            path.display()
        )));
    }
    let mut offset = 40;
    for _ in 0..block_size / entry_len {
        let mut key = [0u8; 9];
        key.copy_from_slice(&data[offset..offset + 9]);
        offset += 9;
        let field = &data[offset..offset + 5];
        offset += 5;
        let mut raw = 0u64;
        for b in field {
            raw = (raw << 8) | *b as u64;
        }
        let size = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
        offset += 4;
        let archive = (raw >> segment_bits) as u16;
        let file_offset = raw & ((1u64 << segment_bits) - 1);
        out.insert(
            key,
            CascLocation {
                archive,
                offset: file_offset,
                encoded_size: size,
            },
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// BLTE
// ---------------------------------------------------------------------------

fn blte_decode(raw: &[u8]) -> Result<Vec<u8>> {
    let start = if raw.len() >= 4 && &raw[..4] == b"BLTE" {
        0
    } else {
        find_bytes(raw, b"BLTE").ok_or_else(|| RetailError::Format("no BLTE header".into()))?
    };
    let data = &raw[start..];
    if data.len() < 8 {
        return Err(RetailError::Format("short BLTE header".into()));
    }
    let header_size = be32(&data[4..8]) as usize;
    let mut out = Vec::new();
    if header_size == 0 {
        let frame = &data[8..];
        decode_frame(frame, &mut out)?;
    } else {
        if data.len() < 12 || data[8] != 0x0f {
            return Err(RetailError::Format("bad BLTE frame table".into()));
        }
        let count = be24(&data[9..12]) as usize;
        if 12 + count * 24 != header_size || header_size > data.len() {
            return Err(RetailError::Format("bad BLTE frame count".into()));
        }
        let mut offset = header_size;
        for i in 0..count {
            let f = &data[12 + i * 24..12 + (i + 1) * 24];
            let encoded = be32(&f[0..4]) as usize;
            if offset + encoded > data.len() {
                return Err(RetailError::Format("BLTE frame out of range".into()));
            }
            decode_frame(&data[offset..offset + encoded], &mut out)?;
            offset += encoded;
        }
    }
    Ok(out)
}

fn decode_frame(frame: &[u8], out: &mut Vec<u8>) -> Result<()> {
    if frame.is_empty() {
        return Err(RetailError::Format("empty BLTE frame".into()));
    }
    match frame[0] {
        b'N' => out.extend_from_slice(&frame[1..]),
        b'Z' => {
            let mut decoder = ZlibDecoder::new(&frame[1..]);
            decoder.read_to_end(out)?;
        }
        b'E' => {
            return Err(RetailError::Unsupported(
                "encrypted BLTE frame (no keyring)".into(),
            ))
        }
        other => {
            return Err(RetailError::Unsupported(format!(
                "BLTE frame mode {}",
                other as char
            )))
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// MPQ (map archives; also works for classic StarDat/BrooDat/Patch_rt)
// ---------------------------------------------------------------------------

struct MpqBlock {
    file_pos: u32,
    compressed_size: u32,
    file_size: u32,
    flags: u32,
}

struct MpqHash {
    name1: u32,
    name2: u32,
    block: u32,
}

/// Read-only MPQ v1 archive.  Shipped StarCraft maps are MPQ v1 files whose
/// only member is `staredit\scenario.chk`.
pub struct MpqArchive {
    data: Vec<u8>,
    base: usize,
    block_shift: u16,
    blocks: Vec<MpqBlock>,
    hashes: Vec<MpqHash>,
}

impl MpqArchive {
    pub fn open(path: &Path) -> Result<Self> {
        let data = fs::read(path)?;
        let base = find_bytes(&data, b"MPQ\x1a")
            .ok_or_else(|| RetailError::Format(format!("{}: no MPQ header", path.display())))?;
        if base + 32 > data.len() {
            return Err(RetailError::Format("short MPQ header".into()));
        }
        let header_size = le32(&data[base + 4..]) as usize;
        let version = u16::from_le_bytes(data[base + 12..base + 14].try_into().unwrap());
        if version > 3 {
            return Err(RetailError::Unsupported(format!(
                "{}: MPQ version {version}",
                path.display()
            )));
        }
        let block_shift = u16::from_le_bytes(data[base + 14..base + 16].try_into().unwrap());
        let hash_pos = le32(&data[base + 16..]) as usize;
        let block_pos = le32(&data[base + 20..]) as usize;
        let hash_count_raw = le32(&data[base + 24..]) as usize;
        let block_count = le32(&data[base + 28..]) as usize;
        let _ = header_size;
        // Some protectors inflate the declared hash table size.  The next
        // table starts immediately after the real one, so derive the count
        // from the table offsets when the declared size is impossible.
        let mut hash_count = hash_count_raw;
        if hash_pos < block_pos {
            let available = (block_pos - hash_pos) / 16;
            if hash_count > available {
                hash_count = available;
            }
        }
        let hash_start = base + hash_pos;
        let block_start = base + block_pos;
        if hash_start + hash_count * 16 > data.len() || block_start + block_count * 16 > data.len()
        {
            return Err(RetailError::Format("MPQ tables out of range".into()));
        }
        // Classic MPQ encrypts both tables with fixed Blizzard keys.
        let mut hash_bytes = data[hash_start..hash_start + hash_count * 16].to_vec();
        mpq_decrypt(&mut hash_bytes, 0xC3AF_3770);
        let mut block_bytes = data[block_start..block_start + block_count * 16].to_vec();
        mpq_decrypt(&mut block_bytes, 0xEC83_B3A3);
        let mut hashes = Vec::with_capacity(hash_count);
        for i in 0..hash_count {
            let p = i * 16;
            hashes.push(MpqHash {
                name1: le32(&hash_bytes[p..]),
                name2: le32(&hash_bytes[p + 4..]),
                block: le32(&hash_bytes[p + 12..]),
            });
        }
        let mut blocks = Vec::with_capacity(block_count);
        for i in 0..block_count {
            let p = i * 16;
            blocks.push(MpqBlock {
                file_pos: le32(&block_bytes[p..]),
                compressed_size: le32(&block_bytes[p + 4..]),
                file_size: le32(&block_bytes[p + 8..]),
                flags: le32(&block_bytes[p + 12..]),
            });
        }
        Ok(Self {
            data,
            base,
            block_shift,
            blocks,
            hashes,
        })
    }

    /// Read a member by archive path (backslashes or slashes).
    pub fn read_file(&self, name: &str) -> Option<Vec<u8>> {
        if self.hashes.is_empty() {
            return None;
        }
        let name = name.replace('/', "\\");
        let index_hash = mpq_hash(&name, 0);
        let target1 = mpq_hash(&name, 1);
        let target2 = mpq_hash(&name, 2);
        let mask = self.hashes.len() as u32 - 1;
        let mut index = index_hash & mask;
        for _ in 0..self.hashes.len() {
            let h = &self.hashes[index as usize];
            if h.block == 0xffff_ffff {
                return None;
            }
            if h.name1 == target1 && h.name2 == target2 {
                let block = self.blocks.get(h.block as usize)?;
                return self.read_block(block, &name);
            }
            index = (index + 1) & mask;
        }
        None
    }

    fn read_block(&self, block: &MpqBlock, name: &str) -> Option<Vec<u8>> {
        const FILE_COMPRESS: u32 = 0x0000_0200;
        const FILE_IMPLODE: u32 = 0x0000_0100;
        const FILE_ENCRYPTED: u32 = 0x0001_0000;
        const FILE_SINGLE_UNIT: u32 = 0x0100_0000;
        if block.flags & 0x8000_0000 == 0 {
            return None;
        }
        let start = self.base + block.file_pos as usize;
        let end = start + block.compressed_size as usize;
        if end > self.data.len() {
            return None;
        }
        let raw = &self.data[start..end];
        let compressed = block.flags & (FILE_COMPRESS | FILE_IMPLODE) != 0;
        let implode_only = block.flags & FILE_IMPLODE != 0 && block.flags & FILE_COMPRESS == 0;
        let mut bytes = raw.to_vec();
        // MPQ file keys are derived from the plain file name (no path).
        let plain = name.rsplit(['\\', '/']).next().unwrap_or(name).to_string();
        let mut key = if block.flags & FILE_ENCRYPTED != 0 {
            Some(mpq_hash(&plain, 3))
        } else {
            None
        };
        if let Some(k) = key.as_mut() {
            if block.flags & 0x0002_0000 != 0 {
                *k = k.wrapping_add(block.file_pos) ^ block.file_size;
            }
            // Multi-sector files encrypt the offset table and each sector
            // separately, so don't decrypt the whole buffer here.
            if !compressed || block.flags & FILE_SINGLE_UNIT != 0 {
                mpq_decrypt(&mut bytes, *k);
            }
        }
        if !compressed {
            bytes.truncate(block.file_size as usize);
            return Some(bytes);
        }
        let sector_size = 512usize << self.block_shift;
        if block.flags & FILE_SINGLE_UNIT != 0 {
            if implode_only {
                return pklib_explode(&bytes, block.file_size as usize);
            }
            let mut out = Vec::new();
            decompress_sector(&bytes, block.file_size as usize, &mut out)?;
            return Some(out);
        }
        if bytes.len() < 4 {
            return None;
        }
        let sectors = (block.file_size as usize).div_ceil(sector_size);
        if bytes.len() < (sectors + 1) * 4 {
            return None;
        }
        if let Some(k) = key {
            mpq_decrypt(&mut bytes[..(sectors + 1) * 4], k.wrapping_sub(1));
        }
        let mut offsets = Vec::with_capacity(sectors + 1);
        for i in 0..=sectors {
            offsets.push(le32(&bytes[i * 4..]) as usize);
        }
        let mut out = Vec::with_capacity(block.file_size as usize);
        for i in 0..sectors {
            let a = offsets[i];
            let b = offsets[i + 1];
            if a > b || b > bytes.len() {
                return None;
            }
            let mut sector = bytes[a..b].to_vec();
            if let Some(k) = key {
                mpq_decrypt(&mut sector, k.wrapping_add(i as u32));
            }
            let expected = (block.file_size as usize - out.len()).min(sector_size);
            if implode_only {
                // FILE_IMPLODE sectors are bare PKWARE streams: no method byte.
                let decoded = pklib_explode(&sector, expected)?;
                out.extend_from_slice(&decoded[..decoded.len().min(expected)]);
            } else {
                decompress_sector(&sector, expected, &mut out)?;
            }
        }
        out.truncate(block.file_size as usize);
        Some(out)
    }
}

unsafe extern "C" {
    fn sc_pklib_explode(
        input: *const u8,
        input_len: u32,
        output: *mut u8,
        output_cap: u32,
        output_len: *mut u32,
    ) -> i32;
}

fn pklib_explode(data: &[u8], capacity: usize) -> Option<Vec<u8>> {
    let mut out = vec![0u8; capacity];
    let mut len = 0u32;
    let rc = unsafe {
        sc_pklib_explode(
            data.as_ptr(),
            data.len() as u32,
            out.as_mut_ptr(),
            out.len() as u32,
            &mut len,
        )
    };
    if rc != 0 && len == 0 {
        return None;
    }
    out.truncate(len as usize);
    Some(out)
}

fn decompress_sector(sector: &[u8], expected: usize, out: &mut Vec<u8>) -> Option<()> {
    if sector.is_empty() {
        return None;
    }
    if sector.len() == expected {
        out.extend_from_slice(sector);
        return Some(());
    }
    let mask = sector[0];
    let body = &sector[1..];
    if mask & 0x02 != 0 {
        let mut decoder = ZlibDecoder::new(body);
        let mut decoded = Vec::new();
        decoder.read_to_end(&mut decoded).ok()?;
        out.extend_from_slice(&decoded[..decoded.len().min(expected)]);
        return Some(());
    }
    if mask & 0x08 != 0 {
        let decoded = pklib_explode(body, expected)?;
        out.extend_from_slice(&decoded[..decoded.len().min(expected)]);
        return Some(());
    }
    if mask == 0 {
        out.extend_from_slice(body);
        return Some(());
    }
    // PkWare/BZip2/ADPCM sectors are rare in the shipped map corpus; report
    // them instead of silently returning corrupt data.
    None
}

fn mpq_hash(name: &str, hash_type: u32) -> u32 {
    let table = mpq_crypt_table();
    let mut seed1 = 0x7fed_7fedu32;
    let mut seed2 = 0xeeee_eeeeu32;
    for byte in name.bytes() {
        let ch = byte.to_ascii_uppercase() as usize;
        seed1 = table[(hash_type as usize * 0x100) + ch] ^ (seed1.wrapping_add(seed2));
        seed2 = (ch as u32)
            .wrapping_add(seed1)
            .wrapping_add(seed2)
            .wrapping_add(seed2 << 5)
            .wrapping_add(3);
    }
    seed1
}

fn mpq_crypt_table() -> [u32; 0x500] {
    let mut table = [0u32; 0x500];
    let mut seed = 0x0010_0001u32;
    // The table is interleaved by byte value, not contiguous: entry
    // `byte + hash_type * 0x100` is what the hash/decrypt loops index.
    for byte in 0..0x100usize {
        for i in 0..5usize {
            seed = (seed * 125 + 3) % 0x2aaaab;
            let a = (seed & 0xffff) << 16;
            seed = (seed * 125 + 3) % 0x2aaaab;
            let b = seed & 0xffff;
            table[byte + i * 0x100] = a | b;
        }
    }
    table
}

fn mpq_decrypt(data: &mut [u8], key: u32) {
    let table = mpq_crypt_table();
    let mut seed1 = key;
    let mut seed2 = 0xeeee_eeeeu32;
    // StormLib rounds the block length down to DWORDs.  The trailing 1..3
    // bytes of an encrypted sector are stored in the clear.
    for i in 0..data.len() / 4 {
        let chunk = &mut data[i * 4..i * 4 + 4];
        let mut value = 0u32;
        for (j, b) in chunk.iter().enumerate() {
            value |= (*b as u32) << (8 * j);
        }
        seed2 = seed2.wrapping_add(table[0x400 + (seed1 & 0xff) as usize]);
        let plain = value ^ seed1.wrapping_add(seed2);
        for (j, b) in chunk.iter_mut().enumerate() {
            *b = ((plain >> (8 * j)) & 0xff) as u8;
        }
        seed1 = ((!seed1 << 21).wrapping_add(0x1111_1111)) | (seed1 >> 11);
        seed2 = plain
            .wrapping_add(seed2)
            .wrapping_add(seed2 << 5)
            .wrapping_add(3);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)?;
    Ok(())
}

fn walk(dir: &Path, callback: &mut impl FnMut(&Path)) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, callback);
        } else {
            callback(&path);
        }
    }
}

fn normalise(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn be16(bytes: &[u8]) -> u16 {
    u16::from_be_bytes([bytes[0], bytes[1]])
}
fn be24(bytes: &[u8]) -> u32 {
    ((bytes[0] as u32) << 16) | ((bytes[1] as u32) << 8) | bytes[2] as u32
}
fn be32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}
fn le32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn hex16(text: &str) -> Option<[u8; 16]> {
    if text.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_tokens(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter(|t| t.len() == 32 && t.chars().all(|c| c.is_ascii_hexdigit()))
        .map(|t| t.to_ascii_lowercase())
        .collect()
}

/// PCX files end with a 0x0C marker and 768 bytes of VGA palette.
fn pcx_palette(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() < 769 || bytes[bytes.len() - 769] != 0x0c {
        return None;
    }
    Some(bytes[bytes.len() - 768..].to_vec())
}

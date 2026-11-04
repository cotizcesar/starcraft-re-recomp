//! End-to-end checks against the player's own retail installation.
//!
//! These tests never read a bundled copy of the game: they open the CASC
//! storage and the shipped MPQ map archives directly.  They are skipped when
//! no installation is available, so `cargo test` still works on a clean CI
//! machine.

use std::sync::Arc;

use galaxycraft::audio::{MusicBank, SoundBank};
use galaxycraft::map::Scenario;
use galaxycraft::render::{era_name, Framebuffer, SpriteBank, Tileset};
use galaxycraft::retail::{RetailInstall, FIXTURE_SOURCES, SOUND_PATHS};
use galaxycraft::sfx::SfxTable;
use galaxycraft::sim::{Race, World};
use galaxycraft::ui_art::{ConsoleArt, GlueArt};
use galaxycraft::voice::{VoiceBank, VoiceCue};
use galaxycraft::{campaign, dds};
use sha2::{Digest, Sha256};

fn install() -> Option<RetailInstall> {
    RetailInstall::from_env().or_else(|| {
        let path = std::env::var("SC_INSTALL").ok()?;
        RetailInstall::open(path).ok()
    })
}

fn sha(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn casc_returns_the_documented_retail_files() {
    let Some(install) = install() else {
        eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
        return;
    };
    assert!(install.path_count() > 50_000);
    assert!(install.encoding_count() > 40_000);
    assert!(install.index_count() > 10_000);
    let expected: &[(&str, &str)] = &[
        (
            "arr/units.dat",
            "da2ee6f116b77329048ec6be9e02721a527fde23d4b05b1b1336e634490abcba",
        ),
        (
            "arr/weapons.dat",
            "80de165015519f9bed46ea8858195c2d2d0120b65b9e55e1c0a50b19135f4b62",
        ),
        (
            "arr/flingy.dat",
            "89013fa6d81edad96b68efcfc0bb5342f1c7fa2ef7a0880a71b0169eab13d1ad",
        ),
        (
            "TileSet/platform.vf4",
            "22185f7ad524acbdbf3387736377fd473cbe1e02611dfeec521ecd523b0c9e0c",
        ),
        (
            "unit/terran/marine.grp",
            "3c3a0a6dcb91f436a54e750872bc925e2b1dde3fcb6226327358d948f079ba69",
        ),
        (
            "rez/stat_txt.tbl",
            "4cbd3b355bd9e9732abbfb7502301c11d2d923090925f5af642824ff98fbfd0c",
        ),
        (
            "scripts/iscript.bin",
            "cd35e04eaa8ea95cbbccc980bfaf4cae236f4b18b620dfb4a765822ec32c1cd0",
        ),
    ];
    for (path, want) in expected {
        let bytes = install
            .read(path)
            .unwrap_or_else(|| panic!("missing {path}"));
        assert_eq!(sha(&bytes), *want, "{path}");
    }
    assert_eq!(install.content_size("arr/units.dat"), Some(19_876));

    // The nine effects the mixer uses are read from the user's CASC storage.
    let sounds: &[(&str, &str)] = &[
        (
            "ui_click",
            "d183413d3cc768e239ddcf15b17271fe0d6f28442777890722ea06b24741c155",
        ),
        (
            "ui_move",
            "f3d369e4af62f3230de8075cf5a326fdc16d76a659730a4fc0fe44569f167874",
        ),
        (
            "ui_swish",
            "ac1462c2a9b2f2dabad04d61cbbb8dd90f62e1e5f285d77897f82d433c239bee",
        ),
        (
            "transmission",
            "4e744b26a46ba494bb8091656a1a29033d545fbb7341afc8583251c3ebf7ce2a",
        ),
        (
            "terran_yes",
            "6f172c7e495d05a9e46d42795c0fd7ecae6509df3d24cd997676f097be6bd835",
        ),
        (
            "zerg_yes",
            "51304558d2006f1cd40ccc4dbe459363387bde649d06ec7a5819459c66b197e9",
        ),
        (
            "protoss_yes",
            "9d7bd2fba0ff30497323c16d76866080a8efda0fd3dcc1ac84c42dab1ecd2c73",
        ),
        (
            "victory",
            "ffd34a45b4ade55f43ccda79158a553e8b0a9a229daf4819664277f29cae1b9e",
        ),
        (
            "defeat",
            "7e594ce6198983b1894449ede4f39507d1d5afd840425404e1dbce2920742376",
        ),
    ];
    for (name, want) in sounds {
        let candidates = SOUND_PATHS
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, c)| *c)
            .expect("sound path table");
        let bytes = candidates
            .iter()
            .find_map(|path| install.read(path))
            .unwrap_or_else(|| panic!("missing retail effect {name}"));
        assert_eq!(sha(&bytes), *want, "effect {name}");
    }
}

#[test]
fn all_shipped_mpq_maps_are_read_from_the_install() {
    let Some(install) = install() else {
        eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
        return;
    };
    let maps = install.maps();
    assert_eq!(maps.len(), 323, "the retail Maps/ tree has 323 archives");
    let mut parsed = 0usize;
    let mut with_triggers = 0usize;
    let mut bottleneck = None;
    for map in &maps {
        let raw = install
            .read_map(map)
            .unwrap_or_else(|e| panic!("{}: {e}", map.path.display()));
        let scenario =
            Scenario::parse(&raw).unwrap_or_else(|e| panic!("{}: {e}", map.path.display()));
        parsed += 1;
        if !scenario.triggers.is_empty() {
            with_triggers += 1;
        }
        if map.name == "(2)Bottleneck" {
            bottleneck = Some(raw);
        }
    }
    assert_eq!(parsed, 323);
    assert_eq!(with_triggers, 322);
    let bottleneck = bottleneck.expect("Bottleneck map");
    assert_eq!(
        sha(&bottleneck),
        "65833b8611ea0b1c24d7eef52d3d762449540f88af08292a704feea16e7e491f"
    );
}

#[test]
fn retail_tables_drive_a_world_from_an_mpq_map() {
    let Some(install) = install() else {
        eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
        return;
    };
    let data = install.game_data().expect("retail game data");
    assert_eq!(data.units.len(), 228);
    assert_eq!(data.weapons.len(), 130);
    assert_eq!(data.orders.len(), 189);
    assert_eq!(data.strings.len(), 1539);

    let map = install
        .maps()
        .into_iter()
        .find(|m| m.name == "(2)Bottleneck")
        .expect("Bottleneck");
    let raw = install.read_map(&map).expect("scenario.chk");
    let scenario = Scenario::parse(&raw).expect("valid CHK");
    let vf4 = install
        .read(&format!("TileSet/{}.vf4", era_name(scenario.era)))
        .expect("vf4");
    let cv5 = install
        .read(&format!("TileSet/{}.cv5", era_name(scenario.era)))
        .expect("cv5");
    let mut world = World::from_scenario(
        Arc::new(data),
        &scenario,
        &galaxycraft::map::TileData::new(&cv5, &vf4),
        &[Race::Terran, Race::Protoss],
    );
    for _ in 0..240 {
        world.step();
    }
    assert!(!world.units.is_empty());
}

#[test]
fn materialised_assets_load_without_the_bundled_asset_directory() {
    let Some(install) = install() else {
        eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
        return;
    };
    let dir = std::env::temp_dir().join(format!(
        "galaxycraft-retail-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    // Removed even when an assertion below fails: the materialised install
    // is ~1.7 GB and used to be left behind in the temp folder.
    struct RemoveOnDrop(std::path::PathBuf);
    impl Drop for RemoveOnDrop {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = RemoveOnDrop(dir.clone());
    let report = install.materialize(&dir).expect("materialise");
    assert_eq!(report.maps, 323);
    assert_eq!(report.campaign_maps, 68);
    assert!(report.video_files >= 80, "{} videos", report.video_files);
    let vmanifest = std::fs::read_to_string(dir.join("videos/manifest.tsv")).unwrap();
    assert!(vmanifest.lines().count() >= 80, "video manifest rows");
    assert!(vmanifest.contains("smk/starintr.webm"));
    assert!(vmanifest.contains("interstitials/tr1010.webm"));
    // The decoded intro poster proves the WebM path end to end.
    let store = galaxycraft::video::VideoStore::load(&dir).expect("video store");
    assert!(store.contains("smk/starintr.webm"));
    let (info, poster) = store.intro_poster(false).expect("intro poster decodes");
    assert_eq!(info.codec, "V_VP9");
    assert!(info.pixel_width >= 320 && info.clusters > 0);
    assert!(poster.rgb.iter().filter(|&&v| v > 10).count() > 1000);
    let (_, bposter) = store
        .poster("interstitials/tr1010.webm")
        .expect("intermission poster");
    assert!(bposter.rgb.iter().filter(|&&v| v > 10).count() > 1000);
    assert!(report.briefings >= 60, "{} briefings", report.briefings);
    assert!(report.glue_files >= 20, "{} glue files", report.glue_files);
    assert!(report.files >= 200, "{} files", report.files);
    assert!(
        report.console_files >= 34,
        "{} console files",
        report.console_files
    );
    // 31 dialogs + `minimap`, `objctdlg` and `tips_dlg` (in-game HUD).
    assert_eq!(report.ui_files, 34);
    assert_eq!(report.music_files, 23);
    assert!(report.voice_files > 1000, "{} voices", report.voice_files);
    assert!(report.missing_sounds.is_empty());
    assert!(
        report.missing_assets.len() <= 8,
        "{:?}",
        report.missing_assets
    );
    let manifest = std::fs::read_to_string(dir.join("campaign/manifest.tsv")).unwrap();
    assert_eq!(manifest.lines().count(), 69);

    let chk = dir.join("maps").join("(2)Bottleneck.chk");
    let scenario = Scenario::parse(&std::fs::read(&chk).unwrap()).unwrap();
    let tileset =
        Tileset::load(&dir.join("TileSet"), scenario.era, &scenario).expect("tileset from cache");
    let mut fb = Framebuffer::new(320, 200);
    fb.clear(galaxycraft::render::Rgb::new(0, 0, 0));
    tileset.draw_terrain(&mut fb, &scenario, (0, 0));
    assert!(fb.pixels.iter().any(|p| *p != 0));

    let sprites = SpriteBank::load(&dir);
    assert!(sprites.has(galaxycraft::ids::unit::MARINE));
    assert!(sprites.has(galaxycraft::ids::unit::ZEALOT));
    assert!(sprites.has(galaxycraft::ids::unit::ZERGLING));
    let sounds = SoundBank::load(&dir.join("sound"));
    assert_eq!(sounds.len(), 9);

    // Retail console plates, command icons, wireframes and the UI layouts.
    let console = ConsoleArt::load(&dir, Race::Terran).expect("terran console");
    assert_eq!(console.race, "terran");
    assert!(console.left.width > 1000 && console.left.height > 600);
    assert!(console.center.width > 1000);
    assert!(console.right.width > 2000);
    assert_eq!(
        console
            .command_icons
            .as_ref()
            .expect("cmdicons")
            .frames
            .len(),
        390
    );
    assert_eq!(
        console
            .command_buttons_hd
            .as_ref()
            .expect("HD button frames")
            .frames
            .len(),
        37
    );
    assert!(console.wireframe.as_ref().expect("wireframe").frames.len() >= 228);
    let statbtn = console.layout("statbtndefault").expect("statbtndefault");
    assert_eq!(
        statbtn.rect,
        galaxycraft::ui::UiRect::new(496, 354, 144, 126)
    );
    assert_eq!(console.layout("statdata").unwrap().rect.y, 388);

    // The console plate triptych and the wireframe render real pixels.
    let mut hud = Framebuffer::new(1280, 240);
    hud.clear(galaxycraft::render::Rgb::new(0, 0, 0));
    console.draw_plates(&mut hud, 0, 240);
    assert!(hud.pixels.iter().any(|p| *p & 0x00ff_ffff != 0));
    let mut icon = Framebuffer::new(72, 68);
    assert!(console.draw_wireframe(
        &mut icon,
        galaxycraft::ids::unit::MARINE,
        (0, 0, 64, 64),
        None
    ));
    assert!(icon.pixels.iter().any(|p| *p & 0x00ff_ffff != 0));

    // Main-menu title art and the shipped score backdrops.
    let glue = GlueArt::load(&dir);
    assert!(glue.has_art());
    assert!(glue.title.as_ref().unwrap().width > 3000);
    assert!(glue.menu.is_some() && glue.score.is_some() && glue.mission.is_some());
    for (race, victory) in [
        (Race::Terran, true),
        (Race::Terran, false),
        (Race::Zerg, true),
        (Race::Protoss, true),
    ] {
        let panel = GlueArt::load_score_panel(&dir, race, victory)
            .unwrap_or_else(|| panic!("score panel {race:?} {victory}"));
        assert!(panel.width > 3000 && panel.height > 1800);
        assert!(panel.pixels.iter().any(|p| p & 0x00ff_ffff != 0));
    }

    // Complete voice sets and the shipped Ogg music decode from the cache.
    let table = SfxTable::parse(&std::fs::read(dir.join("tables/sfxdata.tbl")).unwrap())
        .expect("sfxdata.tbl");
    let voices = VoiceBank::new(dir.join("voices"), table);
    assert!(voices.available() > 1000, "{}", voices.available());
    let marine = voices
        .cue(galaxycraft::ids::unit::MARINE, VoiceCue::Yes, 0)
        .expect("marine acknowledgement");
    assert!(marine.len() > 1000, "{} samples", marine.len());
    assert!(!voices.played().is_empty());
    let music = MusicBank::new(dir.join("music"));
    assert_eq!(music.names().len(), 23);
    let title = music.track("title").expect("title theme decodes");
    assert!(title.len() > 1_000_000, "{} samples", title.len());
    let _ = std::fs::remove_dir_all(&dir);
}
#[test]
fn retail_campaign_enumerates_every_shipped_mission() {
    let Some(install) = install() else {
        eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
        return;
    };
    let episodes = campaign::episodes(&install);
    let counts: Vec<usize> = episodes.iter().map(|e| e.missions.len()).collect();
    // Terran 12 + tutorial, Zerg 10 + tutorial, Protoss 10 + tutorial,
    // Brood War Terran 9 (the 1.23 storage has no expterran tutorial CHK),
    // Brood War Zerg 16 (04a..04f and bonus), Brood War Protoss 8.
    assert_eq!(counts, vec![13, 11, 11, 9, 16, 8], "{counts:?}");
    let total: usize = counts.iter().sum();
    assert_eq!(total, 68);
    assert_eq!(episodes[0].missions[0].name, "Terran Tutorial");
    assert_eq!(episodes[0].missions[1].name, "Wasteland");
    assert_eq!(episodes[0].missions[4].name, "The Jacobs Installation");

    for episode in &episodes {
        for mission in &episode.missions {
            let raw = install
                .read(&mission.chk_path)
                .unwrap_or_else(|| panic!("{} missing", mission.chk_path));
            let scenario = Scenario::parse(&raw).expect("campaign CHK parses");
            assert!(scenario.has_placed_force(), "{}", mission.key);
            assert!(!mission.name.is_empty(), "{}", mission.key);
            // The Zerg Brood War `bonus` mission has no establishing shot.
            if mission.key == "bonus" {
                assert!(mission.briefing_key.is_none());
                continue;
            }
            let key = mission
                .briefing_key
                .as_deref()
                .unwrap_or_else(|| panic!("{} has no retail briefing", mission.key));
            assert!(install.contains(&format!("locales/enus/assets/rez/{key}.txt")));
        }
    }
}

#[test]
fn retail_briefings_parse_and_their_backdrops_decode() {
    let Some(install) = install() else {
        eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
        return;
    };
    let mut art: std::collections::BTreeSet<String> = Default::default();
    let mut pages = 0usize;
    for episode in campaign::episodes(&install) {
        for mission in &episode.missions {
            if mission.briefing_key.is_none() {
                continue;
            }
            let script = campaign::load_briefing(&install, mission)
                .unwrap_or_else(|| panic!("{} briefing", mission.key));
            assert!(!script.pages.is_empty(), "{} has no pages", mission.key);
            pages += script.pages.len();
            for page in &script.pages {
                if let Some(background) = &page.background {
                    art.insert(background.clone());
                }
            }
        }
    }
    assert!(pages >= 68, "{pages} briefing pages");
    assert!(art.len() >= 10, "{} distinct backdrops", art.len());
    for path in art {
        let bytes = install.read(&path).unwrap_or_else(|| panic!("{path}"));
        let image = dds::parse(&bytes).unwrap_or_else(|| panic!("{path} is not DDS"));
        assert!(image.width >= 320 && image.height >= 200);
        assert!(image.pixels.iter().any(|p| *p != 0), "{path} decoded black");
    }
}

#[test]
fn campaign_human_seat_follows_ownr_and_is_not_always_slot_zero() {
    let Some(install) = install() else {
        eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
        return;
    };
    let seats: &[(&str, usize)] = &[
        ("sd/campaign/terran/terran01/staredit/scenario.chk", 1),
        ("sd/campaign/terran/terran02/staredit/scenario.chk", 1),
        ("sd/campaign/zerg/zerg01/staredit/scenario.chk", 3),
        ("sd/campaign/protoss/protoss05/staredit/scenario.chk", 7),
        ("sd/campaign/expprotoss/protoss04/staredit/scenario.chk", 1),
    ];
    for (path, expected) in seats {
        let raw = install.read(path).unwrap_or_else(|| panic!("{path}"));
        let scenario = Scenario::parse(&raw).unwrap();
        let human = scenario
            .player_slots()
            .into_iter()
            .find(|s| s.active && s.human)
            .unwrap_or_else(|| panic!("{path} has no human seat"));
        assert_eq!(human.slot, *expected, "{path}");
        assert_eq!(human.owner, 6, "{path}");
    }

    // The bundled Enslavers mission is the same case and needs no install.
    let raw = std::fs::read("assets/maps/Enslavers 01.chk").unwrap();
    let scenario = Scenario::parse(&raw).unwrap();
    let human = scenario
        .player_slots()
        .into_iter()
        .find(|s| s.active && s.human)
        .expect("human");
    assert_eq!(human.slot, 6);
    assert_eq!(human.owner, 6);
}

#[test]
fn intro_movie_decodes_both_video_and_audio() {
    let Some(install) = install() else {
        eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
        return;
    };
    // The client streams from its materialised cache (`videos/smk/...`).
    let path = install.default_cache_dir().join("videos/smk/starintr.webm");
    if !path.exists() {
        eprintln!("skipping: run `sc_assets ... extract` so videos/smk/starintr.webm exists");
        return;
    }
    let info = galaxycraft::video::probe_info(&path).expect("webm header");
    assert!(info.has_audio, "the retail intro carries a soundtrack");
    let frame =
        galaxycraft::video::decode_frame_at_path(&path, &info, 5.0, 480).expect("frame at 5 s");
    assert!(
        frame.rgb.iter().any(|b| *b > 32),
        "frame at 5 s is not black"
    );
    let pcm = galaxycraft::video::decode_audio_pcm(&path).expect("audio track");
    let seconds = pcm.len() as f32 / galaxycraft::video::AUDIO_RATE as f32;
    assert!(
        seconds > 60.0,
        "intro audio is {seconds:.1} s, expected the full movie"
    );
    assert!(
        pcm.iter().any(|s| s.abs() > 200),
        "audio decodes to silence"
    );
}

#[test]
fn campaign_intermission_clip_streams_moving_frames() {
    let Some(install) = install() else {
        eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
        return;
    };
    let cache = install.default_cache_dir();
    // Any materialised intermission clip will do; the Terran first mission's
    // briefing names TR1010.
    let candidates = ["videos/interstitials/tr1010.webm"];
    let Some(path) = candidates
        .iter()
        .map(|rel| cache.join(rel))
        .find(|p| p.exists())
    else {
        eprintln!("skipping: run `sc_assets ... extract` so videos/interstitials exists");
        return;
    };
    let info = galaxycraft::video::probe_info(&path).expect("webm header");
    let mut player = galaxycraft::cinematic::Cinematic::open(&path, &info, 12.0, 480)
        .expect("open intermission");
    let first = player.advance(0.0).expect("first frame");
    // The decoder runs in a worker thread: keep ticking until it delivers a
    // different frame (bounded, so a broken player fails instead of hanging).
    let mut moved = 0u64;
    for _ in 0..100 {
        std::thread::sleep(std::time::Duration::from_millis(50));
        if let Some(later) = player.advance(0.1) {
            moved = first
                .rgb
                .iter()
                .zip(later.rgb.iter())
                .map(|(a, b)| (*a as i64 - *b as i64).unsigned_abs())
                .sum();
            if moved > 10_000 {
                break;
            }
        }
    }
    assert!(
        moved > 10_000,
        "intermission frames are identical ({moved}); the clip is not moving"
    );
    // Playback past the end reports finished, so the briefing can move on.
    let _ = player.advance(60.0);
    assert!(player.finished());
}

#[test]
fn fixture_sources_are_unique_flat_files_with_casc_paths() {
    let mut names = std::collections::HashSet::new();
    for &(name, src) in FIXTURE_SOURCES {
        assert!(!name.is_empty() && !name.contains('/'), "{name}");
        assert!(names.insert(name), "duplicate fixture {name}");
        assert!(src.ends_with(name), "{name} <- {src}");
        assert!(!src.starts_with('/') && !src.contains(".."), "{src}");
    }
}

#[test]
fn write_fixtures_reproduces_the_pinned_tables() {
    let Some(install) = install() else {
        eprintln!("SC_INSTALL not set; skipping");
        return;
    };
    let out = std::env::temp_dir().join(format!("gc-fixtures-{}", std::process::id()));
    let written = install.write_fixtures(&out).expect("write fixtures");
    for &(name, _) in FIXTURE_SOURCES {
        assert!(written.iter().any(|w| w == name), "{name} not written");
    }
    assert!(written.iter().any(|w| w == "ui/flc.json"));
    let sums = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/SHA256SUMS"),
    )
    .expect("SHA256SUMS");
    let mut checked = 0;
    for line in sums.lines() {
        let Some((hash, name)) = line.split_once("  ") else {
            continue;
        };
        if let Ok(bytes) = std::fs::read(out.join(name)) {
            assert_eq!(sha(&bytes), hash, "{name}");
            checked += 1;
        }
    }
    assert_eq!(checked, 3, "units/weapons/flingy.dat are pinned");
    let _ = std::fs::remove_dir_all(&out);
}

/// The install's `x86/StarCraft.exe` carries the command-card table the
/// fixture was generated from (menu-time dump): identical set by set.
#[test]
fn command_cards_read_from_the_install_match_the_dump_fixture() {
    let Some(install) = install() else {
        eprintln!("skip: no retail install");
        return;
    };
    let sets = install.button_sets().expect("table in x86/StarCraft.exe");
    assert_eq!(sets.len(), 250);
    assert_eq!(sets.iter().filter(|s| !s.is_empty()).count(), 161);
    if galaxycraft::button_sets::BUTTON_SETS.is_empty() {
        eprintln!("no button_sets.in fixture: skipped the comparison");
        return;
    }
    for (unit, set) in sets.iter().enumerate() {
        let fixture = galaxycraft::button_sets::BUTTON_SETS
            .get(unit)
            .copied()
            .unwrap_or(&[]);
        assert_eq!(&set[..], fixture, "unit {unit}");
    }
}

//! Verify and materialise the retail installation without redistributing it.
//!
//! ```text
//! sc_assets --install "/path/to/StarCraft" verify
//! sc_assets --install "/path/to/StarCraft" extract --out /tmp/galaxycraft-retail
//! sc_assets --install "/path/to/StarCraft" path TileSet/platform.vf4
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use galaxycraft::map::Scenario;
use galaxycraft::retail::{RetailInstall, RetailMap};
use sha2::{Digest, Sha256};

fn sha(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut install_path: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut command: Option<String> = None;
    let mut path_arg: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--install" => {
                install_path = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--out" => {
                out = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "verify" | "extract" | "fixtures" | "maps" | "path" | "mpq" | "manifest" | "list" => {
                command = Some(args[i].clone());
                let takes_arg = matches!(args[i].as_str(), "path" | "mpq" | "manifest" | "list");
                if takes_arg && i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    path_arg = Some(args[i + 1].clone());
                    i += 1;
                }
                i += 1;
            }
            other => {
                eprintln!("unknown argument {other}");
                return ExitCode::from(2);
            }
        }
    }
    let command = command.unwrap_or_else(|| "verify".into());

    let install = match install_path {
        Some(path) => match RetailInstall::open(&path) {
            Ok(install) => install,
            Err(e) => {
                eprintln!("cannot open {}: {e}", path.display());
                return ExitCode::FAILURE;
            }
        },
        None => match RetailInstall::from_env() {
            Some(install) => install,
            None => {
                eprintln!("no install found; pass --install /path/to/StarCraft or set SC_INSTALL");
                return ExitCode::FAILURE;
            }
        },
    };

    match command.as_str() {
        // `list PREFIX`: CASC ROOT paths under a prefix, one per line.
        "list" => {
            for path in install.list(path_arg.as_deref().unwrap_or(""), "") {
                println!("{path}");
            }
            ExitCode::SUCCESS
        }
        // `manifest [DIR] [--out FILE]`: every materialised file (default:
        // the player's cache) with its CASC / MPQ origin; fails if any file
        // cannot be attributed.
        "manifest" => {
            let dir = path_arg
                .map(PathBuf::from)
                .unwrap_or_else(|| install.default_cache_dir());
            let rows = match install.origin_manifest(&dir) {
                Ok(rows) => rows,
                Err(e) => {
                    eprintln!("manifest of {} failed: {e}", dir.display());
                    return ExitCode::FAILURE;
                }
            };
            let mut text = String::from(galaxycraft::retail::ORIGIN_TSV_HEADER);
            text.push('\n');
            let mut counts = std::collections::BTreeMap::new();
            for row in &rows {
                text.push_str(&row.tsv());
                text.push('\n');
                let kind = row.tsv().split('\t').nth(3).unwrap_or("").to_string();
                *counts.entry(kind).or_insert(0usize) += 1;
            }
            match &out {
                Some(file) => {
                    if let Err(e) = std::fs::write(file, &text) {
                        eprintln!("cannot write {}: {e}", file.display());
                        return ExitCode::FAILURE;
                    }
                }
                None => print!("{text}"),
            }
            eprintln!("{} files in {}: {counts:?}", rows.len(), dir.display());
            if counts.contains_key("unknown") {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        "path" => {
            let Some(path) = path_arg else {
                eprintln!("path needs a CASC path");
                return ExitCode::from(2);
            };
            match install.read(&path) {
                Some(bytes) => {
                    use std::io::Write;
                    let _ = std::io::stdout().write_all(&bytes);
                    eprintln!("\n{path}: {} bytes sha256 {}", bytes.len(), sha(&bytes));
                    ExitCode::SUCCESS
                }
                None => {
                    eprintln!("not present: {path}");
                    ExitCode::FAILURE
                }
            }
        }
        "extract" => {
            let Some(out) = out else {
                eprintln!("extract needs --out DIR");
                return ExitCode::from(2);
            };
            match install.materialize(&out) {
                Ok(report) => {
                    println!(
                        "materialised {} files ({} bytes) and {} maps into {}",
                        report.files,
                        report.bytes,
                        report.maps,
                        out.display()
                    );
                    println!(
                        "console {}, ui {}, music {}, voices {}, campaign {}, briefings {}, glue {}",
                        report.console_files,
                        report.ui_files,
                        report.music_files,
                        report.voice_files,
                        report.campaign_maps,
                        report.briefings,
                        report.glue_files
                    );
                    if !report.missing_sounds.is_empty() {
                        println!("missing effects: {:?}", report.missing_sounds);
                    }
                    if !report.missing_assets.is_empty() {
                        println!("missing optional assets: {:?}", report.missing_assets);
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("extract failed: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        "fixtures" => {
            let out = out.unwrap_or_else(|| PathBuf::from("fixtures"));
            match install.write_fixtures(&out) {
                Ok(written) => {
                    println!("wrote {} fixtures into {}", written.len(), out.display());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("fixtures failed: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        "mpq" => {
            let Some(path) = path_arg else {
                eprintln!("mpq needs a map path");
                return ExitCode::from(2);
            };
            match galaxycraft::retail::MpqArchive::open(std::path::Path::new(&path)) {
                Ok(archive) => match archive.read_file("staredit\\scenario.chk") {
                    Some(bytes) => {
                        println!("{} bytes sha256 {}", bytes.len(), sha(&bytes));
                        if let Some(out) = &out {
                            std::fs::write(out, &bytes).ok();
                        }
                        ExitCode::SUCCESS
                    }
                    None => {
                        eprintln!("could not read scenario.chk");
                        ExitCode::FAILURE
                    }
                },
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        "maps" => {
            let maps = install.maps();
            println!("{} MPQ map archives", maps.len());
            let mut chks = 0usize;
            let mut triggers = 0usize;
            let mut failed = 0usize;
            for map in &maps {
                match install.read_map(map).and_then(|raw| {
                    Scenario::parse(&raw)
                        .map(|sc| (raw, sc))
                        .map_err(|e| galaxycraft::retail::RetailError::Format(e.to_string()))
                }) {
                    Ok((_, scenario)) => {
                        chks += 1;
                        if !scenario.triggers.is_empty() {
                            triggers += 1;
                        }
                    }
                    Err(e) => {
                        failed += 1;
                        if failed <= 20 {
                            eprintln!("{}: {e}", map.path.display());
                        }
                    }
                }
            }
            println!(
                "{chks}/{} CHKs parsed, {triggers} with triggers, {failed} failed",
                maps.len()
            );
            if failed == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        _ => verify(&install),
    }
}

fn verify(install: &RetailInstall) -> ExitCode {
    println!("retail install: {}", install.root().display());
    println!("build key:      {}", install.build_key());
    println!("encoding key:   {}", install.encoding_key());
    println!("root key:       {}", install.root_key());
    println!(
        "CASC:           {} paths, {} index entries, {} encoding entries",
        install.path_count(),
        install.index_count(),
        install.encoding_count()
    );

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
    let mut ok = 0usize;
    for (path, want) in expected {
        match install.read(path) {
            Some(bytes) => {
                let got = sha(&bytes);
                if got == *want {
                    println!("PASS {path:34} {:>8} bytes  {got}", bytes.len());
                    ok += 1;
                } else {
                    println!("FAIL {path:34} got {got}, want {want}");
                }
            }
            None => println!("FAIL {path:34} not found in CASC"),
        }
    }

    let maps = install.maps();
    println!("maps:           {} MPQ archives under Maps/", maps.len());
    let mut chks = 0usize;
    let mut trig = 0usize;
    let mut map_fail = 0usize;
    let mut bottleneck: Option<Vec<u8>> = None;
    for map in &maps {
        match read_chk(install, map) {
            Ok(raw) => {
                if map.name == "(2)Bottleneck" {
                    bottleneck = Some(raw.clone());
                }
                match Scenario::parse(&raw) {
                    Ok(sc) => {
                        chks += 1;
                        if !sc.triggers.is_empty() {
                            trig += 1;
                        }
                    }
                    Err(_) => map_fail += 1,
                }
            }
            Err(_) => map_fail += 1,
        }
    }
    println!(
        "MPQ maps:       {chks}/{} CHKs parsed, {trig} with TRIG, {map_fail} failed",
        maps.len()
    );
    if let Some(raw) = bottleneck {
        let got = sha(&raw);
        let want = "65833b8611ea0b1c24d7eef52d3d762449540f88af08292a704feea16e7e491f";
        if got == want {
            println!("PASS Maps/(2)Bottleneck.scm scenario.chk {got}");
            ok += 1;
        } else {
            println!("FAIL Bottleneck got {got}, want {want}");
        }
    }

    // Build a real World from CASC tables + MPQ map + CASC tileset.
    match install.game_data() {
        Ok(data) => {
            println!(
                "game data:      {} units, {} weapons, {} flingies, {} orders, {} strings",
                data.units.len(),
                data.weapons.len(),
                data.flingies.len(),
                data.orders.len(),
                data.strings.len()
            );
            let maps = install.maps();
            if let Some(map) = maps.iter().find(|m| m.name.contains("Bottleneck")) {
                match read_chk(install, map) {
                    Ok(raw) => match Scenario::parse(&raw) {
                        Ok(scenario) => {
                            let era = galaxycraft::render::era_name(scenario.era);
                            let vf4 = install
                                .read(&format!("TileSet/{era}.vf4"))
                                .unwrap_or_default();
                            let cv5 = install
                                .read(&format!("TileSet/{era}.cv5"))
                                .unwrap_or_default();
                            let mut world = galaxycraft::sim::World::from_scenario(
                                std::sync::Arc::new(data),
                                &scenario,
                                &galaxycraft::map::TileData::new(&cv5, &vf4),
                                &[
                                    galaxycraft::sim::Race::Terran,
                                    galaxycraft::sim::Race::Protoss,
                                ],
                            );
                            for _ in 0..240 {
                                world.step();
                            }
                            println!(
                                "world:          {} units after 240 ticks on MPQ Bottleneck",
                                world.units.len()
                            );
                            ok += 1;
                        }
                        Err(e) => println!("FAIL scenario parse: {e}"),
                    },
                    Err(e) => println!("FAIL MPQ read: {e}"),
                }
            }
        }
        Err(e) => println!("FAIL game data: {e}"),
    }

    let total_ui = 10;
    let mut ui_ok = 0usize;
    // Retail console / menu / score art.
    for (label, path, check) in [
        (
            "terran console center",
            "game/consoles/terran/console_center.dds",
            "dds",
        ),
        (
            "terran command button frames",
            "game/consoles/terran/cmdbtns.dds.grp",
            "ddsgrp",
        ),
        ("command icons", "unit/cmdicons/cmdicons.grp", "grp390"),
        ("unit wireframes", "unit/wirefram/wirefram.grp", "grp228"),
        (
            "main menu title frame",
            "glue/mainmenu/titleframe_overlay.dds",
            "dds",
        ),
        ("title art", "glue/title/title.dds", "dds"),
        ("terran score panel", "glue/scoretd/pmain.dds", "dds"),
        (
            "ui layout statdata",
            "locales/enus/assets/rez/statdata.ui.json",
            "ui",
        ),
        (
            "ui layout gluscore",
            "locales/enus/assets/rez/gluscore.ui.json",
            "ui",
        ),
        (
            "ui layout msnstats",
            "locales/enus/assets/rez/msnstats.ui.json",
            "ui",
        ),
    ] {
        let Some(bytes) = install.read(path) else {
            println!("FAIL {label:34} not found ({path})");
            continue;
        };
        let passed = match check {
            "dds" => galaxycraft::dds::parse(&bytes)
                .is_some_and(|i| i.pixels.iter().any(|p| p & 0x00ff_ffff != 0)),
            "ddsgrp" => {
                galaxycraft::dds::DdsGrp::parse(&bytes).is_some_and(|g| g.frames.len() >= 37)
            }
            "grp390" => {
                galaxycraft::render::Grp::parse(&bytes).is_some_and(|g| g.frames.len() == 390)
            }
            "grp228" => {
                galaxycraft::render::Grp::parse(&bytes).is_some_and(|g| g.frames.len() == 228)
            }
            "ui" => galaxycraft::ui::UiDialog::parse(&bytes)
                .is_some_and(|d| d.rect.w == 640 || d.rect.w == 320 || d.rect.w == 270),
            _ => false,
        };
        if passed {
            println!("PASS {label:34} {:>8} bytes  {path}", bytes.len());
            ui_ok += 1;
        } else {
            println!("FAIL {label:34} could not decode {path}");
        }
    }
    ok += ui_ok;
    // Video: the retail title intro and one briefing intermission parse as
    // WebM and decode a non-black poster frame through ffmpeg.
    let mut video_ok = 0usize;
    let video_total: usize = 3;
    for (label, path) in [
        ("title intro", "smk/starintr.webm"),
        ("expansion intro", "smk/starxintr.webm"),
        ("briefing intermission", "interstitials/tr1010.webm"),
    ] {
        match install.read(path) {
            Some(bytes) => match galaxycraft::video::parse_webm(&bytes) {
                Ok(info) if info.pixel_width >= 320 && info.clusters > 0 => {
                    let poster = galaxycraft::video::decode_first_frame(&bytes, &info);
                    let nonblack = poster
                        .as_ref()
                        .map(|p| p.rgb.iter().filter(|&&v| v > 10).count())
                        .unwrap_or(0);
                    if nonblack > 1000 {
                        println!(
                            "PASS {:<34} {}x{} {} {} clusters  {path}",
                            label, info.pixel_width, info.pixel_height, info.codec, info.clusters
                        );
                        video_ok += 1;
                    } else {
                        println!("FAIL {label:34} poster is black ({path})");
                    }
                }
                Ok(_) => println!("FAIL {label:34} header too small ({path})"),
                Err(e) => println!("FAIL {label:34} {e} ({path})"),
            },
            None => println!("FAIL {label:34} not found ({path})"),
        }
    }
    ok += video_ok;
    let audio_total: usize = 2;
    let total = expected.len() + 2 + total_ui + audio_total + video_total;
    // Audio: the retail title theme decodes and the full voice index resolves
    // every shipped voice file through CASC.
    let mut audio_ok = 0usize;
    match install
        .read("music/title.ogg")
        .and_then(|b| galaxycraft::audio::decode_ogg(&b))
    {
        Some(samples) if samples.len() > 1_000_000 => {
            println!(
                "PASS {:<34} {} samples  music/title.ogg",
                "title theme",
                samples.len()
            );
            audio_ok += 1;
        }
        Some(samples) => println!("FAIL title theme decoded {} samples", samples.len()),
        None => println!("FAIL music/title.ogg did not decode"),
    }
    match install
        .read("arr/sfxdata.tbl")
        .and_then(|b| galaxycraft::sfx::SfxTable::parse(&b))
    {
        Some(table) => {
            let present = (0..table.len() as u16)
                .filter(|id| table.casc_path(*id).is_some_and(|p| install.contains(&p)))
                .count();
            if present >= 1100 {
                println!(
                    "PASS {:<34} {present}/{} paths  arr/sfxdata.tbl",
                    "voice set",
                    table.len()
                );
                audio_ok += 1;
            } else {
                println!("FAIL voice set only {present}/{} files", table.len());
            }
        }
        None => println!("FAIL arr/sfxdata.tbl did not parse"),
    }
    ok += audio_ok;
    println!("verify:         {ok}/{total} checks passed");
    if ok == total && map_fail == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn read_chk(
    install: &RetailInstall,
    map: &RetailMap,
) -> Result<Vec<u8>, galaxycraft::retail::RetailError> {
    install.read_map(map)
}

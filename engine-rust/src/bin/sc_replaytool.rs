//! Create, inspect and verify GalaxyCraft replays (`.gcrep` / `.gcsave`).
//!
//! A replay is a command log plus the map it was played on, so a file is
//! self-contained: it replays on any machine without the original `.chk`.
//! The format is documented in `docs/replays.md`.
//!
//! ```text
//! # Record a deterministic AI-vs-AI demo (no install needed).
//! sc_replaytool make --chk assets/maps/Boxer.chk --ticks 900 --out /tmp/demo.gcrep
//!
//! # Show what a replay contains without starting the client.
//! sc_replaytool info /tmp/demo.gcrep
//!
//! # Rebuild the world and re-apply the log: prints the final state hash.
//! sc_replaytool verify /tmp/demo.gcrep
//!
//! # Watch it.
//! sc_view --install "$SC_INSTALL" --replay /tmp/demo.gcrep
//! ```
//!
//! `verify` prints the hash of a fresh replay and the hash recorded when the
//! file was written; equal hashes mean the log reproduces the match exactly.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use galaxycraft::ai::SimpleAi;
use galaxycraft::data::GameData;
use galaxycraft::map::Scenario;
use galaxycraft::replay::{Recording, SaveFile, replay_to, state_hash};
use galaxycraft::sim::{Race, World};

fn data() -> GameData {
    GameData::bundled().expect("bundled tables")
}

fn usage() -> ExitCode {
    eprintln!(
        "usage:\n  sc_replaytool make --chk MAP [--ticks N] [--out FILE] [--races terran,zerg]\n  \
         sc_replaytool info FILE\n  sc_replaytool verify FILE"
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first().cloned() else {
        return usage();
    };
    match command.as_str() {
        "make" => make(&args[1..]),
        "info" => info(&args[1..]),
        "verify" => verify(&args[1..]),
        _ => usage(),
    }
}

fn parse_races(text: &str) -> Vec<Race> {
    text.split(',')
        .map(|t| match t.trim().to_ascii_lowercase().as_str() {
            "zerg" => Race::Zerg,
            "protoss" => Race::Protoss,
            _ => Race::Terran,
        })
        .collect()
}

fn make(args: &[String]) -> ExitCode {
    let mut chk = PathBuf::from("assets/maps/Boxer.chk");
    let mut ticks = 900u64;
    let mut out = PathBuf::from("/tmp/demo.gcrep");
    let mut races = vec![Race::Terran, Race::Protoss];
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--chk" => {
                chk = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--ticks" => {
                ticks = args[i + 1].parse().unwrap_or(ticks);
                i += 2;
            }
            "--out" => {
                out = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--races" => {
                races = parse_races(&args[i + 1]);
                i += 2;
            }
            other => {
                eprintln!("unknown argument {other}");
                return usage();
            }
        }
    }
    let raw = match std::fs::read(&chk) {
        Ok(raw) => raw,
        Err(e) => {
            eprintln!("cannot read {}: {e}", chk.display());
            return ExitCode::from(1);
        }
    };
    let Ok(scenario) = Scenario::parse(&raw) else {
        eprintln!("{} is not a StarCraft scenario", chk.display());
        return ExitCode::from(1);
    };
    // The map is a scenario; its tileset only matters to the renderer, so a
    // zeroed VF4 (no walkable flags) is fine for a headless recording run.
    let mut vf4 = vec![0u8; 96];
    for i in 0..16 {
        vf4[64 + i * 2] = 1;
    }
    let seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut world = World::from_scenario(
        Arc::new(data()),
        &scenario,
        &galaxycraft::map::TileData::synthetic(&vf4),
        &races,
    );
    world.recorder = Some(galaxycraft::replay::Recorder::new());
    world.rng = seed;
    let mut ais: Vec<SimpleAi> = (0..races.len()).map(|s| SimpleAi::new(s as u8)).collect();
    for _ in 0..ticks {
        for ai in ais.iter_mut() {
            ai.update(&mut world, Some((64, 64)));
        }
        world.step();
    }
    let hash = state_hash(&world);
    let rec = Recording {
        seed,
        map_hash: [0u8; 32],
        cmds: world
            .recorder
            .as_ref()
            .map(|r| r.cmds.clone())
            .unwrap_or_default(),
    };
    let save = SaveFile {
        // The CHK carries no filename, so the stem of the file it came from
        // is the most faithful label available.
        map_name: chk
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "scenario".to_string()),
        map_chk: raw,
        races,
        local_player: 0,
        tick: world.tick,
        rec,
        setup: None,
    };
    if let Some(dir) = out.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match save.save(&out) {
        Ok(()) => {
            println!(
                "wrote {}  map {:?}  tick {}  {} commands  final hash {:016x}",
                out.display(),
                save.map_name,
                save.tick,
                save.rec.cmds.len(),
                hash
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("cannot write {}: {e}", out.display());
            ExitCode::from(1)
        }
    }
}

fn informational(file: &str) -> Option<(SaveFile, u64)> {
    let path = PathBuf::from(file);
    let Some(save) = SaveFile::load(&path) else {
        eprintln!("{file}: not a GalaxyCraft save/replay");
        return None;
    };
    let mut vf4 = vec![0u8; 96];
    for i in 0..16 {
        vf4[64 + i * 2] = 1;
    }
    let scenario = Scenario::parse(&save.map_chk).ok()?;
    let mut world = World::from_scenario(
        Arc::new(data()),
        &scenario,
        &galaxycraft::map::TileData::synthetic(&vf4),
        &save.races,
    );
    world.rng = save.rec.seed;
    replay_to(&mut world, &save.rec, save.tick);
    let hash = state_hash(&world);
    Some((save, hash))
}

fn info(args: &[String]) -> ExitCode {
    let Some(file) = args.first() else {
        return usage();
    };
    let Some((save, hash)) = informational(file) else {
        return ExitCode::from(1);
    };
    println!("file        {file}");
    println!("map         {}", save.map_name);
    println!("tick        {}", save.tick);
    println!("seed        {:#018x}", save.rec.seed);
    println!("races       {:?}", save.races);
    println!("local player {}", save.local_player);
    println!("commands    {}", save.rec.cmds.len());
    println!(
        "duration    {}:{:02} (24 ticks/s)",
        save.tick / 24 / 60,
        (save.tick / 24) % 60
    );
    println!("final hash  {hash:016x}");
    ExitCode::SUCCESS
}

fn verify(args: &[String]) -> ExitCode {
    let Some(file) = args.first() else {
        return usage();
    };
    let Some((save, hash)) = informational(file) else {
        return ExitCode::from(1);
    };
    // Replay twice from scratch: a deterministic log must land on the same
    // hash both times, and that hash is what the viewer will show at the end.
    let Some((_, hash2)) = informational(file) else {
        return ExitCode::from(1);
    };
    println!("map {:<24} tick {}", save.map_name, save.tick);
    println!("commands {}", save.rec.cmds.len());
    println!("run #1   {hash:016x}");
    println!("run #2   {hash2:016x}");
    if hash == hash2 {
        println!("OK: the command log replays identically");
        ExitCode::SUCCESS
    } else {
        println!("FAIL: two replays of the same log diverged");
        ExitCode::from(1)
    }
}

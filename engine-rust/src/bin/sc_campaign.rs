//! Load and simulate every shipped `scenario.chk` extracted from the retail
//! maps, as a coverage harness for the reconstruction.
//!
//! ```text
//! sc_campaign --maps /tmp/sc_maps --tiles /tmp/tiles/TileSet --ticks 2400
//! ```
//!
//! Each map is parsed, converted into a [`World`], stepped for the requested
//! number of logical ticks with a deterministic AI per active player, and the
//! trigger VM running.  The command prints one line per map plus an aggregate
//! and exits non-zero when a map fails to load or panics.
//!
//! The tile set is selected from the map's `ERA` section:
//! 0 badlands, 1 platform/space, 2 install, 3 ashworld, 4 jungle,
//! 5 desert, 6 ice, 7 twilight.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use galaxycraft::ai::SimpleAi;
use galaxycraft::aiscript::CampaignAi;
use galaxycraft::data::GameData;
use galaxycraft::map::Scenario;
use galaxycraft::map::TileTables;
use galaxycraft::retail::RetailInstall;
use galaxycraft::sim::{Race, World};

const TILESETS: [&str; 8] = [
    "badlands.vf4",
    "platform.vf4",
    "install.vf4",
    "ashworld.vf4",
    "jungle.vf4",
    "Desert.vf4",
    "Ice.vf4",
    "Twilight.vf4",
];

struct Args {
    maps: PathBuf,
    tiles: PathBuf,
    ticks: u64,
    install: Option<PathBuf>,
}

fn main() {
    let mut args = Args {
        maps: PathBuf::from("/tmp/sc_maps"),
        tiles: PathBuf::from("/tmp/tiles/TileSet"),
        ticks: 2400,
        install: None,
    };
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--maps" => {
                args.maps = PathBuf::from(raw.get(i + 1).cloned().unwrap_or_default());
                i += 2;
            }
            "--tiles" => {
                args.tiles = PathBuf::from(raw.get(i + 1).cloned().unwrap_or_default());
                i += 2;
            }
            "--install" => {
                args.install = Some(PathBuf::from(raw.get(i + 1).cloned().unwrap_or_default()));
                i += 2;
            }
            "--ticks" => {
                args.ticks = raw
                    .get(i + 1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(args.ticks);
                i += 2;
            }
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }

    let retail = args.install.as_ref().map(|path| {
        RetailInstall::open(path).unwrap_or_else(|e| {
            eprintln!("cannot open retail install {}: {e}", path.display());
            std::process::exit(2);
        })
    });
    let data = Arc::new(match &retail {
        Some(install) => install.game_data().unwrap_or_else(|e| {
            eprintln!("cannot read retail tables: {e}");
            std::process::exit(2);
        }),
        None => GameData::bundled().expect("bundled tables"),
    });
    // Each era needs its cv5 (MTXM -> megatile) next to the vf4 flags.
    let tiles: Vec<Option<Arc<TileTables>>> = TILESETS
        .iter()
        .map(|name| {
            let cv5_name = std::path::Path::new(name).with_extension("cv5");
            let cv5_name = cv5_name.to_string_lossy();
            let (vf4, cv5) = match &retail {
                Some(install) => (
                    install.read(&format!("TileSet/{name}")),
                    install.read(&format!("TileSet/{cv5_name}")),
                ),
                None => (
                    std::fs::read(args.tiles.join(name)).ok(),
                    std::fs::read(args.tiles.join(&*cv5_name)).ok(),
                ),
            };
            Some(Arc::new(TileTables {
                cv5: cv5?,
                vf4: vf4?,
            }))
        })
        .collect();

    let mut maps = Vec::new();
    if let Some(install) = &retail {
        for map in install.maps() {
            let raw = install.read_map(&map).unwrap_or_else(|e| {
                eprintln!("{}: {e}", map.path.display());
                Vec::new()
            });
            if !raw.is_empty() {
                maps.push((map.relative.display().to_string(), raw));
            }
        }
        // Single-player campaign CHKs (68 non-empty missions) live outside
        // the MPQ tree under `sd/campaign/`; include them in the same run.
        for episode in galaxycraft::campaign::episodes(install) {
            for mission in episode.missions {
                if let Some(raw) = install.read(&mission.chk_path)
                    && !raw.is_empty()
                {
                    maps.push((format!("{}/{}", episode.key, mission.key), raw));
                }
            }
        }
    } else {
        let mut paths = Vec::new();
        collect(&args.maps, &mut paths);
        paths.sort();
        for path in paths {
            let name = path
                .strip_prefix(&args.maps)
                .unwrap_or(&path)
                .display()
                .to_string();
            match std::fs::read(&path) {
                Ok(raw) => maps.push((name, raw)),
                Err(e) => eprintln!("{}: {e}", path.display()),
            }
        }
    }
    if maps.is_empty() {
        eprintln!("no maps found");
        std::process::exit(2);
    }

    let mut ok = 0usize;
    let mut failed = 0usize;
    let mut trigger_maps = 0usize;
    let mut ai_scripts = 0usize;
    let mut total_units = 0usize;
    let mut total_triggers = 0usize;
    for (name, raw) in &maps {
        let result = catch_unwind(AssertUnwindSafe(|| {
            run_map_bytes(&data, &tiles, raw, args.ticks)
        }));
        match result {
            Ok(Ok(report)) => {
                ok += 1;
                total_units += report.units;
                total_triggers += report.triggers;
                if report.triggers > 0 {
                    trigger_maps += 1;
                }
                ai_scripts += report.ai_scripts;
                println!(
                    "ok   {name}: {}x{} era {} units {} triggers {} ai {} end {}{}",
                    report.width,
                    report.height,
                    report.era,
                    report.units,
                    report.triggers,
                    report.ai_scripts,
                    report.end_tick,
                    if report.winner { " winner" } else { "" }
                );
            }
            Ok(Err(e)) => {
                failed += 1;
                println!("FAIL {name}: {e}");
            }
            Err(_) => {
                failed += 1;
                println!("PANIC {name}");
            }
        }
    }
    println!(
        "campaign coverage: {ok}/{} maps simulated, {failed} failed, \
         {total_units} placed units, {total_triggers} triggers on {trigger_maps} maps, \
         {ai_scripts} AI script starts, {} ticks each",
        maps.len(),
        args.ticks
    );
    if failed > 0 {
        std::process::exit(1);
    }
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("chk") {
            out.push(path);
        }
    }
}

struct Report {
    width: usize,
    height: usize,
    era: u16,
    units: usize,
    triggers: usize,
    ai_scripts: usize,
    end_tick: u64,
    winner: bool,
}

fn run_map_bytes(
    data: &Arc<GameData>,
    tiles: &[Option<Arc<TileTables>>],
    bytes: &[u8],
    ticks: u64,
) -> Result<Report, String> {
    let scenario = Scenario::parse(bytes).map_err(|e| e.to_string())?;
    let era = scenario.era as usize;
    let vf4 = tiles.get(era).and_then(|t| t.as_ref()).ok_or_else(|| {
        format!(
            "missing tileset {} (era {era})",
            TILESETS.get(era).copied().unwrap_or("?")
        )
    })?;
    let slots = scenario.player_slots();
    let human = slots
        .iter()
        .find(|s| s.active && s.human)
        .map(|s| s.slot)
        .unwrap_or(0) as u8;
    let races: Vec<Race> = slots.iter().map(|s| s.race(Race::Terran)).collect();
    let active: Vec<u8> = slots
        .iter()
        .filter(|s| s.computer)
        .map(|s| s.slot as u8)
        .collect();
    let mut world = World::from_scenario(data.clone(), &scenario, &vf4.data(), &races);
    for s in &slots {
        if let Some(p) = world.players.get_mut(s.slot) {
            p.defeated = !s.active;
            p.is_computer = s.computer;
        }
    }
    let units = world.units.iter().filter(|u| u.alive).count();
    let triggers = world
        .triggers
        .as_ref()
        .map(|t| t.triggers.len())
        .unwrap_or(0);
    let mut ais: Vec<SimpleAi> = active.iter().map(|&p| SimpleAi::new(p)).collect();
    let mut campaign: Vec<CampaignAi> = Vec::new();
    let mut ai_scripts = 0usize;
    while world.tick < ticks && world.winner.is_none() {
        let target = world
            .units
            .iter()
            .find(|u| u.alive && u.owner != human)
            .map(|u| (u.x / 256, u.y / 256));
        for ai in &mut ais {
            ai.update(&mut world, target);
        }
        for ai in &mut campaign {
            ai.update(&mut world, target);
        }
        world.step();
        if let Some(t) = world.triggers.as_mut() {
            let pending = std::mem::take(&mut t.pending_ai_scripts);
            for (player, id, at) in pending {
                if campaign.iter().any(|c| c.player == player as u8) {
                    continue;
                }
                if let Some(script) = world
                    .data
                    .aiscripts
                    .get(&id)
                    .or_else(|| world.data.bwscripts.get(&id))
                {
                    let mut ai = CampaignAi::new(player as u8, script);
                    ai.location = at;
                    campaign.push(ai);
                    ai_scripts += 1;
                }
            }
        }
    }
    Ok(Report {
        width: scenario.width,
        height: scenario.height,
        era: scenario.era,
        units,
        triggers,
        ai_scripts,
        end_tick: world.tick,
        winner: world.winner.is_some(),
    })
}

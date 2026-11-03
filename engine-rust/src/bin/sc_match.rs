//! Headless match runner.
//!
//! ```text
//! sc_match --chk map.chk --vf4 platform.vf4 --races terran,protoss --ticks 20000
//! sc_match                       # synthetic 64x64 open map
//! ```
//!
//! Extract `staredit\scenario.chk` and the tileset `vf4` from the retail CASC
//! with `tools/mpq_extract.cpp` / `tools/casc_extract.cpp` first.

use std::sync::Arc;

use galaxycraft::ai::SimpleAi;
use galaxycraft::aiscript::CampaignAi;
use galaxycraft::data::GameData;
use galaxycraft::map::{Scenario, Terrain};
use galaxycraft::retail::RetailInstall;
use galaxycraft::sim::{Race, World};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut chk: Option<String> = None;
    let mut vf4: Option<String> = None;
    let mut races = vec![Race::Terran, Race::Protoss];
    let mut max_ticks: u64 = 60_000;
    let mut verbose = false;
    let mut ai_script: Option<String> = None;
    let mut fog = false;
    let mut install: Option<String> = None;
    let mut map_name: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--chk" => {
                chk = args.get(i + 1).cloned();
                i += 2;
            }
            "--vf4" => {
                vf4 = args.get(i + 1).cloned();
                i += 2;
            }
            "--races" => {
                if let Some(spec) = args.get(i + 1) {
                    races = spec.split(',').map(parse_race).collect();
                }
                i += 2;
            }
            "--ticks" => {
                max_ticks = args
                    .get(i + 1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(max_ticks);
                i += 2;
            }
            "--verbose" => {
                verbose = true;
                i += 1;
            }
            "--ai-script" => {
                ai_script = args.get(i + 1).cloned();
                i += 2;
            }
            "--fog" => {
                fog = true;
                i += 1;
            }
            "--install" => {
                install = args.get(i + 1).cloned();
                i += 2;
            }
            "--map" => {
                map_name = args.get(i + 1).cloned();
                i += 2;
            }
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }

    let retail = install.as_ref().map(|path| {
        RetailInstall::open(path).unwrap_or_else(|e| {
            eprintln!("cannot open retail install {path}: {e}");
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
    if let (Some(install), Some(name)) = (&retail, &map_name) {
        let maps = install.maps();
        let map = maps
            .iter()
            .find(|m| m.name.eq_ignore_ascii_case(name) || m.name.contains(name.as_str()))
            .unwrap_or_else(|| {
                eprintln!("no shipped map matches {name} ({} maps)", maps.len());
                std::process::exit(2);
            });
        let raw = install.read_map(map).unwrap_or_else(|e| {
            eprintln!("cannot read {}: {e}", map.path.display());
            std::process::exit(2);
        });
        let scenario = Scenario::parse(&raw).expect("valid CHK");
        let vf4_bytes = install
            .read(&format!(
                "TileSet/{}.vf4",
                galaxycraft::render::era_name(scenario.era)
            ))
            .unwrap_or_else(|| {
                eprintln!("missing tileset vf4 for era {}", scenario.era);
                std::process::exit(2);
            });
        std::fs::write("/tmp/sc_match_mpq.chk", &raw).ok();
        std::fs::write("/tmp/sc_match_mpq.vf4", &vf4_bytes).ok();
        if let Some(cv5) = install.read(&format!(
            "TileSet/{}.cv5",
            galaxycraft::render::era_name(scenario.era)
        )) {
            std::fs::write("/tmp/sc_match_mpq.cv5", cv5).ok();
        }
        chk = Some("/tmp/sc_match_mpq.chk".into());
        vf4 = Some("/tmp/sc_match_mpq.vf4".into());
    }
    let campaign_script = ai_script.as_ref().and_then(|id| {
        data.aiscripts
            .get_str(id)
            .or_else(|| data.bwscripts.get_str(id))
            .cloned()
    });
    let mut world = if let (Some(chk), Some(vf4)) = (chk, vf4) {
        let chk = std::fs::read(chk).expect("scenario.chk");
        let scenario = Scenario::parse(&chk).expect("valid CHK");
        let tiles = galaxycraft::map::TileTables::read_vf4(std::path::Path::new(&vf4))
            .expect("tileset vf4 + cv5");
        World::from_scenario(data, &scenario, &tiles.data(), &races)
    } else {
        let mut w = World::new(data, Terrain::open(64, 64), races.len());
        for (p, race) in races.iter().enumerate() {
            w.players[p].race = *race;
            w.players[p].minerals = 1000;
            w.players[p].gas = 500;
            let (x, y) = if p == 0 {
                (12 * 32, 12 * 32)
            } else {
                (52 * 32, 52 * 32)
            };
            let start = match race {
                Race::Terran => galaxycraft::ids::unit::COMMAND_CENTER,
                Race::Protoss => galaxycraft::ids::unit::NEXUS,
                Race::Zerg => galaxycraft::ids::unit::HATCHERY,
                Race::Neutral => galaxycraft::ids::unit::COMMAND_CENTER,
            };
            let _ = w.spawn_px(p, start, x, y);
            // A handful of workers and a mineral line.
            let worker = match race {
                Race::Terran => galaxycraft::ids::unit::SCV,
                Race::Protoss => galaxycraft::ids::unit::PROBE,
                Race::Zerg => galaxycraft::ids::unit::DRONE,
                Race::Neutral => galaxycraft::ids::unit::SCV,
            };
            for k in 0..4 {
                let _ = w.spawn_px(p, worker, x + 48 + k * 24, y + 64);
            }
            if *race == Race::Zerg {
                let _ = w.spawn_px(p, galaxycraft::ids::unit::OVERLORD, x + 64, y + 16);
            }
            for k in 0..4 {
                let mf = w
                    .spawn_px(
                        p,
                        galaxycraft::ids::unit::MINERAL_FIELD,
                        x + 96 + k * 32,
                        y + 96,
                    )
                    .expect("mineral field");
                w.units[mf].resource_amount = 1500;
            }
        }
        w
    };

    if fog {
        world.enable_fog();
    }
    let mut campaign_ais: Vec<CampaignAi> = campaign_script
        .as_ref()
        .map(|s| vec![CampaignAi::new(0, s)])
        .unwrap_or_default();
    let mut ais: Vec<SimpleAi> = (0..world.players.len() as u8).map(SimpleAi::new).collect();
    let mut last_report = 0u64;
    while world.tick < max_ticks && world.winner.is_none() {
        let enemy_base = |world: &World| {
            world
                .players
                .iter()
                .enumerate()
                .find(|(i, _)| *i != 0)
                .and_then(|(i, _)| {
                    world
                        .units
                        .iter()
                        .find(|u| u.alive && u.owner as usize == i && u.completed)
                        .map(|u| (u.x / 256, u.y / 256))
                })
        };
        for ai in &mut campaign_ais {
            let target = enemy_base(&world);
            ai.update(&mut world, target);
        }
        for ai in &mut ais {
            if campaign_ais.iter().any(|c| c.player == ai.player) {
                continue;
            }
            let target = enemy_base(&world);
            ai.update(&mut world, target);
        }
        world.step();
        // Trigger action 15/16 (`Run AI Script`) queues a script id; start it
        // for the requested player.
        if let Some(t) = world.triggers.as_mut() {
            let pending = std::mem::take(&mut t.pending_ai_scripts);
            for (player, id, at) in pending {
                if campaign_ais.iter().any(|c| c.player == player as u8) {
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
                    campaign_ais.push(ai);
                }
            }
        }
        if verbose && world.tick / 600 != last_report {
            last_report = world.tick / 600;
            println!(
                "t={} units={} p0={} p1={} minerals={:?}",
                world.tick,
                world.units.iter().filter(|u| u.alive).count(),
                world.unit_ids(0).len(),
                world.unit_ids(1).len(),
                world.players.iter().map(|p| p.minerals).collect::<Vec<_>>()
            );
        }
    }

    println!(
        "match over at tick {} ({}s): winner {:?}",
        world.tick,
        world.tick / 24,
        world.winner
    );
    for p in 0..world.players.len() {
        let units = world.unit_ids(p as u8);
        let army = units
            .iter()
            .filter(|id| {
                world
                    .unit(**id)
                    .map(|u| !world.data.unit(u.type_id).is_building())
                    .unwrap_or(false)
            })
            .count();
        println!(
            "  p{} {:?}: {} units ({} mobile), {} minerals, {} gas, {} techs",
            p,
            world.players[p].race,
            units.len(),
            army,
            world.players[p].minerals,
            world.players[p].gas,
            world.players[p].techs.iter().filter(|t| **t).count()
        );
        if verbose {
            let mut by_type: std::collections::BTreeMap<&str, usize> =
                std::collections::BTreeMap::new();
            for id in &units {
                if let Some(u) = world.unit(*id) {
                    *by_type.entry(world.data.unit_name(u.type_id)).or_default() += 1;
                }
            }
            println!("    {by_type:?}");
        }
    }
}

fn parse_race(s: &str) -> Race {
    match s.trim().to_ascii_lowercase().as_str() {
        "z" | "zerg" => Race::Zerg,
        "p" | "protoss" => Race::Protoss,
        "t" | "terran" => Race::Terran,
        _ => Race::Neutral,
    }
}

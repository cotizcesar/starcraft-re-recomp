//! Headless graphics smoke test: render one real map frame to a BMP.
//!
//! ```text
//! cargo run --release --bin sc_shot -- --ticks 0 --out /tmp/sc_shot.bmp
//! ```

use std::path::PathBuf;
use std::sync::Arc;

use galaxycraft::data::GameData;
use galaxycraft::map::Scenario;
use galaxycraft::render::{
    Framebuffer, Rgb, SpriteBank, Tileset, asset_root, draw_unit_box, era_name,
};
use galaxycraft::retail::RetailInstall;
use galaxycraft::sim::{Race, World};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut chk = asset_root().join("bottleneck.chk");
    let mut tiles = asset_root().join("TileSet");
    let mut assets = asset_root();
    let mut ticks = 0u64;
    let mut out = PathBuf::from("/tmp/sc_shot.bmp");
    let mut player = 0usize;
    let mut install: Option<PathBuf> = None;
    let mut cache: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--chk" => {
                chk = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--tiles" => {
                tiles = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--assets" => {
                assets = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--ticks" => {
                ticks = args[i + 1].parse().unwrap_or(0);
                i += 2;
            }
            "--out" => {
                out = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--install" => {
                install = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--cache" => {
                cache = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--player" => {
                player = args[i + 1].parse().unwrap_or(0);
                i += 2;
            }
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }

    let mut retail_data: Option<Arc<GameData>> = None;
    if let Some(path) = install {
        let install = RetailInstall::open(&path).unwrap_or_else(|e| {
            eprintln!("cannot open retail install {}: {e}", path.display());
            std::process::exit(2);
        });
        let cache = cache.unwrap_or_else(|| install.default_cache_dir());
        if let Err(e) = install.ensure_cache(&cache) {
            eprintln!(
                "cannot materialise retail assets into {}: {e}",
                cache.display()
            );
            std::process::exit(2);
        }
        eprintln!("retail assets: {}", cache.display());
        assets = cache.clone();
        tiles = cache.join("TileSet");
        if chk == asset_root().join("bottleneck.chk") {
            chk = cache.join("maps").join("(2)Bottleneck.chk");
        }
        retail_data = install.game_data().ok().map(Arc::new);
    }
    let data =
        retail_data.unwrap_or_else(|| Arc::new(GameData::bundled().expect("bundled tables")));
    let raw = std::fs::read(&chk).expect("scenario");
    let scenario = Scenario::parse(&raw).expect("valid CHK");
    let tileset = Tileset::load(&tiles, scenario.era, &scenario).expect("tileset assets");
    let tables = galaxycraft::map::TileTables::read_vf4(
        &tiles.join(format!("{}.vf4", era_name(scenario.era))),
    )
    .expect("tileset vf4 + cv5");
    let slots = scenario.player_slots();
    let races: Vec<Race> = slots.iter().map(|s| s.race(Race::Terran)).collect();
    let mut world = World::from_scenario(data.clone(), &scenario, &tables.data(), &races);
    for s in &slots {
        if let Some(p) = world.players.get_mut(s.slot) {
            p.defeated = !s.active;
            p.is_computer = s.computer;
        }
    }
    for _ in 0..ticks {
        world.step();
    }
    world.enable_fog();

    let start = world
        .units
        .iter()
        .find(|u| {
            u.alive
                && u.owner as usize == player
                && matches!(
                    u.type_id,
                    galaxycraft::ids::unit::COMMAND_CENTER
                        | galaxycraft::ids::unit::NEXUS
                        | galaxycraft::ids::unit::HATCHERY
                        | galaxycraft::ids::unit::LAIR
                        | galaxycraft::ids::unit::HIVE
                )
        })
        .or_else(|| {
            world
                .units
                .iter()
                .find(|u| u.alive && u.owner as usize == player)
        })
        .map(|u| (u.x / 256, u.y / 256))
        .unwrap_or((512, 512));

    let mut fb = Framebuffer::new(1280, 800);
    fb.clear(Rgb::new(0, 0, 0));
    let cam = (start.0 - 640, start.1 - 400);
    tileset.draw_terrain(&mut fb, &scenario, cam);
    {
        let mut creep = galaxycraft::render::CreepTiles::default();
        let mut rand = galaxycraft::render::CrtRand::new(0);
        creep.update(&mut rand, world.creep_grid(), scenario.width);
        tileset.draw_creep(&mut fb, &scenario, cam, &|x, y| creep.get(x, y));
    }

    let sprites = SpriteBank::load(&assets);
    let mut order: Vec<&galaxycraft::sim::SimUnit> =
        world.units.iter().filter(|u| u.alive).collect();
    order.sort_by_key(|u| u.y);
    for u in order {
        let sx = u.x / 256 - cam.0;
        let sy = u.y / 256 - cam.1;
        if sx < -128 || sy < -128 || sx > fb.width + 128 || sy > fb.height + 128 {
            continue;
        }
        if std::env::var("SC_SHOT_DEBUG").is_ok() && (420..480).contains(&sy) {
            eprintln!(
                "draw id={} {} owner={} screen=({}, {}) frame_dir={}",
                u.id,
                world.data.unit_name(u.type_id),
                u.owner,
                sx,
                sy,
                u.facing
            );
        }
        if !sprites.draw_unit(&mut fb, u, (sx, sy), world.tick) {
            draw_unit_box(&mut fb, u, &world.data, (sx, sy), false);
        }
        sprites.draw_health(&mut fb, u, &world.data, (sx, sy));
    }

    if std::env::var("SC_SHOT_DEBUG").is_ok() {
        for u in world.units.iter().filter(|u| u.alive && u.owner == 0) {
            let ty = world.data.unit(u.type_id);
            eprintln!(
                "id={} type={} {} at {} {} hp={}",
                u.id,
                u.type_id,
                world.data.unit_name(u.type_id),
                u.x / 256,
                u.y / 256,
                u.hp
            );
            let _ = ty;
        }
    }
    fb.text(12, 12, "GALAXYCRAFT", Rgb::new(255, 220, 80), 2);
    fb.text(
        12,
        36,
        &format!(
            "{}  composite {}  tick {}  units {}",
            chk.display(),
            tileset.name,
            world.tick,
            world.units.iter().filter(|u| u.alive).count()
        ),
        Rgb::new(220, 220, 220),
        1,
    );
    fb.write_bmp(&out).expect("write screenshot");
    println!(
        "rendered {}x{} {} at tick {} to {}",
        fb.width,
        fb.height,
        tileset.name,
        world.tick,
        out.display()
    );
    println!(
        "units alive: {}  player {} units: {}  assets: {}",
        world.units.iter().filter(|u| u.alive).count(),
        player,
        world
            .units
            .iter()
            .filter(|u| u.alive && u.owner as usize == player)
            .count(),
        assets.display()
    );
}

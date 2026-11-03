//! Graphics-layer checks: bundled tileset chain, GRP decoder and sprite bank.

use galaxycraft::map::Scenario;
use galaxycraft::render::{
    asset_root, Framebuffer, Grp, MinimapDot, MinimapTerrain, Rgb, SpriteBank, Tileset,
};
use std::fs;

#[test]
fn bundled_grp_decoder_reads_a_real_marine_frame() {
    let path = asset_root().join("unit/terran/marine.grp");
    let bytes = fs::read(&path).expect("bundled marine.grp");
    let grp = Grp::parse(&bytes).expect("valid GRP");
    assert_eq!(grp.frames.len(), 229);
    assert_eq!((grp.width, grp.height), (64, 64));
    let frame = grp.frame(0).expect("frame 0");
    assert_eq!((frame.w, frame.h), (20, 26));
    assert!(
        frame.indices.iter().filter(|&&c| c != 0).count() > 50,
        "frame 0 has visible pixels"
    );
}

#[test]
fn bundled_tileset_renders_and_builds_a_minimap() {
    let assets = asset_root();
    let raw = fs::read(assets.join("bottleneck.chk")).expect("bundled map");
    let scenario = Scenario::parse(&raw).expect("valid CHK");
    assert_eq!(scenario.era, 1);
    let tileset = Tileset::load(&assets.join("TileSet"), scenario.era, &scenario)
        .expect("bundled platform tileset");
    assert_eq!(tileset.name, "platform");
    assert_eq!(tileset.minimap_size, (scenario.width, scenario.height));
    assert_eq!(tileset.minimap.len(), scenario.width * scenario.height);

    let mut fb = Framebuffer::new(64, 64);
    fb.clear(Rgb::new(0, 0, 0));
    // Render four map tiles; a real floor tile must produce non-black pixels.
    for (i, &mtxm) in scenario.tiles.iter().take(4).enumerate() {
        tileset.draw_tile(&mut fb, mtxm, (i as i32 % 2) * 32, (i as i32 / 2) * 32);
    }
    let lit = fb.pixels.iter().filter(|&&p| p != 0).count();
    assert!(lit > 500, "tile renderer produced only {lit} lit pixels");
}

#[test]
fn bundled_sprite_bank_covers_the_playable_races() {
    let bank = SpriteBank::load(&asset_root());
    for id in [
        galaxycraft::ids::unit::MARINE,
        galaxycraft::ids::unit::SCV,
        galaxycraft::ids::unit::ZEALOT,
        galaxycraft::ids::unit::PROBE,
        galaxycraft::ids::unit::ZERGLING,
        galaxycraft::ids::unit::DRONE,
        galaxycraft::ids::unit::COMMAND_CENTER,
        galaxycraft::ids::unit::NEXUS,
        galaxycraft::ids::unit::HATCHERY,
        galaxycraft::ids::unit::MINERAL_FIELD,
        galaxycraft::ids::unit::VESPENE_GEYSER,
    ] {
        assert!(bank.has(id), "missing bundled sprite for unit {id}");
    }
}

#[test]
fn screenshot_binary_writes_a_bmp() {
    let out = std::env::temp_dir().join("galaxycraft_render_test.bmp");
    let mut fb = Framebuffer::new(16, 8);
    fb.clear(Rgb::new(12, 34, 56));
    fb.write_bmp(&out).expect("write BMP");
    let bytes = fs::read(&out).expect("read BMP");
    assert_eq!(&bytes[0..2], b"BM");
    assert_eq!(bytes.len(), 54 + 16 * 3 * 8);
    let _ = fs::remove_file(out);
}

#[test]
fn creep_overlay_is_animated_from_the_gameplay_maturity_grid() {
    use galaxycraft::ids::unit as uid;
    use galaxycraft::render::draw_creep;
    use galaxycraft::sim::{Race, World};
    use std::sync::Arc;

    let assets = asset_root();
    let raw = fs::read(assets.join("maps/Enslavers 01.chk")).expect("bundled map");
    let scenario = Scenario::parse(&raw).expect("valid CHK");
    let vf4 = fs::read(assets.join("TileSet").join(format!(
        "{}.vf4",
        galaxycraft::render::era_name(scenario.era)
    )))
    .expect("vf4");
    let cv5 = fs::read(assets.join("TileSet").join(format!(
        "{}.cv5",
        galaxycraft::render::era_name(scenario.era)
    )))
    .expect("cv5");
    let data = Arc::new(galaxycraft::data::GameData::bundled().unwrap());
    let slots = scenario.player_slots();
    let races: Vec<Race> = slots.iter().map(|s| s.race(Race::Terran)).collect();
    let mut world = World::from_scenario(
        data,
        &scenario,
        &galaxycraft::map::TileData::new(&cv5, &vf4),
        &races,
    );
    // The shipped mission places Zerg structures; finish them so creep exists
    // even on the first tick, as it would in a running match.
    for u in &mut world.units {
        if matches!(
            u.type_id,
            uid::HATCHERY
                | uid::LAIR
                | uid::HIVE
                | uid::CREEP_COLONY
                | uid::SUNKEN_COLONY
                | uid::SPORE_COLONY
        ) {
            u.completed = true;
            u.alive = true;
            if u.hp <= 0 {
                u.hp = 256;
            }
        }
    }
    let grid = world.creep_grid().to_vec();
    let mature: Vec<(i32, i32)> = grid
        .iter()
        .enumerate()
        .filter(|(_, &s)| s >= 3)
        .map(|(i, _)| ((i % scenario.width) as i32, (i / scenario.width) as i32))
        .collect();
    assert!(
        !mature.is_empty(),
        "map has no creep after completing sources"
    );
    let (tx, ty) = mature[mature.len() / 2];
    let cam = (tx * 32 - 64, ty * 32 - 48);
    let tileset =
        Tileset::load(&assets.join("TileSet"), scenario.era, &scenario).expect("bundled tileset");

    let mut first = Framebuffer::new(160, 128);
    tileset.draw_terrain(&mut first, &scenario, cam);
    let terrain_only = first.pixels.clone();
    draw_creep(&mut first, &scenario, cam, &grid, 0);
    assert_ne!(
        first.pixels, terrain_only,
        "creep did not alter the terrain pixels"
    );

    let mut later = Framebuffer::new(160, 128);
    tileset.draw_terrain(&mut later, &scenario, cam);
    draw_creep(&mut later, &scenario, cam, &grid, 48);
    assert_ne!(
        first.pixels, later.pixels,
        "creep texture has no animation between ticks"
    );
}

#[test]
fn bundled_map_pack_loads_and_runs_with_the_playable_assets() {
    use galaxycraft::sim::{Race, World};
    use std::sync::Arc;

    let assets = asset_root();
    let maps_dir = assets.join("maps");
    let entries = fs::read_dir(&maps_dir).expect("bundled maps directory");
    let mut loaded = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("chk") {
            continue;
        }
        let raw = fs::read(&path).expect("map bytes");
        let scenario = Scenario::parse(&raw).expect("valid bundled map");
        let tileset = Tileset::load(&assets.join("TileSet"), scenario.era, &scenario)
            .unwrap_or_else(|| panic!("missing tileset for {}", path.display()));
        assert_eq!(tileset.minimap.len(), scenario.width * scenario.height);
        let vf4 = fs::read(assets.join("TileSet").join(format!(
            "{}.vf4",
            galaxycraft::render::era_name(scenario.era)
        )))
        .expect("vf4");
        let cv5 = fs::read(assets.join("TileSet").join(format!(
            "{}.cv5",
            galaxycraft::render::era_name(scenario.era)
        )))
        .expect("cv5");
        let starts = scenario.start_locations().len();
        let players = if starts > 0 { starts.clamp(2, 8) } else { 2 };
        let races = vec![Race::Terran; players];
        let data = Arc::new(galaxycraft::data::GameData::bundled().unwrap());
        let mut world = World::from_scenario(
            data,
            &scenario,
            &galaxycraft::map::TileData::new(&cv5, &vf4),
            &races,
        );
        world.enable_fog();
        for _ in 0..120 {
            world.step();
        }
        assert!(!world.units.is_empty(), "{} has no units", path.display());
        loaded += 1;
    }
    assert!(
        loaded >= 13,
        "expected at least 13 bundled maps, got {loaded}"
    );
}

#[test]
fn minimap_blips_are_outlined_and_prepainted_backgrounds_survive() {
    let assets = asset_root();
    let raw = fs::read(assets.join("bottleneck.chk")).expect("bundled map");
    let scenario = Scenario::parse(&raw).expect("valid CHK");
    let tileset = Tileset::load(&assets.join("TileSet"), scenario.era, &scenario)
        .expect("bundled platform tileset");
    let (mw, mh) = tileset.minimap_size;
    let (w, h) = (mw as i32 * 3, mh as i32 * 3);
    // A 4 x 3-tile building whose top-left tile is (10, 10).
    let unit = MinimapDot {
        x: 10 * 32 + 64,
        y: 10 * 32 + 48,
        owner: 0,
        placement: (128, 96),
        building: true,
        resource: false,
    };
    let draw = |mode| {
        let mut fb = Framebuffer::new(w as usize, h as usize);
        fb.clear(Rgb::new(200, 0, 0));
        tileset.draw_minimap_with_terrain(
            &mut fb,
            (0, 0, w, h),
            &[unit],
            (0, 0, 0, 0),
            None,
            0,
            mode,
        );
        fb
    };
    let painted = draw(MinimapTerrain::Prepainted);
    // The blip fills tiles 10..14 x 10..13 (3 px each) with a 1 px black
    // outline; the caller's background stays everywhere else.
    assert_eq!(painted.get(30, 30), Rgb::new(16, 252, 24));
    assert_eq!(painted.get(41, 38), Rgb::new(16, 252, 24));
    assert_eq!(painted.get(29, 30), Rgb::new(0, 0, 0), "left outline");
    assert_eq!(painted.get(42, 30), Rgb::new(0, 0, 0), "right outline");
    assert_eq!(painted.get(30, 39), Rgb::new(0, 0, 0), "bottom outline");
    assert_eq!(
        painted.get(100, 100),
        Rgb::new(200, 0, 0),
        "background kept"
    );
    let hidden = draw(MinimapTerrain::Hidden);
    assert_eq!(hidden.get(100, 100), Rgb::new(0, 0, 0), "black radar");
}

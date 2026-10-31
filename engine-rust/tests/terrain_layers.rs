//! Terrain must be opaque: the CV5 group background is drawn under each
//! megatile, so no map tile may leave holes in the framebuffer.
//!
//! Regression for the "black rectangles in the terrain" bug: before the
//! two-layer fix, 2 of 698 distinct tiles of Boxer.chk drew nothing and
//! dozens were only 30-90% covered.

use galaxycraft::map::Scenario;
use galaxycraft::render::{Framebuffer, Rgb, Tileset};
use std::collections::BTreeSet;

#[test]
fn every_shipped_map_tile_is_fully_opaque() {
    let raw = std::fs::read("assets/maps/Boxer.chk").expect("bundled Boxer");
    let sc = Scenario::parse(&raw).expect("parse");
    let ts = Tileset::load(std::path::Path::new("assets/TileSet"), sc.era, &sc)
        .expect("bundled tileset");

    let distinct: BTreeSet<u16> = sc.tiles.iter().copied().collect();
    assert!(distinct.len() > 100, "expected a varied map");

    let mut holes = Vec::new();
    for &tile in &distinct {
        let mut fb = Framebuffer::new(32, 32);
        fb.clear(Rgb::new(0, 0, 0));
        ts.draw_tile(&mut fb, tile, 0, 0);
        let mut painted = 0;
        for y in 0..32 {
            for x in 0..32 {
                if fb.get(x, y).0 != 0 {
                    painted += 1;
                }
            }
        }
        if painted < 1024 {
            holes.push((tile, painted));
        }
    }
    assert!(
        holes.is_empty(),
        "tiles with transparent holes (tile, painted): {holes:?}"
    );
}

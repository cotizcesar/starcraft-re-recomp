//! Print the recovered region graph for a scenario.
//!
//! ```text
//! sc_regions Bottleneck.chk platform.vf4 [--list]
//! ```
//!
//! The retail builder (`0x7a302c10`) decomposes the map's tile grid into
//! pathfinding regions, stores a bounding-box centre for each and links
//! adjacent regions.  This tool exposes exactly what the reconstruction
//! built so the graph can be compared against in-game observations.

use std::path::PathBuf;

use galaxycraft::map::{Scenario, Terrain};
use galaxycraft::path::RegionMap;

fn main() {
    let mut args = std::env::args().skip(1);
    let chk = args.next().unwrap_or_else(|| {
        eprintln!("usage: sc_regions <scenario.chk> <tileset.vf4> [--list]");
        std::process::exit(2)
    });
    let vf4_path = args.next().unwrap_or_else(|| {
        eprintln!("usage: sc_regions <scenario.chk> <tileset.vf4> [--list]");
        std::process::exit(2)
    });
    let list = args.any(|a| a == "--list");
    let bytes = std::fs::read(&chk).unwrap_or_else(|e| {
        eprintln!("cannot read {chk}: {e}");
        std::process::exit(1)
    });
    let tiles = galaxycraft::map::TileTables::read_vf4(std::path::Path::new(&vf4_path))
        .unwrap_or_else(|e| {
            eprintln!("cannot read {vf4_path} (+ .cv5): {e}");
            std::process::exit(1)
        });
    let scenario = Scenario::parse(&bytes).unwrap_or_else(|e| {
        eprintln!("cannot parse {chk}: {e}");
        std::process::exit(1)
    });
    let terrain = Terrain::from_scenario(&scenario, &tiles.data());
    let regions = RegionMap::from_terrain(&terrain);
    let total_tiles = (regions.width * regions.height) as u64;
    let walkable: u64 = regions.regions().iter().map(|r| r.size as u64).sum();
    let largest = regions.regions().iter().map(|r| r.size).max().unwrap_or(0);
    let smallest = regions.regions().iter().map(|r| r.size).min().unwrap_or(0);
    let edges: usize = regions.regions().iter().map(|r| r.neighbours.len()).sum();
    println!(
        "{}: {}x{} tiles, {} walkable, {} regions, largest {}, smallest {}, {} directed edges",
        PathBuf::from(&chk)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&chk),
        regions.width,
        regions.height,
        walkable,
        regions.count(),
        largest,
        smallest,
        edges
    );
    let contours = galaxycraft::contour::Contours::from_terrain(&terrain);
    println!(
        "contours: up {} right {} down {} left {} segments",
        contours.lists[0].len(),
        contours.lists[1].len(),
        contours.lists[2].len(),
        contours.lists[3].len()
    );
    if list {
        let mut list: Vec<_> = regions.regions().iter().collect();
        list.sort_by_key(|r| std::cmp::Reverse(r.size));
        for r in list {
            println!(
                "region {}: size {} bbox ({},{})-({},{}) centre {:?} neighbours {:?}",
                r.id,
                r.size,
                r.min_tile.0,
                r.min_tile.1,
                r.max_tile.0,
                r.max_tile.1,
                r.centre_cell(),
                r.neighbours
            );
        }
    }
    let _ = total_tiles;
}

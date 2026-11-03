//! The Remastered HD megatile archive (`hd2/tileset/<era>.dds.vr4`) is what the
//! retail client draws the terrain from. These checks fail if the container
//! parsing regresses; they skip when the retail install is not materialised.

use std::path::Path;

use galaxycraft::render::load_hd_megatiles;

fn hd_path() -> Option<std::path::PathBuf> {
    let root = std::env::var("GC_RETAIL_CACHE").unwrap_or_else(|_| {
        let base = std::env::var("XDG_CACHE_HOME")
            .unwrap_or_else(|_| format!("{}/.cache", std::env::var("HOME").unwrap_or_default()));
        format!("{base}/galaxycraft/retail-864772b9")
    });
    let p = Path::new(&root).join("hd2/tileset/badlands.dds.vr4");
    p.exists().then_some(p)
}

#[test]
fn hd_megatile_archive_decodes() {
    let Some(path) = hd_path() else {
        eprintln!("skip: HD tileset not materialised on this machine");
        return;
    };
    let tiles = load_hd_megatiles(&path).expect("decode the HD megatile archive");
    assert!(
        tiles.len() > 6000,
        "expected the full megatile set, got {}",
        tiles.len()
    );
    let decoded = tiles.iter().filter(|t| t.is_some()).count();
    assert!(
        decoded * 10 >= tiles.len() * 9,
        "at least 90% of megatiles must decode, got {decoded}/{}",
        tiles.len()
    );
    // Every decoded megatile is 32x32 (the SD world scale). The archive starts
    // with unused slots, so the alpha check runs over a sample of real tiles.
    let sample: Vec<_> = tiles.iter().flatten().take(200).collect();
    for t in &sample {
        assert_eq!(t.len(), 32 * 32);
    }
    let opaque = sample
        .iter()
        .filter(|t| t.iter().any(|p| (*p >> 24) != 0))
        .count();
    assert!(
        opaque * 2 >= sample.len(),
        "most sampled megatiles must carry pixels, got {opaque}/{}",
        sample.len()
    );
}

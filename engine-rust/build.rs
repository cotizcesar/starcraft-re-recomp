use std::path::{Path, PathBuf};

/// Retail-derived files the crate embeds.  They are Blizzard data, so the
/// repository does not carry them (see `fixtures/README.md` for how to
/// regenerate each one from your own install).  Each is mirrored into
/// `$OUT_DIR/embed/` and read through `embedded!`; a missing file becomes a
/// placeholder so the crate (and `sc_assets fixtures`, which writes them)
/// still builds.  Placeholders are not valid data: `GameData::bundled()`
/// returns an error and the fixture-backed tests fail until they exist.
const EMBEDDED: &[(&str, &str)] = &[
    ("fixtures/units.dat", ""),
    ("fixtures/weapons.dat", ""),
    ("fixtures/flingy.dat", ""),
    ("fixtures/orders.dat", ""),
    ("fixtures/techdata.dat", ""),
    ("fixtures/upgrades.dat", ""),
    ("fixtures/sprites.dat", ""),
    ("fixtures/images.dat", ""),
    ("fixtures/sfxdata.dat", ""),
    ("fixtures/sfxdata.tbl", ""),
    ("fixtures/stat_txt.tbl", ""),
    ("fixtures/iscript.bin", ""),
    ("fixtures/aiscript.bin", ""),
    ("fixtures/bwscript.bin", ""),
    ("fixtures/melee.trg", ""),
    ("fixtures/damage_multipliers.bin", ""),
    ("fixtures/hotkeys.json", "{\"actions\":{},\"units\":{}}"),
    ("fixtures/button_sets.in", "&[]"),
    ("src/menu_masks/profile.bin", ""),
    ("src/menu_masks/version.bin", ""),
    ("src/menu_masks/single.bin", ""),
    ("src/menu_masks/multi.bin", ""),
    ("src/menu_masks/options.bin", ""),
    ("src/menu_masks/collections.bin", ""),
    ("src/menu_masks/hotkeys.bin", ""),
    ("src/menu_masks/credits.bin", ""),
    ("src/menu_masks/intro.bin", ""),
    ("src/menu_masks/exit.bin", ""),
    ("src/menu_masks/single_hover.bin", ""),
    ("src/menu_masks/multi_hover.bin", ""),
];

fn mirror_embedded(out: &Path) {
    // Watch the directories (a missing watched file would rerun every build).
    println!("cargo:rerun-if-changed=fixtures");
    println!("cargo:rerun-if-changed=src/menu_masks");
    let mut missing = Vec::new();
    for &(rel, placeholder) in EMBEDDED {
        let dst = out.join("embed").join(rel);
        std::fs::create_dir_all(dst.parent().expect("embed path has a parent"))
            .expect("create embed dir");
        let bytes = match std::fs::read(rel) {
            Ok(bytes) => bytes,
            Err(_) => {
                missing.push(rel);
                placeholder.as_bytes().to_vec()
            }
        };
        // Only rewrite on change so dependants are not rebuilt needlessly.
        if std::fs::read(&dst).ok().as_deref() != Some(&bytes[..]) {
            std::fs::write(&dst, &bytes).expect("write embedded file");
        }
    }
    if !missing.is_empty() {
        println!(
            "cargo:warning={} retail-derived data files are missing ({}...); \
             see fixtures/README.md (`sc_assets --install DIR fixtures`)",
            missing.len(),
            missing[0]
        );
    }
}

fn main() {
    // MIT-licensed StormLib PKWARE DCL explode implementation.  It is only
    // used for the fifth compression method found in shipped MPQ maps.
    cc::Build::new()
        .file("third_party/pklib/explode.c")
        .file("third_party/pklib/crc32.c")
        .file("third_party/pklib/sc_shim.c")
        .include("third_party/pklib")
        .warnings(false)
        .compile("sc_pklib");
    println!("cargo:rerun-if-changed=third_party/pklib");

    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    mirror_embedded(&out);
}

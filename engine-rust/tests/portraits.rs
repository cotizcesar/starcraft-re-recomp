//! Retail portraits ship as short WebM clips; the client decodes the first
//! painted frame on demand. Skips when the retail install is absent.

use std::path::{Path, PathBuf};

fn install_root() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("SC_INSTALL") {
        let p = PathBuf::from(p);
        if p.is_dir() {
            return Some(p);
        }
    }
    None
}

#[test]
fn marine_portrait_decodes_from_the_install() {
    let Some(root) = install_root() else {
        eprintln!("skip: no retail install");
        return;
    };
    let inst = galaxycraft::retail::RetailInstall::open(Path::new(&root)).expect("open install");
    let path = "portrait/tmarine/tmafid00.webm";
    let bytes = inst.read_by_path(path).expect("portrait present");
    assert!(bytes.len() > 1000, "portrait clip is suspiciously small");
    let info = galaxycraft::video::parse_webm(&bytes).expect("webm header");
    assert!(info.pixel_width > 0 && info.pixel_height > 0);
    let frame = galaxycraft::video::decode_frame_at(&bytes, &info, Some(0.6), 256)
        .expect("decoded portrait frame");
    assert_eq!(frame.rgb.len(), (frame.width * frame.height * 3) as usize);
    // A portrait must not be a blank image.
    let bright = frame.rgb.iter().filter(|b| **b > 24).count();
    assert!(bright > 1000, "portrait frame looks empty");
}

/// Full-motion HUD portraits: the retail `portdata.dat` weights sum to 100
/// for every portrait, every idle stem has its `...0.webm` clip, and the
/// Marine's idle / talking clips decode to several distinct frames at the
/// shipped 10 fps (played frame by frame by `sc_view`).
#[test]
fn portrait_clips_resolve_from_portdata_and_decode_as_motion() {
    use galaxycraft::portrait::{PORTRAIT_FPS, PORTRAITS, PortData, PortraitClip, clip_path};
    let Some(root) = install_root() else {
        eprintln!("skip: no retail install");
        return;
    };
    let inst = galaxycraft::retail::RetailInstall::open(Path::new(&root)).expect("open install");
    let dat =
        PortData::parse(&inst.read("arr/portdata.dat").expect("portdata.dat")).expect("1320 bytes");
    let tbl = galaxycraft::tables::StringTable::parse(
        &inst.read("arr/portdata.tbl").expect("portdata.tbl"),
    )
    .expect("portdata.tbl parses");
    let mut stems = 0;
    for p in 0..PORTRAITS as u16 {
        let w = dat.idle_weights(p);
        assert_eq!(
            w.iter().map(|&v| v as u32).sum::<u32>(),
            100,
            "portrait {p} weights {w:?}"
        );
        let idle = PortraitClip {
            portrait: p,
            talking: false,
            clip: 0,
        };
        if let Some(path) = clip_path(&dat, &tbl, idle, 0) {
            assert!(inst.read_by_path(&path).is_ok(), "{path} missing");
            stems += 1;
        }
    }
    assert!(stems > 60, "{stems} idle stems");

    // Marine: units.dat portrait id via the bundled tables.
    let data = galaxycraft::data::GameData::bundled().expect("bundled tables");
    let marine = data.unit(galaxycraft::ids::unit::MARINE).portrait;
    for (talking, n) in [(false, 0u8), (false, 3), (true, 0), (true, 2)] {
        let c = PortraitClip {
            portrait: marine,
            talking,
            clip: n,
        };
        let path = clip_path(&dat, &tbl, c, n).expect("marine clip path");
        assert!(path.starts_with("portrait/tmarine/tma"), "{path}");
        let bytes = inst.read_by_path(&path).expect("clip present");
        let info = galaxycraft::video::parse_webm(&bytes).expect("webm header");
        let frames = galaxycraft::video::decode_frames(&bytes, &info, 0.0, 64, PORTRAIT_FPS, 256);
        assert!(frames.len() >= 10, "{path}: {} frames", frames.len());
        let distinct: std::collections::HashSet<&[u8]> =
            frames.iter().map(|f| &f.rgb[..]).collect();
        assert!(
            distinct.len() >= 3,
            "{path}: only {} distinct frames",
            distinct.len()
        );
    }
}

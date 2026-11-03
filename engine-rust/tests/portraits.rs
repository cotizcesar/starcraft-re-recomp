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
    let default = PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join("Games/battlenet/drive_c/Program Files (x86)/StarCraft");
    default.is_dir().then_some(default)
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

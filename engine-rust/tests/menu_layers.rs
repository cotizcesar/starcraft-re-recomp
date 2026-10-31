//! The Remastered menu layers must actually animate: the decoder has to
//! return several *different* frames, and the picker has to cycle them.
//!
//! Skips silently when the retail install is not materialised, so the suite
//! stays runnable on a clean machine.

use galaxycraft::video::{decode_frames, parse_webm};

fn layer_path() -> Option<std::path::PathBuf> {
    let cache = std::env::var("GALAXYCRAFT_CACHE")
        .map(std::path::PathBuf::from)
        .or_else(|_| {
            std::env::var("XDG_CACHE_HOME")
                .map(std::path::PathBuf::from)
                .or_else(|_| {
                    std::env::var("HOME").map(|h| std::path::PathBuf::from(h).join(".cache"))
                })
        })
        .ok()?
        .join("galaxycraft");
    let dir = std::fs::read_dir(cache).ok()?;
    for entry in dir.flatten() {
        let p = entry.path().join("glue/mainmenu/single.webm");
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

#[test]
fn menu_layer_yields_distinct_frames() {
    let Some(path) = layer_path() else {
        eprintln!("skip: retail menu layer not materialised");
        return;
    };
    let bytes = std::fs::read(&path).expect("read layer");
    let info = parse_webm(&bytes).expect("webm header");
    let frames = decode_frames(&bytes, &info, 1.5, 6, 6.0, 1280);
    assert!(
        frames.len() >= 2,
        "expected several frames, got {}",
        frames.len()
    );
    let first = &frames[0].rgb;
    let later = &frames[frames.len() - 1].rgb;
    assert_eq!(frames[0].width, frames[1].width);
    assert!(
        first != later,
        "all decoded frames are identical: the layer is not animating"
    );
    for f in &frames {
        assert_eq!(f.rgb.len(), (f.width * f.height * 3) as usize);
    }
}

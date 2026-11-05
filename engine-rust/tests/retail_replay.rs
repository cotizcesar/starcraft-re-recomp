//! Reads whatever retail `.rep` files are present and reports their contents.
//!
//! The unit tests live next to the parser; this integration test is the
//! observable evidence: it prints the container version, the map, the size of
//! the embedded CHK, the named sections and whether an action stream is present
//! (the shipped autosaves have none; a player-saved replay does).

use std::path::{Path, PathBuf};

use galaxycraft::retail_replay::RetailReplay;

fn replay_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(dir) = std::env::var("SC_REPLAY_DIR") {
        out.push(PathBuf::from(dir));
    }
    if let Some(base) = galaxycraft::retail_replay::default_replay_base() {
        out.push(base.join("AutoSave/20261027"));
        out.push(base.join("AutoSave/20261025"));
        out.push(base);
    }
    out
}

fn replays() -> Vec<PathBuf> {
    let mut files = Vec::new();
    for dir in replay_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("rep") {
                files.push(p);
            }
        }
    }
    files.sort();
    files
}

#[test]
fn every_available_replay_parses_and_reports_its_contents() {
    let files = replays();
    if files.is_empty() {
        eprintln!("skip: no .rep files on this machine");
        return;
    }
    let mut with_commands = 0usize;
    for path in &files {
        let bytes = std::fs::read(path).expect("read replay");
        let rep = RetailReplay::parse(&bytes)
            .unwrap_or_else(|| panic!("{} did not parse as a replay", path.display()));
        println!(
            "{}: v{:#x} map={:?} chk={} B sections={:?} commands={} B ({})",
            path.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
            rep.version,
            rep.map_name,
            rep.chk.len(),
            rep.sections,
            rep.commands.len(),
            if rep.has_commands() {
                "action stream present"
            } else {
                "map + settings only"
            }
        );
        if rep.sections.is_empty() {
            // Newer container revision (0x8f8e, e.g. a replay saved from a
            // 1v1): the map block does not start with `VER ` and the named
            // sections live in a tail this reader does not split yet, but the
            // payload and the action stream are still exposed.
            assert!(
                !rep.chk.is_empty() || rep.has_commands(),
                "{} has neither a map block nor an action stream",
                path.display()
            );
        } else if rep.chk.starts_with(b"VER ") {
            // Old container revision: the CHK starts at the first block.
        } else {
            // Some 1.23 replays carry the map in a later framed run (the reader
            // now walks it) or none at all; only require a payload, never a
            // specific first four bytes.
            assert!(!rep.chk.is_empty(), "{} has no map payload", path.display());
        }
        if rep.has_commands() {
            with_commands += 1;
        }
    }
    println!(
        "{} replay(s) parsed, {} with an action stream",
        files.len(),
        with_commands
    );
    let _ = Path::new(".");
}

//! Inspect retail `.rep` files: container, embedded map and action stream.
//!
//! ```text
//! sc_replay <file.rep> [...]
//! ```
//!
//! Prints the container version, the map, the size of the embedded CHK, the
//! named sections and — when the file was saved by a player (F10 → Save
//! Replay) — the decoded action stream: number of commands, whether the
//! 4-byte layout holds, the last frame and the per-player counts.

use std::process::exit;

use galaxycraft::retail_replay::RetailReplay;

fn main() {
    let files: Vec<String> = std::env::args().skip(1).collect();
    if files.is_empty() {
        eprintln!("usage: sc_replay <file.rep> [...]");
        exit(2);
    }
    for path in &files {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("{path}: cannot read: {e}");
                continue;
            }
        };
        let Some(rep) = RetailReplay::parse(&bytes) else {
            eprintln!("{path}: not a StarCraft Remastered replay");
            continue;
        };
        println!("== {path}");
        println!(
            "   container v{:#x}  file {} B  map {:?}",
            rep.version,
            bytes.len(),
            rep.map_name
        );
        println!("   chk {} B  sections {:?}", rep.chk.len(), rep.sections);
        // Does the embedded map actually rebuild a scenario? That is the
        // difference between "container parsed" and "replay playable".
        match galaxycraft::map::Scenario::parse(&rep.chk) {
            Ok(sc) => println!(
                "   scenario: {}x{} tiles, {} units, {} triggers, {} strings",
                sc.width,
                sc.height,
                sc.units.len(),
                sc.triggers.len(),
                sc.strings.len()
            ),
            Err(e) => println!("   scenario: cannot rebuild ({e})"),
        }
        println!(
            "   players: {:?}",
            rep.slots.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
        match rep.command_stream() {
            None => println!("   action stream: none (map + settings only)"),
            Some(st) => {
                println!(
                    "   action stream: {} B, {} commands, layout plausible: {}, last frame {} ({:.1} s)",
                    rep.commands.len(),
                    st.len(),
                    st.plausible,
                    st.last_frame(),
                    st.last_frame() as f32 / 24.0
                );
                println!("   per player: {:?}", st.per_player());
                for c in st.commands.iter().take(8) {
                    println!(
                        "     frame {:6}  player {}  action {:#06x}",
                        c.frame, c.player, c.action
                    );
                }
                if st.len() > 8 {
                    println!("     … {} more", st.len() - 8);
                }
            }
        }
    }
}

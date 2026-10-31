//! Audit the shipped map corpus for content the rules engine may not cover.
//!
//! ```text
//! sc_audit --maps /tmp/sc_maps
//! ```
//!
//! The tool parses every extracted `scenario.chk` with the same parser the
//! simulator uses and prints histograms of
//!
//! * trigger condition ids (`TRIG` byte 15 of each 20-byte condition),
//! * trigger action ids (`TRIG` byte 26 of each 32-byte action),
//! * placed unit type ids (`UNIT`),
//! * the maps' tileset eras,
//!
//! together with the ids that fall outside the recovered classic trigger set
//! (`conditions 0..=23`, `actions 1..=59`).  The output is deterministic and is
//! used by `every_shipped_map_uses_only_classic_trigger_ids`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use galaxycraft::map::Scenario;
use galaxycraft::trigger::RECORD_SIZE;

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut maps = PathBuf::from("/tmp/sc_maps");
    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--maps" => {
                maps = PathBuf::from(raw.get(i + 1).cloned().unwrap_or_default());
                i += 2;
            }
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }

    let mut files = Vec::new();
    collect(&maps, &mut files);
    files.sort();
    if files.is_empty() {
        eprintln!("no .chk files under {}", maps.display());
        std::process::exit(2);
    }

    let mut conditions: BTreeMap<u8, usize> = BTreeMap::new();
    let mut actions: BTreeMap<u8, usize> = BTreeMap::new();
    let mut unit_types: BTreeMap<u16, usize> = BTreeMap::new();
    let mut eras: BTreeMap<u16, usize> = BTreeMap::new();
    let mut loaded = 0usize;
    let mut failed = 0usize;
    let mut trigger_maps = 0usize;
    let mut extra_condition_uses = 0usize;
    let mut extra_action_uses = 0usize;
    let mut extra_examples: Vec<String> = Vec::new();

    for path in &files {
        let name = path
            .strip_prefix(&maps)
            .unwrap_or(path)
            .display()
            .to_string();
        let Ok(bytes) = std::fs::read(path) else {
            failed += 1;
            println!("FAIL {name}: unreadable");
            continue;
        };
        let Ok(scenario) = Scenario::parse(&bytes) else {
            failed += 1;
            println!("FAIL {name}: parse");
            continue;
        };
        loaded += 1;
        *eras.entry(scenario.era & 7).or_default() += 1;
        for u in &scenario.units {
            *unit_types.entry(u.unit_id).or_default() += 1;
        }
        let mut has_trigger = false;
        for record in scenario.triggers.as_chunks::<RECORD_SIZE>().0 {
            has_trigger = true;
            for c in 0..16 {
                let kind = record[c * 20 + 15];
                if kind == 0 {
                    continue;
                }
                *conditions.entry(kind).or_default() += 1;
                if kind > 23 {
                    extra_condition_uses += 1;
                    if extra_examples.len() < 24 {
                        extra_examples.push(format!("{name}: condition {kind}"));
                    }
                }
            }
            for a in 0..64 {
                let kind = record[320 + a * 32 + 26];
                if kind == 0 {
                    continue;
                }
                *actions.entry(kind).or_default() += 1;
                if kind > 59 {
                    extra_action_uses += 1;
                    if extra_examples.len() < 24 {
                        extra_examples.push(format!("{name}: action {kind}"));
                    }
                }
            }
        }
        if has_trigger {
            trigger_maps += 1;
        }
    }

    println!(
        "audit: {loaded}/{} maps parsed, {failed} failed, {trigger_maps} with TRIG",
        files.len()
    );
    print_hist("condition ids", &conditions);
    print_hist("action ids", &actions);
    print_hist_u16("tileset eras", &eras);
    println!(
        "unit type ids: {} distinct, {} placements",
        unit_types.len(),
        unit_types.values().sum::<usize>()
    );
    let mut units: Vec<(&u16, &usize)> = unit_types.iter().collect();
    units.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for (id, n) in units.into_iter().take(24) {
        println!("  unit {id:>3}: {n}");
    }
    println!(
        "outside classic set: {extra_condition_uses} condition uses, \
         {extra_action_uses} action uses"
    );
    for line in &extra_examples {
        println!("  {line}");
    }
    if failed > 0 {
        std::process::exit(1);
    }
}

fn print_hist(label: &str, hist: &BTreeMap<u8, usize>) {
    print!("{label}:");
    for (id, n) in hist {
        print!(" {id}={n}");
    }
    println!();
}

fn print_hist_u16(label: &str, hist: &BTreeMap<u16, usize>) {
    print!("{label}:");
    for (id, n) in hist {
        print!(" {id}={n}");
    }
    println!();
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("chk") {
            out.push(path);
        }
    }
}

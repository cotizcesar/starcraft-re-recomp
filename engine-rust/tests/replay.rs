//! Round trip: save at tick T, rebuild, replay, identical state hash.

use std::sync::Arc;

use galaxycraft::data::GameData;
use galaxycraft::ids::unit as uid;
use galaxycraft::map::Terrain;
use galaxycraft::replay::{replay_to, state_hash, Recorder, Recording, SaveFile};
use galaxycraft::sim::{Order, World};

fn data() -> GameData {
    GameData::bundled().expect("bundled tables")
}

/// Identical setup both runs: 2 marines P0, 1 marine P1, fixed positions.
fn fresh() -> (World, Vec<u32>) {
    let mut w = World::new(Arc::new(data()), Terrain::open(32, 32), 2);
    let ia = w.spawn(0, uid::MARINE, 100, 100).unwrap();
    let ib = w.spawn(0, uid::MARINE, 140, 100).unwrap();
    let ic = w.spawn(1, uid::MARINE, 500, 500).unwrap();
    let ids = [ia, ib, ic].map(|i| w.units[i].id).to_vec();
    (w, ids)
}

#[test]
fn save_reload_replays_to_identical_state() {
    // Live run with recording on.
    let (mut w, ids) = fresh();
    w.recorder = Some(Recorder::new());
    for t in 0..60u64 {
        match t {
            0 => {
                w.issue(ids[0], Order::Move { pos: (300, 300) }).unwrap();
            }
            10 => {
                w.issue(ids[1], Order::AttackMove { pos: (500, 500) })
                    .unwrap();
            }
            20 => {
                w.issue(ids[2], Order::Move { pos: (300, 300) }).unwrap();
            }
            _ => {}
        }
        w.step();
    }
    assert_eq!(w.tick, 60);
    let h1 = state_hash(&w);

    // Save at tick 60, reload from disk.
    let rec = Recording {
        seed: 0x1234_5678_9abc_def0,
        map_hash: [0u8; 32],
        cmds: w.recorder.as_ref().unwrap().cmds.clone(),
    };
    assert_eq!(rec.cmds.len(), 3);
    let path = std::env::temp_dir().join("galaxycraft_replay_test.gcsave");
    rec.save(&path).expect("save");
    let raw = std::fs::read(&path).expect("read back");
    assert_eq!(&raw[0..4], &0x4743_5331u32.to_le_bytes()); // "GCS1"
    let back = Recording::load(&path).expect("load");
    assert_eq!(back, rec);
    let _ = std::fs::remove_file(&path);

    // Fresh world, fast-forward replay, identical hash.
    let (mut w2, _) = fresh();
    replay_to(&mut w2, &back, 60);
    assert_eq!(w2.tick, 60);
    let h2 = state_hash(&w2);
    assert_eq!(h1, h2, "replayed state must hash like the live run");
}

fn tiny_chk() -> Vec<u8> {
    let mut chk = Vec::new();
    let mut push = |name: &[u8; 4], bytes: &[u8]| {
        chk.extend_from_slice(name);
        chk.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        chk.extend_from_slice(bytes);
    };
    let mut dim = Vec::new();
    dim.extend_from_slice(&8u16.to_le_bytes());
    dim.extend_from_slice(&8u16.to_le_bytes());
    push(b"DIM ", &dim);
    push(b"ERA ", &1u16.to_le_bytes());
    let mut mtxm = Vec::new();
    for _ in 0..64 {
        mtxm.extend_from_slice(&32u16.to_le_bytes());
    }
    push(b"MTXM", &mtxm);
    push(b"MASK", &[1u8; 64]);
    let mut units = Vec::new();
    let mut u = vec![0u8; 36];
    u[4..6].copy_from_slice(&128u16.to_le_bytes());
    u[6..8].copy_from_slice(&128u16.to_le_bytes());
    u[8..10].copy_from_slice(&galaxycraft::ids::unit::START_LOCATION.to_le_bytes());
    units.extend_from_slice(&u);
    push(b"UNIT", &units);
    let _ = push;
    chk
}

#[test]
fn savefile_roundtrip_with_embedded_map() {
    use galaxycraft::map::Scenario;
    use galaxycraft::replay::{replay_to, state_hash, SaveFile};
    use galaxycraft::sim::Race;

    let chk = tiny_chk();
    let sc = Scenario::parse(&chk).expect("parse");
    let mut vf4 = vec![0u8; 96];
    for i in 0..16 {
        vf4[64 + i * 2] = 1;
    }
    let races = vec![Race::Terran, Race::Protoss];
    let mut w = World::from_scenario(Arc::new(data()), &sc, &vf4, &races);
    let a = w.spawn(0, uid::MARINE, 100, 100).unwrap();
    let ids: Vec<u32> = [a].iter().map(|&i| w.units[i].id).collect();
    w.recorder = Some(galaxycraft::replay::Recorder::new());
    for t in 0..40u64 {
        if t == 0 {
            w.issue(ids[0], Order::Move { pos: (300, 300) }).unwrap();
        }
        w.step();
    }
    let h1 = state_hash(&w);
    let save = SaveFile {
        map_name: "tiny-test".to_string(),
        map_chk: chk.clone(),
        races: races.clone(),
        local_player: 0,
        tick: w.tick,
        rec: galaxycraft::replay::Recording {
            seed: 0x1234_5678_9abc_def0,
            map_hash: [7u8; 32],
            cmds: w.recorder.as_ref().unwrap().cmds.clone(),
        },
    };
    let path = std::env::temp_dir().join("galaxycraft_save_test.gcsave");
    save.save(&path).expect("save");
    let back = SaveFile::load(&path).expect("load");
    let _ = std::fs::remove_file(&path);
    assert_eq!(back, save);
    assert_eq!(back.map_name, "tiny-test");
    assert_eq!(back.tick, 40);

    // Rebuild from the embedded CHK and replay.
    let sc2 = Scenario::parse(&back.map_chk).expect("reparse");
    let mut w2 = World::from_scenario(Arc::new(data()), &sc2, &vf4, &back.races);
    // Same spawns: ids are deterministic (spawn order), but re-resolve.
    let a2 = w2.spawn(0, uid::MARINE, 100, 100).unwrap();
    assert_eq!(w2.units[a2].id, ids[0]);
    replay_to(&mut w2, &back.rec, back.tick);
    assert_eq!(state_hash(&w2), h1, "loaded save must replay identically");
}

#[test]
fn corrupt_file_is_rejected() {
    let path = std::env::temp_dir().join("galaxycraft_replay_corrupt.gcsave");
    std::fs::write(&path, b"GCS1junkjunkjunk").unwrap();
    assert!(Recording::load(&path).is_none());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn boxer_with_ai_save_reload_replays_identically() {
    use galaxycraft::ai::SimpleAi;
    use galaxycraft::map::Scenario;
    use galaxycraft::sim::Race;

    let raw = std::fs::read("assets/maps/Boxer.chk").expect("bundled Boxer");
    let vf4 = std::fs::read("assets/TileSet/platform.vf4").expect("bundled vf4");
    let races = vec![Race::Terran, Race::Protoss];

    // Live run: both AIs + a direct player order, all recorded.
    let sc = Scenario::parse(&raw).expect("parse");
    let mut w = World::from_scenario(Arc::new(data()), &sc, &vf4, &races);
    w.recorder = Some(galaxycraft::replay::Recorder::new());
    let mut ai0 = SimpleAi::new(0);
    let mut ai1 = SimpleAi::new(1);
    let marine = w
        .units
        .iter()
        .find(|u| u.alive && u.owner == 0)
        .map(|u| u.id);
    for t in 0..300u64 {
        ai0.update(&mut w, Some((60, 60)));
        ai1.update(&mut w, Some((60, 60)));
        if t == 10 {
            if let Some(id) = marine {
                let _ = w.issue(id, Order::AttackMove { pos: (500, 500) });
            }
        }
        w.step();
    }
    let h1 = state_hash(&w);
    assert!(!w.recorder.as_ref().unwrap().cmds.is_empty());
    let save = SaveFile {
        map_name: "Boxer".to_string(),
        map_chk: raw.clone(),
        races: races.clone(),
        local_player: 0,
        tick: w.tick,
        rec: Recording {
            seed: 0x1234_5678_9abc_def0,
            map_hash: [0u8; 32],
            cmds: w.recorder.as_ref().unwrap().cmds.clone(),
        },
    };
    let path = std::env::temp_dir().join("galaxycraft_boxer_test.gcsave");
    save.save(&path).expect("save");
    let back = SaveFile::load(&path).expect("load");
    let _ = std::fs::remove_file(&path);
    assert_eq!(back, save);

    // Fresh rebuild from the embedded CHK, replay only (no AI).
    let sc2 = Scenario::parse(&back.map_chk).expect("reparse");
    let mut w2 = World::from_scenario(Arc::new(data()), &sc2, &vf4, &back.races);
    replay_to(&mut w2, &back.rec, back.tick);
    assert_eq!(w2.tick, 300);
    assert_eq!(
        state_hash(&w2),
        h1,
        "AI-driven Boxer run must replay identically"
    );
}

//! LAN lockstep acceptance test over real TCP loopback.
//!
//! Two peers, one map, twenty synchronized turns with commands issued on
//! both sides. The test fails if either peer detects a desync or if the final
//! state hashes differ, which is the same guarantee the 1v1 LAN goal needs.

use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use galaxycraft::data::GameData;
use galaxycraft::ids::unit as uid;
use galaxycraft::map::Scenario;
use galaxycraft::net::{
    accept_rejoin, apply_resync, client_turn, connect, host_turn, request_resync, Lobby, Lockstep,
    DEFAULT_TICKS_PER_TURN,
};
use galaxycraft::replay::{state_hash, RecAct};
use galaxycraft::sim::{Order, Race, World};

fn data() -> GameData {
    GameData::bundled().expect("bundled tables")
}

fn tiny_chk() -> Vec<u8> {
    let mut chk = Vec::new();
    let mut push = |name: &[u8; 4], bytes: &[u8]| {
        chk.extend_from_slice(name);
        chk.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        chk.extend_from_slice(bytes);
    };
    let mut dim = Vec::new();
    dim.extend_from_slice(&16u16.to_le_bytes());
    dim.extend_from_slice(&16u16.to_le_bytes());
    push(b"DIM ", &dim);
    push(b"ERA ", &1u16.to_le_bytes());
    let mut mtxm = Vec::new();
    for _ in 0..256 {
        mtxm.extend_from_slice(&32u16.to_le_bytes());
    }
    push(b"MTXM", &mtxm);
    push(b"MASK", &[1u8; 256]);
    let mut units = Vec::new();
    let mut u = vec![0u8; 36];
    u[4..6].copy_from_slice(&256u16.to_le_bytes());
    u[6..8].copy_from_slice(&256u16.to_le_bytes());
    u[8..10].copy_from_slice(&uid::START_LOCATION.to_le_bytes());
    units.extend_from_slice(&u);
    push(b"UNIT", &units);
    let _ = push;
    chk
}

fn world_for(races: &[Race]) -> World {
    let chk = tiny_chk();
    let sc = Scenario::parse(&chk).expect("parse");
    let mut vf4 = vec![0u8; 96];
    for i in 0..16 {
        vf4[64 + i * 2] = 1;
    }
    World::from_scenario(Arc::new(data()), &sc, &vf4, races)
}

fn first_unit_of(w: &World, owner: u8) -> u32 {
    w.units
        .iter()
        .find(|u| u.alive && u.owner == owner)
        .map(|u| u.id)
        .expect("owner has a unit")
}

#[test]
fn two_peers_stay_in_sync_over_tcp() {
    let map_hash = [7u8; 32];
    let seed = 0x1234_5678_9abc_def0;
    let races = vec![Race::Terran, Race::Protoss];
    let turns = 20u32;

    let (addr_tx, addr_rx) = mpsc::channel();

    // Client side runs in this thread; host runs in the spawned one so both
    // really go through the socket.
    let client_races = races.clone();
    let host = thread::spawn(move || -> Result<u64, String> {
        let lobby = Lobby::bind("127.0.0.1:0", map_hash, seed).map_err(|e| e.to_string())?;
        addr_tx
            .send(lobby.local_addr().map_err(|e| e.to_string())?)
            .unwrap();
        let (mut clients, got_races) = lobby.accept_peers(2).map_err(|e| e.to_string())?;
        assert_eq!(got_races, vec![Race::Terran, Race::Protoss]);

        let mut lock = Lockstep::new(0, 2, DEFAULT_TICKS_PER_TURN);
        let mut world = world_for(&got_races);
        let my_unit = first_unit_of(&world, 0);

        for turn in 0..turns {
            // Host commands the map: a move on turn 0, a train on turn 6.
            let acts: Vec<RecAct> = match turn {
                0 => vec![RecAct::Issue {
                    unit: my_unit,
                    order: Order::Move { pos: (400, 400) },
                }],
                6 => {
                    let cc = world
                        .units
                        .iter()
                        .find(|u| u.alive && u.owner == 0 && u.type_id == uid::COMMAND_CENTER)
                        .map(|u| u.id);
                    cc.map(|building| RecAct::Train {
                        building,
                        unit: uid::SCV,
                    })
                    .into_iter()
                    .collect()
                }
                _ => vec![],
            };
            let desync =
                host_turn(&mut lock, &mut world, &mut clients, acts).map_err(|e| e.to_string())?;
            if desync {
                return Err(format!("host saw desync at turn {turn}"));
            }
        }
        Ok(state_hash(&world))
    });

    let addr = addr_rx.recv().expect("host address");
    let (mut host_conn, setup) =
        connect(&addr.to_string(), map_hash, seed, client_races[1]).expect("connect");
    let races_from_host = match setup {
        galaxycraft::net::Msg::Setup { races, .. } => races,
        other => panic!("unexpected {other:?}"),
    };

    let mut lock = Lockstep::new(1, 2, DEFAULT_TICKS_PER_TURN);
    let mut world = world_for(&races_from_host);
    let my_unit = first_unit_of(&world, 1);

    for turn in 0..turns {
        // Client commands on turn 3, so both sides contribute.
        let acts: Vec<RecAct> = if turn == 3 {
            vec![RecAct::Issue {
                unit: my_unit,
                order: Order::AttackMove { pos: (100, 100) },
            }]
        } else {
            vec![]
        };
        let desync =
            client_turn(&mut lock, &mut world, &mut host_conn, acts).map_err(|e| e.to_string());
        match desync {
            Ok(false) => {}
            Ok(true) => panic!("client detected desync at turn {turn}"),
            Err(e) => panic!("client transport error at turn {turn}: {e}"),
        }
    }
    let client_hash = state_hash(&world);
    let host_hash = host.join().unwrap().expect("host run");

    assert_eq!(
        client_hash, host_hash,
        "both peers must end on the same state hash"
    );
    assert_eq!(lock.turn, turns, "client advanced every turn");
}

#[test]
fn joining_with_a_different_map_is_refused() {
    let (addr_tx, addr_rx) = mpsc::channel();
    let host = thread::spawn(move || {
        let lobby = Lobby::bind("127.0.0.1:0", [1u8; 32], 99).expect("bind");
        addr_tx.send(lobby.local_addr().unwrap()).unwrap();
        // The mismatch surfaces while reading Join; the accept call errors out.
        lobby.accept_peers(2).is_err()
    });
    let addr = addr_rx.recv().unwrap();
    let _ = connect(&addr.to_string(), [2u8; 32], 99, Race::Terran);
    assert!(host.join().unwrap(), "host must reject a different map");
}

#[test]
fn peer_reconnects_and_resyncs_from_the_command_log() {
    let map_hash = [11u8; 32];
    let seed = 0x1234_5678_9abc_def0;
    let races = [Race::Terran, Race::Protoss];
    let split = 8u32;
    let total = 16u32;

    let (addr_tx, addr_rx) = mpsc::channel();

    let host = thread::spawn(move || -> Result<u64, String> {
        let lobby = Lobby::bind("127.0.0.1:0", map_hash, seed).map_err(|e| e.to_string())?;
        addr_tx
            .send(lobby.local_addr().map_err(|e| e.to_string())?)
            .unwrap();
        let (mut clients, got) = lobby.accept_peers(2).map_err(|e| e.to_string())?;
        let mut lock = Lockstep::new(0, 2, DEFAULT_TICKS_PER_TURN);
        let mut world = world_for(&got);
        let my_unit = first_unit_of(&world, 0);

        // Phase 1: both peers connected.
        for turn in 0..split {
            let acts: Vec<RecAct> = if turn == 0 {
                vec![RecAct::Issue {
                    unit: my_unit,
                    order: Order::Move { pos: (300, 300) },
                }]
            } else {
                vec![]
            };
            let desync =
                host_turn(&mut lock, &mut world, &mut clients, acts).map_err(|e| e.to_string())?;
            if desync {
                return Err(format!("desync in phase 1 at turn {turn}"));
            }
        }

        // The client vanished: the next turn must fail, not hang forever.
        let dropped = host_turn(&mut lock, &mut world, &mut clients, vec![]);
        assert!(dropped.is_err(), "a closed peer must surface as an error");

        // Accept it back and hand over the log.
        let back = accept_rejoin(&lobby, &lock).map_err(|e| format!("rejoin: {e}"))?;
        let resumed_turn = lock.turn;

        // Phase 2 continues with the reconnected transport only.
        let mut one = [back];
        for turn in resumed_turn..total {
            let desync =
                host_turn(&mut lock, &mut world, &mut one, vec![]).map_err(|e| e.to_string())?;
            if desync {
                return Err(format!("desync in phase 2 at turn {turn}"));
            }
        }
        Ok(state_hash(&world))
    });

    let addr = addr_rx.recv().expect("host address");
    let (mut conn, setup) = connect(&addr.to_string(), map_hash, seed, races[1]).expect("connect");
    let host_races = match setup {
        galaxycraft::net::Msg::Setup { races, .. } => races,
        other => panic!("unexpected {other:?}"),
    };
    let mut lock = Lockstep::new(1, 2, DEFAULT_TICKS_PER_TURN);
    let mut world = world_for(&host_races);
    let my_unit = first_unit_of(&world, 1);

    for turn in 0..split {
        let acts: Vec<RecAct> = if turn == 3 {
            vec![RecAct::Issue {
                unit: my_unit,
                order: Order::AttackMove { pos: (120, 120) },
            }]
        } else {
            vec![]
        };
        client_turn(&mut lock, &mut world, &mut conn, acts).expect("phase 1");
    }

    // Simulate the crash: the socket is gone and so is the world.
    drop(conn);
    drop(world);

    let (mut conn2, resumed_turn, rec) =
        request_resync(&addr.to_string(), map_hash, seed, 1).expect("resync");
    assert_eq!(resumed_turn, split, "host resumes at the dropped turn");
    assert!(!rec.cmds.is_empty(), "resync log carries the commands");

    // Rebuild from our own map copy and fast-forward to the host's turn.
    let mut world2 = world_for(&host_races);
    let mut lock2 = Lockstep::new(1, 2, DEFAULT_TICKS_PER_TURN);
    apply_resync(&mut world2, &mut lock2, resumed_turn, &rec);
    assert_eq!(lock2.turn, resumed_turn);
    assert_eq!(
        world2.tick,
        resumed_turn as u64 * DEFAULT_TICKS_PER_TURN as u64
    );

    for _ in resumed_turn..total {
        client_turn(&mut lock2, &mut world2, &mut conn2, vec![]).expect("phase 2");
    }

    let client_hash = state_hash(&world2);
    let host_hash = host.join().unwrap().expect("host run");
    assert_eq!(
        client_hash, host_hash,
        "reconnected peer must land on the host's exact state"
    );
}

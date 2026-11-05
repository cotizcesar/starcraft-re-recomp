//! LAN lockstep acceptance test over real TCP loopback.
//!
//! Two peers, one map, twenty synchronized turns with commands issued on
//! both sides. The test fails if either peer detects a desync or if the final
//! state hashes differ, which is the same guarantee the 1v1 LAN goal needs.

use std::sync::Arc;
use std::sync::mpsc;
use std::thread;

use galaxycraft::data::GameData;
use galaxycraft::ids::unit as uid;
use galaxycraft::map::Scenario;
use galaxycraft::net::{
    DEFAULT_TICKS_PER_TURN, Lobby, Lockstep, accept_rejoin, apply_resync, client_turn, connect,
    host_turn, request_resync,
};
use galaxycraft::replay::{RecAct, state_hash};
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
    World::from_scenario(
        Arc::new(data()),
        &sc,
        &galaxycraft::map::TileData::synthetic(&vf4),
        races,
    )
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

/// Host loop used by the multi-peer tests: `turns` turns where every peer
/// contributes a command on a turn of its own.
fn scripted_acts(world: &World, peer: u8, turn: u32) -> Vec<RecAct> {
    if turn == 2 + peer as u32 {
        vec![RecAct::Issue {
            unit: first_unit_of(world, peer),
            order: Order::Move {
                pos: (200 + 40 * peer as i32, 300),
            },
        }]
    } else {
        vec![]
    }
}

fn setup_of(msg: galaxycraft::net::Msg) -> (u8, u8, Vec<Race>) {
    match msg {
        galaxycraft::net::Msg::Setup {
            peers, you, races, ..
        } => (peers, you, races),
        other => panic!("expected Setup, got {other:?}"),
    }
}

#[test]
fn three_peers_join_a_polled_lobby_and_stay_in_sync() {
    use galaxycraft::net::JoinQueue;
    let map_hash = [21u8; 32];
    let seed = 0x1234_5678_9abc_def0;
    let turns = 12u32;
    let (addr_tx, addr_rx) = mpsc::channel();
    let host = thread::spawn(move || -> Result<(u64, Vec<Race>), String> {
        let lobby = Lobby::bind("127.0.0.1:0", map_hash, seed).map_err(|e| e.to_string())?;
        addr_tx.send(lobby.local_addr().unwrap()).unwrap();
        // The menu's way: poll without blocking until the lobby is full.
        let mut queue = JoinQueue::new(3);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while queue.players() < 3 {
            assert!(std::time::Instant::now() < deadline, "peers never joined");
            if lobby
                .poll_join(&mut queue, false)
                .map_err(|e| e.to_string())?
                .is_none()
            {
                thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        assert_eq!(queue.members().len(), 2);
        let (mut clients, races) = lobby.start(queue, Race::Zerg).map_err(|e| e.to_string())?;
        let mut lock = Lockstep::new(0, 3, DEFAULT_TICKS_PER_TURN);
        let mut world = world_for(&races);
        for turn in 0..turns {
            let acts = scripted_acts(&world, 0, turn);
            if host_turn(&mut lock, &mut world, &mut clients, acts).map_err(|e| e.to_string())? {
                return Err(format!("desync at {turn}"));
            }
        }
        Ok((state_hash(&world), races))
    });
    let addr = addr_rx.recv().unwrap().to_string();
    // `connect` returns once the host starts, so each client joins from its
    // own thread; the stagger keeps the join (= peer) order deterministic.
    let workers: Vec<_> = [Race::Terran, Race::Protoss]
        .into_iter()
        .enumerate()
        .map(|(k, race)| {
            let addr = addr.clone();
            thread::spawn(move || {
                thread::sleep(std::time::Duration::from_millis(150 * k as u64));
                let (mut conn, setup) = connect(&addr, map_hash, seed, race).expect("connect");
                let (peers, you, races) = setup_of(setup);
                let mut lock = Lockstep::new(you, peers, DEFAULT_TICKS_PER_TURN);
                let mut world = world_for(&races);
                for turn in 0..turns {
                    let acts = scripted_acts(&world, you, turn);
                    assert!(!client_turn(&mut lock, &mut world, &mut conn, acts).expect("turn"));
                }
                (you, peers, state_hash(&world))
            })
        })
        .collect();
    let (host_hash, races) = host.join().unwrap().expect("host");
    assert_eq!(races, vec![Race::Zerg, Race::Terran, Race::Protoss]);
    let mut seen = Vec::new();
    for w in workers {
        let (you, peers, hash) = w.join().unwrap();
        assert_eq!(peers, 3);
        assert_eq!(hash, host_hash, "peer {you} diverged");
        seen.push(you);
    }
    seen.sort();
    assert_eq!(seen, vec![1, 2], "each client learns its own index");
}

#[test]
fn a_dropped_peer_rejoins_with_backoff_while_the_others_wait() {
    use galaxycraft::net::{
        Backoff, JoinQueue, host_turn_peers, rejoin_with_backoff, try_accept_rejoin,
    };
    let map_hash = [33u8; 32];
    let seed = 0x1234_5678_9abc_def0;
    let (drop_at, total) = (6u32, 14u32);
    let (addr_tx, addr_rx) = mpsc::channel();
    let host = thread::spawn(move || -> Result<(u64, Vec<u8>), String> {
        let lobby = Lobby::bind("127.0.0.1:0", map_hash, seed).map_err(|e| e.to_string())?;
        addr_tx.send(lobby.local_addr().unwrap()).unwrap();
        let mut queue = JoinQueue::new(3);
        while queue.players() < 3 {
            lobby
                .poll_join(&mut queue, true)
                .map_err(|e| e.to_string())?;
        }
        let (mut clients, races) = lobby
            .start(queue, Race::Terran)
            .map_err(|e| e.to_string())?;
        let mut lock = Lockstep::new(0, 3, DEFAULT_TICKS_PER_TURN);
        let mut world = world_for(&races);
        let mut lost = Vec::new();
        while lock.turn < total {
            let acts = scripted_acts(&world, 0, lock.turn);
            match host_turn_peers(&mut lock, &mut world, &mut clients, acts) {
                Ok(false) => {}
                Ok(true) => return Err(format!("desync at {}", lock.turn)),
                Err(f) => {
                    // Wait (polling, as the game loop does) for the peer to
                    // come back on the lobby listener.
                    lost.push(f.peer);
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
                    loop {
                        assert!(std::time::Instant::now() < deadline, "no rejoin");
                        match try_accept_rejoin(&lobby, &lock, false).map_err(|e| e.to_string())? {
                            Some((peer, t)) => {
                                assert_eq!(peer, f.peer, "the lost peer is the one back");
                                lock.forget_peer_turn(peer);
                                clients[peer as usize - 1] = t;
                                break;
                            }
                            None => thread::sleep(std::time::Duration::from_millis(5)),
                        }
                    }
                }
            }
        }
        Ok((state_hash(&world), lost))
    });
    let addr = addr_rx.recv().unwrap().to_string();
    // Peer 1 joins first (its own thread: `connect` returns at the start)
    // and plays straight through.
    let steady_addr = addr.clone();
    let steady = thread::spawn(move || {
        let (c1, s1) = connect(&steady_addr, map_hash, seed, Race::Protoss).expect("peer 1");
        let (peers, you, races) = setup_of(s1);
        assert_eq!(you, 1);
        let mut conn = c1;
        let mut lock = Lockstep::new(you, peers, DEFAULT_TICKS_PER_TURN);
        let mut world = world_for(&races);
        while lock.turn < total {
            let acts = scripted_acts(&world, you, lock.turn);
            client_turn(&mut lock, &mut world, &mut conn, acts).expect("steady peer");
        }
        state_hash(&world)
    });
    // Peer 2 crashes after `drop_at` turns, then rejoins on the backoff
    // schedule and rebuilds its world from the host's command log.
    thread::sleep(std::time::Duration::from_millis(150));
    let (c2, s2) = connect(&addr, map_hash, seed, Race::Zerg).expect("peer 2");
    let (peers, you, races) = setup_of(s2);
    assert_eq!(you, 2);
    let mut conn = c2;
    let mut lock = Lockstep::new(you, peers, DEFAULT_TICKS_PER_TURN);
    let mut world = world_for(&races);
    while lock.turn < drop_at {
        let acts = scripted_acts(&world, you, lock.turn);
        client_turn(&mut lock, &mut world, &mut conn, acts).expect("before the drop");
    }
    drop(conn);
    drop(world);
    let mut backoff = Backoff::lan(std::time::Instant::now());
    let (mut conn, turn, rec) =
        rejoin_with_backoff(&addr, map_hash, seed, you, &mut backoff).expect("rejoin");
    assert_eq!(turn, drop_at, "host resumes at the interrupted turn");
    let mut world = world_for(&races);
    let mut lock = Lockstep::new(you, peers, DEFAULT_TICKS_PER_TURN);
    apply_resync(&mut world, &mut lock, turn, &rec);
    while lock.turn < total {
        let acts = scripted_acts(&world, you, lock.turn);
        client_turn(&mut lock, &mut world, &mut conn, acts).expect("after the rejoin");
    }
    let rejoined = state_hash(&world);
    let steady = steady.join().unwrap();
    let (host_hash, lost) = host.join().unwrap().expect("host");
    assert_eq!(lost, vec![2], "the host named the dropped peer once");
    assert_eq!(steady, host_hash, "the peer that stayed is in sync");
    assert_eq!(rejoined, host_hash, "the rejoined peer is in sync");
}

#[test]
fn rejoin_backoff_retries_until_the_host_listens() {
    use galaxycraft::net::{Backoff, rejoin_with_backoff};
    use std::time::{Duration, Instant};
    // Reserve a port, then free it: nobody listens there for a while.
    let addr = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let map_hash = [44u8; 32];
    let seed = 7;
    let host = thread::spawn(move || {
        thread::sleep(Duration::from_millis(400));
        let lobby = Lobby::bind(&addr.to_string(), map_hash, seed).expect("late bind");
        let lock = Lockstep::new(0, 2, DEFAULT_TICKS_PER_TURN);
        accept_rejoin(&lobby, &lock).expect("rejoin accepted");
    });
    let mut backoff = Backoff::new(
        Duration::from_millis(50),
        Duration::from_millis(200),
        20,
        Instant::now(),
    );
    let started = Instant::now();
    let (_, turn, _) =
        rejoin_with_backoff(&addr.to_string(), map_hash, seed, 1, &mut backoff).expect("rejoins");
    host.join().unwrap();
    assert_eq!(turn, 0);
    assert!(
        backoff.failures() >= 2,
        "refused attempts were retried: {}",
        backoff.failures()
    );
    assert!(started.elapsed() >= Duration::from_millis(350));
    // With no host at all the schedule runs out instead of looping forever.
    let gone = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let mut short = Backoff::new(
        Duration::from_millis(10),
        Duration::from_millis(20),
        3,
        Instant::now(),
    );
    let err = rejoin_with_backoff(&gone.to_string(), map_hash, seed, 1, &mut short)
        .err()
        .expect("gives up");
    assert!(
        err.to_string().contains("gave up after 3 attempts"),
        "{err}"
    );
}

/// Protocol 3 lobby over loopback: a named join is seated, sees the lobby,
/// picks a race the host applies, and starts with the resolved setup (its
/// own slot, computers and teams included).
#[test]
fn a_joiner_sits_in_the_lobby_and_starts_with_the_setup() {
    use galaxycraft::multiplayer::{
        GameType, Lobby as GameLobby, Occupant, RaceChoice, SetupOccupant, race_choice_byte,
    };
    use galaxycraft::net::{JoinQueue, Msg, Transport, connect_lobby};
    use std::time::{Duration, Instant};

    let hash = [5u8; 32];
    let host = Lobby::bind("127.0.0.1:0", hash, galaxycraft::net::WORLD_SEED).unwrap();
    let addr = host.local_addr().unwrap().to_string();
    let mut queue = JoinQueue::new(4);
    let mut link = connect_lobby(
        &addr,
        hash,
        galaxycraft::net::WORLD_SEED,
        Race::Terran,
        "Raynor",
    )
    .expect("connect");
    host.poll_join(&mut queue, true).unwrap().expect("joined");
    assert_eq!(queue.name(1), Some("Raynor"));

    // Host seats the joiner in the first Open slot, adds a computer and
    // sends the lobby.
    let mut lobby = GameLobby::new_host("LinKa", "Boxer", 4, GameType::TopVsBottom, (2, 2));
    lobby.slots[0].race = RaceChoice::Protoss;
    let seat = lobby.first_open().unwrap();
    lobby.slots[seat].occupant = Occupant::Human {
        name: "Raynor".into(),
        peer: 1,
    };
    lobby.set_occupant(2, Occupant::Computer);
    assert!(queue.send_lobby(&lobby).is_empty());

    let deadline = Instant::now() + Duration::from_secs(5);
    let seen = loop {
        if let Some(m) = link.try_recv().unwrap() {
            break m;
        }
        assert!(Instant::now() < deadline, "no lobby state");
        thread::sleep(Duration::from_millis(5));
    };
    let Msg::LobbyState { you, lobby: theirs } = seen else {
        panic!("expected LobbyState, got {seen:?}");
    };
    assert_eq!((you, &theirs), (1, &lobby));
    assert!(link.try_recv().unwrap().is_none(), "nothing else queued");

    // The joiner picks Zerg; the host applies it.
    link.send(&Msg::LobbyRace {
        race: race_choice_byte(RaceChoice::Zerg),
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let (msgs, gone) = loop {
        let (m, g) = queue.poll_lobby();
        if !m.is_empty() || !g.is_empty() {
            break (m, g);
        }
        assert!(Instant::now() < deadline, "no race message");
        thread::sleep(Duration::from_millis(5));
    };
    assert!(gone.is_empty());
    assert_eq!(msgs, vec![(1, Msg::LobbyRace { race: 1 })]);
    lobby.slots[seat].race = RaceChoice::Zerg;

    // Start: the joiner receives the setup and finds its slot in it.
    let setup = lobby.resolve(galaxycraft::net::WORLD_SEED);
    let (clients, races) = host.start_with_setup(queue, &setup).unwrap();
    assert_eq!((clients.len(), races.len()), (1, 4));
    let Msg::Setup {
        you,
        races: got,
        setup: Some(theirs),
        ..
    } = link.recv().unwrap()
    else {
        panic!("expected Setup with a game setup");
    };
    assert_eq!((you, got, &theirs), (1, races, &setup));
    assert_eq!(theirs.slot_of_peer(1), Some(seat));
    assert_eq!(theirs.slots[seat].race, Race::Zerg);
    assert_eq!(theirs.slots[2].occupant, SetupOccupant::Computer);
    assert_eq!(theirs.slots[seat].team, 0, "slot 1 is on the Home Team");
}

/// A joiner that drops out of the lobby is reported gone, and the lobby
/// reopens its slot.
#[test]
fn a_joiner_leaving_the_lobby_reopens_its_slot() {
    use galaxycraft::multiplayer::{GameType, Lobby as GameLobby, Occupant};
    use galaxycraft::net::{JoinQueue, connect_lobby};
    use std::time::{Duration, Instant};

    let hash = [6u8; 32];
    let host = Lobby::bind("127.0.0.1:0", hash, galaxycraft::net::WORLD_SEED).unwrap();
    let addr = host.local_addr().unwrap().to_string();
    let mut queue = JoinQueue::new(4);
    let link = connect_lobby(
        &addr,
        hash,
        galaxycraft::net::WORLD_SEED,
        Race::Terran,
        "Fenix",
    )
    .unwrap();
    host.poll_join(&mut queue, true).unwrap().expect("joined");
    let mut lobby = GameLobby::new_host("LinKa", "Boxer", 2, GameType::Melee, (1, 1));
    lobby.slots[1].occupant = Occupant::Human {
        name: "Fenix".into(),
        peer: 1,
    };
    drop(link);
    let deadline = Instant::now() + Duration::from_secs(5);
    let gone = loop {
        let (_, g) = queue.poll_lobby();
        if !g.is_empty() {
            break g;
        }
        assert!(Instant::now() < deadline, "the drop was not noticed");
        thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(gone, vec![1]);
    queue.remove(1);
    lobby.remove_peer(1);
    assert_eq!(queue.players(), 1);
    assert_eq!(lobby.first_open(), Some(1));
}

/// The joiner's colour pick reaches the host, and the host's countdown
/// (start and cancel) reaches the joiner, so both lobbies show "Starting in".
#[test]
fn colour_picks_and_the_countdown_cross_the_lobby_link() {
    use galaxycraft::multiplayer::COLOR_CHOICES;
    use galaxycraft::net::{JoinQueue, Msg, Transport, connect_lobby};
    use std::time::{Duration, Instant};

    let hash = [7u8; 32];
    let host = Lobby::bind("127.0.0.1:0", hash, galaxycraft::net::WORLD_SEED).unwrap();
    let addr = host.local_addr().unwrap().to_string();
    let mut queue = JoinQueue::new(2);
    let mut link = connect_lobby(
        &addr,
        hash,
        galaxycraft::net::WORLD_SEED,
        Race::Zerg,
        "Kerrigan",
    )
    .unwrap();
    host.poll_join(&mut queue, true).unwrap().expect("joined");

    let green = COLOR_CHOICES.iter().position(|c| *c == "Green").unwrap() as u8;
    link.send(&Msg::LobbyColor { color: green }).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let msgs = loop {
        let (m, gone) = queue.poll_lobby();
        assert!(gone.is_empty());
        if !m.is_empty() {
            break m;
        }
        assert!(Instant::now() < deadline, "no colour message");
        thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(msgs, vec![(1, Msg::LobbyColor { color: green })]);

    let mut next = || {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(m) = link.try_recv().unwrap() {
                return m;
            }
            assert!(Instant::now() < deadline, "nothing from the host");
            thread::sleep(Duration::from_millis(5));
        }
    };
    queue.send_all(&Msg::LobbyCountdown { on: true });
    assert_eq!(next(), Msg::LobbyCountdown { on: true });
    queue.send_all(&Msg::LobbyCountdown { on: false });
    assert_eq!(next(), Msg::LobbyCountdown { on: false });
}

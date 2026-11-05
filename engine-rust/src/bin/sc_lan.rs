//! sc_lan — lockstep LAN match over TCP (2..=8 peers, headless).
//!
//! Both peers must pass the same map: the CHK SHA-256 is exchanged in the
//! handshake and a mismatch is refused before the game starts.
//!
//! Host (player 0):
//!   sc_lan --host --chk map.chk --vf4 platform.vf4 --races terran,protoss
//! Client (player 1, same map):
//!   sc_lan --join 192.168.1.20:7777 --chk map.chk --vf4 platform.vf4 --race zerg
//!
//! Every turn both peers exchange only commands; a state hash after each
//! turn detects desync immediately. `--ticks` bounds the match (default
//! 28800 = 20 minutes at 24 Hz).

use std::sync::Arc;

use galaxycraft::data::GameData;
use galaxycraft::map::Scenario;
use galaxycraft::map::{TileData, TileTables};
use galaxycraft::net::{
    Lobby, Lockstep, Msg, Transport, client_turn, connect, host_turn, udp_accept, udp_join,
};
use galaxycraft::replay::{RecAct, state_hash};
use galaxycraft::sim::{Order, Race, World};
use sha2::{Digest, Sha256};

fn parse_race(s: &str) -> Race {
    match s.to_ascii_lowercase().as_str() {
        "zerg" | "z" => Race::Zerg,
        "protoss" | "p" => Race::Protoss,
        _ => Race::Terran,
    }
}

fn load_world(chk: &[u8], vf4: &TileData, races: &[Race]) -> Result<World, String> {
    let sc = Scenario::parse(chk).map_err(|e| format!("bad CHK: {e}"))?;
    Ok(World::from_scenario(
        Arc::new(GameData::bundled().map_err(|e| format!("tables: {e}"))?),
        &sc,
        vf4,
        races,
    ))
}

/// Deterministic per-peer commands so a long match keeps producing traffic
/// without needing a human at the keyboard.
fn scripted_acts(world: &World, owner: u8, turn: u32) -> Vec<RecAct> {
    let mut acts = Vec::new();
    if turn % 240 == 1 {
        let mine: Vec<u32> = world
            .units
            .iter()
            .filter(|u| u.alive && u.owner == owner && !u.completed)
            .map(|u| u.id)
            .collect();
        let army: Vec<u32> = world
            .units
            .iter()
            .filter(|u| u.alive && u.owner == owner && u.completed)
            .map(|u| u.id)
            .collect();
        let _ = mine;
        let target = if owner == 0 { (200, 200) } else { (100, 100) };
        for id in army.iter().take(12) {
            acts.push(RecAct::Issue {
                unit: *id,
                order: Order::AttackMove { pos: target },
            });
        }
    }
    if turn % 300 == 60 {
        let main = world
            .units
            .iter()
            .find(|u| {
                u.alive
                    && u.owner == owner
                    && matches!(
                        u.type_id,
                        galaxycraft::ids::unit::COMMAND_CENTER
                            | galaxycraft::ids::unit::NEXUS
                            | galaxycraft::ids::unit::HATCHERY
                    )
            })
            .map(|u| u.id);
        if let Some(building) = main {
            let worker = match world.players[owner as usize].race {
                Race::Zerg => galaxycraft::ids::unit::ZERGLING,
                Race::Protoss => galaxycraft::ids::unit::ZEALOT,
                _ => galaxycraft::ids::unit::SCV,
            };
            acts.push(RecAct::Train {
                building,
                unit: worker,
            });
        }
    }
    acts
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut host = false;
    let mut join: Option<String> = None;
    let mut chk_path: Option<String> = None;
    let mut vf4_path: Option<String> = None;
    let mut port: u16 = 7777;
    let mut races_arg: Option<String> = None;
    let mut race = Race::Terran;
    let mut ticks: u64 = 28_800;
    let mut udp = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--host" => host = true,
            "--udp" => udp = true,
            "--join" => {
                join = args.get(i + 1).cloned();
                i += 1;
            }
            "--chk" => {
                chk_path = args.get(i + 1).cloned();
                i += 1;
            }
            "--vf4" => {
                vf4_path = args.get(i + 1).cloned();
                i += 1;
            }
            "--port" => {
                port = args.get(i + 1).and_then(|p| p.parse().ok()).unwrap_or(port);
                i += 1;
            }
            "--races" => {
                races_arg = args.get(i + 1).cloned();
                i += 1;
            }
            "--race" => {
                race = parse_race(args.get(i + 1).map(|s| s.as_str()).unwrap_or("terran"));
                i += 1;
            }
            "--ticks" => {
                ticks = args
                    .get(i + 1)
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(ticks);
                i += 1;
            }
            "-h" | "--help" => {
                println!(
                    "sc_lan --host --chk MAP --vf4 VF4 [--port N] [--races t,p] [--ticks N]\n\
                     sc_lan --join HOST:PORT --chk MAP --vf4 VF4 [--race terran|zerg|protoss]\n\
                     add --udp to either side to use the UDP transport"
                );
                return;
            }
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
        i += 1;
    }

    let Some(chk_path) = chk_path else {
        eprintln!("missing --chk");
        std::process::exit(2);
    };
    let Some(vf4_path) = vf4_path else {
        eprintln!("missing --vf4");
        std::process::exit(2);
    };
    let chk = std::fs::read(&chk_path).unwrap_or_else(|e| {
        eprintln!("cannot read {chk_path}: {e}");
        std::process::exit(1)
    });
    let tables = TileTables::read_vf4(std::path::Path::new(&vf4_path)).unwrap_or_else(|e| {
        eprintln!("cannot read {vf4_path} (+ .cv5): {e}");
        std::process::exit(1)
    });
    let mut map_hash = [0u8; 32];
    map_hash.copy_from_slice(&Sha256::digest(&chk));
    let seed = 0x1234_5678_9abc_def0u64;
    let ticks_per_turn = galaxycraft::net::DEFAULT_TICKS_PER_TURN;
    let total_turns = (ticks / ticks_per_turn as u64) as u32;

    let m = Match {
        udp,
        chk: &chk,
        vf4: &tables.data(),
        map_hash: &map_hash,
        seed,
        total_turns,
        ticks_per_turn,
    };
    let outcome: Result<(), String> = if host {
        run_host(&m, port, races_arg.as_deref())
    } else {
        match join {
            Some(addr) => run_client(&m, &addr, race),
            None => Err("need --host or --join HOST:PORT".to_string()),
        }
    };

    if let Err(e) = outcome {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

/// Everything both roles need; keeps the run functions small.
struct Match<'a> {
    udp: bool,
    chk: &'a [u8],
    vf4: &'a TileData<'a>,
    map_hash: &'a [u8; 32],
    seed: u64,
    total_turns: u32,
    ticks_per_turn: u32,
}

/// Drive the host side of a match over any transport.
fn host_loop<T: Transport>(
    lock: &mut Lockstep,
    world: &mut World,
    peers: &mut [T],
    total_turns: u32,
) -> Result<(), String> {
    for turn in 0..total_turns {
        let acts = scripted_acts(world, 0, turn);
        let desync = host_turn(lock, world, peers, acts).map_err(|e| e.to_string())?;
        if desync {
            return Err(format!("DESYNC at turn {turn}"));
        }
        if turn % 600 == 0 {
            println!(
                "turn {turn} tick {} hash {:016x} units {}",
                world.tick,
                state_hash(world),
                world.units.iter().filter(|u| u.alive).count()
            );
        }
    }
    println!(
        "host: done at tick {} final hash {:016x}",
        world.tick,
        state_hash(world)
    );
    Ok(())
}

/// Drive the client side of a match over any transport.
fn client_loop<T: Transport>(
    lock: &mut Lockstep,
    world: &mut World,
    host: &mut T,
    total_turns: u32,
) -> Result<(), String> {
    for turn in 0..total_turns {
        let acts = scripted_acts(world, 1, turn);
        let desync = client_turn(lock, world, host, acts).map_err(|e| e.to_string())?;
        if desync {
            return Err(format!("DESYNC reported at turn {turn}"));
        }
        if turn % 600 == 0 {
            println!(
                "turn {turn} tick {} hash {:016x} units {}",
                world.tick,
                state_hash(world),
                world.units.iter().filter(|u| u.alive).count()
            );
        }
    }
    println!(
        "client: done at tick {} final hash {:016x}",
        world.tick,
        state_hash(world)
    );
    Ok(())
}

fn run_host(m: &Match<'_>, port: u16, races_arg: Option<&str>) -> Result<(), String> {
    let Match {
        udp,
        chk,
        vf4,
        map_hash,
        seed,
        total_turns,
        ticks_per_turn,
    } = *m;
    let races: Vec<Race> = races_arg
        .map(|s| s.split(',').map(parse_race).collect())
        .unwrap_or_else(|| vec![Race::Terran, Race::Zerg]);
    let lobby = Lobby::bind(&format!("0.0.0.0:{port}"), *map_hash, seed)
        .map_err(|e| format!("bind: {e}"))?;
    println!(
        "host: listening on {} (map sha256 {})",
        lobby
            .local_addr()
            .map(|a| a.to_string())
            .unwrap_or_default(),
        hex32(map_hash)
    );
    if udp {
        let sock = std::net::UdpSocket::bind(("0.0.0.0", port)).map_err(|e| e.to_string())?;
        println!("host: waiting for a UDP peer on :{port}");
        let (mut peer, race) =
            udp_accept(sock, *map_hash, seed, races[0]).map_err(|e| format!("udp accept: {e}"))?;
        let got = vec![races[0], race];
        println!("host: UDP peer joined, races {got:?}");
        let mut world = load_world(chk, vf4, &got)?;
        let mut lock = Lockstep::new(0, got.len() as u8, ticks_per_turn);
        return host_loop(
            &mut lock,
            &mut world,
            std::slice::from_mut(&mut peer),
            total_turns,
        );
    }
    let (mut clients, got) = lobby
        .accept_peers(races.len() as u8)
        .map_err(|e| e.to_string())?;
    println!("host: all peers joined, races {got:?}");
    let mut world = load_world(chk, vf4, &got)?;
    let mut lock = Lockstep::new(0, got.len() as u8, ticks_per_turn);
    host_loop(&mut lock, &mut world, &mut clients, total_turns)
}

fn run_client(m: &Match<'_>, addr: &str, race: Race) -> Result<(), String> {
    let Match {
        udp,
        chk,
        vf4,
        map_hash,
        seed,
        total_turns,
        ticks_per_turn,
    } = *m;
    if udp {
        let (mut peer, setup) =
            udp_join(addr, *map_hash, seed, race).map_err(|e| format!("udp join: {e}"))?;
        let Msg::Setup { races, .. } = setup else {
            return Err("unexpected setup message".to_string());
        };
        let mut world = load_world(chk, vf4, &races)?;
        let mut lock = Lockstep::new(1, races.len() as u8, ticks_per_turn);
        println!("client: joined {addr} over UDP, races {races:?}");
        return client_loop(&mut lock, &mut world, &mut peer, total_turns);
    }
    let (mut conn, setup) =
        connect(addr, *map_hash, seed, race).map_err(|e| format!("connect: {e}"))?;
    let Msg::Setup { races, you, .. } = setup else {
        return Err("unexpected setup message".to_string());
    };
    let mut world = load_world(chk, vf4, &races)?;
    let mut lock = Lockstep::new(you, races.len() as u8, ticks_per_turn);
    println!("client: joined {addr} as peer {you}, races {races:?}");
    client_loop(&mut lock, &mut world, &mut conn, total_turns)
}

fn hex32(b: &[u8; 32]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

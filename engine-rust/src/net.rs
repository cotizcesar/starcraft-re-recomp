//! LAN lockstep networking (TCP first, UDP later behind the same API).
//!
//! The simulation is deterministic, so peers never exchange state: each turn
//! they send only their commands, every peer applies the merged bundle in the
//! same order, then steps the same number of ticks. A periodic state hash
//! detects desync early instead of letting the games drift apart silently.
//!
//! Topology: star. The host (peer 0) accepts one connection per other peer,
//! collects their turn commands, and broadcasts the merged bundle. That keeps
//! the merge order identical everywhere, which is what determinism needs.
//!
//! Frame layout on the wire: `u32 len` + payload (little endian).

use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

use crate::replay::{RecAct, decode_act, encode_act};
use crate::sim::{Race, World};

/// Bumped whenever the message encoding changes.
pub const PROTOCOL: u16 = 2;
/// Ticks advanced per network turn. 4 ticks at 24 Hz = 166 ms of input lag.
pub const DEFAULT_TICKS_PER_TURN: u32 = 4;
/// Hard cap so a corrupt frame cannot allocate unbounded memory.
const MAX_FRAME: usize = 1 << 20;

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
    /// Client announces itself and the setup it expects.
    Join {
        protocol: u16,
        map_hash: [u8; 32],
        seed: u64,
        race: Race,
    },
    /// Host confirms the lobby setup for everyone; `you` is the receiving
    /// peer's index (protocol 2: clients no longer assume they are peer 1).
    Setup {
        peers: u8,
        you: u8,
        map_hash: [u8; 32],
        seed: u64,
        races: Vec<Race>,
        ticks_per_turn: u32,
    },
    /// One peer's commands for `turn`.
    Turn {
        turn: u32,
        acts: Vec<RecAct>,
    },
    /// Host's merged view of `turn` (one vec per peer).
    Bundle {
        turn: u32,
        per_peer: Vec<Vec<RecAct>>,
    },
    /// State hash after `turn` was simulated.
    Sum {
        turn: u32,
        hash: u64,
    },
    /// Host reporting a hash disagreement at `turn`.
    Desync {
        turn: u32,
        peer: u8,
    },
    /// Chat line (lobby or in game).
    Chat {
        from: u8,
        text: String,
    },
    /// Host handing a (re)joining peer the whole command log, so it can
    /// rebuild the world by fast-forward instead of transferring state.
    Resync {
        turn: u32,
        log: Vec<u8>,
    },
    /// A peer that was dropped asking to come back.
    Rejoin {
        peer: u8,
        map_hash: [u8; 32],
        seed: u64,
    },
    Bye,
}

fn race_byte(r: Race) -> u8 {
    match r {
        Race::Zerg => 0,
        Race::Terran => 1,
        Race::Protoss => 2,
        Race::Neutral => 3,
    }
}

fn race_from(b: u8) -> Option<Race> {
    Some(match b {
        0 => Race::Zerg,
        1 => Race::Terran,
        2 => Race::Protoss,
        3 => Race::Neutral,
        _ => return None,
    })
}

impl Msg {
    pub fn encode(&self) -> Vec<u8> {
        use crate::replay::{w_u8, w_u16, w_u32, w_u64};
        let mut out = Vec::new();
        w_u16(&mut out, PROTOCOL);
        match self {
            Msg::Join {
                protocol,
                map_hash,
                seed,
                race,
            } => {
                w_u8(&mut out, 1);
                w_u16(&mut out, *protocol);
                out.extend_from_slice(map_hash);
                w_u64(&mut out, *seed);
                w_u8(&mut out, race_byte(*race));
            }
            Msg::Setup {
                peers,
                you,
                map_hash,
                seed,
                races,
                ticks_per_turn,
            } => {
                w_u8(&mut out, 2);
                w_u8(&mut out, *peers);
                w_u8(&mut out, *you);
                out.extend_from_slice(map_hash);
                w_u64(&mut out, *seed);
                w_u32(&mut out, races.len() as u32);
                for r in races {
                    w_u8(&mut out, race_byte(*r));
                }
                w_u32(&mut out, *ticks_per_turn);
            }
            Msg::Turn { turn, acts } => {
                w_u8(&mut out, 3);
                w_u32(&mut out, *turn);
                w_u32(&mut out, acts.len() as u32);
                for a in acts {
                    encode_act(&mut out, a);
                }
            }
            Msg::Bundle { turn, per_peer } => {
                w_u8(&mut out, 4);
                w_u32(&mut out, *turn);
                w_u32(&mut out, per_peer.len() as u32);
                for peer in per_peer {
                    w_u32(&mut out, peer.len() as u32);
                    for a in peer {
                        encode_act(&mut out, a);
                    }
                }
            }
            Msg::Sum { turn, hash } => {
                w_u8(&mut out, 5);
                w_u32(&mut out, *turn);
                w_u64(&mut out, *hash);
            }
            Msg::Desync { turn, peer } => {
                w_u8(&mut out, 6);
                w_u32(&mut out, *turn);
                w_u8(&mut out, *peer);
            }
            Msg::Chat { from, text } => {
                w_u8(&mut out, 7);
                w_u8(&mut out, *from);
                let b = text.as_bytes();
                let n = b.len().min(512);
                w_u32(&mut out, n as u32);
                out.extend_from_slice(&b[..n]);
            }
            Msg::Resync { turn, log } => {
                w_u8(&mut out, 9);
                w_u32(&mut out, *turn);
                w_u32(&mut out, log.len() as u32);
                out.extend_from_slice(log);
            }
            Msg::Rejoin {
                peer,
                map_hash,
                seed,
            } => {
                w_u8(&mut out, 10);
                w_u8(&mut out, *peer);
                out.extend_from_slice(map_hash);
                w_u64(&mut out, *seed);
            }
            Msg::Bye => w_u8(&mut out, 8),
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> Option<Msg> {
        use crate::replay::Cursor;
        let mut c = Cursor::new(bytes);
        let proto = c.u16()?;
        if proto != PROTOCOL {
            return None;
        }
        let m = match c.u8()? {
            1 => Msg::Join {
                protocol: c.u16()?,
                map_hash: c.take32()?,
                seed: c.u64()?,
                race: race_from(c.u8()?)?,
            },
            2 => {
                let peers = c.u8()?;
                let you = c.u8()?;
                let map_hash = c.take32()?;
                let seed = c.u64()?;
                let n = c.u32()? as usize;
                if n > 8 {
                    return None;
                }
                let mut races = Vec::with_capacity(n);
                for _ in 0..n {
                    races.push(race_from(c.u8()?)?);
                }
                Msg::Setup {
                    peers,
                    you,
                    map_hash,
                    seed,
                    races,
                    ticks_per_turn: c.u32()?,
                }
            }
            3 => {
                let turn = c.u32()?;
                let n = c.u32()? as usize;
                if n > 4096 {
                    return None;
                }
                let mut acts = Vec::with_capacity(n);
                for _ in 0..n {
                    acts.push(decode_act(&mut c)?);
                }
                Msg::Turn { turn, acts }
            }
            4 => {
                let turn = c.u32()?;
                let peers = c.u32()? as usize;
                if peers > 8 {
                    return None;
                }
                let mut per_peer = Vec::with_capacity(peers);
                for _ in 0..peers {
                    let n = c.u32()? as usize;
                    if n > 4096 {
                        return None;
                    }
                    let mut v = Vec::with_capacity(n);
                    for _ in 0..n {
                        v.push(decode_act(&mut c)?);
                    }
                    per_peer.push(v);
                }
                Msg::Bundle { turn, per_peer }
            }
            5 => Msg::Sum {
                turn: c.u32()?,
                hash: c.u64()?,
            },
            6 => Msg::Desync {
                turn: c.u32()?,
                peer: c.u8()?,
            },
            7 => {
                let from = c.u8()?;
                let n = c.u32()? as usize;
                if n > 512 {
                    return None;
                }
                let b = c.take_bytes(n)?;
                Msg::Chat {
                    from,
                    text: String::from_utf8_lossy(b).into_owned(),
                }
            }
            9 => {
                let turn = c.u32()?;
                let n = c.u32()? as usize;
                if n > MAX_FRAME / 2 {
                    return None;
                }
                Msg::Resync {
                    turn,
                    log: c.take_bytes(n)?.to_vec(),
                }
            }
            10 => Msg::Rejoin {
                peer: c.u8()?,
                map_hash: c.take32()?,
                seed: c.u64()?,
            },
            8 => Msg::Bye,
            _ => return None,
        };
        if !c.at_end() {
            return None;
        }
        Some(m)
    }
}

// ---------------------------------------------------------------------------
// Framing over any byte stream
// ---------------------------------------------------------------------------

/// A connected peer as a length-prefixed message channel.
pub trait Transport {
    fn send(&mut self, msg: &Msg) -> io::Result<()>;
    fn recv(&mut self) -> io::Result<Msg>;
}

/// TCP transport with a read timeout so a stalled peer surfaces as an error
/// instead of hanging the game loop.
pub struct TcpTransport {
    stream: TcpStream,
    timeout: Duration,
}

impl TcpTransport {
    pub fn new(stream: TcpStream) -> io::Result<Self> {
        let t = Self {
            stream,
            timeout: Duration::from_secs(10),
        };
        t.stream.set_read_timeout(Some(t.timeout))?;
        t.stream.set_nodelay(true)?; // commands are tiny and latency matters
        Ok(t)
    }

    pub fn with_timeout(mut self, timeout: Duration) -> io::Result<Self> {
        self.timeout = timeout;
        self.stream.set_read_timeout(Some(timeout))?;
        Ok(self)
    }

    pub fn peer_addr(&self) -> io::Result<std::net::SocketAddr> {
        self.stream.peer_addr()
    }

    /// Change the read timeout (the UI uses a short one so a half-open
    /// handshake cannot freeze the menu).
    pub fn stream_timeout(&mut self, timeout: Duration) -> io::Result<()> {
        self.timeout = timeout;
        self.stream.set_read_timeout(Some(timeout))
    }
}

impl Transport for TcpTransport {
    fn send(&mut self, msg: &Msg) -> io::Result<()> {
        let payload = msg.encode();
        self.stream
            .write_all(&(payload.len() as u32).to_le_bytes())?;
        self.stream.write_all(&payload)?;
        self.stream.flush()
    }

    fn recv(&mut self) -> io::Result<Msg> {
        let mut len = [0u8; 4];
        self.stream.read_exact(&mut len)?;
        let n = u32::from_le_bytes(len) as usize;
        if n == 0 || n > MAX_FRAME {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("bad frame length {n}"),
            ));
        }
        let mut buf = vec![0u8; n];
        self.stream.read_exact(&mut buf)?;
        Msg::decode(&buf).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "bad message"))
    }
}

// ---------------------------------------------------------------------------
// UDP transport: stop-and-wait ARQ behind the same `Transport` trait
// ---------------------------------------------------------------------------

const UDP_DATA: u8 = 0;
const UDP_ACK: u8 = 1;
/// Handshake datagrams are prefixed with this so a late copy can never be
/// mistaken for a data frame or an ACK once sequencing has started.
const UDP_HELLO: u8 = 0xFF;
const UDP_HEADER: usize = 5; // kind + seq
/// Conservative payload cap so datagrams survive any sane LAN MTU.
pub const MAX_DATAGRAM: usize = 1200;
/// Datagrams that do not fit are refused rather than silently fragmented.
const MAX_ATTEMPTS: u32 = 12;

/// Reliable message channel over UDP.
///
/// Lockstep is strictly turn-based, so at most one message is in flight in
/// each direction: stop-and-wait is enough and keeps the code small. While
/// waiting for an ACK the transport keeps draining incoming data, so two
/// peers can call `send` at the same time without deadlocking.
pub struct UdpTransport {
    sock: UdpSocket,
    peer: SocketAddr,
    seq_out: u32,
    seq_in: u32,
    timeout: Duration,
    inbox: VecDeque<Msg>,
    acked: Option<u32>,
    /// Retransmissions performed (diagnostics / desync forensics).
    pub retransmits: u64,
    /// Duplicate datagrams received and discarded.
    pub duplicates: u64,
}

impl UdpTransport {
    /// Wrap a bound socket and the address of the other end.
    pub fn new(sock: UdpSocket, peer: SocketAddr) -> io::Result<Self> {
        sock.set_read_timeout(Some(Duration::from_millis(50)))?;
        Ok(Self {
            sock,
            peer,
            seq_out: 0,
            seq_in: 0,
            timeout: Duration::from_millis(200),
            inbox: VecDeque::new(),
            acked: None,
            retransmits: 0,
            duplicates: 0,
        })
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn peer(&self) -> SocketAddr {
        self.peer
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.sock.local_addr()
    }

    fn frame(kind: u8, seq: u32, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(UDP_HEADER + payload.len());
        out.push(kind);
        out.extend_from_slice(&seq.to_le_bytes());
        out.extend_from_slice(payload);
        out
    }

    fn ack(&mut self, seq: u32) -> io::Result<()> {
        let f = Self::frame(UDP_ACK, seq, &[]);
        self.sock.send_to(&f, self.peer)?;
        Ok(())
    }

    /// Read and handle exactly one datagram. Returns `Ok(true)` if it
    /// delivered a message to the inbox.
    fn pump_once(&mut self) -> io::Result<bool> {
        let mut buf = [0u8; MAX_DATAGRAM];
        let (n, from) = match self.sock.recv_from(&mut buf) {
            Ok(v) => v,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                return Ok(false);
            }
            Err(e) => return Err(e),
        };
        if from != self.peer {
            return Ok(false); // stray datagram from another host
        }
        if n < UDP_HEADER {
            return Ok(false);
        }
        let kind = buf[0];
        let seq = u32::from_le_bytes([buf[1], buf[2], buf[3], buf[4]]);
        match kind {
            UDP_ACK => {
                self.acked = Some(seq);
                Ok(false)
            }
            UDP_DATA => {
                if seq == self.seq_in {
                    match Msg::decode(&buf[UDP_HEADER..n]) {
                        Some(m) => {
                            self.ack(seq)?;
                            self.seq_in = self.seq_in.wrapping_add(1);
                            self.inbox.push_back(m);
                            Ok(true)
                        }
                        // Corrupt payload: do not ACK, sender retransmits.
                        None => Ok(false),
                    }
                } else {
                    // Duplicate (already delivered): re-ACK, do not deliver.
                    self.duplicates += 1;
                    self.ack(seq)?;
                    Ok(false)
                }
            }
            _ => Ok(false),
        }
    }

    /// Pump until the pending ACK arrives or the deadline passes.
    fn wait_ack(&mut self, want: u32) -> io::Result<bool> {
        let deadline = Instant::now() + self.timeout;
        loop {
            if self.acked == Some(want) {
                self.acked = None;
                return Ok(true);
            }
            if Instant::now() >= deadline {
                return Ok(false);
            }
            self.pump_once()?;
        }
    }
}

impl Transport for UdpTransport {
    fn send(&mut self, msg: &Msg) -> io::Result<()> {
        let payload = msg.encode();
        if payload.len() + UDP_HEADER > MAX_DATAGRAM {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "message of {} B exceeds the {} B datagram budget",
                    payload.len(),
                    MAX_DATAGRAM - UDP_HEADER
                ),
            ));
        }
        let seq = self.seq_out;
        let frame = Self::frame(UDP_DATA, seq, &payload);
        for attempt in 0..MAX_ATTEMPTS {
            self.sock.send_to(&frame, self.peer)?;
            if attempt > 0 {
                self.retransmits += 1;
            }
            if self.wait_ack(seq)? {
                self.seq_out = self.seq_out.wrapping_add(1);
                return Ok(());
            }
        }
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            format!("no ACK for seq {seq} after {MAX_ATTEMPTS} attempts"),
        ))
    }

    fn recv(&mut self) -> io::Result<Msg> {
        if let Some(m) = self.inbox.pop_front() {
            return Ok(m);
        }
        loop {
            self.pump_once()?;
            if let Some(m) = self.inbox.pop_front() {
                return Ok(m);
            }
        }
    }
}

/// UDP client handshake: send a plain `Join` and wait for the host `Setup`.
///
/// The Join is a single unframed datagram (no sequence yet, since the peer
/// address is still unknown to the host). Once Setup arrives both sides wrap
/// the socket in [`UdpTransport`] and normal sequenced traffic starts.
pub fn udp_join(
    addr: &str,
    map_hash: [u8; 32],
    seed: u64,
    race: Race,
) -> io::Result<(UdpTransport, Msg)> {
    let target: SocketAddr = addr
        .parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "need IP:PORT"))?;
    let sock = UdpSocket::bind("0.0.0.0:0")?;
    sock.set_read_timeout(Some(Duration::from_millis(400)))?;
    let mut join = vec![UDP_HELLO];
    join.extend_from_slice(
        &Msg::Join {
            protocol: PROTOCOL,
            map_hash,
            seed,
            race,
        }
        .encode(),
    );
    let mut buf = [0u8; MAX_DATAGRAM];
    for _ in 0..10 {
        sock.send_to(&join, target)?;
        match sock.recv_from(&mut buf) {
            Ok((n, from)) => {
                if n > 1
                    && buf[0] == UDP_HELLO
                    && let Some(m @ Msg::Setup { .. }) = Msg::decode(&buf[1..n])
                {
                    return Ok((UdpTransport::new(sock, from)?, m));
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::TimedOut,
        "no Setup from host (is it hosting on this port?)",
    ))
}

/// UDP host handshake: wait for one `Join` and answer with `Setup`.
pub fn udp_accept(
    sock: UdpSocket,
    map_hash: [u8; 32],
    seed: u64,
    my_race: Race,
) -> io::Result<(UdpTransport, Race)> {
    sock.set_read_timeout(Some(Duration::from_secs(60)))?;
    let mut buf = [0u8; MAX_DATAGRAM];
    loop {
        let (n, from) = sock.recv_from(&mut buf)?;
        if n < 2 || buf[0] != UDP_HELLO {
            continue;
        }
        let Some(Msg::Join {
            protocol,
            map_hash: m,
            seed: s,
            race,
        }) = Msg::decode(&buf[1..n])
        else {
            continue;
        };
        if protocol != PROTOCOL {
            continue;
        }
        if m != map_hash || s != seed {
            continue;
        }
        // Answer unreliably (three copies): the client cannot ACK before it
        // knows our address, so a sequenced send here would deadlock.
        let mut setup = vec![UDP_HELLO];
        setup.extend_from_slice(
            &Msg::Setup {
                peers: 2,
                you: 1,
                map_hash,
                seed,
                races: vec![my_race, race],
                ticks_per_turn: DEFAULT_TICKS_PER_TURN,
            }
            .encode(),
        );
        sock.set_write_timeout(Some(Duration::from_millis(200)))?;
        for _ in 0..3 {
            let _ = sock.send_to(&setup, from);
        }
        let t = UdpTransport::new(sock, from)?;
        return Ok((t, race));
    }
}

// ---------------------------------------------------------------------------
// Lockstep core (pure, no I/O)
// ---------------------------------------------------------------------------

/// Turn bookkeeping shared by host and clients.
#[derive(Clone, Debug)]
pub struct Lockstep {
    pub local_peer: u8,
    pub peers: u8,
    pub ticks_per_turn: u32,
    pub turn: u32,
    pending: Vec<RecAct>,
    /// Every command committed so far, for reconnect resync and replays.
    log: Vec<crate::replay::RecCmd>,
    /// Host: each client's commands for the current turn once received, so
    /// a turn interrupted by a dropped peer resumes without waiting again
    /// for messages it already read.
    inbox: Vec<Option<Vec<RecAct>>>,
}

impl Lockstep {
    pub fn new(local_peer: u8, peers: u8, ticks_per_turn: u32) -> Self {
        Self {
            local_peer,
            peers,
            ticks_per_turn: ticks_per_turn.max(1),
            turn: 0,
            pending: Vec::new(),
            log: Vec::new(),
            inbox: Vec::new(),
        }
    }

    /// Queue a command for the turn that has not been sent yet.
    pub fn queue(&mut self, act: RecAct) {
        self.pending.push(act);
    }

    /// Take this peer's commands for the current turn.
    pub fn take_local(&mut self) -> Vec<RecAct> {
        std::mem::take(&mut self.pending)
    }

    /// Merge per-peer command lists into one deterministic order.
    ///
    /// The order is (peer index, insertion index). Every peer sees the same
    /// bundle, so applying it in order is all determinism needs.
    pub fn merge(per_peer: &[Vec<RecAct>]) -> Vec<RecAct> {
        let mut out = Vec::new();
        for peer in per_peer {
            out.extend(peer.iter().cloned());
        }
        out
    }

    /// Apply a merged bundle and advance one turn.
    pub fn commit(&mut self, world: &mut World, per_peer: &[Vec<RecAct>]) {
        for act in Self::merge(per_peer) {
            self.log.push(crate::replay::RecCmd {
                tick: world.tick,
                act: act.clone(),
            });
            crate::replay::apply_act(world, &act);
        }
        for _ in 0..self.ticks_per_turn {
            world.step();
        }
        self.turn += 1;
        self.inbox.clear();
    }

    /// Forget what a peer sent for the current turn (it rejoined and will
    /// send the turn again on its new connection).
    /// `inbox[0]` is the host's own commands, so peer `n` is slot `n`.
    pub fn forget_peer_turn(&mut self, peer: u8) {
        if peer == 0 {
            return;
        }
        if let Some(slot) = self.inbox.get_mut(peer as usize) {
            *slot = None;
        }
    }

    /// The match so far, in the same format a `.gcrep` replay uses.
    pub fn recording(&self) -> crate::replay::Recording {
        let mut rec = crate::replay::Recording::new(world_seed(), [0u8; 32]);
        rec.cmds = self.log.clone();
        rec
    }

    /// Replace the log after a resync (the peer receives the host's copy).
    pub fn adopt_log(&mut self, rec: &crate::replay::Recording) {
        self.log = rec.cmds.clone();
    }
}

/// `World::new` seeds the LCG with this constant; the log records it so a
/// reconstructed match starts from the same stream.
pub const WORLD_SEED: u64 = 0x1234_5678_9abc_def0;

fn world_seed() -> u64 {
    WORLD_SEED
}

// ---------------------------------------------------------------------------
// Host and client drivers (2..=8 peers)
// ---------------------------------------------------------------------------

/// A host that has accepted its peers but not started the game yet.
pub struct Lobby {
    pub listener: TcpListener,
    pub map_hash: [u8; 32],
    pub seed: u64,
    pub ticks_per_turn: u32,
}

impl Lobby {
    pub fn bind(addr: &str, map_hash: [u8; 32], seed: u64) -> io::Result<Self> {
        Ok(Self {
            listener: TcpListener::bind(addr)?,
            map_hash,
            seed,
            ticks_per_turn: DEFAULT_TICKS_PER_TURN,
        })
    }

    pub fn local_addr(&self) -> io::Result<std::net::SocketAddr> {
        self.listener.local_addr()
    }

    /// Wait for `peers - 1` clients, agree the setup, and hand back the
    /// transports in join order (peer 1..N).  The host plays Terran.
    pub fn accept_peers(&self, peers: u8) -> io::Result<(Vec<TcpTransport>, Vec<Race>)> {
        let mut queue = JoinQueue::new(peers);
        while queue.players() < peers {
            self.poll_join(&mut queue, true)?;
        }
        self.start(queue, Race::Terran)
    }

    /// Accept at most one connection and validate its `Join` (protocol, map,
    /// seed).  `block = false` returns `Ok(None)` when nobody is waiting, so a
    /// menu can poll every frame; a full queue accepts nothing.  An invalid
    /// `Join` is an error for that connection only: the queue is unchanged.
    pub fn poll_join(&self, queue: &mut JoinQueue, block: bool) -> io::Result<Option<SocketAddr>> {
        if queue.players() >= queue.capacity {
            return Ok(None);
        }
        let Some((stream, addr)) = self.accept(block)? else {
            return Ok(None);
        };
        let mut t = TcpTransport::new(stream)?;
        match t.recv()? {
            Msg::Join {
                protocol,
                map_hash,
                seed,
                race,
            } => {
                if protocol != PROTOCOL {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "protocol mismatch",
                    ));
                }
                if map_hash != self.map_hash {
                    return Err(io::Error::other("map mismatch"));
                }
                if seed != self.seed {
                    return Err(io::Error::other("seed mismatch"));
                }
                queue.joined.push((t, race, addr));
                Ok(Some(addr))
            }
            other => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("expected Join, got {other:?}"),
            )),
        }
    }

    /// Close the lobby: tell every joined client the setup (each with its
    /// own `you`) and hand back the transports (peer 1..N) and races (host
    /// first).
    pub fn start(
        &self,
        queue: JoinQueue,
        host_race: Race,
    ) -> io::Result<(Vec<TcpTransport>, Vec<Race>)> {
        let mut races = vec![host_race];
        races.extend(queue.joined.iter().map(|(_, race, _)| *race));
        let peers = races.len() as u8;
        let mut clients = Vec::with_capacity(queue.joined.len());
        for (i, (mut t, _, _)) in queue.joined.into_iter().enumerate() {
            t.send(&Msg::Setup {
                peers,
                you: (i + 1) as u8,
                map_hash: self.map_hash,
                seed: self.seed,
                races: races.clone(),
                ticks_per_turn: self.ticks_per_turn,
            })?;
            clients.push(t);
        }
        Ok((clients, races))
    }

    fn accept(&self, block: bool) -> io::Result<Option<(TcpStream, SocketAddr)>> {
        self.listener.set_nonblocking(!block)?;
        let accepted = self.listener.accept();
        self.listener.set_nonblocking(false)?;
        match accepted {
            Ok((stream, addr)) => {
                stream.set_nonblocking(false)?;
                Ok(Some((stream, addr)))
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e),
        }
    }
}

/// Clients that joined a lobby and wait for the host to start (2..=8
/// players including the host).
pub struct JoinQueue {
    capacity: u8,
    joined: Vec<(TcpTransport, Race, SocketAddr)>,
}

impl JoinQueue {
    /// `capacity` counts the host; it is clamped to 2..=8.
    pub fn new(capacity: u8) -> Self {
        Self {
            capacity: capacity.clamp(2, 8),
            joined: Vec::new(),
        }
    }

    /// Players in the lobby, the host included.
    pub fn players(&self) -> u8 {
        1 + self.joined.len() as u8
    }

    pub fn capacity(&self) -> u8 {
        self.capacity
    }

    /// Address and race of each joined client, in peer order (1..).
    pub fn members(&self) -> Vec<(SocketAddr, Race)> {
        self.joined.iter().map(|(_, r, a)| (*a, *r)).collect()
    }
}

/// Client side: connect, join, and receive the agreed setup.
pub fn connect(
    addr: &str,
    map_hash: [u8; 32],
    seed: u64,
    race: Race,
) -> io::Result<(TcpTransport, Msg)> {
    let stream = TcpStream::connect(addr)?;
    let mut t = TcpTransport::new(stream)?;
    t.send(&Msg::Join {
        protocol: PROTOCOL,
        map_hash,
        seed,
        race,
    })?;
    let setup = t.recv()?;
    match &setup {
        Msg::Setup { map_hash: m, .. } if *m != map_hash => {
            return Err(io::Error::other("map mismatch"));
        }
        Msg::Setup { .. } => {}
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("expected Setup, got {other:?}"),
            ));
        }
    }
    Ok((t, setup))
}

/// A client the host lost during a turn (`peer` is 1-based, the index the
/// client has in the lockstep).
#[derive(Debug)]
pub struct PeerFailure {
    pub peer: u8,
    pub error: io::Error,
}

/// One host turn: collect every peer's commands, broadcast the bundle, and
/// compare state hashes.  Returns true when a desync was detected.  A lost
/// peer is an error (see [`host_turn_peers`] for which one).
pub fn host_turn<T: Transport>(
    lock: &mut Lockstep,
    world: &mut World,
    clients: &mut [T],
    local_acts: Vec<RecAct>,
) -> io::Result<bool> {
    host_turn_peers(lock, world, clients, local_acts).map_err(|f| f.error)
}

/// [`host_turn`] that names the lost peer and can be resumed.
///
/// * Collecting: commands already received are kept in the lockstep, so
///   after a failure (and a rejoin, see [`try_accept_rejoin`]) the same call
///   only waits for the peers it has not heard from; local commands given
///   on a retry are appended to the ones the first attempt stored.  A
///   stale `Sum` from a peer that never got the previous turn's reply is
///   acknowledged and skipped.
/// * Once every command is in, the turn is committed for all live peers: a
///   peer that fails while the bundle or the hashes go round does not stop
///   the others, and the failure is reported after the commit.
pub fn host_turn_peers<T: Transport>(
    lock: &mut Lockstep,
    world: &mut World,
    clients: &mut [T],
    local_acts: Vec<RecAct>,
) -> Result<bool, PeerFailure> {
    let turn = lock.turn;
    let fail = |i: usize, error: io::Error| PeerFailure {
        peer: (i + 1) as u8,
        error,
    };
    if lock.inbox.len() != clients.len() + 1 {
        lock.inbox = vec![None; clients.len() + 1];
    }
    // A retry adds the commands queued since the interrupted attempt.
    lock.inbox[0]
        .get_or_insert_with(Vec::new)
        .extend(local_acts);
    for (i, c) in clients.iter_mut().enumerate() {
        while lock.inbox[i + 1].is_none() {
            match c.recv().map_err(|e| fail(i, e))? {
                Msg::Turn { turn: t, acts } if t == turn => lock.inbox[i + 1] = Some(acts),
                Msg::Turn { turn: t, .. } => {
                    return Err(fail(
                        i,
                        io::Error::other(format!("peer {} out of order: {t} != {turn}", i + 1)),
                    ));
                }
                Msg::Sum { turn: t, hash } if t < turn => {
                    c.send(&Msg::Sum { turn: t, hash })
                        .map_err(|e| fail(i, e))?;
                }
                Msg::Bye => return Err(fail(i, io::Error::other("peer left"))),
                other => {
                    return Err(fail(
                        i,
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("expected Turn, got {other:?}"),
                        ),
                    ));
                }
            }
        }
    }
    let per_peer: Vec<Vec<RecAct>> = lock
        .inbox
        .iter()
        .map(|slot| slot.clone().unwrap_or_default())
        .collect();
    let bundle = Msg::Bundle {
        turn,
        per_peer: per_peer.clone(),
    };
    let mut lost: Option<PeerFailure> = None;
    let mut live = vec![true; clients.len()];
    for (i, c) in clients.iter_mut().enumerate() {
        if let Err(e) = c.send(&bundle) {
            live[i] = false;
            lost.get_or_insert(fail(i, e));
        }
    }
    lock.commit(world, &per_peer);
    let mine = crate::replay::state_hash(world);

    // Hash exchange: every live peer reports the hash after the same turn.
    let mut desync = false;
    for (i, c) in clients.iter_mut().enumerate() {
        if !live[i] {
            continue;
        }
        let reply = match c.recv() {
            Ok(Msg::Sum { turn: t, hash }) => {
                if t != turn || hash != mine {
                    desync = true;
                    Msg::Desync {
                        turn,
                        peer: (i + 1) as u8,
                    }
                } else {
                    Msg::Sum { turn, hash: mine }
                }
            }
            Ok(Msg::Bye) => {
                lost.get_or_insert(fail(i, io::Error::other("peer left")));
                continue;
            }
            Ok(other) => {
                lost.get_or_insert(fail(
                    i,
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("expected Sum, got {other:?}"),
                    ),
                ));
                continue;
            }
            Err(e) => {
                lost.get_or_insert(fail(i, e));
                continue;
            }
        };
        if let Err(e) = c.send(&reply) {
            lost.get_or_insert(fail(i, e));
        }
    }
    match lost {
        Some(f) => Err(f),
        None => Ok(desync),
    }
}

/// One client turn: send local commands, apply the host bundle, report hash.
/// Returns true when the host reported a desync.
pub fn client_turn<T: Transport>(
    lock: &mut Lockstep,
    world: &mut World,
    host: &mut T,
    local_acts: Vec<RecAct>,
) -> io::Result<bool> {
    let turn = lock.turn;
    host.send(&Msg::Turn {
        turn,
        acts: local_acts,
    })?;
    match host.recv()? {
        Msg::Bundle { turn: t, per_peer } => {
            if t != turn {
                return Err(io::Error::other(format!("bundle turn {t} != {turn}")));
            }
            lock.commit(world, &per_peer);
        }
        Msg::Bye => return Err(io::Error::other("host closed")),
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("expected Bundle, got {other:?}"),
            ));
        }
    }
    let hash = crate::replay::state_hash(world);
    host.send(&Msg::Sum { turn, hash })?;
    match host.recv()? {
        Msg::Sum { .. } => Ok(false),
        Msg::Desync { .. } => Ok(true),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("expected Sum/Desync, got {other:?}"),
        )),
    }
}

// ---------------------------------------------------------------------------
// LAN discovery: broadcasting lobbies on a well-known UDP port
// ---------------------------------------------------------------------------

/// Fixed port every client listens on for lobby announcements.
pub const DISCOVERY_PORT: u16 = 7778;

/// A lobby advertising itself on the local network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Announce {
    pub name: String,
    pub map_hash: [u8; 32],
    pub port: u16,
    pub players: u8,
    pub started: bool,
}

impl Announce {
    pub fn encode(&self) -> Vec<u8> {
        use crate::replay::{w_u8, w_u16, w_u32};
        let mut out = Vec::new();
        w_u16(&mut out, PROTOCOL);
        let name = self.name.as_bytes();
        let n = name.len().min(120);
        w_u32(&mut out, n as u32);
        out.extend_from_slice(&name[..n]);
        out.extend_from_slice(&self.map_hash);
        w_u16(&mut out, self.port);
        w_u8(&mut out, self.players);
        w_u8(&mut out, self.started as u8);
        out
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        use crate::replay::Cursor;
        let mut c = Cursor::new(bytes);
        if c.u16()? != PROTOCOL {
            return None;
        }
        let n = c.u32()? as usize;
        if n > 120 {
            return None;
        }
        let name = String::from_utf8_lossy(c.take_bytes(n)?).into_owned();
        Some(Announce {
            name,
            map_hash: c.take32()?,
            port: c.u16()?,
            players: c.u8()?,
            started: c.u8()? != 0,
        })
    }
}

/// Socket a host uses to broadcast. Binds an ephemeral port with broadcast on.
pub fn announcer() -> io::Result<UdpSocket> {
    let sock = UdpSocket::bind("0.0.0.0:0")?;
    sock.set_broadcast(true)?;
    Ok(sock)
}

/// Socket a client uses to listen for lobbies (`0` picks an ephemeral port,
/// used by tests; games pass [`DISCOVERY_PORT`]).
pub fn discovery_socket(port: u16) -> io::Result<UdpSocket> {
    let sock = UdpSocket::bind(("0.0.0.0", port))?;
    sock.set_read_timeout(Some(Duration::from_millis(250)))?;
    Ok(sock)
}

/// Broadcast `announce` to the whole LAN (and to loopback, which is where
/// two instances on one machine find each other).
pub fn broadcast(sock: &UdpSocket, announce: &Announce) -> io::Result<()> {
    let payload = announce.encode();
    sock.send_to(&payload, ("255.255.255.255", DISCOVERY_PORT))?;
    // Some setups drop directed broadcast; loopback covers same-machine
    // testing and is harmless on a real LAN.
    sock.send_to(&payload, ("127.0.0.1", DISCOVERY_PORT))?;
    Ok(())
}

/// Collect announcements for `window`, de-duplicated by address.
pub fn discover(sock: &UdpSocket, window: Duration) -> io::Result<Vec<(SocketAddr, Announce)>> {
    let deadline = Instant::now() + window;
    let mut seen: Vec<(SocketAddr, Announce)> = Vec::new();
    let mut buf = [0u8; MAX_DATAGRAM];
    while Instant::now() < deadline {
        match sock.recv_from(&mut buf) {
            Ok((n, from)) => {
                if let Some(a) = Announce::decode(&buf[..n])
                    && !seen.iter().any(|(addr, _)| *addr == from)
                {
                    seen.push((from, a));
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(seen)
}

// ---------------------------------------------------------------------------
// Reconnection: resync by replaying the host's command log
// ---------------------------------------------------------------------------

/// Host side: accept one reconnecting peer on the lobby listener and send it
/// the command log up to the current turn (blocking).
pub fn accept_rejoin(lobby: &Lobby, lock: &Lockstep) -> io::Result<TcpTransport> {
    loop {
        if let Some((_, t)) = try_accept_rejoin(lobby, lock, true)? {
            return Ok(t);
        }
    }
}

/// [`accept_rejoin`] for a game loop: `block = false` returns `Ok(None)` when
/// nobody is reconnecting.  On success returns the peer index the client
/// claims (its `Rejoin`) and its new transport; the host forgets what that
/// peer sent for the current turn, which it will send again.
pub fn try_accept_rejoin(
    lobby: &Lobby,
    lock: &Lockstep,
    block: bool,
) -> io::Result<Option<(u8, TcpTransport)>> {
    let Some((stream, _)) = lobby.accept(block)? else {
        return Ok(None);
    };
    let mut t = TcpTransport::new(stream)?;
    let peer = match t.recv()? {
        Msg::Rejoin {
            peer,
            map_hash,
            seed,
        } => {
            if map_hash != lobby.map_hash {
                return Err(io::Error::other("rejoin map mismatch"));
            }
            if seed != lobby.seed {
                return Err(io::Error::other("rejoin seed mismatch"));
            }
            if peer == 0 || peer >= lock.peers {
                return Err(io::Error::other(format!("rejoin from unknown peer {peer}")));
            }
            peer
        }
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("expected Rejoin, got {other:?}"),
            ));
        }
    };
    t.send(&Msg::Resync {
        turn: lock.turn,
        log: lock.recording().to_bytes(),
    })?;
    Ok(Some((peer, t)))
}

/// How long one reconnect attempt waits for the host to answer the TCP
/// handshake.
pub const REJOIN_CONNECT_TIMEOUT: Duration = Duration::from_millis(1500);

/// Client side: ask to rejoin and receive the log. The caller rebuilds the
/// world from its own map copy and calls [`apply_resync`].
pub fn request_resync(
    addr: &str,
    map_hash: [u8; 32],
    seed: u64,
    peer: u8,
) -> io::Result<(TcpTransport, u32, crate::replay::Recording)> {
    let target = addr
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no address"))?;
    let stream = TcpStream::connect_timeout(&target, REJOIN_CONNECT_TIMEOUT)?;
    let mut t = TcpTransport::new(stream)?;
    t.send(&Msg::Rejoin {
        peer,
        map_hash,
        seed,
    })?;
    match t.recv()? {
        Msg::Resync { turn, log } => {
            let rec = crate::replay::Recording::from_bytes(&log)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "bad resync log"))?;
            Ok((t, turn, rec))
        }
        other => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("expected Resync, got {other:?}"),
        )),
    }
}

/// Exponential reconnect schedule: attempt `n` (0-based) waits
/// `min(base * 2^n, max)` after the previous failure, for at most
/// `attempts` attempts.  Pure (the caller passes the clock), so a game loop
/// can poll [`Backoff::due`] every frame instead of sleeping.
#[derive(Clone, Debug)]
pub struct Backoff {
    base: Duration,
    max: Duration,
    attempts: u32,
    failures: u32,
    next_at: Instant,
}

impl Backoff {
    /// Defaults for a dropped LAN peer: 250 ms doubling to 4 s, 8 attempts
    /// (about 17 s in all).
    pub fn lan(now: Instant) -> Self {
        Self::new(Duration::from_millis(250), Duration::from_secs(4), 8, now)
    }

    /// The first attempt is due immediately.
    pub fn new(base: Duration, max: Duration, attempts: u32, now: Instant) -> Self {
        Self {
            base,
            max,
            attempts: attempts.max(1),
            failures: 0,
            next_at: now,
        }
    }

    /// Wait before attempt `n + 1` after `n + 1` failures.
    pub fn delay(&self, failures: u32) -> Duration {
        let factor = 1u32
            .checked_shl(failures.saturating_sub(1))
            .unwrap_or(u32::MAX);
        self.base.saturating_mul(factor).min(self.max)
    }

    pub fn due(&self, now: Instant) -> bool {
        !self.exhausted() && now >= self.next_at
    }

    /// Record a failed attempt at `now`; `false` once no attempt is left.
    pub fn fail(&mut self, now: Instant) -> bool {
        self.failures += 1;
        self.next_at = now + self.delay(self.failures);
        !self.exhausted()
    }

    pub fn exhausted(&self) -> bool {
        self.failures >= self.attempts
    }

    pub fn failures(&self) -> u32 {
        self.failures
    }

    pub fn next_at(&self) -> Instant {
        self.next_at
    }
}

/// Blocking [`request_resync`] retried on the [`Backoff`] schedule (tools and
/// tests; the client polls the same schedule from its frame loop).
pub fn rejoin_with_backoff(
    addr: &str,
    map_hash: [u8; 32],
    seed: u64,
    peer: u8,
    backoff: &mut Backoff,
) -> io::Result<(TcpTransport, u32, crate::replay::Recording)> {
    loop {
        let now = Instant::now();
        if now < backoff.next_at() {
            std::thread::sleep(backoff.next_at() - now);
        }
        match request_resync(addr, map_hash, seed, peer) {
            Ok(done) => return Ok(done),
            Err(e) => {
                if !backoff.fail(Instant::now()) {
                    return Err(io::Error::new(
                        e.kind(),
                        format!("gave up after {} attempts: {e}", backoff.failures()),
                    ));
                }
            }
        }
    }
}

/// Fast-forward a freshly rebuilt world to `turn` and continue from there.
///
/// The world must already be rebuilt from the same map and races; after this
/// the two peers hold identical state without transferring any of it.
pub fn apply_resync(
    world: &mut World,
    lock: &mut Lockstep,
    turn: u32,
    rec: &crate::replay::Recording,
) {
    let target = turn as u64 * lock.ticks_per_turn as u64;
    crate::replay::replay_to(world, rec, target);
    lock.turn = turn;
    lock.adopt_log(rec);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::unit as uid;
    use crate::sim::Order;

    #[test]
    fn backoff_doubles_to_its_cap_and_gives_up() {
        let t0 = Instant::now();
        let ms = Duration::from_millis;
        let mut b = Backoff::new(ms(250), Duration::from_secs(4), 8, t0);
        assert!(b.due(t0), "the first attempt is immediate");
        let delays: Vec<Duration> = (1..=8).map(|n| b.delay(n)).collect();
        assert_eq!(
            delays,
            [250, 500, 1000, 2000, 4000, 4000, 4000, 4000]
                .map(ms)
                .to_vec()
        );
        assert!(b.fail(t0));
        assert!(!b.due(t0 + ms(249)));
        assert!(b.due(t0 + ms(250)));
        for _ in 2..8 {
            assert!(b.fail(t0));
        }
        assert!(!b.fail(t0), "the eighth failure is the last");
        assert!(b.exhausted() && !b.due(t0 + Duration::from_secs(60)));
        assert_eq!(b.failures(), 8);
        // Huge failure counts saturate instead of overflowing.
        assert_eq!(b.delay(200), Duration::from_secs(4));
    }

    #[test]
    fn a_lobby_holds_two_to_eight_players() {
        assert_eq!(JoinQueue::new(1).capacity(), 2);
        assert_eq!(JoinQueue::new(12).capacity(), 8);
        let q = JoinQueue::new(4);
        assert_eq!((q.players(), q.capacity()), (1, 4));
        assert!(q.members().is_empty());
    }

    #[test]
    fn every_message_roundtrips() {
        let acts = vec![
            RecAct::Issue {
                unit: 7,
                order: Order::Move { pos: (10, 20) },
            },
            RecAct::Train {
                building: 3,
                unit: uid::MARINE,
            },
        ];
        let msgs = vec![
            Msg::Join {
                protocol: PROTOCOL,
                map_hash: [9u8; 32],
                seed: 42,
                race: Race::Zerg,
            },
            Msg::Setup {
                peers: 3,
                you: 2,
                map_hash: [9u8; 32],
                seed: 42,
                races: vec![Race::Terran, Race::Zerg, Race::Protoss],
                ticks_per_turn: 4,
            },
            Msg::Turn {
                turn: 5,
                acts: acts.clone(),
            },
            Msg::Bundle {
                turn: 5,
                per_peer: vec![acts.clone(), vec![]],
            },
            Msg::Sum {
                turn: 5,
                hash: 0xdead_beef,
            },
            Msg::Desync { turn: 5, peer: 1 },
            Msg::Chat {
                from: 0,
                text: "gg".into(),
            },
            Msg::Bye,
        ];
        for m in &msgs {
            let bytes = m.encode();
            assert_eq!(Msg::decode(&bytes), Some(m.clone()), "roundtrip {m:?}");
        }
    }

    #[test]
    fn bad_frames_are_rejected() {
        assert!(Msg::decode(&[]).is_none());
        assert!(Msg::decode(&[1, 0, 99]).is_none()); // unknown tag
        let mut m = Msg::Chat {
            from: 0,
            text: "hi".into(),
        }
        .encode();
        m.push(0); // trailing garbage
        assert!(Msg::decode(&m).is_none());
    }

    #[test]
    fn merge_is_peer_then_insertion_order() {
        let a = vec![RecAct::Train {
            building: 1,
            unit: 2,
        }];
        let b = vec![
            RecAct::Train {
                building: 3,
                unit: 4,
            },
            RecAct::Train {
                building: 5,
                unit: 6,
            },
        ];
        let merged = Lockstep::merge(&[a, b]);
        assert_eq!(merged.len(), 3);
        assert!(matches!(merged[0], RecAct::Train { building: 1, .. }));
        assert!(matches!(merged[1], RecAct::Train { building: 3, .. }));
        assert!(matches!(merged[2], RecAct::Train { building: 5, .. }));
    }

    #[test]
    fn udp_transport_really_is_reliable_both_ways() {
        use std::net::UdpSocket;
        use std::thread;

        let a = UdpSocket::bind("127.0.0.1:0").unwrap();
        let b = UdpSocket::bind("127.0.0.1:0").unwrap();
        let (a_addr, b_addr) = (a.local_addr().unwrap(), b.local_addr().unwrap());
        let mut ta = UdpTransport::new(a, b_addr).unwrap();
        let mut tb = UdpTransport::new(b, a_addr).unwrap();

        let peer = thread::spawn(move || {
            for i in 0..5u32 {
                tb.send(&Msg::Chat {
                    from: 1,
                    text: format!("pong{i}"),
                })
                .expect("udp send b");
                match tb.recv().expect("udp recv b") {
                    Msg::Chat { from, .. } => assert_eq!(from, 0),
                    other => panic!("unexpected {other:?}"),
                }
            }
        });

        for i in 0..5u32 {
            ta.send(&Msg::Chat {
                from: 0,
                text: format!("ping{i}"),
            })
            .expect("udp send a");
            match ta.recv().expect("udp recv a") {
                Msg::Chat { from, .. } => assert_eq!(from, 1),
                other => panic!("unexpected {other:?}"),
            }
        }
        peer.join().unwrap();
    }

    #[test]
    fn udp_discards_duplicates_and_reacks() {
        use std::net::UdpSocket;

        let listener = UdpSocket::bind("127.0.0.1:0").unwrap();
        let listen_addr = listener.local_addr().unwrap();
        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
        let sender_addr = sender.local_addr().unwrap();
        let mut t = UdpTransport::new(listener, sender_addr).unwrap();

        let raw = |seq: u32, text: &str| {
            let payload = Msg::Chat {
                from: 0,
                text: text.to_string(),
            }
            .encode();
            let mut f = vec![UDP_DATA];
            f.extend_from_slice(&seq.to_le_bytes());
            f.extend_from_slice(&payload);
            f
        };
        // seq 0 twice (the second is a duplicate), then seq 1.
        sender.send_to(&raw(0, "first"), listen_addr).unwrap();
        sender.send_to(&raw(0, "first"), listen_addr).unwrap();
        sender.send_to(&raw(1, "second"), listen_addr).unwrap();

        match t.recv().unwrap() {
            Msg::Chat { text, .. } => assert_eq!(text, "first"),
            other => panic!("unexpected {other:?}"),
        }
        match t.recv().unwrap() {
            Msg::Chat { text, .. } => assert_eq!(text, "second", "duplicate must not be delivered"),
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(t.duplicates, 1, "duplicate accounted for");
    }

    #[test]
    fn announce_roundtrips_and_discovery_finds_a_lobby() {
        use std::net::UdpSocket;
        use std::thread;

        let a = Announce {
            name: "Boxer 1v1".into(),
            map_hash: [3u8; 32],
            port: 7777,
            players: 2,
            started: false,
        };
        assert_eq!(Announce::decode(&a.encode()), Some(a.clone()));

        let listener = discovery_socket(0).unwrap();
        let listen_addr = listener.local_addr().unwrap();
        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();

        let h = thread::spawn(move || {
            // Give the reader a moment to start, then announce twice (the
            // second is a duplicate that must be de-duplicated).
            thread::sleep(std::time::Duration::from_millis(30));
            let payload = a.encode();
            sender.send_to(&payload, listen_addr).unwrap();
            sender.send_to(&payload, listen_addr).unwrap();
        });

        let found = discover(&listener, Duration::from_millis(400)).unwrap();
        h.join().unwrap();
        assert_eq!(found.len(), 1, "one lobby, deduplicated");
        assert_eq!(found[0].1.name, "Boxer 1v1");
        assert_eq!(found[0].1.port, 7777);
    }

    #[test]
    fn udp_dedups_and_transport_share_the_message_encoding() {
        // Same bytes on both transports: the wire format is `Msg::encode`.
        let m = Msg::Sum {
            turn: 9,
            hash: 0x1bad_cafe,
        };
        assert_eq!(Msg::decode(&m.encode()), Some(m));
    }
}

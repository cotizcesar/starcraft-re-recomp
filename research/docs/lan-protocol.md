# LAN protocol — final status

Implemented in `engine-rust/src/net.rs` + `engine-rust/src/bin/sc_lan.rs` +
lobby in `sc_view`. Everything below was executed; what could not be executed
is marked as such.

## 1. Transports (same `Transport` interface)

| Transport | Status | Mechanics |
|---|---|---|
| TCP | **done, tested** | framing `u32 len + payload`, `nodelay`, 10 s timeout |
| UDP | **done, tested** | stop-and-wait ARQ (seq + ACK + 12 retries), duplicate discard with re-ACK, `0xFF`-prefixed handshake so a late hello is not confused with an ACK |

Both implement `trait Transport { send(&Msg); recv() -> Msg }`, and
`host_turn`/`client_turn` are generic over it: the lockstep logic is
identical for TCP and UDP.

UDP extras: `MAX_DATAGRAM = 1200` (a larger message is rejected with an error
instead of being fragmented), and `retransmits`/`duplicates` counters for
diagnostics.

## 2. Synchronization model

- 4 ticks per turn (`DEFAULT_TICKS_PER_TURN`), checksum **every turn**.
- Star topology: the host collects and broadcasts a `Bundle` with each
  peer's commands; everyone applies them in the same order
  (`peer 0..N`, within each peer by insertion).
- Only commands travel (`RecAct`), never state. Same encoding as a
  `.gcrep`, so network and replay share the format (DRY).

## 3. Lobby inside `sc_view`

- `MenuScreen::Lan` screen, reachable with **L** in the game type
  menu (and `--shot-screen lan` for headless verification).
- **S: scan** — discovers real games via UDP broadcast
  (`DISCOVERY_PORT = 7778`), lists `name - ip [port]`.
- **H: host** — opens the lobby on `:7777` with one seat per start
  location of the map (2..=8, `JoinQueue`), announces itself every second
  with the current player count, and takes joins without blocking
  (`Lobby::poll_join`).  The panel lists every seat (host, joined clients
  with their race, open seats).  The match starts when the lobby is full or
  on **ENTER** with at least two players (`Lobby::start`).
- **ENTER: join** — handshake with the selected lobby; the host decides
  map and races, and every peer builds the world with `build_from_save` to
  start in an identical state.  Protocol 2: `Setup` carries `you`, the
  receiving client's peer index (protocol 1 clients assumed peer 1, which
  broke any lobby above two players).
- In game, all orders go through `send_order`/`send_train`: if there is a
  LAN session they are queued for the next turn; otherwise they are applied locally.

## 3b. Retail multiplayer flow (protocol 3, 2026-11-04)

The Multiplayer entry now follows the retail client as recorded on
2026-11-04 21:11:48 (session log 2026-11-04 (86)): Select Game Type ->
Select Connection (Local Area Network; Battle.net stays listed but Ok is
only enabled for LAN) -> Registry -> Games -> Create -> "Creating game..."
-> lobby -> countdown -> game.  Screens: `src/bin/sc_view/mp.rs`; rules:
`src/multiplayer.rs`.  The key-**L** screen above remains.

- **Games** keeps a non-blocking discovery socket and lists announces with
  `upsert_game` (a game heard by broadcast and on loopback is one row).
  `Announce` (protocol 3) also carries the game type, map name, map size
  and speed index the information plate shows.
- **Create** binds the lobby on `LOBBY_PORT` (7777) and announces it.
  Joins are polled every frame; a joiner sits in the first Open slot.
- Protocol 3 messages: `Join { .., name }` (Registry name); host ->
  client `LobbyState { you, lobby }` on join, on every change and once a
  second (`multiplayer::Lobby::encode`, slots/teams/observers, names
  capped at 32 bytes); client -> host `LobbyRace { race }`; `Chat` relayed
  by the host; `Setup { .., setup }` carries the resolved
  `multiplayer::GameSetup` (races by slot, occupants, teams), from which
  each peer finds its slot (`slot_of_peer`).  The joiner reads with
  `TcpTransport::try_recv` (peeks a whole frame, never blocks), so its UI
  keeps running in the lobby.  A peer that leaves the lobby is dropped
  from the queue and later peers move down one index (`JoinQueue::remove`,
  `Lobby::remove_peer`).
- Computer slots run inside `Lockstep::commit` (`ai::run_computers`,
  before every tick, on every peer) and are re-run by `apply_resync`
  (`replay::replay_to_with`), so they never cross the wire and a rejoin
  rebuilds them.
- Evidence: `tests/lan.rs::a_joiner_sits_in_the_lobby_and_starts_with_the_setup`,
  `a_joiner_leaving_the_lobby_reopens_its_slot`,
  `tests/multiplayer.rs::lobby_computers_run_identically_on_every_peer_and_after_a_resync`,
  and the two-process run of `scripts/mp_flow_gate.py` (host and joiner end
  on the same state hash with no desync), `engine-rust/docs/gates/mp-flow.txt`.
- Deviation (FACT about this client): the retail Games footer reads "Using
  UDP ports: 5353 and 6112."; this client shows its own ports (TCP 7777,
  UDP 7778).  Protocol 3 does not talk to protocol 2 peers (the version is
  checked on every message).

## 4. Reconnection after a drop

Design: **no state is transferred, the log is transferred**.

1. The connection drops → `host_turn_peers` names the lost peer
   (`PeerFailure`).  Commands already received for the turn stay in the
   lockstep inbox, so the retry never waits again for a message it read;
   once all commands are in, a peer failing during the bundle or hash
   exchange does not stop the others (the turn is committed, the failure
   reported after).  A stale `Sum` is acknowledged and skipped.
2. The host pauses and polls `try_accept_rejoin` without blocking for up to
   30 s (`REJOIN_WAIT`); the lobby listener stays open during the match.
   The in-match read timeout (40 s) is longer, so peers that stayed do not
   give up on a host that is waiting for someone else.
3. The dropped client retries on an exponential `Backoff` (`Backoff::lan`:
   250 ms doubling to 4 s, 8 attempts ≈ 17 s; connect timeout 1.5 s) and
   sends `Rejoin { peer, map_hash, seed }`.
4. The host replies `Resync { turn, log }` with the complete `Recording` and
   forgets what that peer sent for the interrupted turn.
5. The peer rebuilds the world with its copy of the map and does a
   deterministic fast-forward up to `turn`; then it continues lockstep.

Covered by `tests/lan.rs`: `peer_reconnects_and_resyncs_from_the_command_log`
(2 peers), `a_dropped_peer_rejoins_with_backoff_while_the_others_wait`
(3 peers: peer 2 drops at turn 6, peer 1 keeps its turn, all three end on
the host's hash), `rejoin_backoff_retries_until_the_host_listens` (refused
attempts retried; a schedule that runs out gives up).  In the client:
`engine-rust/docs/gates/lan-lobby-reconnect.txt`.

## 5. Tests executed (evidence)

| Test | Result |
|---|---|
| `cargo test --lib net::` | 9/9 (messages, merge, bidirectional UDP, duplicates, discovery, backoff, lobby capacity) |
| `cargo test --test lan` | 6/6 (TCP 20 turns, different map rejected, reconnection+resync, 3-peer polled lobby, 3-peer drop + backoff rejoin, backoff until the host listens); 5 consecutive runs green (2026-11-04) |
| `cargo test` (all, 2026-11-04) | **436 passed, 0 failed** |
| `cargo clippy --all-targets` | 0 warnings |
| TCP, 2 processes, Boxer.chk, 600/1200/2400 ticks | identical hash in both, exit 0 (2400 = 600 turns/checksums) |
| UDP, 2 processes, Boxer.chk, 600 ticks | identical hash `bdf9377e0487c36f` in both, exit 0 |
| `sc_view --shot-screen lan` | render 1280x960 OK |

## 6. Physical test on two machines — NOT EXECUTED

I cannot run it: this session runs on a single machine and I have no access
to a second one. Harness ready and syntactically verified:

```bash
# PC 1
scripts/lan_two_machines.sh host  192.168.1.20 terran  /path/map.chk /path/platform.vf4 tcp
# PC 2
scripts/lan_two_machines.sh join  192.168.1.20 protoss /path/map.chk /path/platform.vf4 tcp
# (add "udp" as the 6th argument to test UDP)
```

PASS criterion: both exit 0, no `DESYNC` line, and the two
`LAN_RESULT` lines with the **same** `final hash`.

## 7. Pending (honest)

- Chat in the UI (the `Chat` message exists and is tested in the codec).
- Host migration: if the host drops, the clients retry for ≈17 s and the
  game ends (out of scope).
- Commands a peer issued in the turn interrupted by its own drop are lost
  (it resends that turn after the resync with what it has queued since).
- Done 2026-11-04: lobby with up to 8 players in the UI, automatic
  reconnection with backoff (see §3, §4).

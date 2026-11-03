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
- **H: host** — opens the lobby on `:7777`, announces itself every second and accepts
  the first peer (non-blocking handshake: the menu keeps drawing).
- **ENTER: join** — handshake with the selected lobby; the host decides
  map and races, and both build the world with `build_from_save` to
  start in an identical state.
- In game, all orders go through `send_order`/`send_train`: if there is a
  LAN session they are queued for the next turn; otherwise they are applied locally.

## 4. Reconnection after a drop

Design: **no state is transferred, the log is transferred**.

1. The connection drops → `host_turn` returns an error → the client is left out.
2. The peer comes back and requests `Rejoin { peer, map_hash, seed }`.
3. The host replies `Resync { turn, log }` with the complete `Recording`.
4. The peer rebuilds the world with its copy of the map and does a deterministic
   fast-forward up to `turn`; then it continues normal lockstep.

Covered by `tests/lan.rs::peer_reconnects_and_resyncs_from_the_command_log`
(8 turns → drop → resync → 8 turns → final hash identical to the host's).

## 5. Tests executed (evidence)

| Test | Result |
|---|---|
| `cargo test --lib net::` | 7/7 (messages, merge, bidirectional UDP, duplicates, discovery) |
| `cargo test --test lan` | 3/3 (TCP 20 turns, different map rejected, reconnection+resync) |
| `cargo test` (all) | **256 passed, 0 failed** |
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

- Lobby with more than 2 players in the UI (the protocol supports up to 8;
  `sc_lan` TCP already accepts N-1 clients, the UI opens 1v1).
- Chat in the UI (the `Chat` message exists and is tested in the codec).
- Automatic reconnection (today it requires the peer to re-enter; there is no
  retry with backoff).
- Host migration: if the host drops, the game ends (out of scope).

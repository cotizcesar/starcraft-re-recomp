# Save/Load + Replays — design (P0, shares infra with LAN)

Decision: **command-log + fast-forward**, not a `World` snapshot.
Reason: `World` contains `Arc<GameData>`, caches (`index`, `obstacles`),
scratch and RNG; the sim is deterministic (LCG) and `World::issue` is the only
entry point for orders (player in `sc_view.rs`, AI in `ai.rs`).
Save = initial state + commands; load = rebuild and re-execute.

## `.gcsave` / `.gcrep` format (binary, magic + version)

- `GCS1` magic u32 + version u16.
- seed u64, map_hash (sha256 CHK) 32 B, setup (races, slots, base tick).
- cmds: `[(tick u64, unit u32, order Order)]`, `Order` with a u8 discriminant
  + manual fields (no serde: stable across versions).
- final checksum (corruption/desync detection).

## Restore

`World::from_scenario` + apply cmds per tick up to T. Measured:
5 000 ticks in ~12 s wall (release) → a 20 min game (~28 800 ticks)
loads in ~70 s. Acceptable with a notice; the full snapshot is discarded
as too invasive (caches/`Arc`/scratch).

## Reuse

- Replays: the same log (+ result header). Player = same
  fast-forward with pause/speed.
- LAN: only human commands per turn + periodic state checksum
  (`sc_state_hash` exists in `sc-c`; port the idea to Rust). See
  `research/docs/lan-protocol.md`.
- Hook: `World::issue` with an optional recorder (`Option<Recorder>`, zero
  cost if absent). The AI also goes through `issue`: its commands are
  recorded the same way (deterministic, harmless).

## P0 acceptance

Save mid-mission, quit, load, continue identically (state hash
equal to the original session's at the same tick); a replay that
reproduces the same result; profile/campaign persisted to disk.

## Player profiles

Registry IDs, settings, statistics and campaign progress are not part of a
save: they live in `profiles.bin` (`GCPF`), described with the retail
`.spc`/`.mpc` evidence in
[`research/docs/profile-format.md`](../../research/docs/profile-format.md).

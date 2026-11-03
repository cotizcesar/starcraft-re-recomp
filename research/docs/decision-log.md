# Decision log

- 2026-10-28: Hermes out of the chain. Reason: simplify to OpenCode+REA,
  verified E2E. Mentions in earlier docs remain as history.
- 2026-10-28: The master prompt's structure (`original-manifest/`,
  `analysis/discovery/`, …) is adopted additively; prior artifacts
  (`manifests/`, `analysis/file-inventory.csv`) are not deleted so as not to
  break references. (Later merged: `original-manifest/` → `manifests/`.)
- 2026-10-28: `GAME_PATH` = Battle.net install of StarCraft 1.23.10.13515
  (confirmed by the user). Read-only.
- 2026-10-28: Ghidra 12.1.4 provider (REA pin); 12.2 installed but not used.
- 2026-10-28: the Rust reconstruction (now `engine-rust/`) is reused
  as the compatible implementation; this directory keeps evidence and plans.
- 2026-10-28: LAN multiplayer IN scope (user decision).
  Battle.net/online remains out. Design: deterministic lockstep on the current
  sim (LCG; only per-turn commands + periodic checksum), LAN lobby,
  countdown, lag/drop handling, pre-game chat. Acceptance: 1v1 on 2 PCs
  20+ min without desync. Shares infrastructure with P0 replays (per-tick
  command log).
- 2026-10-28: LAN order: protocol design IN PARALLEL with P0
  (implementation after P0). Transport: BOTH (abstract layer; TCP first
  for correctness, UDP with retransmission later). User decision.

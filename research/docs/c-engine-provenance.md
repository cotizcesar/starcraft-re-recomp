# C engine: salvage assessment and provenance (removed 2026-10-28)

## Provenance summary

Provenance: C engine removed (2026-10-28, final structure).

A single engine remains ([`engine-rust/`](../../engine-rust/)). The `engine-c/` tree was removed
after migrating everything useful (save/load+replay+LAN design, determinism
harnesses). Verification of that migration: identical lists
(354/354, 113/113), identical aggregate checksums of `src/`, identical
`Cargo.lock`. Witnesses: [`re-c.bundle`](../re-c.bundle) in `research/` (git history) and
`rea-evidence-c.md` (local-only, gitignored; target identity + MPQ crypto + CHK).

### What is there

- [`engine-rust/`](../../engine-rust/): complete Rust reconstruction WITHOUT `target/` (regenerable
  with `cargo build`). Gems: `src/handlers.rs` (137
  routines + runtime addresses at base `0x79fa0000` used as a fingerprint),
  `src/orders.rs` (189 rows + VA), `src/retail.rs` (CASC/MPQ reader),
  `src/trigger.rs` + `src/map.rs` (CHK formats), `tools/` (oracle,
  `gen_handlers.py`, `verify_runtime_tables.py`, C++ extractors),
  `fixtures/` (test tables), `docs/` (`COVERAGE.md`, `EVIDENCE.md`,
  evidence), `features.md`, `Makefile`.
- C engine (removed): its useful content already lives ported in `engine-rust`
  (save/replay/LAN design and tests). Witnesses: [`re-c.bundle`](../re-c.bundle) in
  `research/` (git history) and `rea-evidence-c.md` (local-only).

### What was NOT migrated (and why)

- Rust `target/` (1.4 GB): build artifacts, regenerable.
- The C tree: removed after the migration; its history remains in
  [`re-c.bundle`](../re-c.bundle).

### Path note

`~/Documents/galaxycraft` → [`engine-rust/`](../../engine-rust/),
`~/ReverseEngineering` → `research/` (this workspace). The
`~/Documents/sc-cpp` link was removed along with the C engine. Old documents that
cite the original paths should be read with this mapping.

## Salvage assessment (closed 2026-10-28: `engine-c` removed)

> Outcome: everything useful was migrated; the C tree was deleted. Witnesses:
> [`research/re-c.bundle`](../re-c.bundle) (history) and `research/docs/rea-evidence-c.md` (local-only, gitignored).
> This report is kept as historical analysis.

Date: 2026-10-28. Question: is there anything salvageable in those folders for the
current goal (P0 + LAN + validation)?

### What each thing is

- `galaxycraft` (Rust, no git): **full-fidelity rules** reconstruction
  — 228 units, 130 weapons, 189 orders, 137 transcribed handlers,
  triggers 1–59, complete iscript/aiscript, 241 tests, 73/73 tables
  verified, 391 maps. Missing P0 (save/replays/profile) and networking.
- `sc-cpp` (C, git, 2 commits, 3 737 lines, 22 bins, 368+ tests):
  **simplified but complete-as-a-game** reconstruction — deterministic
  sim at 24 Hz/512 entities, 44-unit DB, CHK 68/68, MPQ
  crypto, fog/BFS, AI, 9 matchups, tournament, playable client, `save/load`,
  `replay` and `net`. `docs/REA-EVIDENCE.md` with target identity
  (SHAs match our install).

### Verdict: yes, salvageable — patterns and harnesses, NOT data tables

| sc-cpp piece | Actual state | Use for galaxycraft |
|---|---|---|
| `sc_save.c` (18 lines) | raw struct dump, no magic/version | pattern to port to Rust **with** magic+version (P0) |
| `sc_replay.c` (31 lines) | periodic state hashes + comparison | determinism verification harness and basis of the LAN anti-desync checksum |
| `sc_net.c` (13 lines) | `socketpair` + sending 1 int order | seed of the LAN design's TCP `NetTransport` |
| `sc_match.c`, `play.c` | game loop, tournament, playable client | fixed-tick discipline + map sweep (align with `sc_campaign`) |
| 44-unit DB | simplified | **DO NOT port**: regression versus galaxycraft's 228 |
| `docs/REA-EVIDENCE.md` | imports = 2 DLLs (`MessageBoxW`), entry `0x1400b0af0`, `.text 00aabfe4` | corroborates engine/protector (minimal packed stub) |

### Cross-validation opportunity (recommended)

Two independent reconstructions (full-fidelity Rust + simplified C)
that sweep the same 68 maps: a differential "same map, compare
outcomes" test as cheap equivalence validation (Phase 12). Record as a
pending task.

### Honest limits

- `sc-cpp` does not replace binary analysis: its DB is approximate and its
  replay is hash-based, not command-based (not usable as a playable `.gcrep`).
- The current `net` is socket plumbing, not a protocol: the
  `docs/lan-protocol.md` design is still needed.

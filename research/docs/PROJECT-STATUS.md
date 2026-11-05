# Project status — reconstruction of StarCraft 1.23.10.13515

Internal state in one page.  Feature status and percentages:
[`PROGRESS.md`](../../PROGRESS.md).  How we got here, entry by entry:
[`session-log.md`](session-log.md).

Last update: 2026-11-05.

## Target

- **Game:** StarCraft: Remastered 1.23.10.13515 (exact, from `.build.info`),
  read from `$SC_INSTALL` (~5.5 GB, read-only, never modified).
- **Engine:** proprietary Blizzard C/C++, Win32 x86 and x86-64, data in
  CASC plus one MPQ per map; CEF only for the front end
  ([`engine-identification.md`](engine-identification.md)).
- **Goal:** a verifiable reconstruction of the behaviour (rules, data,
  presentation) in Rust, driven by evidence; no retail bytes in the
  repository.

## Where things stand

- **Engine (`engine-rust`):** rules, data formats, front end, campaign
  flow, persistence and LAN lockstep are implemented and tested (486 tests,
  0 failed with `SC_INSTALL`; visual gate 51/51; walkthrough, ultrawide,
  timing, hover and swish gates pass).  Rust 1.99.1, edition 2024.
- **Campaign 1:** Episode I is fitted to retail recordings of Boot Camp,
  Wasteland and Backwater Station (session log 87–95).
- **Reverse engineering:** Phase 5 closed: architecture map (gate
  `0x70e810`, WndProc `0x945050`, message pump `0x946500`, simulation step
  `0x969f0d`), both memory dumps analysed, in-game heap captured,
  `0x8aee30` explained, every extracted file traced to CASC/MPQ
  ([`architecture.md`](architecture.md)).

## Open (details and evidence in `PROGRESS.md`)

- Gameplay gaps: single-player cheat codes, the special rules of Greed /
  Slaughter / Capture the Flag / Sudden Death, in-game chat, observer mode
  after defeat, Save Replay on the campaign score screen.  (Alliances, the
  multiplayer front end, lobby chat and OpenBW mining came with the LAN
  flow branch, session log 2026-11-04 (86–87).)
- Campaign: missions 4–11 and Episodes II–VI against retail recordings,
  audio timing (no recording so far has sound), Zerg/Protoss ready rooms.
- Physical two-machine LAN test; locked-campaign prompt; localisation.
- Planned: controller support
  ([`controller-support.md`](../../engine-rust/docs/controller-support.md)).
- Inferences that need the retail client: [`unresolved-questions.md`](unresolved-questions.md).

## Tooling

REA (`rea-agents@6.8.0`) over MCP in OpenCode, Ghidra 12.1.4 with JDK 21,
Python 3.14, Rust 1.99.1 ([`toolchain.md`](toolchain.md),
[`rea-opencode-integration.md`](rea-opencode-integration.md)).  Known tool
limits: [`blockers.md`](blockers.md), [`tool-errors.md`](tool-errors.md).

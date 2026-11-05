---
title: "Toolchain 1.99.1, edition 2024, latest deps, docs validated"
date: 2026-11-05
proyecto: starcraft-re-recomp
tags: [session, toolchain, dependencies, documentation]
tipo: session
origen: opencode
---

## Goal

Latest libraries (Rust 1.99.1), simpler validated docs without local
paths, complete done/missing/planned feature list. Record:
`research/docs/session-log.md` 2026-11-05 (98).

## Progress

- [x] Rust 1.99.1 pinned, edition 2024, latest crates/actions/ffmpeg.
- [x] Docs checked against code; features.md and COVERAGE.md merged into PROGRESS.md.
- [x] Local paths removed from tracked files.

## Next steps (from PROGRESS.md)

- [ ] Alliances in `World::hostile`.
- [ ] Missions 4-11 and audio against retail.
- [ ] Controller support (planned).

## FACT vs INFERENCE

- FACT: allied players fight (`hostile` ignores `team`); no cheat codes; no chat UI.
- INFERENCE: drop-order changes of edition 2024 are harmless (tests pass).

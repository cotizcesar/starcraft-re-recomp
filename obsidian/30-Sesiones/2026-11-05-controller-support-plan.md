---
title: Controller (gamepad) support plan
date: 2026-11-05
proyecto: starcraft-re-recomp
tags: [sesion]
tipo: sesion
origen: opencode
---

## Goal
Plan controller (gamepad) support for `sc_view` and write the plan as a readable `.md` document.

## Progress
- Read how input works today in `engine-rust` (FACT):
  - `minifb` 0.29 has no gamepad API, and no gamepad crate is a dependency.
  - `sc_view.rs` reads `&Window` directly in 7 handlers (intro, menu, briefing, replay, pause, result, in-game), with 29 direct input call sites.
  - `MenuInput` filled from `--menu-script` is the existing pattern for capturing one tick's input.
- Wrote `engine-rust/docs/controller-support.md` (248 lines). It covers:
  - goals and non-goals
  - the current facts, with file and line references
  - a comparison of input libraries
  - an `InputFrame` design with a virtual cursor
  - default button mappings for menus, in-game play and replays
  - a radial command card
  - hot-plug handling, configuration settings, 7 headless tests, 4 phases and risks
- Added entry 88 to `research/docs/session-log.md`.
- `python3 scripts/progress.py --check` → `OK (96 % overall, 6 areas)`.
- No code changed, so `cargo test` and clippy were not run.

## Decisions
- Proposed, pending user approval: use `gilrs` behind an optional `gamepad` cargo feature. Alternatives were SDL2 or one raw back-end per OS. See [[controller-support]].
- Keyboard, mouse, script and gamepad all fill one `InputFrame`. The sim and `src/net.rs` stay untouched because commands still enter through `World::issue`, which keeps replays, saves and LAN deterministic.
- `PROGRESS.md` items are not added until the plan is approved. They are drafted in §9 of the doc.
- INFERENCE: retail 1.23.10.13515 has no gamepad support. The evidence is weak (no `xinput`/`dinput` files in the install inventory). Falsifier: check the retail exe's import table with REA.

## Pending
- [ ] User confirms `gilrs` as the library.
- [ ] Record the decision in `research/docs/decision-log.md` and [[10-Decisiones]].
- [ ] Add the 4 unchecked `PROGRESS.md` items and run `progress.py --write`.
- [ ] Run the REA import check on the retail executable to confirm or refute the INFERENCE.
- [ ] Phase 1: `InputFrame` refactor of the 7 handlers with no behaviour change; existing gates must stay byte-identical.
- [ ] Find out who changed `engine-rust/docs/EVIDENCE.md`. It was modified outside this session and left untouched.

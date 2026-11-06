---
title: "Episode I won through input; movement, depletion and campaign AI fixes"
date: 2026-11-06
tags: [session, in-game, campaign, simulation]
type: session
source: opencode
area: campaign
log: ["2026-11-06 (107)", "2026-11-06 (108)", "2026-11-06 (109)", "2026-11-06 (110)", "2026-11-06 (111)", "2026-11-06 (112)"]
open: 1
---

## Goal

Play the Episode I missions through input only, as on the recordings, and fix what breaks.

## Outcome
- Wasteland: victory through `--game-script` input (`scripts/mission_gate.py`, 0 failures at 720p / 1080p / 3440x1440).
- Simulation fixes: chase in pixels, retaliation, waypoint reach / stall / crowd arrival, walk animation from any resting state, the build walk every tick. Each has a test in `tests/reconstruction.rs`.
- Minimap right click / target clicks, Ctrl+click type selection.
- (112) Trigger timelines within 0.4 s of the recordings; Bunker cargo slots and right-click loading.
- (111) Cancel construction, targeted spells (Spider Mines), placement where clicked with the retail preview, box hit test for clicks.
- (109) Desperate Alliance waves checked against the recording minimap; Terran scaffold / AlmostBuilt stages and SCV welding as on Boot Camp.
- (108) All four missions won through input; mined-out fields; campaign AI waves as scripted.

## Pending
- [x] Compare each mission's opening timeline with the recordings. Closed 2026-11-06: `timeline_gate.py` 10/10 (session log 112).
- [ ] Player-driven event times (replay each recording's own build order).
- [x] Boot Camp playthrough in `mission_gate.py`. Closed 2026-11-06: `mission-gate.txt` (tick 6,542).
- [x] Backwater Station playthrough. Closed 2026-11-06: `mission-gate.txt` (tick 9,728), depletion fix.
- [x] Desperate Alliance playthrough. Closed 2026-11-06: `mission-gate.txt` (tick 29,025), AI wave fix.
- [x] Multi-selection panel (see [[2026-11-05-ingame-input-card-buildtimes-perf]]). Closed 2026-11-06: session log (110), `game_input_gate.py`.

## FACT vs INFERENCE
- FACT: each bug traced with `dump-units` (positions, orders, anim state).
- INFERENCE: retaliation help radius 8 tiles and computer-only; crowd arrival 96 px. Falsifier: retail recordings of those cases.

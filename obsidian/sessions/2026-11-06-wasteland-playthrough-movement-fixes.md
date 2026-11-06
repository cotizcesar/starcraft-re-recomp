---
title: "Episode I won through input; movement, depletion and campaign AI fixes"
date: 2026-11-06
tags: [session, in-game, campaign, simulation]
type: session
source: opencode
area: campaign
log: ["2026-11-06 (107)", "2026-11-06 (108)"]
open: 2
---

## Goal

Play the Episode I missions through input only, as on the recordings, and fix what breaks.

## Outcome
- Wasteland: victory through `--game-script` input (`scripts/mission_gate.py`, 0 failures at 720p / 1080p / 3440x1440).
- Simulation fixes: chase in pixels, retaliation, waypoint reach / stall / crowd arrival, walk animation from any resting state, the build walk every tick. Each has a test in `tests/reconstruction.rs`.
- Minimap right click / target clicks, Ctrl+click type selection.
- (108) All four missions won through input; mined-out fields; campaign AI waves as scripted.

## Pending
- [ ] Compare each mission's timeline and the AI waves' strength with the recordings.
- [x] Boot Camp playthrough in `mission_gate.py`. Closed 2026-11-06: `mission-gate.txt` (tick 6,542).
- [x] Backwater Station playthrough. Closed 2026-11-06: `mission-gate.txt` (tick 9,728), depletion fix.
- [x] Desperate Alliance playthrough. Closed 2026-11-06: `mission-gate.txt` (tick 29,025), AI wave fix.
- [ ] Terran construction graphics and the multi-selection panel (see [[2026-11-05-ingame-input-card-buildtimes-perf]]).

## FACT vs INFERENCE
- FACT: each bug traced with `dump-units` (positions, orders, anim state).
- INFERENCE: retaliation help radius 8 tiles and computer-only; crowd arrival 96 px. Falsifier: retail recordings of those cases.

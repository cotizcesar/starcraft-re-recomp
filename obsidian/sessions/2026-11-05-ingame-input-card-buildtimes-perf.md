---
title: "In game: input at every scale, retail command card, retail build times, 20x faster"
date: 2026-11-05
tags: [session, in-game, input, performance]
type: session
source: opencode
area: in-game
log: ["2026-11-05 (105)"]
open: 4
---

## Goal

Make Boot Camp playable: frame rate, selection, orders, building (user report). Mission parity with the four recordings is the next step.

## Outcome
- Input in logical coordinates. Retail card, hotkeys and right click. Test: `scripts/game_input_gate.py`, 0 failures at 720p / 1080p / 3440x1440.
- Build times are frames ([[build-times-are-game-frames]]).
- Command log v3 records the card mutators.
- Frame time 181 -> 7.7 ms at 3440x1440 (`ingame-draw-time.txt`); visual gate unchanged.

## Decisions
- [[build-times-are-game-frames]]

## Pending
- [ ] SCV stays on its Terran building until it completes.
- [ ] Mineral delivery distance (~320 px from the CC on Boot Camp).
- [ ] Cancel construction, targeted spells, Land preview on the card.
- [ ] Boot Camp, Wasteland, Backwater Station, Desperate Alliance step by step against the recordings.

## FACT vs INFERENCE
- FACT: coordinate bugs (code), action classes (button table + tooltips), frame-based build times (progress bars, 42 ms ticks).
- INFERENCE: card actions not seen in the recordings (Rally, Lift) follow their class. Falsifier: a recording where they act otherwise.

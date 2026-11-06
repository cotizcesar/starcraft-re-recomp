---
title: "units.dat / techdata / upgrades times are game frames"
date: 2026-11-05
status: accepted
type: decision
tags: [decision, simulation, timing]
source: opencode
---

## Context

`build_ticks` converted `units.dat` build times from an assumed 1/15 s unit
(SCV 300 -> 480 ticks); upgrades from 1/60 s.

## Alternatives

1. Keep the assumed scalings.
2. Frames, i.e. ticks, as OpenBW counts them.

## Decision

Option 2.

## Rationale

FACT: Backwater Station recording, selection progress bars: an SCV takes
12.6 s = 300 x 42 ms, a Marine ~14.6 s ~ 360 x 42 ms.

## Evidence
- Session log 2026-11-05 (105); `tests/reconstruction.rs::time_units_are_game_frames`.

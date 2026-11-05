---
title: "Ultrawide: every screen is the 16:9 picture between black bars"
date: 2026-11-04
status: accepted
type: decision
tags: [decision, ultrawide, rendering]
source: opencode
---

## Context

At 3440x1440 the client let menu arms, the in-game console, briefings and
the score screen leave the 16:9 box, and misplaced the Episode console
plates. The older doc claimed "ultrawide expands (no bars)".

## Alternatives

1. Expand every screen to the window width (old claim).
2. Pillarbox every screen: black bars outside the centred 16:9 box.

## Decision

Option 2, enforced once in `App::draw` (`mask_pillarbox`) and for
full-frame screens by drawing into a box-sized framebuffer
(`draw_in_box`).

## Rationale

FACT: all 56 retail captures at 3440x1440 are black outside the box (max
channel <= 8), menus and game alike. Gate `scripts/ultrawide_diff.py`
checks that 3440x1440 = 2560x1440 between black bars (31/31).

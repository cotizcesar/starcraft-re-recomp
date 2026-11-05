---
title: "SingleOn hover clip restarts at the small grid"
date: 2026-11-05
status: accepted
type: decision
tags: [decision, main-menu, clips, inference]
source: opencode
---

## Context

The hover clip ran on a continuous clock (2026-11-04 reading of the retail
recording), so a hover started wherever the loop was, usually with the
grid full size. The user reports the retail grid always starts small and
the animation resets when the hover ends.

## Alternatives

1. Keep one continuous clock.
2. Restart at file frame 0 (the grid is already full size there).
3. Restart at the grow-in frame 31 and loop the 76-frame cycle (31..106).

## Decision

Option 3 (`hover_clip_window`, `MenuClip::rewind`). Hidden mouse-over clips
are not pulled. Leaving the hover rewinds the clip, and the worker
pre-decodes its first chunk for the next hover.

## Rationale

FACT: alpha coverage of `singleon.webm` shows the grid absent at 18..30
and growing at 31..36, period 76. INFERENCE: retail restarts mouse-over clips
per hover. Falsifier: a retail recording where a second hover opens on the
full grid.

## Evidence
- `main_menu_hover_tests` in `src/bin/sc_view.rs`; session log 2026-11-05 (102).

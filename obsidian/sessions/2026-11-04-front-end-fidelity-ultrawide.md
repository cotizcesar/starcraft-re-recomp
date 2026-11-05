---
title: "Front-end fidelity pass and ultrawide"
date: 2026-11-04
tags: [session, front-end, ultrawide, main-menu]
type: session
source: opencode
area: front-end
log: ["2026-11-04 (82)"]
open: 0
---

## Goal

Fix the front-end defects the user listed at 3440x1440 and make the menus
1:1 with retail, in ultrawide as in 16:9. Engineering record:
`research/docs/session-log.md` 2026-11-04 (82).

## Progress

- [x] Ultrawide: every frame masked to the 16:9 box; Episode plates use the
  box origin; briefing/score drawn in a box-sized framebuffer. Gate
  `scripts/ultrawide_diff.py` 31/31.
- [x] Main menu: hover clips start on hover (142 → 0 skipped frames),
  per-button clip order, footer labels recolour only, build footer in
  `blizzard-regular`, Battle.net tag removed, animated `loadingcircle.gif`
  spinner (new `src/gif.rs`).
- [x] Registry / Load Saved Game from `glulogin` / `gluload` and their
  `pList` plates; `FLCBTN` labels with hotkey colour.
- [x] Load Replay: information panel with natural spacing and real
  metadata; stray yellow scrollbar removed.
- [x] Collections: four debug rectangles removed.
- [x] Glue transitions (Registry, Saved Games, Load Replay): measured on the
  user's 60 fps retail recording and implemented; gate
  `scripts/glue_swish_check.py` (session log 84).

## Decisions (detail in `decisions/` and link)

- [[ultrawide-pillarbox-every-screen]]
- [[battlenet-tag-hidden-offline]]
- [[footer-label-hover-recolour-only]]

## Pending
- [x] Commit/PR of this pass after the user reviews it. Closed 2026-11-05: `f28fb59`.

## FACT vs INFERENCE

- FACT: 56/56 retail captures black outside the 16:9 box.
- FACT: `glulogin` uses `glue\login\pList`; text faces from
  `RC_BIGGERTEXT` / `RC_BIGTEXT`, matched glyph by glyph.
- FACT: spinner = `loadingcircle.gif`, 30 × 30 ms; decoder equals PIL.
- FACT (measured): hover clip starvation 142 → 0.
- INFERENCE: hover On clips restart at frame 0 on hover (phases differ from
  the idle clips on every reference; falsifier: a retail video).
- FACT: swish in = ×0.75 per 1/30 s step from the box edge; out = 2^n − 1
  UI px; text only when settled (retail recording).
- INFERENCE: footer-label hover only recolours (user observation, no
  retail capture).

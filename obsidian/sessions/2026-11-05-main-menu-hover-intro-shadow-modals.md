---
title: "Main menu: SingleOn grid, label hover, View Intro, no whoosh, shadows, modal panel"
date: 2026-11-05
tags: [session, main-menu, front-end]
type: session
source: opencode
area: front-end
log: ["2026-11-05 (102)"]
open: 1
---

## Goal

Fix six main-menu issues the user reported against the retail client.

## Outcome
- SingleOn grid starts small and restarts per hover. Tests: `main_menu_hover_tests`.
- Single Player / Multiplayer keep their layout on hover. Gate: `menu-hover-text.txt` 14/14.
- View Intro → Select Game Type → that game's movie. Check: `view_intro_check` in `scripts/menu_walkthrough.py`.
- No whoosh when leaving Main through an entry. Code and test: `menu::screen_change_swish`; gate: `menu-walkthrough.txt`.
- Labels use the footer drop shadow (measured (3,3) px at 1440p on 130637).
- Empty panel behind modals; Collections closer to 131005 (4.38 → 4.25). Gate: `ui-visual-diff.txt` 51/51.

## Decisions
- [[main-menu-labels-recolour-only]] (supersedes [[footer-label-hover-recolour-only]])
- [[singleon-hover-restarts-at-grow-in]]

## Pending
- [ ] Retail recording of two consecutive Single Player hovers to confirm the clip restarts (falsifier of [[singleon-hover-restarts-at-grow-in]]).

## FACT vs INFERENCE
- FACT: `singleon.webm` 76-frame cycle, grow-in at frame 31; label shadow offset (3,3) px on 130637.
- INFERENCE: retail restarts mouse-over clips and keeps label faces on hover (user reports); falsifiers in the decision notes.

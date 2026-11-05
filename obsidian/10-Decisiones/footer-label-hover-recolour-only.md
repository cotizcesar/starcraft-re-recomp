---
title: "Main-menu footer labels only recolour on hover"
date: 2026-11-04
estado: aceptada
tags: [decision, main-menu, inference]
origen: opencode
---

## Context

Hovering Options, Collections, Hotkeys, Show Credits or View Intro switched
the whole word to the extended face. At fractional UI scales (maximised
window, e.g. 3440x1387) that re-spaced the glyphs, which the user saw as the
letters sliding left; the user reports the retail labels do not move.

## Alternatives

1. Keep the face switch (as measured for Single Player / Multiplayer on
   130704 / 130712) and fix the rounding.
2. Keep the idle glyph layout and change only the colour.

## Decision

Option 2 for the centred footer entries; Single Player / Multiplayer keep
the measured face switch.

## Rationale

INFERENCE (no retail hover capture of these labels): the user's observation
of the retail client. Falsifier: a retail capture with a footer label
hovered. Gate `scripts/menu_hover_text.py` (10/10; 9/10 with the old code).

---
title: "Every main-menu label only recolours on hover"
date: 2026-11-05
status: accepted
type: decision
tags: [decision, main-menu, inference]
source: opencode
---

## Context

[[footer-label-hover-recolour-only]] kept the face switch for Single Player /
Multiplayer (measured on 130704 / 130712). The user reports those two also
move on the retail client.

## Alternatives

1. Keep the measured face switch for the two big entries.
2. One rule for all seven entries: idle glyph layout, colour change only.

## Decision

Option 2 (`main_label_hover_colors` in `src/bin/sc_view.rs`). Supersedes
[[footer-label-hover-recolour-only]].

## Rationale

One behaviour, no per-entry exception. INFERENCE: the user's view of the
retail client outweighs our reading of two stills. Falsifier: a retail
recording where the Single Player glyphs change face on hover.

## Evidence
- `scripts/menu_hover_text.py` 14/14, ink IoU >= 0.94; previous build fails
  Single Player (0.927) and Multiplayer (0.844-0.894). Session log 2026-11-05 (102).

---
title: "Credits: only Esc and Enter leave the screen"
date: 2026-11-04
proyecto: starcraft-re-recomp
tags: [decision, credits, input]
tipo: decision
estado: accepted
origen: opencode
---

## Decision

On the Credits screen, only Esc and Enter return to the main menu. Mouse
clicks and Space no longer leave it.

## Why

FACT: the shipped `webui/dist/creditspanel/creditspanel.js` sends `Exit`
only for keyCode 27 (Esc) and 13 (Enter) and registers no click handler.
The page covers the whole screen, so clicks reach it rather than the
native menu. INFERENCE: the native layer gets no click either. Falsifier:
a retail capture where a click leaves Credits.

## Alternatives

- Keep click-to-skip (the old behaviour): no evidence for it in retail.
- Keep Space: not in the script's key list.

## Consequences

`scripts/menu_walkthrough.py` now leaves Credits with `key:Escape`; its
step log shows a click staying on Credits.

---
title: "16:9 from 720p up, ultrawide pillarboxed, narrower windows letterboxed"
date: 2026-11-05
status: accepted
type: decision
tags: [decision, front-end, resolution]
source: opencode
---

## Context

The layout lives in a 16:9 box sized by the window height. A 1277x954
window cut off its right side (user report). The user asked for 16:9 from
720p upwards plus ultrawide.

## Alternatives

1. Re-lay out every screen for 4:3.
2. Letterbox: draw a 16:9 surface that fits the width, between black bars.

## Decision

Option 2 (`letterbox`, `present_into`, `surface_point`). Default window
1280x720, which is also the smallest supported size.

## Rationale

The retail layouts are 16:9. One surface keeps a single layout and a single
hit-test geometry for every window.

## Evidence
- `scripts/resolution_diff.py` 245/245 (`engine-rust/docs/gates/resolution-diff.txt`); session log 2026-11-05 (104).

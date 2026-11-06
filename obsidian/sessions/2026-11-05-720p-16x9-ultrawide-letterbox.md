---
title: "720p and up, 16:9 and ultrawide: letterbox, HD world below 1080p"
date: 2026-11-05
tags: [session, front-end, resolution]
type: session
source: opencode
area: front-end
log: ["2026-11-05 (104)"]
open: 0
---

## Goal

Every screen right at 16:9 from 720p up and at ultrawide (user report: Select Connection cut off at 1277x954).

## Outcome
- Narrow windows letterbox; default window 1280x720. Tests: `letterbox_tests`.
- New gate `scripts/resolution_diff.py`: 245/245. The previous build fails 1277x954.
- Fixed below 1080p: HD world, stale shot camera, pause / LAN / briefing text, lobby arrows.
- Ultrawide, visual and hover-text gates byte-identical (1440p unchanged).

## Decisions
- [[letterbox-narrow-windows-720p-minimum]]

## Pending

## FACT vs INFERENCE
- FACT: gate numbers in `resolution-diff.txt`; the previous build's 1277x954 means of 13.9-32.3.
- INFERENCE: retail also uses HD art at 720p. Falsifier: a retail 720p capture showing SD sprites.

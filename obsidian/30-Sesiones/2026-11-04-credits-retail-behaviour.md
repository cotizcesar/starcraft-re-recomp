---
title: "Credits screen behaves like retail"
date: 2026-11-04
proyecto: starcraft-re-recomp
tags: [session, front-end, credits, cef]
tipo: session
origen: opencode
---

## Goal

Make the Credits section behave exactly like the user's retail recording
`Screencast_20261104_210917.webm` (user recording). Engineering record:
`research/docs/session-log.md` 2026-11-04 (85).

## Progress

- [x] Timing measured on the recording: hard cut in, 2.085 s hold, then
  89.2–89.4 px/s scroll.
- [x] Scroll logic ported from the shipped `creditspanel.js`
  (`src/credits.rs` `CreditsScroll`): auto-scroll delay, key acceleration,
  wheel pause, Esc/Enter exit.
- [x] CSS layout ported (`CreditsLayout`): 4:3 CEF view at DSF 1.5,
  641–1280 px media block, DirectWrite metrics plus Blink rounding.
  Every heading lands within ±0.75 px of the recording.
- [x] `sc_view` draws with the CEF rasteriser, synthetic bold, text-shadow
  halo and clipping. Backdrop cached: 35 → 2.2 ms per frame.
- [x] Gates: credits 2.84 → 1.10, 51/51; walkthrough PASS; tests 456/0;
  clippy 0.

## Decisions (detail in `10-Decisiones/` and link)

- [[credits-only-esc-enter-leave]]

## Pending
- [ ] Commit: the working tree also holds another session's uncommitted
  front-end work (`sc_view.rs`, `gif.rs` removal), so nothing was committed.
- [ ] `ui_diff.py` Registry/Saved failures exist on committed HEAD too
  (not from this change).

## FACT vs INFERENCE

- FACT: hard cut, 2.085 s hold, 89.2–89.4 px/s (recording, timestamp seeks).
- FACT: scroll constants, exit keys, no click handler, CSS
  (`creditspanel.js`).
- FACT (measured): CEF view = 1280×960 CSS at DSF 1.5; DirectWrite
  metrics; Blink descent borrow + floored half leading.
- INFERENCE: the list sits 1 px left of the CSS box (residual
  1.0 ± 0.4 px, cause unknown); 4-frame view-to-page latency and a 59.5 Hz
  effective frame rate are measured fits, not derived.
- INFERENCE: 100 CSS px per wheel notch (Chromium default; not in the
  recording).
- Unknown: sounds on entering/leaving Credits (the recording has no audio).

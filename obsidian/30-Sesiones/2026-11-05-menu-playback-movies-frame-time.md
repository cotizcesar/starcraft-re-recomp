---
title: "Menu playback, movies, swish sounds and frame time"
date: 2026-11-05
proyecto: starcraft-re-recomp
tags: [session, main-menu, performance, audio]
tipo: session
origen: opencode
---

## Goal

User list: accelerated menu animations after visiting another screen,
Blizzard logo aspect, cartoon-speed movie audio, Single Player grid loop,
Collections look, game introduction in the panel, transition whooshes,
menu frame rate. Record: `research/docs/session-log.md` 2026-11-05 (86).

## Progress

- [x] Clip clocks pause off screen, never fast-forward (98 → 0).
- [x] Mouse-over loops on one clock (falsifies the hover restart of 2026-11-04).
- [x] Movie audio at the mixer rate; intros at 15 fps; films fill the 16:9 box.
- [x] Collections translucent over the live menu (6.39 → 4.38).
- [x] Panel introduction instead of the spinner; `gif.rs` removed.
- [x] swishin / swishout on screen changes.
- [x] Frame time: Options 101 → 15 ms, Replays 45 → 18, Episode 40 → 8.

## Decisions (detail in `10-Decisiones/` and link)

- [[battlenet-tag-hidden-offline]] extends to the panel: no Battle.net
  content, an introduction instead.

## Pending
- [ ] Retail capture of the Blizzard logo to confirm its aspect.

## FACT vs INFERENCE

- FACT: 22 050 Hz decode played at 44 100 Hz; SingleOn frame-40 marker
  0.12 s after hover on the retail recording.
- INFERENCE: logo stretched to 16:9; swish sounds on every change (user
  report, the recording has no audio).

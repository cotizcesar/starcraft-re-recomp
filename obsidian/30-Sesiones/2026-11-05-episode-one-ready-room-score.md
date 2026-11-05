---
title: "Episode I: forces, ready room, end of mission (retail recordings)"
date: 2026-11-05
proyecto: starcraft-re-recomp
tags: [session, campaign, briefing, score-screen]
tipo: session
origen: opencode
---

## Goal

Campaign 1 exactly like retail, checked against the user's three retail
recordings (video only). Record: `research/docs/session-log.md`
2026-11-05 (89).

## Progress

- [x] Client harness drives the result screen; full loop verified in the client.
- [x] Force (`FORC`) triggers and start-of-game evaluation: objectives in all 11 missions.
- [x] Scripted-only mission AI; rescue range 2 tiles (measured).
- [x] Terran ready room from `MBRF` + `glurdyt` (timeline matches the recording).
- [x] `wmission` dialog and `gluscore` score screen with real score accounting.

## Pending

- [ ] Audio timing against a recording with sound.
- [ ] Zerg/Protoss ready rooms; score count-up animation.

## FACT vs INFERENCE

- FACT: MBRF action layout; room timeline (9.10 s, 36.70 s, 64.90 s);
  `iscore` frame layout; `strNATION_<RACE>_<slot>` names; Force 1 triggers.
- INFERENCE: scripted-only mission AI; rescue rule; score formulas
  (Overview = Units + Structures + Resources holds on the recording).

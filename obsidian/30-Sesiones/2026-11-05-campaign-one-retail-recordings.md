---
title: "Campaign 1 against retail recordings: Boot Camp, Backwater, docs"
date: 2026-11-05
proyecto: starcraft-re-recomp
tags: [session, campaign, triggers, documentation]
tipo: session
origen: opencode
---

## Goal

Campaign 1 exactly like retail. Record: `research/docs/session-log.md`
2026-11-05 (93-96).

## Progress

- [x] Boot Camp and Backwater Station compared with full retail recordings.
- [x] Engine bugs fixed: MRGN x32, same-cycle wait-blocking, rescue poll on tick 1.
- [x] Trigger text lifetime by characters (9 samples); ready-room scroll; DISPLAYTIME per page.
- [x] All docs updated (README, features, COVERAGE, EVIDENCE, PROJECT-STATUS, architecture, unresolved).

## Pending

- [ ] Audio timing (recordings had no sound).
- [ ] Missions 4-11 against retail recordings.
- [ ] Zerg/Protoss ready rooms.

## FACT vs INFERENCE

- FACT: MRGN in pixels; wait-blocking; text lifetimes; scroll hold 4 s; video pages wait for a click (user).
- INFERENCE: 11-line limit; rescue timer starts at 0; Skip Tutorial progress-only.

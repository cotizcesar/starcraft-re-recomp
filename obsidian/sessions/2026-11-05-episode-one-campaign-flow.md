---
title: "Episode I campaign flow against the retail tables"
date: 2026-11-05
tags: [session, campaign, triggers, rescue]
type: session
source: opencode
area: campaign
log: ["2026-11-05 (87)"]
open: 0
---

## Goal

Campaign 1 (Terran, Rebel Yell) exactly like retail. Record:
`research/docs/session-log.md` 2026-11-05 (87).

## Progress

- [x] Mission order from `arr/mapdata.tbl` (no unused terran07/terran10).
- [x] `gluhist` mission titles (New Gettysburg, Boot Camp).
- [x] Card + film between missions from the `MenuEntry` table.
- [x] Rescue Passive players: Raynor rescued, not shot (Jacobs defeat fixed).
- [x] Trigger-only victory in missions; Transmissions hold their trigger.

## Pending

- [x] Retail recording of campaign 1 (briefing, opening minutes, Wasteland
  rescue, victory → score → next briefing) to compare 1:1. Closed 2026-11-05:
  recordings used in session log 2026-11-05 (89)-(95).

## FACT vs INFERENCE

- FACT: mapdata order; terran11 STR 1 is a trigger text; Pre* cards name
  their films; own Marines killed Raynor (trace); order 160 polls every 15.
- INFERENCE: rescue range 96 px and human-only rescuers; Transmission
  blocking; trigger-only victory (UMS semantics).

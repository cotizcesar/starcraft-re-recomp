---
title: "Wasteland against the recordings: score count-up, trigger text, victory, HUD"
date: 2026-11-05
tags: [session, campaign, score-screen, triggers, hud]
type: session
source: opencode
area: campaign
log: ["2026-11-05 (90)", "2026-11-05 (91)", "2026-11-05 (92)"]
open: 2
---

Backfilled on 2026-11-05 from the session log; the evidence is there.

## Goal

Make Wasteland (Episode I, mission 1) match the retail recordings
`Screencast_20261105_074011`, `_073713` and `_074531`.

## Outcome
- Score panel slide and count-up, retail trigger text, Waits resume on their tick — `ba65fac`.
- Victory/Defeat apply to the trigger's player (Wasteland could never be won); paused transmission text — `b624bf0`, `wasteland_victory_fires_within_one_trigger_cycle`.
- `stat_txt` error lines, Terran supply rows, trigger leaderboard — `6cfc0ff`.

## Pending
- [ ] Explain why Raynor's 3-line transmission text left after 4.0 s (the paused-transmission rule rests on one sample).
- [ ] Measure the trigger leaderboard row pitch with several players (36 px is INFERENCE).

## FACT vs INFERENCE
- FACT: count-up fractions, trigger text font/pitch/lifetimes, victory within one 30-tick cycle, error line and leaderboard ink boxes (log 90-92).
- INFERENCE: paused transmission text lives exactly its length (falsifier: a paused transmission whose text outlives it); error line lasts 4 s.

---
title: "Desperate Alliance against its retail recording"
date: 2026-11-05
tags: [session, campaign, triggers, timing]
type: session
source: opencode
area: campaign
log: ["2026-11-05 (99)"]
open: 0
---

Backfilled on 2026-11-05 from the session log; the evidence is there.

## Goal

Match `Desperate Alliance.webm` (1362 s, video only): ready room, 30-minute
countdown, ending.

## Outcome
- Game seconds are 16 ticks (`trigger::GAME_SECOND_TICKS`); the countdown was 30 and Elapsed Time 24 — `80a212a`.
- 42 ms per tick (`sim::TICK_MS`); score Elapsed Time 20:20 as retail.
- Countdown drawn at the top (`--shot-screen countdown`); flow pinned by `desperate_alliance_follows_the_retail_recording`.

## Pending
- Audio timing (the recording had no sound): already tracked in [[2026-11-05-campaign-one-retail-recordings]].

## FACT vs INFERENCE
- FACT: 446-447 timer seconds per 300 real seconds; 1209.75 s for 28,800 ticks; countdown ink box; transmissions within 0.45 s.
- Environment: the `materialised_assets…` failure was the full per-user `/tmp` quota, not a flaky test.

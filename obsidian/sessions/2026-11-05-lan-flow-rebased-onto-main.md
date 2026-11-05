---
title: "LAN flow branch rebased onto main"
date: 2026-11-05
tags: [session, multiplayer, lan, merge]
type: session
source: opencode
area: multiplayer
log: ["2026-11-05 (100)"]
open: 2
---

Backfilled on 2026-11-05 from the session log; the evidence is there.

## Goal

Merge `multiplayer-lan-flow` ([[2026-11-04-multiplayer-lan-flow]],
[[2026-11-04-multiplayer-lan-flow-countdown-colours-mining]]), written on a
base 40 commits old.

## Outcome
- Rebased as `mp-lan-flow-rebased`, merged as `c688d76`.
- One implementation per behaviour: `credit_scores` (OpenBW, FACT) over `credit_completion`; `hostile` built on `is_enemy`/`same_side`; one `condition_owners`.
- New test `set_alliance_status_stops_two_players_fighting`; `mp_flow_gate.py` PASS.

## Pending
- [ ] Special rules of Greed / Slaughter / CTF / Sudden Death (`GameType` only names them).
- [ ] In-game chat (only lobby chat exists).

## FACT vs INFERENCE
- FACT: starting units count in the score (retail melee score screen).
- INFERENCE (dropped): main's rule that pre-placed units are excluded.

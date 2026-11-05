---
title: "Retail multiplayer flow (LAN) from a recording"
date: 2026-11-04
tags: [session, multiplayer, lan, lobby, score-screen]
type: session
source: opencode
area: multiplayer
log: ["2026-11-04 (86)"]
open: 1
---

## Goal

Make Multiplayer behave like the user's retail screencast
`Screencast_20261104_211148.webm` and the score screenshot
`Screenshot_20261104_211404.png`. Engineering record:
`research/docs/session-log.md` 2026-11-04 (86). Branch
`multiplayer-lan-flow` (worktree `../starcraft-re-recomp-mp`), commit
`ed7cd30`, not merged yet.

## Progress

- [x] Select Connection -> Registry -> Games -> Create -> "Creating game..."
  -> 8-slot lobby -> countdown -> game, each step 2.0-3.6 grey levels from
  the recording (`engine-rust/docs/gates/mp-flow.txt`).
- [x] Lobby games: teams ally, share vision, win together; computers play
  (in the LAN lockstep too); empty slots empty; saves keep the setup (v2).
- [x] LAN protocol 3: named joins, joiner sees the lobby live, race pick,
  chat, setup at start; host + joiner processes end on the same hash.
- [x] Retail score screen (`gluscore`) for lobby games.
- [x] Fixed: trigger Victory/Defeat used `action.group` (empty slot
  defeated player 0); kills credited to the wrong unit (index as id).
- [x] Tests 471/471, clippy 0, `menu_walkthrough.py` and `mp_flow_gate.py`
  PASS.

## Decisions (detail in `decisions/` and link)

- Worktree instead of the main tree (another session had uncommitted work).
- Protocol bump 2 -> 3 rather than trailing fields (`Msg::decode` requires
  the whole frame to be consumed).
- Games footer shows this client's real ports, not retail's 5353/6112.

## Pending
- [x] Merge / PR of `multiplayer-lan-flow` (user decision). Closed 2026-11-05: `c688d76` (session log 2026-11-05 (100)).
- [x] Worker mining time + one worker per patch (OpenBW 75/37 frames;
  income 1 448 -> 520 in 1 857 ticks) — session log (87), commit `8bf58c4`.
- [x] Colour picks applied in game; joiner sees the host's countdown
  (`LobbyColor`, `LobbyCountdown`) — session log (87).
- [ ] Retail recordings to settle: Teams dropdown open, Cancel from each
  screen, one-sided lobby.

## FACT vs INFERENCE

- FACT: layout, timings, fonts, countdown text, score columns and bars
  measured on the recording/screenshot (session log 86).
- INFERENCE: Teams list order, Cancel chain, two-side start rule, colour
  picks read-only.

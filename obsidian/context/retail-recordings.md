---
title: "Retail recordings and what they validate"
date: 2026-11-05
type: context
tags: [context, evidence, recordings]
status: current
source: opencode
---

## Summary

The project's ground truth for behaviour and timing is the user's own
screen recordings of the retail client (local-only, never committed). Before
asking for a new recording, check whether one below already answers the
question. All are 3440x1440; unless noted they are **video only** (no audio),
which is why every audio-timing item is still open.

| Recording | Content | Session log | Pinned by |
|---|---|---|---|
| Glue swish recording (60 fps, 21.8 s) | Single Player → Original → Registry → Load Saved → Load Replay | 2026-11-04 (84) | `scripts/glue_swish_check.py` |
| `Screencast_20261104_210917.webm` (60 fps, 9.8 s) | Main menu → Show Credits, hold, scroll | 2026-11-04 (85) | `src/credits.rs` tests, `ui_visual_diff.py` |
| `Screencast_20261104_211148.webm` (60 fps, 60.5 s) + `Screenshot_20261104_211404.png` | LAN flow: connection → games → create → lobby → countdown; 8-player score | 2026-11-04 (86), (87) | `scripts/mp_flow_gate.py` |
| `Screencast_20261105_073713` | Wasteland briefing | 2026-11-05 (89), (90), (94) | `tests/campaign_episode1.rs` |
| `Screencast_20261105_074011` | Wasteland in game: trigger text, HUD, error line | 2026-11-05 (90), (92) | `--shot-screen trigger-text`, `game-error`, `leaderboard-marines` |
| `Screencast_20261105_074531` | Wasteland victory → score panel slide and count-up | 2026-11-05 (90), (91) | `wasteland_victory_fires_within_one_trigger_cycle` |
| `Screencast_20261105_111714` (397 s) + `Screenshot_20261105_111243` | Boot Camp: ready room and full mission | 2026-11-05 (94) | `tests/campaign_episode1.rs` |
| `Screencast_20261105_113518` (765 s) | Backwater Station: ready room → victory → score | 2026-11-05 (95) | `tests/campaign_episode1.rs` |
| `Desperate Alliance.webm` (1362 s) | Ready room, 30-minute countdown, ending, score 20:20 | 2026-11-05 (99) | `desperate_alliance_follows_the_retail_recording` |

## Still missing (asked in open session items)

- Any recording **with sound** (audio timing, Credits enter/leave sounds).
- Episode I missions not listed above, and Zerg/Protoss ready rooms.
- Multiplayer edge cases: Teams dropdown open, Cancel from each screen, one-sided lobby.
- The Blizzard logo movie (aspect ratio is INFERENCE).

## How measurements are taken

Frames are extracted and compared by template matching / ink boxes; the
method and numbers for each recording are in its session-log entry. Times
in tests are pinned to those numbers, so a test failure after a timing
change means "re-measure", not "loosen the tolerance".

## Related notes

- [[where-to-look]] · [[retail-file-glossary]]

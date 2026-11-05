---
title: "Multiplayer LAN flow: joiner countdown, lobby colours, mining time"
date: 2026-11-04
tags: [session]
type: session
source: opencode
area: multiplayer
log: ["2026-11-04 (87)"]
open: 3
---

## Goal

Make Multiplayer behave like the user's retail recording `Screencast_20261104_211148.webm` and the 8-player score screenshot. This session closed three gaps left by the first pass: joiners did not see the host's countdown, lobby colours were read-only, and income ran ~2.5x retail because harvesting was instant. Engineering record: `research/docs/session-log.md` entries (86) and (87).

## Progress

- [x] Retail flow built on branch `multiplayer-lan-flow` (worktree `../starcraft-re-recomp-mp`, commit `ed7cd30`):
  - Select Connection → Registry → Games → Create → "Creating game..." → 8-slot lobby → countdown → game.
  - Retail score screen for lobby games.
  - LAN protocol 3.
  - Trigger fix: Victory/Defeat now apply to the player running the trigger.
  - Kill-credit fix: kills now go to the attacker.
- [x] Commit `8bf58c4` closes the three gaps:
  - **Countdown for joiners:** new `LobbyCountdown { on }` message (host → joiners). Joiners show "Starting in: N seconds" and the launch swish, then stay black until the host's `Setup`.
  - **Lobby colours:** the colour dropdown offers only free colours. The resolved pick is stored as `SetupSlot.color` → `Player.color` (hashed) and drawn through `render::player_rgb`. It colours units, buildings, minimap blips and score bars (`color_score_group`). Joiners send picks with `LobbyColor`.
  - **Mining time, as OpenBW:** 75 frames per mineral trip, 37 inside a refinery, one worker per field. A worker that finds its field busy moves to a free field within 8 tiles. Implemented with `SimUnit.gather_timer`.
- [x] Mining effect, measured with SimpleAi on Boxer over 1857 ticks: income went from 1448 to 520 minerals (the retail sheet shows ~376 per computer). Score-sheet difference to the retail screenshot went from 6.23 to 5.53.
- [x] Evidence:
  - `cargo test --release`: 474 passed, 0 failed. Clippy and fmt clean.
  - `mp_flow_gate.py` PASS: the joiner shows the host's countdown, and Green is `players[1].color = 11` in both games, which end on the same state hash with no desync.
  - `menu_walkthrough.py` PASS (158 steps).
- [x] Docs updated: [[PROGRESS]] (97 %), CHANGELOG, `lan-protocol.md` §3b, session log (87).

## Decisions

- A Random colour resolves to the slot's default colour, else the first free one. This fixed rule keeps every peer identical (INFERENCE: retail's Random is random).
- The eight default colours equal `PLAYER_COLORS`, so a lobby game left on Random renders as before.
- The RGB values of the extra Remastered colours are approximations by name (INFERENCE).
- The local-only symlink `src/menu_masks` was removed from the commit (amended).

## Pending

- [x] Push `multiplayer-lan-flow` and open the PR against `main`: needs the user's decision. Expect merge conflicts with the other session's uncommitted `sc_view.rs` and session-log edits (it holds entry 85). Closed 2026-11-05: rebased and merged as `c688d76` (session log 2026-11-05 (100)).
- [ ] Physical LAN test on two machines.
- [ ] Skirmish AI is not the retail AI, so score numbers still differ.
- [ ] Retail captures needed to settle remaining guesses: the Teams dropdown open, Cancel from each screen, and a one-sided lobby.

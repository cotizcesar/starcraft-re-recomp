---
title: "Episode I won through input; movement, depletion and campaign AI fixes"
date: 2026-11-06
tags: [session, in-game, campaign, simulation]
type: session
source: opencode
area: campaign
log: ["2026-11-06 (107)", "2026-11-06 (108)", "2026-11-06 (109)", "2026-11-06 (110)", "2026-11-06 (111)", "2026-11-06 (112)", "2026-11-06 (113)", "2026-11-06 (114)", "2026-11-06 (115)", "2026-11-06 (116)", "2026-11-06 (117)", "2026-11-06 (118)", "2026-11-06 (119)", "2026-11-06 (120)", "2026-11-06 (121)", "2026-11-06 (122)", "2026-11-06 (123)", "2026-11-06 (124)", "2026-11-06 (125)", "2026-11-06 (126)", "2026-11-06 (127)", "2026-11-06 (128)", "2026-11-06 (129)", "2026-11-06 (130)", "2026-11-06 (131)", "2026-11-06 (132)", "2026-11-06 (133)", "2026-11-06 (134)", "2026-11-06 (135)", "2026-11-06 (136)", "2026-11-06 (137)"]
open: 2
---

## Goal

Play the Episode I missions through input only, as on the recordings, and fix what breaks.

## Outcome
- Wasteland: victory through `--game-script` input (`scripts/mission_gate.py`, 0 failures at 720p / 1080p / 3440x1440).
- Simulation fixes: chase in pixels, retaliation, waypoint reach / stall / crowd arrival, walk animation from any resting state, the build walk every tick. Each has a test in `tests/reconstruction.rs`.
- Minimap right click / target clicks, Ctrl+click type selection.
- (137) Field-18 extra time is a braked final approach to the drop-off (same cruise speed); braking with a crawl floor retried and reverted; end-dialog gate edge case fixed.
- (136) Contour lookups decompiled; free-box detour tried (field 18 227 -> 236) and reverted (late DA wave gate); retail route only 4 px longer, so the extra time is turning / slowing.
- (135) Contour short-path search mapped: free-box growth to 64 px bounded by unit boxes and terrain contours, corners as nodes; 23 routines to port (plan in session log 135).
- (134) Unit path routine located: region A* then a contour short-path search (FUN_007661b0); port pending.
- (133) Region creation recovered (rectangles <= 8 x 7, FUN_00763840) and ported: Boot Camp 82 regions (was 1); DA late-wave check split out at +-20 s.
- (132) Uniform path margin around standing units falsified (step response; cannot speed field 14 while slowing field 18).
- (131) Retail field-18 SCV route tracked: dogleg through the gap between Marine groups; our planner grazes them (0 px margin); BW-style path margin open.
- (130) Marines-in-the-path hypothesis falsified (our path passes them; steering changes legs by 1 frame).
- (129) Two-SCV rhythm is not a lock (gap stable, equal natural cycles); field 18's retail route is ~55 px longer each way (Marines in the path, inference).
- (128) Boot Camp fields confirmed (14 / 18); serialized hand-in falsified for the two-SCV lock.
- (127) End dialog recovered from the client: game-end check every 46 frames (FUN_007ab9a0), replacing the 24-frame guess.
- (126) Boot Camp per-field gap: drop-off measured, contact braking tested and reverted, two-SCV rhythm differs (open).
- (125) Land preview (per-tile red) and fly-then-land as on Desperate Alliance 58-66 s; first waves met by the player away from the base.
- (124) Waves 1-2: guard / attack_prepare training hypothesis falsified (later waves stop matching); reverted.
- (123) Desperate Alliance waves reach the base within ~10 s of the recording from wave 3 on (mission gate); waves 1-2 open.
- (122) Mineral trip measured phase by phase on Boot Camp: mining 93 frames; Wasteland income -1.7 % (was +6 %).
- (121) Lighting decided on the references (fitted model kept); training-slot "1" and arrow as recorded.
- (120) Decompiler unblocked (noreturn=all, redo); Gamma option found (100 likely neutral); retail shader without AO beats the fitted model on the matched depot but not on the references.
- (119) Light buffer / gamma traced (no depot lights; decompiler blocked by bad no-return flags); queued units take supply as they start (Backwater Station).
- (118) Gas queue stalls fixed (harvest restart, 80 px range), contact hand-in, flingy turning: Boot Camp gas 1.89 s vs 1.97 s with the recorded rhythm.
- (117) Shader port retried (linear space, eye sweep): still ~15-30 % dark, missing additive light buffer / bloom; Wasteland Marine timeline matches (360 frames); end-delay 0.6 s reading falsified.
- (116) Shader ids / uniform table found (data setter not yet); Wasteland income read from the HUD digits; contact mining.
- (115) Matched-camera depot comparison; retail GLSL shaders found (`shadersglsl/`), port tried and reverted pending the run-time uniforms.
- (114) Gas rate vs Boot Camp; flingy acceleration; grpwire closed (Remastered uses the HD wireframes); scaffold brightness measured.
- (113) Endings as recorded (world runs on, box ~1 s after the result, Elapsed Time frozen); single-unit unload.
- (112) Trigger timelines within 0.4 s of the recordings; Bunker cargo slots and right-click loading.
- (111) Cancel construction, targeted spells (Spider Mines), placement where clicked with the retail preview, box hit test for clicks.
- (109) Desperate Alliance waves checked against the recording minimap; Terran scaffold / AlmostBuilt stages and SCV welding as on Boot Camp.
- (108) All four missions won through input; mined-out fields; campaign AI waves as scripted.

## Pending
- [ ] Port the retail contour short-path search (`FUN_007661b0` and its contour builders, session log 134) and check the Boot Camp field-18 route.
- [x] Building lighting: fitted model vs the retail shader decided on the in-game references per lit pixel. Closed 2026-11-06: session log (121), the fitted model wins (18.8 vs 22.5).
- [x] Compare each mission's opening timeline with the recordings. Closed 2026-11-06: `timeline_gate.py` 10/10 (session log 112).
- [ ] Player-driven event times (replay each recording's own build order).
- [x] Boot Camp playthrough in `mission_gate.py`. Closed 2026-11-06: `mission-gate.txt` (tick 6,542).
- [x] Backwater Station playthrough. Closed 2026-11-06: `mission-gate.txt` (tick 9,728), depletion fix.
- [x] Desperate Alliance playthrough. Closed 2026-11-06: `mission-gate.txt` (tick 29,025), AI wave fix.
- [x] Multi-selection panel (see [[2026-11-05-ingame-input-card-buildtimes-perf]]). Closed 2026-11-06: session log (110), `game_input_gate.py`.

## FACT vs INFERENCE
- FACT: each bug traced with `dump-units` (positions, orders, anim state).
- INFERENCE: retaliation help radius 8 tiles and computer-only; crowd arrival 96 px. Falsifier: retail recordings of those cases.

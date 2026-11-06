---
title: "Where to look: area → code → tests → gates"
date: 2026-11-05
type: context
tags: [context, onboarding]
status: current
source: opencode
---

## Summary

First stop for a new task: which files own each area of `engine-rust/` and
what proves a change there. Paths are relative to `engine-rust/` unless they
start with `scripts/`. Verified against the tree on 2026-11-05; when a file
moves, fix this note in the same commit.

| Area (`area:` in session notes) | Code | Tests | Gates / tools |
|---|---|---|---|
| Simulation rules | `src/sim.rs` (World, tick), `src/handlers.rs` (transcribed order handlers), `src/orders.rs`, `src/combat.rs`, `src/path.rs`, `src/iscript.rs`, `src/economy.rs` | `tests/handlers.rs`, `tests/reconstruction.rs`, `tests/economy_original.rs`, `tests/original_parity.rs` | `cargo run --bin sc_orders`, `python3 tools/oracle.py fixtures`, `make verify` |
| Determinism, save, replay | `src/replay.rs`, `World::issue` / `replay_to` in `src/sim.rs` | `tests/replay.rs`, `tests/retail_replay.rs` | `docs/save-format.md`, `docs/replays.md` |
| campaign | `src/trigger.rs` (trigger VM, `GAME_SECOND_TICKS`), `src/campaign.rs`, `src/campaign_progress.rs` (mapdata, gluhist), `src/ready_room.rs` (MBRF), `src/transmission.rs`, `src/aiscript.rs` | `tests/campaign_episode1.rs` | [[retail-recordings]] |
| front-end | `src/menu.rs`, `src/ui.rs`, `src/ui_art.rs`, `src/credits.rs`, `src/video.rs`, `src/text.rs`, menu handlers in `src/bin/sc_view.rs` | `tests/menus.rs`, `tests/menu_layers.rs` | `scripts/ui_visual_diff.py`, `scripts/ui_diff.py`, `scripts/menu_walkthrough.py`, `scripts/menu_time_diff.py`, `scripts/menu_hover_text.py`, `scripts/glue_swish_check.py`, `scripts/ultrawide_diff.py`, `scripts/resolution_diff.py` |
| in-game | HUD and console in `src/bin/sc_view.rs`, `src/portrait.rs`, `src/buttons.rs`, `src/button_sets.rs`, `src/hotkey_data.rs` | `tests/portraits.rs`, `tests/render.rs` | `sc_view --shot … --shot-screen <screen>` (list: `match shot_screen` in `src/bin/sc_view.rs`) |
| rendering | `src/render.rs`, `src/hd_sprites.rs`, `src/hd_anim.rs`, `src/hd_lights.rs`, `src/dds.rs`, `src/vision.rs` | `tests/render.rs`, `tests/terrain_layers.rs`, `tests/hd_tileset.rs` | `scripts/compare_shot.py`, `scripts/render_parity.py` |
| multiplayer | `src/net.rs` (lockstep, lobby protocol), `src/multiplayer.rs`, `src/bin/sc_view/mp.rs`, `src/bin/sc_lan.rs` | `tests/lan.rs`, `tests/multiplayer.rs` | `scripts/mp_flow_gate.py`, `scripts/lan_two_machines.sh`, `research/docs/lan-protocol.md` |
| platform | `src/retail.rs` (install, CASC/MPQ, cache), `src/audio.rs`, `src/video.rs` (ffmpeg), `packaging/` | `tests/retail.rs` (needs `SC_INSTALL`), `tests/audio.rs` | `scripts/package_*.sh`, `scripts/stage_clean_tree.sh` |
| input | input handlers in `src/bin/sc_view.rs` | — | plan: `docs/controller-support.md` |
| re | `research/tools/` (Ghidra Java, memory capture), `tools/verify_runtime_tables.py` | — | `research/docs/README.md`, root `AGENTS.md` §5 |
| tooling | `scripts/progress.py`, `scripts/vault_check.py`, `.github/workflows/` | `scripts/test_progress.py`, `scripts/test_vault_check.py` | CI python job |

## Gotchas worth knowing before you start

- `src/bin/sc_view.rs` is ~20k lines; grep for the screen or handler name rather than reading it.
- Most tests need the gitignored `fixtures/` (see `fixtures/README.md`); CI runs only the fixture-free subset (root `AGENTS.md` §2).
- Front-end gates need a release `sc_view` build, `SC_INSTALL` and the local reference captures (`scripts/README.md`).
- Committed gate outputs live in `docs/gates/`; refresh the matching file when a gate result changes.

## Related notes

- [[retail-recordings]] · [[retail-file-glossary]] · [[OBSIDIAN]]

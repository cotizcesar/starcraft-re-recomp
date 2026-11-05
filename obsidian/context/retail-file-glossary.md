---
title: "Retail file and chunk names used in the code"
date: 2026-11-05
type: context
tags: [context, glossary, retail-data]
status: current
source: opencode
---

## Summary

Short names from the retail data show up in code, tests and the session log
without explanation. This maps each to what it is and where it is read
(files found with `grep -rl` on 2026-11-05; paths under `engine-rust/`).
Format details and evidence: `research/docs/format-coverage.md` and the
session-log entries.

| Name | What it is | Read in |
|---|---|---|
| `stat_txt` | Retail string table (unit names, error lines such as 850 "Not enough minerals…", supply rows 815-826) | `src/ids.rs`, `src/button_sets.rs`, `src/bin/sc_view.rs` |
| `MBRF` | Map chunk with the mission briefing script (ready-room actions) | `src/map.rs`, `src/ready_room.rs` |
| `FORC` | Map chunk: forces (player groups) used by trigger owners 18-21 | `src/map.rs`, `src/trigger.rs` |
| `MRGN` | Map chunk: trigger locations, in pixels | `src/map.rs`, `src/trigger.rs`, `src/sim.rs` |
| `cv5` / `vf4` | Tileset tile groups / walkability of a tileset | `src/map.rs`, tools `sc_regions`, `sc_assets` |
| `mapdata` | Campaign mission order table | `src/campaign_progress.rs` |
| `gluhist` | Campaign history/titles dialog | `src/campaign.rs`, `src/campaign_progress.rs` |
| `glurdyt` | Terran ready-room layout | `src/ui.rs`, `src/ui_art.rs`, `src/retail.rs` |
| `wmission` | End-of-mission (Victory/Defeat) dialog | `src/ui.rs` |
| `gluscore` | Single-player score screen | `src/ui.rs`, `src/bin/sc_view/mp.rs` |
| `pmain` / `iscore` | Score panel frame / score frame layout | `src/retail.rs`, `src/ui_art.rs`, `src/multiplayer.rs` |
| `glulogin` / `gluload` | Registry and Load Saved Game dialogs | `src/menu.rs`, `src/ui.rs`, `src/retail.rs` |
| `portdata.dat` | Portrait clip weights (idle/talking re-pick) | `src/portrait.rs` |
| `creditspanel.js` | Shipped CEF Credits page (scroll rules, CSS) | `src/credits.rs` |
| `hotkeypanel.js` | Shipped CEF Hotkeys page data | `src/hotkey_data.rs` |

## Conventions

- Retail bytes are never vendored: everything is read from `$SC_INSTALL` at
  run time (`src/retail.rs`) or extracted to the local cache.
- Literal in-game strings are quoted as-is, even in English docs.

## Related notes

- [[where-to-look]] · [[retail-recordings]]

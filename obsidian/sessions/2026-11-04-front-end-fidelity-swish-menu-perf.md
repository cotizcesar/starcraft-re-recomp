---
title: "Front-end fidelity, glue swish and menu performance"
date: 2026-11-04
tags: [session]
type: session
source: opencode
area: front-end
log: ["2026-11-04 (83)", "2026-11-04 (84)"]
open: 1
---

## Goal

- Make the `sc_view` front end match the retail StarCraft: Remastered client 1:1, in ultrawide (3440x1440) as well as 16:9.
- Second round of the same goal:
  - stop menu animations from speeding up after leaving the menu and coming back;
  - fix the Blizzard logo's aspect ratio and the sped-up movie audio;
  - make the Single Player grid loop play correctly;
  - make Collections match retail;
  - put a game description where the loader sits above Exit;
  - add the swish sounds on screen changes;
  - fix the low menu frame rate.

## Progress

### Merged as PR #12 (`24908ae`)

- **Ultrawide:** every screen is pillarboxed to the 16:9 box. New gate `scripts/ultrawide_diff.py` passes 31/31.
- **Main menu:**
  - clip draw order is set per button;
  - the Battle.net tag is removed;
  - the build footer uses the retail `blizzard-regular` font;
  - footer labels no longer re-space on hover (`scripts/menu_hover_text.py` 10/10).
- **Registry and Load Saved Game:** drawn from the shipped `glulogin` and `gluload` dialogs and their `pList` plates. Mean difference against retail: Registry 3.19 → 1.32, Saved Games 6.99 → 1.18.
- **Load Replay:** shows real replay metadata. Mean difference 2.41 → 1.96.
- **Glue swish:** measured on the user's 60 fps retail recording.
  - In: the remaining distance is ×0.75 per 1/30 s step.
  - Out: plates move 2^n − 1 UI px per step.
  - Gate `scripts/glue_swish_check.py` was added.
- **Static reverse engineering of the transition:** a dead end, recorded in session log 83. The code is protected.

### Second round (uncommitted; backup at `~/.cache/galaxycraft/wip/menu-fixes-20261104-212751.patch`)

- **Clip speed-up after returning to the menu:**
  - fixed with pause-gap and no-fast-forward clock rules;
  - fast-forward events on the same tour dropped from 98 to 0;
  - hover clips now run continuously, because the recording shows the retail client does not restart them.
- **Movie audio:**
  - movie audio is now decoded at 44.1 kHz (it was 22.05 kHz);
  - the Blizzard logo's audio length now matches the video at 11.41 s;
  - intros decode at 15 fps instead of 12.
- **Blizzard logo:** the SD movie now fills the 16:9 box. This is an INFERENCE.
- **Collections:** now a translucent modal over the live menu, and unselected cells have no frame. Mean difference 6.39 → 4.38.
- **Panel above Exit:** the loader spinner is replaced by a "Welcome, Commander" text, and `src/gif.rs` was removed.
- **Swish sounds:** `swishin` plays on every screen change and `swishout` when a list dialog leaves; the walkthrough gate checks them.
- **Menu draw time at 3440x1440:**

  | Screen | Before | After |
  |---|---|---|
  | Options | 101 ms | 19 ms |
  | Registry | 37 ms | 12 ms |
  | Collections | 40 ms | 20 ms |
  | Episodes | 40 ms | 10 ms |
  | Hotkeys | 25 ms | 13 ms |

  The visual gate shows 39 of 51 screens pixel-identical.

## Decisions

- [[ultrawide-pillarbox-every-screen]]
- [[battlenet-tag-hidden-offline]]
- [[footer-label-hover-recolour-only]]
- **Hover clips:** they run on one continuous clock. This reverses the earlier hover-restart rule; the retail recording falsified it.
- **Panel above Exit:** an intro text replaces the retail news/spinner, at the user's request.

## Pending

- [x] Wait for the parallel multiplayer and credits sessions to finish and commit, then re-verify the merged tree and open the PR for the second round. Closed 2026-11-05: `e533a77`.
- [x] Update `PROGRESS.md`, the session log and `CHANGELOG.md` for the second round. Closed 2026-11-05: session log 2026-11-05 (86), `e533a77`.
- [x] Re-check the flaky retail test `materialised_assets_load_without_the_bundled_asset_directory`, which failed decoding the intro poster during the parallel build. Closed 2026-11-05: not flaky; the per-user `/tmp` quota was full (session log 2026-11-05 (99)).
- [ ] Get a retail capture of the Blizzard logo to confirm its aspect ratio.

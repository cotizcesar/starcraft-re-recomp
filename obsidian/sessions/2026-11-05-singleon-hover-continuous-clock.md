---
title: "SingleOn hover follows the retail screencast: one continuous clock"
date: 2026-11-05
tags: [session, main-menu, front-end, clips]
type: session
source: opencode
area: front-end
log: ["2026-11-05 (103)"]
open: 1
---

## Goal

Replicate the retail Single Player hover recorded in `Screencast_20261105_204138`.

## Outcome
- Restart/rewind from [[2026-11-05-main-menu-hover-intro-shadow-modals]] reverted. Hover clips run on the idle clip's clock and are only hidden. Test: `hidden_mouse_over_clips_keep_running_and_only_show_on_hover`.

## Decisions
- [[menu-hover-clips-share-the-idle-clock]] (supersedes [[singleon-hover-restarts-at-grow-in]])

## Pending
- [ ] Explain why the retail menu clips play at ~13.7 fps in this recording against the 10.5 fps of 2026-10-31 (user kept 10.5): compare a capture with the FPS cap on and off.

## FACT vs INFERENCE
- FACT: continuous clock fits within 2 frames; the pause model misses by up to 22; the hover clip has the same phase as the idle clip; both run at ~13.7 fps here.
- INFERENCE: the rate depends on a retail setting or the render rate. Falsifier: two captures with different FPS cap settings showing the same rate.

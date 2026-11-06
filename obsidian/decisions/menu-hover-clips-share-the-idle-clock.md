---
title: "Main-menu hover clips share the idle clip's clock"
date: 2026-11-05
status: accepted
type: decision
tags: [decision, main-menu, clips]
source: opencode
---

## Context

[[singleon-hover-restarts-at-grow-in]] restarted SingleOn at frame 31 on every
hover. The user then recorded the retail menu (`Screencast_20261105_204138`)
and asked for it to be replicated.

## Alternatives

1. Restart per hover (as in (102)).
2. Pause while unhovered, resume on hover.
3. One continuous clock, hidden while unhovered.

## Decision

Option 3 (`menu_clip_visibility`; every clip pulled every frame).

## Rationale

FACT: a continuous clock fits all 324 hovered frames within 2 frames. The
pause model misses by up to 22 frames. No hover opens on frame 0 or 31.
SingleOn shows the same frame index as the fleet clip.

## Evidence
- Session log 2026-11-05 (103); test `hidden_mouse_over_clips_keep_running_and_only_show_on_hover`.

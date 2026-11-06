---
title: "Keep the fitted sprite lighting until the client's lighting uniforms are recovered"
date: 2026-11-06
status: accepted
type: decision
tags: [decision, rendering, lighting]
source: opencode
---

## Context

The install ships the retail GLSL (`shadersglsl/util_lighting.glsl`,
`deferred_blit.glsl`, `deferred_sprite.glsl`). Porting its directional term
improves the shading pattern on a matched Boot Camp depot (correlation 0.849
-> 0.893) but leaves levels ~20 % low and worsens the in-game references.

## Alternatives

1. Port the shader now with a fitted gain.
2. Keep the fitted model; recover the run-time `lighting` uniform and the
   compositing (exposure / bloom) with REA first.

## Decision

Option 2.

## Rationale

A fitted gain on top of the retail structure is still a guess and is
measurably worse on the references; the missing values are recoverable
from the client.

## Evidence
- Session log 2026-11-06 (115).
- Session log 2026-11-06 (120): without the AO factor the retail shader beats the fitted model on the matched depot (RMS 24.5 / 24.1 / 23.9 vs 24.7 / 27.2 / 28.0) but raises the in-game reference means by 0.10-0.26; decide with a per-object comparison.
- Session log 2026-11-06 (121): per lit pixel on the three in-game references the fitted model wins (18.8 vs 22.5, correlation higher on all three); decision final for now.

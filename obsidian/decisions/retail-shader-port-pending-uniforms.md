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

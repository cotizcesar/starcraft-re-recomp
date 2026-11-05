---
title: "No Battle.net tag on the main menu"
date: 2026-11-04
estado: aceptada
tags: [decision, main-menu, scope]
origen: opencode
---

## Context

The retail main menu shows the signed-in Battle.net tag top right
(`cotizcesar#1532` on reference 130637). The client faked it from `$USER`.

## Alternatives

1. Keep drawing a tag to match the reference pixels.
2. Draw nothing: this client has no Battle.net session.

## Decision

Option 2 (user request, 2026-11-04).

## Rationale

Battle.net/online is out of scope (`AGENTS.md`); a fabricated account tag
misrepresents the user's identity. The menu gates still improve without it
(130637 1.67 → 1.64 together with the spinner and footer fixes).

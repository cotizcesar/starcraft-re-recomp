---
title: "Vault in English, checked against the session log; AGENTS.md compacted"
date: 2026-11-05
tags: [session, vault, onboarding, agents]
type: session
source: opencode
area: tooling
log: ["2026-11-05 (101)"]
open: 1
---

## Goal

A compact root `AGENTS.md`, the vault committed with every change, and a
vault in English that helps the next developer start quickly.

## Outcome
- `AGENTS.md` ~70 lines; REA config location corrected (user-global config).
- Vault folders, keys, templates and skills in English, no number prefixes.
- `scripts/vault_check.py` in CI keeps notes in step with the session log; `--open --area` lists open work.
- Missing notes backfilled (90-92, 97, 99, 100); 15 stale open items ticked with evidence.
- Onboarding: [[where-to-look]], [[retail-recordings]], [[retail-file-glossary]].

## Pending
- [ ] Open `starcraft-re-recomp.md` in the Obsidian app and confirm the embedded Bases views render (YAML written by hand from the Bases syntax docs).

## FACT vs INFERENCE
- FACT: `vault_check.py` OK on 18 notes; 8 unit tests pass.
- INFERENCE: the `.base` files render as intended (falsifier: Obsidian shows a parse error or empty views).

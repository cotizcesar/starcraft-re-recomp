---
title: "Vault, OpenCode project config and controller plan versioned"
date: 2026-11-05
tags: [session, vault, opencode]
type: session
source: opencode
area: tooling
log: ["2026-11-05 (97)"]
open: 0
---

Backfilled on 2026-11-05 from the session log; the evidence is there.

## Goal

Version everything pending without leaking personal paths or secrets.

## Outcome
- `obsidian/`, `.opencode/skills/`, `engine-rust/docs/controller-support.md` and a portable `opencode.json` committed — `399f9bc`.
- Personal absolute paths and the plugin moved to the git-ignored `.opencode/opencode.json`, which OpenCode merges on top.

## FACT vs INFERENCE
- FACT: `opencode mcp list` shows `obsidian-screcomp connected`; `opencode debug config` still loads the plugin.

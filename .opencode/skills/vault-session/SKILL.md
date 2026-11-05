---
name: Obsidian Session
description: Open and close a work session with the starcraft-re-recomp Obsidian vault. Use at the start and end of non-trivial tasks.
---

# Work session (starcraft-re-recomp)

Paths are relative to the repository root. Vault: `obsidian/` (rules in
`obsidian/OBSIDIAN.md`). The session log `research/docs/session-log.md`
holds the evidence; the vault note is a short pointer plus open work.

## Opening

1. Pick the session's `area` (front-end, campaign, in-game, multiplayer,
   rendering, input, platform, tooling, re).
2. `python3 scripts/vault_check.py --open --area <area>`: open items left by
   earlier sessions in that area. Say which ones this session will close.
3. Read the `context/` notes that match the task (start with
   `context/where-to-look.md`) and the last session-log entry
   (`grep -n '^## ' research/docs/session-log.md | tail -3`).

## Closing (in the same commit as the code)

1. Append the dated entry to `research/docs/session-log.md` (append-only).
2. Create `obsidian/sessions/YYYY-MM-DD-topic.md` from
   `obsidian/templates/session.md`: ~10-20 lines, `log` = the entry keys you
   wrote (e.g. `["2026-11-05 (101)"]`), outcome lines with commit/test.
   Do not copy the session-log evidence.
3. Tick items this session closed in older notes:
   `- [x] … Closed YYYY-MM-DD: <commit or test>`. Never tick without evidence.
4. Decisions with alternatives go to `obsidian/decisions/topic-name.md`
   (template `templates/decision.md`), linked from the session note.
5. If you learned something a future dev would otherwise rediscover (a
   command, a recording, a file-to-test mapping), add it to the matching
   `context/` note.
6. `python3 scripts/vault_check.py --write` then `python3 scripts/vault_check.py`
   → `OK`. Commit the vault changes with the code (CI runs the check).

No secrets. `[[wiki-style]]` links, descriptive filenames,
`source: opencode`. Mark FACT vs INFERENCE.

# Obsidian — starcraft-re-recomp

Working-notes vault for this repository, for humans and agents. It answers
"what was being done, what is still open, and where do I start?". The
curated documents stay the source of truth (`PROGRESS.md`, `research/docs/`,
`engine-rust/docs/`); the detailed, dated evidence is
`research/docs/session-log.md`. The vault links to them and never restates them.

## Structure

- `starcraft-re-recomp.md` — index; start here.
- `context/` — onboarding guides: [[where-to-look]], [[retail-recordings]],
  [[retail-file-glossary]]. Update them when you learn something the next
  dev would otherwise rediscover.
- `sessions/` — one short note per work session (`YYYY-MM-DD-topic.md`):
  goal, outcome with commits, open items, FACT vs INFERENCE.
- `decisions/` — one note per decision (context, alternatives, rationale).
- `reviews/` — end-of-day/week reviews.
- `inbox/` — quick unprocessed captures; move or delete them at the next review.
- `templates/` — `session.md`, `decision.md`, `context.md`.
- `sessions.base`, `decisions.base` — Obsidian Bases views (open work by area,
  all sessions, decisions).

## Session note frontmatter (checked by `scripts/vault_check.py`, run in CI)

- `title`, `date`, `tags`, `type: session`, `source` (`opencode` for agent notes).
- `area`: one of `front-end`, `campaign`, `in-game`, `multiplayer`,
  `rendering`, `input`, `platform`, `tooling`, `re`.
- `log`: the session-log entries the session wrote, e.g. `["2026-11-05 (90)"]`
  (`[]` if none). Every session-log entry since `2026-11-04 (82)` must be
  covered by some note.
- `open`: number of `- [ ]` items; run `python3 scripts/vault_check.py --write`
  after editing tasks.

Useful commands (from the repository root):

```sh
python3 scripts/vault_check.py                      # consistency check (CI)
python3 scripts/vault_check.py --write              # refresh open counts
python3 scripts/vault_check.py --open --area campaign   # open work for an area
```

## Rules

1. Read the index and the matching `context/` notes at the start of a
   non-trivial task, plus the open items of its area.
2. Close a session in the same commit as the code: session note, ticks on
   items it closed (`Closed YYYY-MM-DD: <commit or test>`), decisions.
   Always commit the vault; workspace/cache files are gitignored.
3. Never tick an item without evidence; if two notes track the same item,
   keep one and point the other to it.
4. Notes are in English. `[[wiki-style]]` links, lowercase hyphenated tags,
   descriptive filenames (never `untitled-N.md`).
5. Evidence before assumption: mark FACT vs INFERENCE (with a falsifier),
   as in `research/`.
6. Never store secrets, tokens or API keys.

## Skills (OpenCode, `.opencode/skills/`)

- `vault-session` — open/close a work session with the vault.
- `vault-research` — research a topic; durable findings into `research/docs/`, a guide into `context/`.
- `vault-review` — end-of-day review: backfill missing notes, close stale items.

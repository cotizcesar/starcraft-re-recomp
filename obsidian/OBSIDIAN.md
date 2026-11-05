# Obsidian — starcraft-re-recomp

This project's own vault. Full isolation: never read or write the vault
of another project (each has its own).

## Structure

- `00-Inbox/` — quick unprocessed notes.
- `10-Decisiones/` — one note per decision (date + reason + alternatives).
- `20-Contexto/` — architecture, conventions, project glossary.
- `30-Sesiones/` — one log per work session (`YYYY-MM-DD-topic.md`).
- `templates/` — templates `sesion.md`, `decision.md`, `contexto.md`.
- `starcraft-re-recomp.md` — vault index.

## Note format

- Every note carries YAML frontmatter (see templates): `title`, `date`,
  `tags`, `estado` where applicable, and `origen: opencode` on
  agent-generated notes.
- Descriptive filenames (`topic-name.md`), never `untitled-N.md`.
- `[[wiki-style]]` links, lowercase hyphenated tags,
  tasks `- [ ]` / `- [x]`.

## Skills (invoke with `@id`)

- `@sesion` — open/close a work session with the vault.
- `@investigar` — research and synthesis into `20-Contexto/`.
- `@revision-semanal` — weekly summary of `30-Sesiones/`.

## Rules for opencode (reading and writing)

1. Read this vault as context at the start of non-trivial tasks
   (index + `20-Contexto/` + the latest session in `30-Sesiones/`).
2. When relevant work closes, write or update notes: decisions in
   `10-Decisiones/`, the session in `30-Sesiones/`.
3. The curated repository documents (`README.md`, `PROGRESS.md`,
   `research/docs/`, `engine-rust/docs/`) remain the source of truth; the
   vault holds working notes.  Synthesize into the repository only when
   the project flow requires it (the same commit that moves a checklist).
4. Never store secrets, tokens or API keys in the vault.
5. Notes in English (the project language).  `[[wiki-style]]` links.
6. This project is evidence-before-assumption: mark FACT vs INFERENCE in
   the vault as in `research/`.

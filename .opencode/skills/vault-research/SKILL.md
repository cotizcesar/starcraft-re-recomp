---
name: Research and save to Obsidian
description: Research a topic and save findings to the starcraft-re-recomp Obsidian vault. Use when research is requested or a finding must be documented.
---

# Research (starcraft-re-recomp)

1. Search first: `obsidian/context/`, then `research/docs/README.md`
   (curated RE docs) and `grep -n` in `research/docs/session-log.md`.
   If a note or doc exists, extend it instead of duplicating.
2. Research in code, REA (rules in root `AGENTS.md` §5) and, if needed, the web.
3. Durable RE findings belong in `research/docs/` (source of truth: file +
   address + confidence + falsifier). The vault note in
   `obsidian/context/topic-name.md` (template `templates/context.md`) is
   a short guide that links there: what it is, where it lives in the code,
   how to verify it.
4. Descriptive filename, full frontmatter, `source: opencode`. No secrets.
   Mark FACT vs INFERENCE.

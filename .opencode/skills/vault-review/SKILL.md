---
name: Obsidian review
description: End-of-day (or end-of-week) review of the starcraft-re-recomp Obsidian vault. Backfill missing session notes, close stale open items, plan next work.
---

# Vault review (starcraft-re-recomp)

Run at the end of a working day, or whenever `scripts/vault_check.py` fails.

1. `python3 scripts/vault_check.py`. For each "session-log … has no vault
   note", write a short note in `obsidian/sessions/` from the session-log
   entry (state "Backfilled on DATE from the session log").
2. `python3 scripts/vault_check.py --open`. For each item, check `git log`,
   `PROGRESS.md` and the session log: if it is done, tick it with
   `Closed YYYY-MM-DD: <commit or test>`; if two notes track the same item,
   keep one and point the other to it.
3. Write `obsidian/reviews/YYYY-MM-DD-review.md`: what got done (links to
   session notes), what is still open by area and why, next priorities,
   and any `context/` note that needs updating.
4. `python3 scripts/vault_check.py --write && python3 scripts/vault_check.py`
   → `OK`, then commit the vault.

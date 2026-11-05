#!/usr/bin/env python3
"""Keep the Obsidian vault's session notes in step with the session log.

Every session note in ``obsidian/sessions/`` carries frontmatter with
``area`` (one of AREAS), ``log`` (the ``research/docs/session-log.md``
entries it covers, e.g. ``["2026-11-05 (90)"]``) and ``open`` (the number of
``- [ ]`` items in the note, so Obsidian Bases can list notes with open work).

Checks:
  * every note has the required fields, a known area and existing log keys;
  * ``open`` equals the real count of unchecked items;
  * every log entry from FIRST_COVERED onward is covered by some note.

Usage:
  python3 scripts/vault_check.py                 # check (CI)
  python3 scripts/vault_check.py --write         # rewrite the ``open`` counts
  python3 scripts/vault_check.py --open [--area campaign]   # list open items
"""
from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SESSIONS = ROOT / "obsidian" / "sessions"
LOG = ROOT / "research" / "docs" / "session-log.md"
# First session-log entry written while the vault existed; older entries
# predate it and are not required to have a note.
FIRST_COVERED = "2026-11-04 (82)"
AREAS = (
    "front-end",
    "campaign",
    "in-game",
    "multiplayer",
    "rendering",
    "input",
    "platform",
    "tooling",
    "re",
)
REQUIRED = ("title", "date", "tags", "type", "source", "area", "log", "open")
HEADING_RE = re.compile(r"^## (\d{4}-\d{2}-\d{2}) \((\d+)\)")
OPEN_RE = re.compile(r"^\s*- \[ \] (.*)")
OPEN_FIELD_RE = re.compile(r"^open:.*$", re.MULTILINE)


@dataclass
class Note:
    path: Path
    meta: dict[str, str]
    log: list[str] = field(default_factory=list)
    open_items: list[str] = field(default_factory=list)


def log_keys(text: str) -> list[str]:
    """Session-log entry keys (``YYYY-MM-DD (N)``) in file order."""
    keys = []
    for line in text.splitlines():
        m = HEADING_RE.match(line)
        if m:
            keys.append(f"{m.group(1)} ({m.group(2)})")
    return keys


def parse_list(value: str) -> list[str] | None:
    """Parse the inline YAML list subset used in frontmatter: ``["a", "b"]``."""
    value = value.strip()
    if not (value.startswith("[") and value.endswith("]")):
        return None
    inner = value[1:-1].strip()
    if not inner:
        return []
    return [part.strip().strip("\"'") for part in inner.split(",")]


def parse_note(path: Path, text: str) -> Note:
    meta: dict[str, str] = {}
    lines = text.splitlines()
    if lines and lines[0].strip() == "---":
        for line in lines[1:]:
            if line.strip() == "---":
                break
            if ":" in line and not line.startswith(" "):
                key, _, value = line.partition(":")
                meta[key.strip()] = value.strip()
    note = Note(path, meta)
    note.log = parse_list(meta.get("log", "")) or []
    note.open_items = [m.group(1) for m in map(OPEN_RE.match, lines) if m]
    return note


def load_notes(folder: Path) -> list[Note]:
    return [
        parse_note(p, p.read_text(encoding="utf-8"))
        for p in sorted(folder.glob("*.md"))
    ]


def check(notes: list[Note], keys: list[str]) -> list[str]:
    """Return human-readable problems; empty means the vault is consistent."""
    errors = []
    known = set(keys)
    covered: set[str] = set()
    for note in notes:
        name = note.path.name
        missing = [k for k in REQUIRED if k not in note.meta]
        if missing:
            errors.append(f"{name}: missing frontmatter {', '.join(missing)}")
            continue
        if note.meta["area"] not in AREAS:
            errors.append(f"{name}: unknown area {note.meta['area']!r} (use one of {', '.join(AREAS)})")
        if parse_list(note.meta["log"]) is None:
            errors.append(f'{name}: log must be an inline list, e.g. ["2026-11-05 (90)"]')
        for key in note.log:
            if key not in known:
                errors.append(f"{name}: log entry {key!r} not in session-log.md")
        covered.update(note.log)
        if note.meta["open"] != str(len(note.open_items)):
            errors.append(
                f"{name}: open is {note.meta['open']} but the note has "
                f"{len(note.open_items)} unchecked items (run --write)"
            )
    if FIRST_COVERED in keys:
        for key in keys[keys.index(FIRST_COVERED):]:
            if key not in covered:
                errors.append(f"session-log {key} has no vault note (add its key to a note's log)")
    else:
        errors.append(f"FIRST_COVERED {FIRST_COVERED!r} not found in session-log.md")
    return errors


def write_open_counts(notes: list[Note]) -> int:
    changed = 0
    for note in notes:
        if "open" not in note.meta:
            continue
        text = note.path.read_text(encoding="utf-8")
        new = OPEN_FIELD_RE.sub(f"open: {len(note.open_items)}", text, count=1)
        if new != text:
            note.path.write_text(new, encoding="utf-8")
            changed += 1
    return changed


def list_open(notes: list[Note], area: str | None) -> str:
    out = []
    for name in AREAS:
        if area and name != area:
            continue
        rows = [n for n in notes if n.meta.get("area") == name and n.open_items]
        if not rows:
            continue
        out.append(f"## {name}")
        for note in rows:
            out.append(f"[[{note.path.stem}]]")
            out.extend(f"  - [ ] {item}" for item in note.open_items)
    return "\n".join(out)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--write", action="store_true", help="rewrite the open counts")
    ap.add_argument("--open", action="store_true", help="list unchecked items by area")
    ap.add_argument("--area", choices=AREAS, help="with --open: only this area")
    args = ap.parse_args(argv)

    notes = load_notes(SESSIONS)
    if args.open:
        print(list_open(notes, args.area) or "No open items.")
        return 0
    if args.write:
        print(f"Updated open counts in {write_open_counts(notes)} note(s).")
        notes = load_notes(SESSIONS)
    errors = check(notes, log_keys(LOG.read_text(encoding="utf-8")))
    for err in errors:
        print(f"ERROR {err}")
    if errors:
        return 1
    total = sum(len(n.open_items) for n in notes)
    print(f"OK ({len(notes)} session notes, {total} open items)")
    return 0


if __name__ == "__main__":
    sys.exit(main())

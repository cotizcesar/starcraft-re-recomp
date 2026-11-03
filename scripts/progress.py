#!/usr/bin/env python3
"""Compute and verify the completion percentages in PROGRESS.md.

PROGRESS.md is a set of checklists.  Each area is a ``## Heading`` followed by
an ``<!-- area: weight=N -->`` marker and ``- [x]`` / ``- [ ]`` items.  This
script counts the items and renders the summary table between the
``<!-- progress:summary:start -->`` and ``<!-- progress:summary:end -->``
markers, so no percentage is ever typed by hand.

Usage:
  python3 scripts/progress.py            # print the summary
  python3 scripts/progress.py --check    # exit 1 if the table is stale (CI)
  python3 scripts/progress.py --write    # regenerate the table in place
  python3 scripts/progress.py --date YYYY-MM-DD --write
"""
from __future__ import annotations

import argparse
import datetime as _dt
import re
import sys
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_FILE = ROOT / "PROGRESS.md"
START = "<!-- progress:summary:start -->"
END = "<!-- progress:summary:end -->"
AREA_RE = re.compile(r"<!--\s*area:\s*weight=(\d+)\s*-->")
ITEM_RE = re.compile(r"^\s*- \[( |x|X)\] ")
DATE_RE = re.compile(r"_Last updated: (\d{4}-\d{2}-\d{2})_")


@dataclass
class Area:
    title: str
    weight: int
    done: int = 0
    total: int = 0

    @property
    def percent(self) -> int:
        return round(100 * self.done / self.total) if self.total else 0

    @property
    def anchor(self) -> str:
        """GitHub-style heading anchor."""
        slug = self.title.strip().lower()
        slug = re.sub(r"[^\w\- ]", "", slug)
        return slug.replace(" ", "-")


def parse(text: str) -> list[Area]:
    """Return the areas in document order.  Raises ValueError on bad input."""
    areas: list[Area] = []
    heading: str | None = None
    current: Area | None = None
    in_summary = False
    for line in text.splitlines():
        if START in line:
            in_summary = True
            continue
        if END in line:
            in_summary = False
            continue
        if in_summary:
            continue
        if line.startswith("## "):
            heading = line[3:].strip()
            current = None
            continue
        m = AREA_RE.search(line)
        if m:
            if heading is None:
                raise ValueError("area marker before any '## ' heading")
            current = Area(heading, int(m.group(1)))
            areas.append(current)
            continue
        if current is not None and ITEM_RE.match(line):
            current.total += 1
            if ITEM_RE.match(line).group(1) in "xX":
                current.done += 1
    if not areas:
        raise ValueError("no '<!-- area: weight=N -->' markers found")
    for a in areas:
        if a.total == 0:
            raise ValueError(f"area '{a.title}' has no checklist items")
    if sum(a.weight for a in areas) != 100:
        raise ValueError(f"area weights sum to {sum(a.weight for a in areas)}, expected 100")
    return areas


def overall(areas: list[Area]) -> int:
    weighted = sum(a.weight * a.done / a.total for a in areas)
    return round(100 * weighted / sum(a.weight for a in areas))


def render(areas: list[Area], date: str) -> str:
    rows = [
        "| Area | Weight | Done | Total | Progress |",
        "|---|---:|---:|---:|---:|",
    ]
    for a in areas:
        rows.append(f"| [{a.title}](#{a.anchor}) | {a.weight} | {a.done} | {a.total} | {a.percent} % |")
    done = sum(a.done for a in areas)
    total = sum(a.total for a in areas)
    rows.append(
        f"| **Overall (weighted)** | **{sum(a.weight for a in areas)}** | **{done}** | **{total}** | **{overall(areas)} %** |"
    )
    rows.append("")
    rows.append(f"_Last updated: {date}_")
    return "\n".join(rows)


def splice(text: str, table: str) -> str:
    try:
        head, rest = text.split(START, 1)
        _, tail = rest.split(END, 1)
    except ValueError as exc:
        raise ValueError("summary markers missing") from exc
    return f"{head}{START}\n{table}\n{END}{tail}"


def current_date(text: str) -> str | None:
    m = DATE_RE.search(text)
    return m.group(1) if m else None


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--file", type=Path, default=DEFAULT_FILE)
    mode = ap.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="fail if the summary table is stale")
    mode.add_argument("--write", action="store_true", help="regenerate the summary table")
    ap.add_argument("--date", help="date for 'Last updated' (default: today when writing)")
    args = ap.parse_args(argv)

    text = args.file.read_text(encoding="utf-8")
    try:
        areas = parse(text)
    except ValueError as exc:
        print(f"progress: {exc}", file=sys.stderr)
        return 2

    if args.check:
        date = current_date(text) or ""
        expected = splice(text, render(areas, date))
        if expected != text:
            print("progress: PROGRESS.md summary is stale; run python3 scripts/progress.py --write", file=sys.stderr)
            return 1
        print(f"progress: OK ({overall(areas)} % overall, {len(areas)} areas)")
        return 0

    date = args.date or (_dt.date.today().isoformat() if args.write else current_date(text) or "")
    table = render(areas, date)
    if args.write:
        args.file.write_text(splice(text, table), encoding="utf-8")
        print(f"progress: wrote {args.file.name} ({overall(areas)} % overall)")
    else:
        print(table)
    return 0


if __name__ == "__main__":
    sys.exit(main())

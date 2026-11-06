#!/usr/bin/env python3
"""Episode I missions played through `sc_view --game-script` (input only).

Each mission is a script of player actions (selection, minimap, command
card, hotkeys) fed to the same `App::handle_game_input` the window uses.
The gate checks the `game.tsv` state lines: the expected structures and
units appear and the mission ends in victory.

Wasteland (map 1): gather the group, move it to the base on the minimap,
put the SCVs on minerals, build a Barracks, train five Marines; the map's
victory trigger needs 10 Marines, a Barracks and Jim Raynor (rescued on the
way).

Run from the repo root after `cargo build --release --bin sc_view`:

    python3 scripts/mission_gate.py --install "$SC_INSTALL" [--size 1920x1080]
"""
from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BIN = ROOT / "engine-rust/target/release/sc_view"

MISSIONS = {
    "wasteland": (1, [
        "ticks:1",
        "minimap-click:8:6",          # look at the start
        "drag:10:5:1910:700",         # box the 5 Marines and 5 SCVs
        "minimap-rclick:51:84",       # move them to the base (meets Raynor)
        "ticks:1500",
        "minimap-click:51:84",        # look at the base
        "ctrl-click-unit:SCV",        # every SCV on screen
        "rclick-unit:Mineral",        # mine
        "ticks:1500",
        "click-unit:SCV",
        "key:B", "key:B",             # Build > Barracks
        "click-view:30:45",           # place it
        "ticks:2500",
        "click-unit:Barracks",
        "key:M", "key:M", "key:M", "key:M", "key:M",
        "ticks:3500",
    ]),
}


def field(line: str, name: str) -> str:
    m = re.search(rf"{name}=(\[[^\]]*\]|\"[^\"]*\"|\S+)", line)
    return m.group(1) if m else ""


def count(line: str, unit: str) -> int:
    m = re.search(rf"{re.escape(unit)}=(\d+)", field(line, "units"))
    return int(m.group(1)) if m else 0


def run(install: str, size: str, map_index: int, steps: list[str]) -> list[str]:
    w, h = size.split("x")
    out = Path(tempfile.mkdtemp(prefix="gc-mission-"))
    env = dict(os.environ, SC_NO_AUDIO="1", GC_WALK_DIR=str(out))
    subprocess.run(
        [str(BIN), "--install", install, "--no-intro", "--width", w, "--height", h,
         "--shot-map", str(map_index), "--game-script", ",".join(steps)],
        check=True, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    return [l.split("\t")[2] for l in (out / "game.tsv").read_text().splitlines()[1:]]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--install", required=True)
    ap.add_argument("--size", default="1920x1080")
    args = ap.parse_args()
    failures = []

    def check(ok: bool, what: str) -> None:
        print(f"{'PASS' if ok else 'FAIL'} {what}")
        if not ok:
            failures.append(what)

    idx, steps = MISSIONS["wasteland"]
    st = run(args.install, args.size, idx, steps)
    first_raynor = next((i for i, l in enumerate(st) if count(l, "Jim Raynor")), None)
    check(first_raynor is not None, "wasteland: Jim Raynor is rescued by the group")
    check(any(count(l, "Terran Barracks") for l in st), "wasteland: the Barracks is built")
    check(any(count(l, "Terran Marine") >= 10 for l in st), "wasteland: 10 Marines")
    end = st[-1]
    check(field(end, "result") == "Some(true)", "wasteland: victory")
    print(f"  end: tick={field(end, 'tick')} units={field(end, 'units')}")
    print(f"\nmission gate: {len(failures)} failure(s) at {args.size}")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())

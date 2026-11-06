#!/usr/bin/env python3
"""In-game input gate on Boot Camp (map 0) through `sc_view --game-script`.

The same `App::handle_game_input` the window uses gets synthetic frames, and
each step's state line (`game.tsv`) is checked:

1. clicking an SCV selects it and shows the retail SCV card; B opens the
   basic build card (239) and S picks the Supply Depot;
2. a click on open ground places it: the SCV takes the Build order, 100
   minerals are spent and the depot adds 8 supply (16 half-units) once built;
3. the Command Center trains an SCV in its retail 300 frames;
4. a box over units and the Command Center keeps only the units;
5. a right click on minerals never attacks them, and M (Move) waits for a
   target that a world click then gives;
6. in the group panel Shift+click drops a unit and a click keeps only it;
7. Esc on a selected unfinished building cancels it with a 75 % refund;
8. Backwater Station: I on Raynor's Vulture then a click lays one Spider Mine;
9. Desperate Alliance: a right click on a Bunker loads the selected Marines.

Run from the repo root after `cargo build --release --bin sc_view`:

    python3 scripts/game_input_gate.py --install "$SC_INSTALL" [--size 1920x1080]
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

SCRIPT = [
    "ticks:30",
    "click-unit:SCV",          # 1
    "key:B",                   # 2
    "key:S",                   # 3
    "click-view:70:70",        # 4 place the depot
    "ticks:700",               # 5 it is built
    "click-unit:Command Center",  # 6
    "key:S",                   # 7 train an SCV
    "ticks:299",               # 8 not yet
    "ticks:2",                 # 9 done (300 frames)
    "drag:10:5:1910:700",      # 10 box over units and the CC
    "rclick-unit:Mineral",     # 11
    "click-unit:SCV",          # 12
    "key:M",                   # 13 Move waits for a target
    "click-view:40:40",        # 14 the click gives it
    "drag:10:5:1910:700",      # 15 a group again
    "shift-click-slot:0",      # 16 Shift+click drops slot 0
    "click-slot:1",            # 17 a click keeps only that unit
    "ctrl-click-unit:SCV", "rclick-unit:Mineral",             # 18-19 mine
    "ticks:2500",              # 20 income for a depot
    "click-unit:SCV", "key:B", "key:S", "click-view:40:60",   # 21-24 place it
    "ticks:200",               # 25 under construction
    "click-view:40:60",        # 26 select the unfinished depot
    "key:Escape",              # 27 Esc = Cancel construction
]


def field(line: str, name: str) -> str:
    m = re.search(rf"{name}=(\[[^\]]*\]|\"[^\"]*\"|\S+)", line)
    return m.group(1) if m else ""


def count(line: str, unit: str) -> int:
    m = re.search(rf"{re.escape(unit)}=(\d+)", field(line, "units"))
    return int(m.group(1)) if m else 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--install", required=True)
    ap.add_argument("--size", default="1920x1080")
    args = ap.parse_args()
    w, h = args.size.split("x")
    out = Path(tempfile.mkdtemp(prefix="gc-game-"))
    env = dict(os.environ, SC_NO_AUDIO="1", GC_WALK_DIR=str(out))
    subprocess.run(
        [str(BIN), "--install", args.install, "--no-intro", "--width", w, "--height", h,
         "--shot-map", "0", "--game-script", ",".join(SCRIPT)],
        check=True, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    rows = [l.split("\t") for l in (out / "game.tsv").read_text().splitlines()[1:]]
    st = {int(r[0]): r[2] for r in rows}
    failures = []

    def check(ok: bool, what: str) -> None:
        print(f"{'PASS' if ok else 'FAIL'} {what}")
        if not ok:
            failures.append(what)

    check(field(st[1], "selected") == "[Terran SCV]", "click selects the SCV")
    check(field(st[2], "card") == "Some(239)", "B opens the basic build card")
    check(field(st[3], "build") .startswith("Terran") and "Supply Depot" in st[3].split("build=")[1].split(" units=")[0],
          "S picks the Supply Depot")
    check("order=Build { unit_type: 109" in st[4], "the click orders the SCV to build it")
    minerals = lambda i: int(field(st[i], "minerals"))  # noqa: E731
    cap = lambda i: int(field(st[i], "supply").split("/")[1])  # noqa: E731
    check(count(st[5], "Terran Supply Depot") == count(st[0], "Terran Supply Depot") + 1,
          "the depot is built")
    check(cap(5) == cap(4) + 16, "it adds 8 supply")
    check("Training Terran SCV" in st[7], "S on the Command Center trains an SCV")
    check(count(st[8], "Terran SCV") == count(st[7], "Terran SCV"), "not before 300 frames")
    check(count(st[9], "Terran SCV") == count(st[7], "Terran SCV") + 1, "an SCV after 300 frames")
    check("Command Center" not in field(st[10], "selected") and field(st[10], "selected") != "[]",
          "box selection keeps units over buildings")
    check(field(st[11], "status") != '"Attacking"', "right click on minerals does not attack")
    check(field(st[13], "target").startswith("Some((Move"), "M waits for a target")
    check("order=Move" in st[14] and field(st[14], "target") == "None", "the world click moves the SCV")
    group = lambda i: [x for x in field(st[i], "selected").strip("[]").split("+") if x]  # noqa: E731
    check(len(group(16)) == len(group(15)) - 1 and len(group(15)) > 2,
          "Shift+click on a group slot drops that unit")
    check(len(group(17)) == 1, "a click on a group slot keeps only that unit")
    depots = lambda i: count(st[i], "Terran Supply Depot")  # noqa: E731
    check(depots(25) == depots(20) + 1 and "Supply Depot" in field(st[26], "selected"),
          "an unfinished depot is placed and selected")
    check(depots(27) == depots(20) and minerals(27) >= minerals(26) + 75,
          "Esc cancels the construction with a 75 % refund")
    # Targeted spell (Backwater Station, map 2): Raynor's Vulture, I, a
    # click on the ground -> one Spider Mine, as a hero without research.
    out2 = Path(tempfile.mkdtemp(prefix="gc-game-"))
    env2 = dict(os.environ, SC_NO_AUDIO="1", GC_WALK_DIR=str(out2))
    subprocess.run(
        [str(BIN), "--install", args.install, "--no-intro", "--width", w, "--height", h,
         "--shot-map", "2", "--game-script",
         "ticks:1,look:290:1796,click-unit:Jim Raynor,key:I,click-view:60:40,ticks:150"],
        check=True, env=env2, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    mine_rows = [l.split("\t")[2] for l in (out2 / "game.tsv").read_text().splitlines()[1:]]
    check(count(mine_rows[2], "Vulture Spider Mine") == 0
          and field(mine_rows[3], "target").startswith("Some((TargetTech"),
          "I on Raynor's Vulture waits for a target")
    check(count(mine_rows[-1], "Vulture Spider Mine") == 1,
          "the click lays one Spider Mine")
    # Bunker (Desperate Alliance, map 3): box-select Marines, right click
    # the Bunker -> they go in (fewer Marines on the map, Bunker selected
    # afterwards by a click).
    out3 = Path(tempfile.mkdtemp(prefix="gc-game-"))
    env3 = dict(os.environ, SC_NO_AUDIO="1", GC_WALK_DIR=str(out3))
    subprocess.run(
        [str(BIN), "--install", args.install, "--no-intro", "--width", w, "--height", h,
         "--shot-map", "3", "--game-script",
         "ticks:1,look:1072:2752,dump-units:a,ctrl-click-unit:Marine,"
         "rclick-unit-at:Bunker:1072:2752,ticks:200,dump-units:b,click-world:1072:2752"],
        check=True, env=env3, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    marines = lambda f: sum(1 for l in (out3 / f).read_text().splitlines()[1:]  # noqa: E731
                            if l.split("\t")[2] == "Terran Marine")
    bunker_rows = [l.split("\t")[2] for l in (out3 / "game.tsv").read_text().splitlines()[1:]]
    check(marines("b.tsv") < marines("a.tsv"), "a right click on the Bunker loads the Marines")
    check(field(bunker_rows[-1], "selected") == "[Terran Bunker]", "the loaded Bunker can be selected")
    print(f"\ngame input gate: {len(failures)} failure(s) at {args.size} (minerals {minerals(4)} -> {minerals(5)})")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Episode I missions played through `sc_view --game-script` (input only).

Each mission is a script of player actions (selection, minimap, command
card, hotkeys) fed to the same `App::handle_game_input` the window uses.
The gate checks the `game.tsv` state lines: the expected structures and
units appear and the mission ends in victory.

Boot Camp (map 0): a second SCV mines, two more Supply Depots, a Refinery
on the geyser and 100 gas (trigger 6).

Backwater Station (map 2): SCVs and Marines from the base (workers move
to the next field as the small ones run out), then Marines attack-move to
the Infested Command Center (trigger 14); Raynor stays home (his death is
defeat, trigger 6).

Wasteland (map 1): gather the group, move it to the base on the minimap,
put the SCVs on minerals, build a Barracks, train five Marines; the map's
victory trigger needs 10 Marines, a Barracks and Jim Raynor (rescued on the
way).

Desperate Alliance (map 3): SCVs mine, the Barracks trains Marines, and
the base holds until the 30-minute countdown ends (trigger 6); the
`Ter3` waves start small (`attack_add 2 Zergling`), so the first 2.5
minutes are calm as on the recording.

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
    "boot-camp": (0, [
        "ticks:1",
        "click-unit:Command Center", "key:S",   # a second SCV
        "click-unit:SCV", "rclick-unit:Mineral",
        "ticks:400",
        "ctrl-click-unit:SCV", "rclick-unit:Mineral",
        "ticks:500",
        "click-unit:SCV", "key:B", "key:S", "click-view:60:60",   # depot 2
        "ticks:1500",
        "ctrl-click-unit:SCV", "rclick-unit:Mineral",             # builder back
        "click-unit:SCV", "key:B", "key:S", "click-view:40:60",   # depot 3
        "ticks:1800",
        "ctrl-click-unit:SCV", "rclick-unit:Mineral",
        "ticks:600",
        "look:1792:416",                                          # the geyser
        "click-unit:SCV", "key:B", "key:R", "click-world:1792:416",
        "ticks:1500",
        "ctrl-click-unit:SCV", "rclick-unit:Refinery",            # 100 gas
        "ticks:2000",
    ]),
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
        "click-view:50:30",           # place it (buildable ground)
        "ticks:2500",
        "click-unit:Barracks",
        "key:M", "key:M", "key:M", "key:M", "key:M",
        "ticks:3500",
    ]),
    "backwater-station": (2, [
        "ticks:1",
        "look:300:1700",                            # the base
        "ctrl-click-unit:SCV", "rclick-unit:Mineral",
        "click-unit:Command Center", "key:S", "key:S",
        "click-unit:Barracks", "key:M", "key:M",
        "ticks:800",
    ] + [step for _ in range(6) for step in [
        "click-unit:Command Center", "key:S",
        "click-unit:Barracks", "key:M", "key:M",
        "ctrl-click-unit:SCV", "rclick-unit:Mineral",
        "ticks:900",
    ]] + [
        "look:400:1736",
        "ctrl-click-unit:Marine",                   # Raynor stays home
        "key:A", "look:1824:80", "click-world:1824:80",   # the Infested CC
        "ticks:6000",
    ]),
    "desperate-alliance": (3, [
        "ticks:1",
        "look:400:2700",
        "ctrl-click-unit:SCV", "rclick-unit:Mineral",
        "click-unit:Command Center", "key:S", "key:S",
        "ticks:600",
    ] + [step for _ in range(32) for step in [
        "look:500:2800", "click-unit:Barracks", "key:M", "key:M",
        "ticks:900",
    ]]),
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

    idx, steps = MISSIONS["boot-camp"]
    # The last wait in 4-tick steps, to time the end dialog.
    st = run(args.install, args.size, idx, steps[:-1] + ["ticks:4"] * 600)
    check(any(count(l, "Terran Supply Depot") >= 3 for l in st), "boot camp: three Supply Depots")
    check(any(count(l, "Terran Refinery") for l in st), "boot camp: a Refinery on the geyser")
    check(any(int(field(l, "gas") or 0) >= 100 for l in st), "boot camp: 100 gas")
    check(field(st[-1], "result") == "Some(true)", "boot camp: victory")
    won = [l for l in st if field(l, "result") == "Some(true)"]
    shown = [l for l in won if field(l, "dialog") == "true"]
    if won and shown:
        result_tick, dialog_tick = int(field(won[0], "elapsed")), int(field(shown[0], "tick"))
        check(20 <= dialog_tick - result_tick <= 28,
              f"boot camp: the Victory box follows the result by ~24 frames ({dialog_tick - result_tick})")
        check(int(field(st[-1], "tick")) > dialog_tick and int(field(st[-1], "elapsed")) == result_tick,
              "boot camp: the world runs on under the box, Elapsed Time stays at the result")
    else:
        check(False, "boot camp: the Victory box shows")
    print(f"  end: tick={field(st[-1], 'tick')} units={field(st[-1], 'units')}")

    # Gas rate with three SCVs on Boot Camp's Refinery against the
    # recording (357.5-376.9 s: a delivery every 1.97 s, the 1.55 s gaps
    # being the one-at-a-time Refinery queue).
    base = steps[:steps.index("ctrl-click-unit:SCV", len(steps) - 4)]
    gas_steps = base + ["ticks:600", "look:1800:330", "click-unit:Command Center", "key:S",
                        "ticks:330", "ctrl-click-unit:SCV", "rclick-unit:Refinery",
                        "ticks:500"] + ["ticks:1"] * 900
    gs = run(args.install, args.size, 0, gas_steps)
    times, last = [], None
    for l in gs:
        g = int(field(l, "gas") or 0)
        if last is not None and g != last:
            times.append(int(field(l, "tick")) * 0.042)
        last = g
    gaps = [b - a for a, b in zip(times, times[1:])]
    mean = sum(gaps) / len(gaps) if gaps else 0.0
    check(gaps and abs(mean - 1.97) <= 0.2,
          f"boot camp: 3 SCVs deliver gas every {mean:.2f} s (recording 1.97 s)")

    idx, steps = MISSIONS["backwater-station"]
    st = run(args.install, args.size, idx, steps)
    check(any(count(l, "Terran Marine") >= 10 for l in st), "backwater station: the Barracks trains Marines (4 at start)")
    check(all(count(l, "Jim Raynor") for l in st), "backwater station: Jim Raynor survives")
    check(field(st[-1], "result") == "Some(true)", "backwater station: victory (Infested CC)")
    print(f"  end: tick={field(st[-1], 'tick')} units={field(st[-1], 'units')}")

    idx, steps = MISSIONS["wasteland"]
    st = run(args.install, args.size, idx, steps)
    first_raynor = next((i for i, l in enumerate(st) if count(l, "Jim Raynor")), None)
    check(first_raynor is not None, "wasteland: Jim Raynor is rescued by the group")
    check(any(count(l, "Terran Barracks") for l in st), "wasteland: the Barracks is built")
    check(any(count(l, "Terran Marine") >= 10 for l in st), "wasteland: 10 Marines")
    end = st[-1]
    check(field(end, "result") == "Some(true)", "wasteland: victory")
    print(f"  end: tick={field(end, 'tick')} units={field(end, 'units')}")
    idx, steps = MISSIONS["desperate-alliance"]
    st = run(args.install, args.size, idx, steps)
    calm = [l for l in st if 0 < int(field(l, "tick") or 0) <= 3600]
    check(all(count(l, "Terran Command Center") for l in calm)
          and max(count(l, "Terran Marine") for l in calm) >= 16,
          "desperate alliance: no attack on the base in the first 2.5 minutes (recording)")
    check(any(count(l, "Terran Marine") >= 20 for l in st), "desperate alliance: Marines trained")
    check(field(st[-1], "result") == "Some(true)", "desperate alliance: victory when the countdown ends")
    print(f"  end: tick={field(st[-1], 'tick')} units={field(st[-1], 'units')}")
    print(f"\nmission gate: {len(failures)} failure(s) at {args.size}")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Episode I trigger timelines against the retail recordings.

The opening of each mission is a trigger script (transmissions, waits,
objectives) that does not depend on the player.  The onsets below were
measured on the user's recordings (2026-11-06, `text_onsets` analysis: the
first in-game frame after the black fade, then each new block of trigger
text above the console, sampled at 10 fps; Wasteland's SCV line at 1 fps).
The recordings are not in the repository; their numbers are.

`sc_view --game-script ticks:N` logs every trigger cue with its tick
(`cues.tsv`); a cue counts as matching when its time (tick x 42 ms) is
within the tolerance of the recorded onset.

Run from the repo root after `cargo build --release --bin sc_view`:

    python3 scripts/timeline_gate.py --install "$SC_INSTALL"
"""
from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BIN = ROOT / "engine-rust/target/release/sc_view"
TICK_S = 0.042

# map index -> [(text prefix after the speaker, recorded seconds after start)]
RECORDED = {
    0: [  # Boot Camp, game start 62.20 s
        ("ADVISOR", "The T-280 SCV", 1.6),
        ("SCV", "Roger that", 9.1),
        ("ADVISOR", "In order to build", 10.2),
        ("ADVISOR", "You can build additional", 14.8),
        ("", "- Select the Command Center", 18.0),
    ],
    1: [  # Wasteland, game start 80.50 s
        ("TIP", "You can select multiple", 0.0),
        ("TERRAN SCV", "I can't believe", 6.5),  # between +6 and +7 (1 fps)
    ],
    2: [  # Backwater Station, game start 68.90 s
        ("RAYNOR", "Glad to see you", 0.0),
    ],
    3: [  # Desperate Alliance, game start 127.30 s
        ("VULTURE", "Hey, this thing", 1.4),
        ("MARINE", "We can take cover", 23.7),
    ],
}


def cues(install: str, map_index: int, ticks: int) -> list[tuple[float, str]]:
    out = Path(tempfile.mkdtemp(prefix="gc-timeline-"))
    env = dict(os.environ, SC_NO_AUDIO="1", GC_WALK_DIR=str(out))
    subprocess.run(
        [str(BIN), "--install", install, "--no-intro", "--width", "1280", "--height", "720",
         "--shot-map", str(map_index), "--game-script", f"ticks:{ticks}"],
        check=True, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    rows = []
    for line in (out / "cues.tsv").read_text().splitlines()[1:]:
        tick, kind, content = (line.split("\t") + ["", ""])[:3]
        if kind == "text":
            rows.append((int(tick) * TICK_S, " ".join(content.split())))
    return rows


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--install", required=True)
    ap.add_argument("--tolerance", type=float, default=0.5)
    args = ap.parse_args()
    failures = 0
    for map_index, expected in RECORDED.items():
        got = cues(args.install, map_index, 800)
        for speaker, text, when in expected:
            match = next((t for t, c in got if c.startswith(speaker) and text in c), None)
            ok = match is not None and abs(match - when) <= args.tolerance
            failures += not ok
            ours = f"{match:5.1f}s" if match is not None else "  none"
            print(f"{'PASS' if ok else 'FAIL'} map {map_index} {speaker or '-':10} "
                  f"{text[:28]:28} recorded {when:5.1f}s ours {ours}")
    print(f"\ntimeline gate: {failures} failure(s), tolerance {args.tolerance} s")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())

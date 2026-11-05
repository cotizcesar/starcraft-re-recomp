#!/usr/bin/env python3
"""Ultrawide invariant for every front-end and in-game screen.

The retail client pillarboxes its 16:9 front end: on all 56 reference
captures at 3440x1440 the bars outside the centred 2560x1440 box are black
(max channel <= 8).  So a screen rendered at 3440x1440 must be

1. black outside the content box (no plate, arm or backdrop leaks), and
2. identical inside it to the same screen rendered at 2560x1440 (nothing is
   placed from the window width instead of the content box).

Run from the repo root after `cargo build --release --bin sc_view`:

    python3 scripts/ultrawide_diff.py --install "$SC_INSTALL"
"""
from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
BIN = ROOT / "engine-rust/target/release/sc_view"

# Every front-end `--shot-screen`, plus the in-game screens.
SCREENS = [
    "menu", "menu-hover-single", "menu-hover-multi", "menu-hover-exit",
    "gametype", "registry", "episode",
    "episode-expansion-hover-protoss", "episode-locked-zerg", "mission", "saved", "replays", "lobby", "lan",
    "options-speed", "options-sound", "options-video", "options-game",
    "options-network", "collections", "credits", "hotkeys-terran",
    "hotkeys-reset-dialog",
    # In game: the retail client pillarboxes the game view and console too
    # (references 134905..135121 are black outside the box).
    "ingame-zerg", "ingame-terran2", "ingame-protoss", "objectives", "tips",
    "briefing", "pause", "score",
]
BOX = (440, 3000)
# Clip clock shared by both renders: the menu clips run on the wall clock
# otherwise and two processes would show different frames.
MENU_AT = "2.0"


def render(install: str, screen: str, width: int, out: Path) -> np.ndarray:
    env = dict(os.environ, SC_NO_AUDIO="1")
    subprocess.run(
        [str(BIN), "--install", install, "--no-intro", "--shot", str(out),
         "--shot-screen", screen, "--width", str(width), "--height", "1440",
         "--menu-at", MENU_AT],
        check=True, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    return np.asarray(Image.open(out).convert("RGB")).astype(np.int16)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--install", required=True)
    ap.add_argument("--max-mean", type=float, default=0.5,
                    help="largest mean |16:9 - ultrawide box| per screen")
    ap.add_argument("--screens", nargs="*", default=SCREENS)
    args = ap.parse_args()
    tmp = Path(tempfile.mkdtemp(prefix="gc-ultrawide-"))
    failed = 0
    print(f"{'screen':28} {'bars max':>8} {'box mean':>8} {'box >30':>8} status")
    for screen in args.screens:
        a = render(args.install, screen, 2560, tmp / f"{screen}-16x9.bmp")
        b = render(args.install, screen, 3440, tmp / f"{screen}-uw.bmp")
        bars = int(max(b[:, : BOX[0]].max(), b[:, BOX[1]:].max()))
        d = np.abs(a - b[:, BOX[0]:BOX[1]]).max(axis=2)
        mean, outl = float(d.mean()), float((d > 30).mean() * 100)
        ok = bars == 0 and mean <= args.max_mean
        failed += not ok
        print(f"{screen:28} {bars:8d} {mean:8.2f} {outl:7.2f}% {'PASS' if ok else 'FAIL'}")
    total = len(args.screens)
    print(f"\nultrawide gate: {total - failed}/{total} screens PASS (artifacts in {tmp})")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()

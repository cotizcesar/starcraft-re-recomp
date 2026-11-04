#!/usr/bin/env python3
"""Pixel parity of two client builds over the headless screens.

Renders every screen with both launch commands (for example the developer
build and a release package) and prints the changed-pixel count per screen.
A package is "1:1" with the developer build when every count is 0.  Also
renders the in-game screen at several `--menu-at` clock values, which plays
the full-motion HUD portrait to that second, and reports how many distinct
portrait frames each build shows.

Usage:
  python3 scripts/render_parity.py --install "$SC_INSTALL" \\
      --a engine-rust/target/release/sc_view --b dist/GalaxyCraft-0.1.0-x86_64.AppImage

Exit status is non-zero when any screen differs.
"""
import argparse
import os
import shlex
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image

SCREENS = [
    "menu", "ingame-zerg", "ingame-protoss", "ingame-terran2",
    "hotkeys-terran-marine", "hotkeys-zerg-larva-cards", "hotkeys-protoss-probe",
    "options-game", "briefing", "score", "pause", "registry",
]
CLOCKS = ["0.0", "0.3", "0.6", "0.9", "1.5", "5.0"]
# Portrait well of `statport` at 1920x1080.
PORTRAIT = (slice(930, 1065), slice(1410, 1535))


def render(cmd, install, out, screen, size, clock=None):
    args = shlex.split(cmd) + ["--install", install, "--width", str(size[0]),
                               "--height", str(size[1]), "--shot", str(out),
                               "--shot-screen", screen]
    if clock is not None:
        args += ["--menu-at", clock]
    env = dict(os.environ, SC_NO_AUDIO="1")
    subprocess.run(args, env=env, check=True, capture_output=True)
    return np.asarray(Image.open(out).convert("RGB")).astype(int)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--install", required=True)
    ap.add_argument("--a", required=True, help="reference launch command")
    ap.add_argument("--b", required=True, help="candidate launch command")
    args = ap.parse_args()
    tmp = Path(tempfile.mkdtemp(prefix="gc-parity-"))
    failed = 0
    print(f"a: {args.a}\nb: {args.b}\n")
    print(f"{'screen':28} changed px")
    for s in SCREENS:
        a = render(args.a, args.install, tmp / f"{s}-a.bmp", s, (2560, 1440))
        b = render(args.b, args.install, tmp / f"{s}-b.bmp", s, (2560, 1440))
        n = int((np.abs(a - b).sum(2) > 0).sum())
        failed += n > 0
        print(f"{s:28} {n}")
    frames = {"a": set(), "b": set()}
    for c in CLOCKS:
        a = render(args.a, args.install, tmp / f"p{c}-a.bmp", "ingame", (1920, 1080), c)
        b = render(args.b, args.install, tmp / f"p{c}-b.bmp", "ingame", (1920, 1080), c)
        n = int((np.abs(a - b).sum(2) > 0).sum())
        failed += n > 0
        frames["a"].add(a[PORTRAIT].astype(np.uint8).tobytes())
        frames["b"].add(b[PORTRAIT].astype(np.uint8).tobytes())
        print(f"{'ingame --menu-at ' + c:28} {n}")
    print(f"\nportrait frames over {len(CLOCKS)} clocks: a {len(frames['a'])}, b {len(frames['b'])}")
    print(f"parity: {'FAIL' if failed else 'PASS'} ({failed} differing renders)")
    for f in tmp.iterdir():
        f.unlink()
    tmp.rmdir()
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())

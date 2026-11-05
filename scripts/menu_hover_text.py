#!/usr/bin/env python3
"""Main-menu hover must not move the footer labels.

Hovering Options, Collections, Hotkeys, Show Credits or View Intro only
changes the label colour in the retail client: the glyphs stay where they
are (user report, 2026-11-04).  For each entry this renders the menu with
the pointer away and on the entry, and compares the centre of every glyph
(runs of inked columns) inside the label's row: none may move by more than
1 px, and the colour must change.

Run from the repo root after `cargo build --release --bin sc_view`:

    python3 scripts/menu_hover_text.py --install "$SC_INSTALL"
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

# entry -> (pointer at 2560x1440, label rows y0..y1, columns x0..x1)
ENTRIES = {
    "Options": ((558, 1042), (1030, 1095), (300, 900)),
    "Collections": ((558, 1123), (1110, 1170), (300, 900)),
    "Hotkeys": ((558, 1192), (1180, 1240), (300, 900)),
    "Show Credits": ((558, 1273), (1260, 1320), (300, 900)),
    "View Intro": ((558, 1348), (1335, 1395), (300, 900)),
}


# Window sizes: the native 1440p box, and a maximised ultrawide window
# whose title bar leaves a fractional UI scale (1387 / 480 = 2.89).
SIZES = ((2560, 1440), (3440, 1387))


def to_window(size: tuple[int, int], x: float, y: float) -> tuple[int, int]:
    """Map a 2560x1440 content-box point into a window of `size`."""
    w, h = size
    k = h / 1440
    cw = min(w, round(h * 16 / 9))
    return (w - cw) // 2 + round(x * k), round(y * k)


def frame(install: str, size: tuple[int, int], pointer: tuple[int, int],
          out: Path) -> np.ndarray:
    env = dict(os.environ, SC_NO_AUDIO="1", GC_WALK_DIR=str(out))
    subprocess.run(
        [str(BIN), "--install", install, "--no-intro", "--width", str(size[0]),
         "--height", str(size[1]), "--menu-at", "2.0", "--menu-script",
         f"move:{pointer[0]}:{pointer[1]}"],
        check=True, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    return np.asarray(Image.open(out / "walk-00-Main.bmp").convert("RGB")).astype(int)


def ink(a: np.ndarray) -> np.ndarray:
    return (a[:, :, 1] > 120) & (a[:, :, 1] > a[:, :, 0] + 40)


def glyph_centres(mask: np.ndarray) -> list[float]:
    """Centre column of each run of inked columns (one run per glyph or
    touching glyph pair).  Colour changes move anti-aliased edge pixels
    across any threshold, but not the centre of a glyph."""
    cols = mask.any(axis=0)
    runs, start = [], None
    for x, on in enumerate(list(cols) + [False]):
        if on and start is None:
            start = x
        elif not on and start is not None:
            runs.append((start + x - 1) / 2)
            start = None
    return runs


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--install", required=True)
    args = ap.parse_args()
    tmp = Path(tempfile.mkdtemp(prefix="gc-hover-text-"))
    failed = total = 0
    for size in SIZES:
        tag = f"{size[0]}x{size[1]}"
        idle = frame(args.install, size, (5, 5), tmp / f"{tag}-idle")
        for name, (pointer, (y0, y1), (x0, x1)) in ENTRIES.items():
            px = to_window(size, *pointer)
            (x0, y0), (x1, y1) = to_window(size, x0, y0), to_window(size, x1, y1)
            hover = frame(args.install, size, px, tmp / f"{tag}-{name.replace(' ', '_')}")
            a = glyph_centres(ink(idle[y0:y1, x0:x1]))
            b = glyph_centres(ink(hover[y0:y1, x0:x1]))
            lit = int(np.abs(hover[y0:y1, x0:x1] - idle[y0:y1, x0:x1]).max())
            moved = (max(abs(p - q) for p, q in zip(a, b))
                     if len(a) == len(b) and a else float("inf"))
            ok = moved <= 1.0 and lit > 0
            failed += not ok
            total += 1
            print(f"{tag:9} {name:13} glyph runs {len(a):2d}/{len(b):2d}  largest "
                  f"shift {moved:5.1f} px  colour change {lit:3d}  "
                  f"{'PASS' if ok else 'FAIL'}")
    print(f"\nhover text gate: {total - failed}/{total} PASS")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()

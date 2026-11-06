#!/usr/bin/env python3
"""Every screen must keep its 16:9 layout at every supported window size.

Supported: 16:9 from 1280x720 up and ultrawide (user request, 2026-11-05).
Windows narrower than 16:9 (4:3, 16:10) are letterboxed rather than cut.
For each screen and window size this renders the screen, takes the 16:9
content box (pillarboxed for ultrawide, letterboxed for narrow windows),
scales the 2560x1440 render of the same screen to that box, and compares:

1. the bars outside the box must be black (max channel <= 8), and
2. the box must match the 1440p layout: both are compared on one 640x360
   grid (lightly blurred), mean |difference| at most `--max-mean` grey
   levels and outliers (> 40) reported.  A plate, label or button placed from the
   window size instead of the content box shows up as a large difference
   (the 2026-11-05 bug: Ok / Cancel cut off at 1277x954).

Run from the repo root after `cargo build --release --bin sc_view`:

    python3 scripts/resolution_diff.py --install "$SC_INSTALL"
"""
from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
BIN = ROOT / "engine-rust/target/release/sc_view"
sys.path.insert(0, str(Path(__file__).resolve().parent))
from ultrawide_diff import SCREENS, MENU_AT  # noqa: E402

EXTRA = ["mp-connection", "mp-games", "mp-create", "mp-lobby"]
SIZES = [(1280, 720), (1600, 900), (1920, 1080), (3440, 1440), (2560, 1080),
         (1277, 954), (1920, 1200)]
REFERENCE = (2560, 1440)
COMPARE = (640, 360)


def render(install: str, screen: str, size: tuple[int, int], out: Path) -> np.ndarray:
    env = dict(os.environ, SC_NO_AUDIO="1")
    subprocess.run(
        [str(BIN), "--install", install, "--no-intro", "--shot", str(out),
         "--shot-screen", screen, "--width", str(size[0]), "--height", str(size[1]),
         "--menu-at", MENU_AT],
        check=True, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    return np.asarray(Image.open(out).convert("RGB"))


def layout(img: Image.Image) -> np.ndarray:
    """Both images at one 640x360 grid, lightly blurred: resampling noise
    and 1 px rounding of a fine texture vanish, while a plate or label
    placed tens of pixels off (or cut off) still differs strongly."""
    small = img.resize(COMPARE, Image.LANCZOS).filter(ImageFilter.GaussianBlur(1.0))
    return np.asarray(small).astype(np.int16)


def content_box(w: int, h: int) -> tuple[int, int, int, int]:
    """Mirror of `letterbox` + `content_rect` in sc_view: (x, y, w, h)."""
    if w * 9 >= h * 16:
        cw = min(w, round(h * 16 / 9))
        return (w - cw) // 2, 0, cw, h
    ih = min(h, round(w * 9 / 16))
    return 0, (h - ih) // 2, w, ih


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--install", required=True)
    ap.add_argument("--max-mean", type=float, default=3.0)
    ap.add_argument("--screens", nargs="*", default=SCREENS + EXTRA)
    ap.add_argument("--sizes", nargs="*", default=[f"{w}x{h}" for w, h in SIZES])
    args = ap.parse_args()
    sizes = [tuple(map(int, s.split("x"))) for s in args.sizes]
    tmp = Path(tempfile.mkdtemp(prefix="gc-res-"))
    failed = total = 0
    print(f"{'screen':32} {'size':>10} {'bars':>4} {'mean':>6} {'>40':>7} status")
    for screen in args.screens:
        ref = Image.fromarray(render(args.install, screen, REFERENCE, tmp / f"{screen}-ref.bmp"))
        for w, h in sizes:
            img = render(args.install, screen, (w, h), tmp / f"{screen}-{w}x{h}.bmp")
            x, y, cw, ch = content_box(w, h)
            mask = np.ones(img.shape[:2], bool)
            mask[y:y + ch, x:x + cw] = False
            bars = int(img[mask].max()) if mask.any() else 0
            box = Image.fromarray(img[y:y + ch, x:x + cw])
            d = np.abs(layout(box) - layout(ref)).max(axis=2)
            mean, outl = float(d.mean()), float((d > 40).mean() * 100)
            ok = bars <= 8 and mean <= args.max_mean
            failed += not ok
            total += 1
            print(f"{screen:32} {w:>5}x{h:<4} {bars:4d} {mean:6.2f} {outl:6.2f}% "
                  f"{'PASS' if ok else 'FAIL'}", flush=True)
    print(f"\nresolution gate: {total - failed}/{total} PASS")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()

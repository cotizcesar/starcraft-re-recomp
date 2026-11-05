#!/usr/bin/env python3
"""Deterministic full-frame check of the four main-menu references.

The retail menu clips are ten-second loops.  `sc_view --menu-at SECONDS`
seeks every clip to the same wall-clock second, which makes a headless
capture comparable with a reference instead of depending on how long asset
loading took.  The phases below are the best matches found by sweeping
0.5..10.0 s in 0.25 s steps against each capture.

Usage:
  python3 scripts/menu_time_diff.py --install "/path/to/StarCraft"
"""
import argparse
import os
import subprocess
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
REF = ROOT / "engine-rust/docs/original_screenshots"
BIN = ROOT / "engine-rust/target/release/sc_view"

# reference id -> (shot screen, menu clip second, per-layer phase overrides)
# The six clips are independent FLC loops; the captures do not share one
# global phase for every layer.  Values are the coordinate-descent minima.
CASES = {
    "130637": (
        "menu",
        2.75,
        {"single": 2.75, "multi": 1.875, "exit": 3.0, "spinner": 0.435},
    ),
    "130704": (
        "menu-hover-single",
        8.1875,
        {
            "single": 7.9375,
            "multi": 8.75,
            "exit": 6.5,
            "singleon": 8.1875,
            "spinner": 0.075,
        },
    ),
    "130712": (
        "menu-hover-multi",
        5.875,
        {
            "single": 5.375,
            "multi": 6.0625,
            "exit": 6.1875,
            "multion": 5.875,
            "spinner": 0.585,
        },
    ),
    "130727": ("gametype", 9.8125, {"spinner": 0.345}),
}


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--install", required=True)
    ap.add_argument("--max-mean", type=float, default=8.0)
    ap.add_argument("--max-outlier", type=float, default=8.0)
    ap.add_argument(
        "--calibrated",
        action="store_true",
        help="enable the 3x text masks and per-state correction layers",
    )
    args = ap.parse_args()
    tmp = Path(tempfile.mkdtemp(prefix="menu-time-"))
    failed = 0
    print(f"{'ref':8} {'screen':18} {'t':>5} {'mean':>7} {'>30 px':>8} status")
    for rid, (screen, at, phases) in CASES.items():
        env = dict(os.environ, SC_NO_AUDIO="1")
        if args.calibrated:
            env["GC_CALIBRATED_CAPTURE"] = rid
            env["GC_CALIBRATION_DIR"] = str(
                ROOT / "engine-rust/fixtures/calibration"
            )
        out = tmp / f"{rid}.bmp"
        cmd = [
            str(BIN),
            "--install",
            args.install,
            "--no-intro",
            "--width",
            "2560",
            "--height",
            "1440",
            "--menu-at",
            f"{at:.4f}",
            "--shot",
            str(out),
            "--shot-screen",
            screen,
        ]
        if phases:
            cmd += [
                "--menu-phase",
                ",".join(f"{k}={v:.4f}" for k, v in phases.items()),
            ]
        subprocess.run(
            cmd,
            check=True,
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        ref = np.asarray(
            Image.open(REF / f"Screenshot_20261027_{rid}.png")
            .convert("RGB")
            .crop((440, 0, 3000, 1440))
        ).astype(np.int16)
        cur = np.asarray(Image.open(out).convert("RGB")).astype(np.int16)
        d = np.abs(ref - cur).mean(axis=2)
        mean = float(d.mean())
        outlier = float((d > 30).mean() * 100.0)
        ok = mean <= args.max_mean and outlier <= args.max_outlier
        failed += 0 if ok else 1
        print(
            f"{rid:8} {screen:18} {at:5.2f} {mean:7.2f} {outlier:7.2f}% "
            f"{'PASS' if ok else 'FAIL'}"
        )
    print(f"\nmenu timing gate: {len(CASES) - failed}/{len(CASES)} screens PASS")
    raise SystemExit(0 if failed == 0 else 1)


if __name__ == "__main__":
    main()

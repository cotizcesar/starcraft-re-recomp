#!/usr/bin/env python3
"""Compare one of our renders against a retail reference, per screen region.

Usage:
  compare_shot.py <ours.png> <reference.png> [--grid 8x4]

Reports mean absolute per-channel difference (0..255) for each cell of the
grid plus the global mean. The world area is expected to differ (different
map state), so the HUD band and the layout bands are the actionable numbers.
"""
import sys
from pathlib import Path

import numpy as np
from PIL import Image

def main() -> int:
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    ours_p, ref_p = Path(sys.argv[1]), Path(sys.argv[2])
    cols, rows = 8, 4
    if "--grid" in sys.argv:
        g = sys.argv[sys.argv.index("--grid") + 1]
        cols, rows = (int(x) for x in g.lower().split("x"))

    a = Image.open(ours_p).convert("RGB")
    b = Image.open(ref_p).convert("RGB")
    if a.size != b.size:
        b = b.resize(a.size, Image.BILINEAR)
    A = np.asarray(a, dtype=np.int16)
    B = np.asarray(b, dtype=np.int16)
    diff = np.abs(A - B)

    h, w, _ = diff.shape
    print(f"ours={ours_p.name} ref={ref_p.name} size={w}x{h} grid={cols}x{rows}")
    print(f"GLOBAL mean|diff| = {diff.mean():6.2f}")
    print("per-region (row-major):")
    cell = []
    for r in range(rows):
        row = []
        y0, y1 = r * h // rows, (r + 1) * h // rows
        for c in range(cols):
            x0, x1 = c * w // cols, (c + 1) * w // cols
            v = diff[y0:y1, x0:x1].mean()
            row.append(v)
            cell.append(v)
        print("  " + "  ".join(f"{v:6.2f}" for v in row))
    # HUD band only (bottom quarter)
    y0 = int(h * 0.75)
    print(f"HUD band (bottom 25%) mean|diff| = {diff[y0:].mean():6.2f}")
    print(f"best cell={min(cell):6.2f} worst cell={max(cell):6.2f}")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())

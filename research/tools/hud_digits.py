#!/usr/bin/env python3
"""Segment the HUD mineral number into glyphs and cluster them.
usage: digits.py VIDEO T0 DUR CROP_X CROP_Y CROP_W CROP_H OUT.npz"""
import subprocess, sys, numpy as np
v, t0, dur = sys.argv[1], float(sys.argv[2]), float(sys.argv[3])
x, y, w, h = map(int, sys.argv[4:8])
fps = 2
raw = subprocess.run(["ffmpeg", "-loglevel", "error", "-ss", str(t0), "-t", str(dur), "-i", v,
                      "-vf", f"fps={fps},crop={w}:{h}:{x}:{y},format=rgb24", "-f", "rawvideo", "-"],
                     capture_output=True, check=True).stdout
a = np.frombuffer(raw, np.uint8).reshape(-1, h, w, 3).astype(int)
# HUD numbers are green: G high, R/B low
mask = (a[..., 1] > 120) & (a[..., 1] > a[..., 0] + 40)
frames = []
for m in mask:
    cols = m.any(0)
    glyphs, start = [], None
    for i, c in enumerate(list(cols) + [False]):
        if c and start is None:
            start = i
        elif not c and start is not None:
            g = m[:, start:i]
            rows = np.where(g.any(1))[0]
            if i - start >= 2 and len(rows):
                glyphs.append((start, g[rows.min():rows.max() + 1]))
            start = None
    frames.append(glyphs)
np.save(sys.argv[8], np.array(frames, dtype=object), allow_pickle=True)
print(len(frames), "frames;", sum(len(f) for f in frames), "glyphs")

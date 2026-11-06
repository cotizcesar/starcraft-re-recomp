#!/usr/bin/env python3
"""Read the HUD number series from digits.py output with labelled prototypes."""
import sys, numpy as np
from PIL import Image
fr = np.load(sys.argv[1], allow_pickle=True)
protos = np.load(sys.argv[2])
labels = sys.argv[3]          # one char per prototype
t0, fps = float(sys.argv[4]), float(sys.argv[5])
def key(g):
    return np.asarray(Image.fromarray((g * 255).astype('uint8')).resize((12, 18))) > 127
out = []
for i, f in enumerate(fr):
    s = ""
    for _, g in sorted(f, key=lambda t: t[0]):
        k = key(g)
        d = [(p != k).mean() for p in protos]
        j = int(np.argmin(d))
        s += labels[j] if d[j] < 0.15 else "?"
    out.append((t0 + i / fps, s))
for t, s in out:
    print(f"{t:.1f}\t{s}")

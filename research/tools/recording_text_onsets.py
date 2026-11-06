#!/usr/bin/env python3
"""Recording -> game start and trigger-text onsets (seconds after start).
usage: text_onsets.py VIDEO FROM_S DURATION_S"""
import subprocess, sys, numpy as np
video, t0, dur = sys.argv[1], float(sys.argv[2]), float(sys.argv[3])
fps = 10
W, H = 3440, 1440
def frames(crop):
    cmd = ["ffmpeg", "-loglevel", "error", "-ss", str(t0), "-t", str(dur), "-i", video,
           "-vf", f"fps={fps},crop={crop},format=rgb24", "-f", "rawvideo", "-"]
    raw = subprocess.run(cmd, capture_output=True, check=True).stdout
    w, h = map(int, crop.split(":")[:2])
    return np.frombuffer(raw, np.uint8).reshape(-1, h, w, 3).astype(int)
con = frames("600:200:440:1240").mean(axis=(1, 2, 3))
txt = frames("1400:220:480:640")
lav = ((txt[..., 2] > 170) & (txt[..., 0] > 150) & (txt[..., 1] > 150)).sum(axis=(1, 2))
# game start: first console-visible frame after the last black frame
black = np.where(con < 2)[0]
start_i = None
for i in range(1, len(con)):
    if con[i - 1] < 2 and con[i] > 8:
        start_i = i
        break
start = t0 + start_i / fps if start_i is not None else None
print(f"game start {start:.2f}s" if start else "no start found")
prev = 0
for i, v in enumerate(lav):
    if start_i is not None and i < start_i:
        prev = v; continue
    if (prev < 300 and v >= 800) or (v - prev > 1500):
        t = t0 + i / fps
        print(f"text onset {t:.1f}s  (+{t - start:.1f}s)  px={v}")
    prev = v

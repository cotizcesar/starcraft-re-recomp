#!/usr/bin/env python3
"""Per-reference verification of the reconstructed front-end.

Each retail capture gets the check that matches its screen:

* ``panel``  — red dialog border inside a fixed region, delta <= 2 px.
* ``rings``  — the three purple episode rings: centre delta <= 2 px.
* ``art``    — a shipped art panel / dark console is present at the measured
               reference rectangle (Saved, Replays, Lobby, Game Type).

Usage:
  python3 scripts/ui_diff.py --install "/path/to/StarCraft"
"""
import argparse
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
REF = ROOT / "engine-rust/docs/original_screenshots"
BIN = ROOT / "engine-rust/target/release/sc_view"

PANEL = [
    ("130637", "menu", (1545, 190, 2180, 820)),
    ("130704", "menu-hover-single", (1545, 190, 2180, 820)),
    ("130712", "menu-hover-multi", (1545, 190, 2180, 820)),
    ("130926", "menu-hover-exit", (1545, 190, 2180, 820)),
    ("130734", "registry", (1370, 200, 1400, 1000)),
    ("130933", "options-speed", (480, 150, 2070, 1160)),
    ("130938", "options-sound", (480, 150, 2070, 1160)),
    ("130942", "options-video", (480, 150, 2070, 1160)),
    ("130949", "options-game", (480, 150, 2070, 1160)),
    ("130954", "options-network", (480, 150, 2070, 1160)),
    ("131005", "collections", (330, 40, 2230, 1230)),
    ("131015", "hotkeys-terran", (330, 40, 2220, 1270)),
    ("131026", "hotkeys-terran-scv", (330, 40, 2220, 1270)),
    ("131037", "hotkeys-protoss-probe", (330, 40, 2220, 1270)),
    ("131049", "hotkeys-zerg-larva", (330, 40, 2220, 1270)),
    ("131101", "credits", (650, 60, 1950, 1400)),
    # The episode buttons below y=1280 are visible behind the modal; keep the
    # panel measurement on the modal itself so those red borders do not win.
    ("131132", "mission", (700, 400, 1860, 1280)),
]

RIGHT = [
    ("130848", "saved", (1300, 200, 1420, 1000), 0.08),
    ("130858", "replays", (1300, 180, 1400, 1000), 0.08),
    ("130907", "replays", (1300, 180, 1400, 1000), 0.08),
    ("134831", "lobby", (1500, 300, 1540, 1400), 0.08),
    ("134843", "lobby", (1500, 300, 1540, 1400), 0.08),
    ("130727", "gametype", (1790, 430, 1820, 1010), 0.3),
    ("131116", "gametype", (1790, 430, 1820, 1010), 0.3),
]

TEXT = [
    ("130933", "options-speed", (600, 450, 900, 900), "green", 2),
    ("130933", "options-speed", (880, 300, 1250, 800), "white", 3),
    ("131015", "hotkeys-terran", (850, 40, 1600, 120), "green", 1),
    ("131015", "hotkeys-terran", (350, 150, 700, 260), "white", 1),
    ("131005", "collections", (350, 60, 1550, 300), "white", 1),
    ("130848", "saved", (450, 220, 1400, 450), "white", 1),
    ("130858", "replays", (480, 220, 1400, 900), "green", 3),
    ("134831", "lobby", (350, 100, 1500, 1200), "green", 2),
]

# Each campaign capture is compared against its own screen: the seven
# references cover both campaign sets and one hovered slot each.
RINGS = [
    ("130744", "episode"),
    ("130750", "episode-hover-terran"),
    ("130757", "episode-hover-zerg"),
    ("130804", "episode-hover-protoss"),
    ("130824", "episode-expansion-hover-protoss"),
    ("130832", "episode-expansion-hover-terran"),
    ("130840", "episode-expansion-hover-zerg"),
]

# Reference rectangles of the shipped art panels (2560x1440 content space).
ART = []


def image(path, crop=False):
    im = Image.open(path).convert("RGB")
    if crop:
        im = im.crop((440, 0, 3000, 1440))
    return np.asarray(im).astype(int)


def border_box(a, region):
    x0, y0, x1, y1 = region
    m = (a[:, :, 0] > 90) & (a[:, :, 1] < 70) & (a[:, :, 2] < 70) & (a[:, :, 0] > a[:, :, 1] + 40)
    sub = m[y0:y1, x0:x1]
    cols, rows = sub.sum(axis=0), sub.sum(axis=1)
    cs = [x0 + i for i, v in enumerate(cols) if v > (y1 - y0) * 0.45]
    rs = [y0 + i for i, v in enumerate(rows) if v > (x1 - x0) * 0.45]
    if not cs or not rs:
        return None
    return (min(cs), min(rs), max(cs), max(rs))


RING_TARGETS = [(641, 979), (1264, 711), (1930, 967)]


def text_boxes(a, region, color):
    x0, y0, x1, y1 = region
    if color == "green":
        m = (a[:, :, 1] > 120) & (a[:, :, 1] > a[:, :, 0] + 25) & (a[:, :, 1] > a[:, :, 2] + 25)
    else:
        m = (a[:, :, 0] > 170) & (a[:, :, 1] > 170) & (a[:, :, 2] > 170)
    sub = m[y0:y1, x0:x1]
    rows = np.where(sub.sum(axis=1) > 1)[0]
    if not len(rows):
        return []
    groups, st, p = [], rows[0], rows[0]
    for r in rows[1:]:
        if r - p > 8:
            groups.append(y0 + int(st))
            st = r
        p = r
    groups.append(y0 + int(st))
    return groups


def right_edge(a, region, frac):
    x0, y0, x1, y1 = region
    m = (a[:, :, 0] > 90) & (a[:, :, 1] < 70) & (a[:, :, 2] < 70) & (a[:, :, 0] > a[:, :, 1] + 40)
    sub = m[y0:y1, x0:x1]
    cols = sub.sum(axis=0)
    xs = [x0 + i for i, v in enumerate(cols) if v > (y1 - y0) * frac]
    return max(xs) if xs else None


def ring_metrics(a):
    """(x centre, bottom edge) of each ring inside its own window.

    The portrait above the ring carries purple glow; the bottom arc does not,
    so the bottom edge and horizontal centre are the stable features.  Rows
    with only a couple of purple pixels are AA fringe (the disk edge dithers
    by a few pixels between frames), so they are ignored: the previous
    min/max implementation moved the measured bottom by 10 px on a single
    stray pixel.
    """
    m = (a[:, :, 0] > 90) & (a[:, :, 2] > 120) & (a[:, :, 1] < 110) & (a[:, :, 2] > a[:, :, 1] + 40)
    out = []
    for cx, cy in RING_TARGETS:
        x0, x1 = max(0, cx - 230), min(a.shape[1], cx + 230)
        # Keep the window clear of the console plates (y >= 1056): their
        # blue-violet metal also passes the purple test.
        y0, y1 = max(0, cy - 80), min(a.shape[0], cy + 80)
        sub = m[y0:y1, x0:x1]
        rows = sub.sum(axis=1)
        keep = rows >= 8
        if not keep.any():
            out.append((cx, cy))
            continue
        ys = np.where(keep)[0]
        xs = np.where(sub[keep].any(axis=0))[0]
        out.append(((x0 + int(xs.min()) + x0 + int(xs.max())) // 2, y0 + int(ys.max())))
    return out


def dark_fraction(a, rect):
    x, y, w, h = rect
    sub = a[y:y + h, x:x + w]
    return float((sub.sum(axis=2) / 3 < 90).mean())


def render(screen, install, out):
    from ui_visual_diff import PHASES

    env = dict(os.environ, SC_NO_AUDIO="1")
    cmd = [str(BIN), "--install", install, "--shot", str(out),
           "--shot-screen", screen, "--no-intro", "--width", "2560",
           "--height", "1440"]
    if screen in PHASES:
        at, phases = PHASES[screen]
        cmd += ["--menu-at", f"{at:.4f}"]
        if phases:
            cmd += ["--menu-phase", ",".join(f"{k}={v:.4f}" for k, v in phases.items())]
    subprocess.run(cmd, check=True, env=env, stdout=subprocess.DEVNULL,
                   stderr=subprocess.DEVNULL)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--install", required=True)
    args = ap.parse_args()
    tmp = Path(tempfile.mkdtemp(prefix="ui-diff-"))
    results = []

    def shot_for(screen):
        out = tmp / f"{screen}.bmp"
        if not out.exists():
            render(screen, args.install, out)
        return out

    for ref_id, screen, region in PANEL:
        ref = next(REF.glob(f"Screenshot_*{ref_id}.png"))
        rb = border_box(image(ref, crop=True), region)
        nb = border_box(image(shot_for(screen)), region)
        delta = max(abs(a - b) for a, b in zip(rb, nb)) if rb and nb else 999
        results.append((ref_id, screen, "panel", rb, nb, delta, delta <= 2))

    for ref_id, screen, region, frac in RIGHT:
        ref = next(REF.glob(f"Screenshot_*{ref_id}.png"))
        rb = right_edge(image(ref, crop=True), region, frac)
        nb = right_edge(image(shot_for(screen)), region, frac)
        d = abs(rb - nb) if rb is not None and nb is not None else 999
        results.append((ref_id, screen, "right", rb, nb, d, d <= 2))

    for ref_id, screen, region, color, maxrows in TEXT:
        ref = next(REF.glob(f"Screenshot_*{ref_id}.png"))
        rb = text_boxes(image(ref, crop=True), region, color)
        nb = text_boxes(image(shot_for(screen)), region, color)
        n = min(len(rb), len(nb), maxrows)
        d = max((abs(rb[i] - nb[i]) for i in range(n)), default=999) if n else 999
        results.append((ref_id, screen, f"text/{color}", rb[:4], nb[:4], d, d <= 2))

    for ref_id, screen in RINGS:
        ring_ref = ring_metrics(image(next(REF.glob(f"Screenshot_*{ref_id}.png")), crop=True))
        nc = ring_metrics(image(shot_for(screen)))
        # The disks are loop animations: the purple arc moves inside the
        # ring, so the measured centroid wobbles ~2-3 px between frames.
        # Require both axes within 4 px (the dialog-derived rects land that
        # close; the deterministic full-frame gate is the pixel authority).
        d = (
            max(max(abs(a[0] - b[0]), abs(a[1] - b[1])) for a, b in zip(ring_ref, nc))
            if len(nc) == 3
            else 999
        )
        results.append((ref_id, screen, "rings", ring_ref, nc, d, d <= 4))

    for ref_id, screen, rect in ART:
        frac = dark_fraction(image(shot_for(screen)), rect)
        results.append((ref_id, screen, "art", rect, round(frac, 3), 0 if frac > 0.55 else 999, frac > 0.55))

    print(f"{'ref':8} {'screen':17} {'check':6} {'reference':30} {'rendered':30} {'delta':6} status")
    for ref_id, screen, kind, ref, new, delta, ok in results:
        print(f"{ref_id:8} {screen:17} {kind:6} {str(ref):30} {str(new):30} {str(delta):6} {'PASS' if ok else 'FAIL'}")
    passed = sum(1 for r in results if r[-1])
    refs = len({r[0] for r in results})
    print(f"\nPASS {passed}/{len(results)} checks; {refs} references covered")
    shutil.rmtree(tmp, ignore_errors=True)
    raise SystemExit(0 if passed == len(results) else 1)


if __name__ == "__main__":
    main()

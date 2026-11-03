#!/usr/bin/env python3
"""Measure our renders against all 44 retail references and write a report.

Mapping is by position in the sorted reference list (see gap-audit.md).
Writes research/docs/parity-report.md and prints a summary table.
"""
import glob
import os
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
REFS = sorted(glob.glob(str(ROOT / "engine-rust/docs/original_screenshots/*.png")))
SHOTS = Path("/tmp/shots68")

# index (1-based) -> our screen, verified by looking at every reference:
#   1-3   main menu (hover variants)
#   4     "Select Game Type" modal over the menu
#   5     Registry (moon backdrop)
#   6-12  episode select (race portraits, hover and completion variants)
#   13    Saved Games
#   14-15 Create (replays / maps)
#   16    main menu
#   17-21 Options (Speed, Sound, Video, Game, Network)
#   22-25 Hotkeys (Terran, Protoss, Zerg, extra)
#   26    Collections
#   27    Credits
#   28    "Select Game Type" modal
#   29    episode select + "Select starting mission" dialog
#   30-34 in-game Terran (32 has the Game Menu open)
#   35-36 Create / maps with the gametype dropdown
#   37    in-game Zerg
#   38    Mission Objectives panel
#   39    StarCraft Tips panel
#   40-44 in-game (Zerg then Protoss)
MAP = {1: "menu", 2: "menu", 3: "menu", 4: "gametype"}
MAP[5] = "registry"
for i in range(6, 13):
    MAP[i] = "episode"
MAP[13] = "saved"
MAP[14] = "replays"
MAP[15] = "replays"
MAP[16] = "menu"
# Each reference shows a different tab.
for i, name in enumerate(
    ["options-speed", "options-sound", "options-video", "options-game", "options-network"],
    start=17,
):
    MAP[i] = name
# Verified by looking at each capture: 22 is Collections; 23-26 are the
# Hotkeys panel on its Terran / Protoss / Zerg / Zerg tabs.
MAP[22] = "collections"
for i, name in enumerate(
    ["hotkeys-terran", "hotkeys-protoss", "hotkeys-zerg", "hotkeys-zerg"],
    start=23,
):
    MAP[i] = name
MAP[27] = "credits"
MAP[28] = "gametype"
MAP[29] = "episode"
for i in (30, 31):
    MAP[i] = "ingame-terran"
MAP[32] = "gamemenu"
for i in (33, 34):
    MAP[i] = "ingame-terran"
MAP[35] = "replays"
MAP[36] = "replays"
MAP[37] = "ingame-zerg"
MAP[38] = "objectives"
MAP[39] = "tips"
# 40 is a Terran game; 41-44 are the Protoss one.
MAP[40] = "ingame-terran2"
for i in range(41, 45):
    MAP[i] = "ingame-protoss"


def diff(ours: Path, ref: Path) -> float:
    a = Image.open(ours).convert("RGB")
    b = Image.open(ref).convert("RGB")
    if a.size != b.size:
        b = b.resize(a.size, Image.BILINEAR)
    A = np.asarray(a, dtype=np.int16)
    B = np.asarray(b, dtype=np.int16)
    return float(np.abs(A - B).mean())


def main() -> int:
    rows = []
    for i, ref in enumerate(REFS, start=1):
        screen = MAP.get(i)
        ours = SHOTS / f"{screen}.png" if screen else None
        if ours is None or not ours.exists():
            rows.append((i, os.path.basename(ref), screen or "-", None))
            continue
        rows.append((i, os.path.basename(ref), screen, diff(ours, Path(ref))))

    measured = [r for r in rows if r[3] is not None]
    out = ["# Visual parity report (44 references)", ""]
    out.append("Metric: mean absolute per-channel difference (0..255) between our")
    out.append("headless render at 3440x1440 and the matching retail capture.")
    out.append("Lower is better; 0 would be identical (impossible with a different map state).")
    out.append("")
    out.append("| # | Reference | Our screen | mean\\|diff\\| |")
    out.append("|---:|---|---|---:|")
    for i, name, screen, d in rows:
        out.append(f"| {i} | {name} | {screen} | {d:.2f} |" if d is not None
                   else f"| {i} | {name} | {screen} | (no render) |")
    out.append("")
    if measured:
        vals = [d for *_, d in measured]
        out.append(f"Covered: **{len(measured)}/{len(rows)}**. "
                   f"Global mean **{sum(vals)/len(vals):.2f}**, "
                   f"best **{min(vals):.2f}**, worst **{max(vals):.2f}**.")
    (ROOT / "research/docs/parity-report.md").write_text("\n".join(out) + "\n")

    for i, name, screen, d in rows:
        print(f"{i:2d} {screen:<12} {'--' if d is None else f'{d:6.2f}'}  {name}")
    if measured:
        vals = [d for *_, d in measured]
        print(f"\nCOVERED {len(measured)}/{len(rows)}  mean {sum(vals)/len(vals):.2f}  "
              f"worst {max(vals):.2f}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

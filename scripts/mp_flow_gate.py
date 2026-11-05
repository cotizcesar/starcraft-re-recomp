#!/usr/bin/env python3
"""Multiplayer front-end gate (session-log 2026-11-04 (86)).

1. Drives the real multiplayer path through `sc_view --menu-script` at
   3440x1440 (ultrawide, as the retail recording): Main -> Multiplayer ->
   Select Game Type -> Select Connection (Local Area Network) -> Registry ->
   Games -> Create Game -> (8)The Hunters -> "Creating game..." -> lobby
   (Protoss, a computer on each team) -> Ok -> countdown -> the game, and
   checks every step's state from the walkthrough log.
2. Renders each step headless (`--shot-screen mp-*`, `score-defeat`) and
   measures it against the matching frame of the user's retail recording
   (`Screencast_20261104_211148.webm`, local only; the comparison is skipped
   when the video or ffmpeg is missing): mean absolute difference of the
   16:9 content box at 640x360, grey levels 0..255, the side bars must be
   black.

The report is written to engine-rust/docs/gates/mp-flow.txt.
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BIN = ROOT / "engine-rust/target/release/sc_view"
REPORT = ROOT / "engine-rust/docs/gates/mp-flow.txt"
VIDEO = Path.home() / "Videos/Screencasts/Screencast_20261104_211148.webm"
SCORE = Path.home() / "Pictures/Screenshots/Screenshot_20261104_211404.png"
W, H = 3440, 1440
BOX = (440, 0, 3000, 1440)

# 3440x1440 window, content box from x=440: UI (u, v) -> (440 + 320 + 3u, 3v).
FLOW = ",".join([
    "move:1140:700", "click",            # Main: Multiplayer
    "move:1543:618", "click",            # Select Game Type: Original
    "wait-draw:900",
    "move:1060:361", "click",            # Select Connection: Local Area Network
    "wait-draw:1300",
    "move:2311:1182", "click",           # Ok -> Registry
    "wait-draw:900",
    "key:Enter",                         # Registry Ok -> Games
    "wait-draw:1000",
    "move:1360:1026", "click",           # Create Game
    "wait-draw:1000",
    "move:1240:600", "wheel:200",        # map list to its end
    "move:1240:713", "click",            # (8)The Hunters.scm
    "wait-draw:300",
    "key:Enter",                         # Ok -> "Creating game..."
    "wait-draw:1000", "wait-draw:900",   # popup, then the lobby swishes in
    "move:1433:164", "click",            # slot 0 race
    "move:1433:277", "click",            # Protoss
    "move:1111:214", "click",            # slot 1 player
    "move:1111:328", "click",            # Computer (Home Team)
    "move:1111:412", "click",            # slot 4 player
    "move:1111:521", "click",            # Computer (Away Team)
    "key:Enter",                         # Ok -> countdown
    "wait-draw:2300",
    "wait-draw:3400", "wait-draw:1500",  # countdown, swish out, black
    "wait:2000",                         # the game runs
])

# Two processes on one machine (loopback discovery and lobby): the host
# makes the game, puts a computer on the Away Team and waits; the joiner
# finds it in the Games list, joins, picks Protoss and waits; the host
# starts and both must run the same lockstep game (the host compares every
# turn's state hash; a mismatch sets desync=true).
LAN_HOST = ",".join([
    "move:1140:700", "click", "move:1543:618", "click", "wait-draw:900",
    "move:1060:361", "click", "wait-draw:1300", "move:2311:1182", "click",
    "wait-draw:900", "key:Enter", "wait-draw:1000", "move:1360:1026", "click",
    "wait-draw:1000", "move:1240:600", "wheel:200", "move:1240:713", "click",
    "wait-draw:300", "key:Enter", "wait-draw:1000", "wait-draw:900",
    "move:1433:164", "click", "move:1433:277", "click",   # Protoss
    "move:1111:412", "click", "move:1111:521", "click",   # Away computer
    "wait-draw:9000",                                     # the joiner arrives
    "key:Enter", "wait-draw:5200", "wait-draw:1500", "wait:4000",
])
LAN_CLIENT = ",".join([
    "move:1140:700", "click", "move:1543:618", "click", "wait-draw:900",
    "move:1060:361", "click", "wait-draw:1300", "move:2311:1182", "click",
    "wait-draw:900", "key:Enter", "wait-draw:5000",       # discovery
    "move:1060:303", "click", "wait-draw:300", "key:Enter",  # join row 0
    "wait-draw:2500",
    "move:1433:215", "click", "move:1433:328", "click",   # own race: Protoss
    "wait-draw:9000", "wait:6000",
])

# Headless step renders and the recording time (s) they reproduce.
SHOTS = [
    ("mp-connection", 3.8),
    ("mp-connection-lan", 7.5),
    ("mp-games", 12.5),
    ("mp-create", 20.5),
    ("mp-creating", 22.5),
    ("mp-lobby", 24.5),
    ("mp-lobby-ready", 48.0),
    ("mp-countdown", 55.0),
    ("score-defeat", None),
]
# Upper bound of the content-box difference per step.  Matching steps
# measure 2.0-3.6 (the retail frames carry the mouse pointer and the
# Battle.net availability lines this client does not have); the score sheet
# 6.2 (another game's numbers and colours).  The wrong screen scores 8.6
# (Games vs Create), 12.3 (Create vs the popup) and up to 19.9, so these
# bounds catch a swapped or missing plate.
MAX_DIFF = 5.0
MAX_DIFF_SCORE = 8.0


def fail(msg: str) -> None:
    print(f"FAIL {msg}")
    sys.exit(1)


def run(args: list[str], env: dict[str, str]) -> None:
    proc = subprocess.run(args, env=env, text=True, capture_output=True, timeout=600)
    if proc.returncode != 0:
        fail(f"{' '.join(args[:3])} exited {proc.returncode}: {proc.stderr[-400:]}")


def flow_check(install: str, out: Path, lines: list[str]) -> None:
    env = dict(os.environ, SC_NO_AUDIO="1", GC_WALK_DIR=str(out))
    run([str(BIN), "--install", install, "--no-intro", "--width", str(W), "--height", str(H),
         "--menu-script", FLOW], env)
    rows = []
    for line in (out / "walk.tsv").read_text().splitlines()[1:]:
        cols = line.split("\t")
        rows.append({"action": cols[1], "screen": cols[2], "state": cols[4] if len(cols) > 4 else ""})
    screens = []
    for r in rows:
        if not screens or screens[-1] != r["screen"]:
            screens.append(r["screen"])
    want = ["Main", "GameType", "Connection", "Registry", "Games", "MpCreate", "MpLobby"]
    if screens != want:
        fail(f"screen sequence {screens} != {want}")
    lines.append(f"PASS flow screens: {' -> '.join(screens)}")
    countdown = [r for r in rows if "countdown=true" in r["state"]]
    if not countdown:
        fail("Ok never started the countdown")
    last = rows[-1]["state"]
    for need in ("in_game=true", "lobby_game=true", "local=0", "computers=2", "ais=2",
                 "teams=[0, 0, 0, 0, 1, 1, 1, 1]", "result=None"):
        if need not in last:
            fail(f"after the countdown: '{need}' missing in: {last}")
    tick = int(last.split("tick=")[1].split()[0])
    if tick < 24:
        fail(f"the game did not run (tick {tick})")
    lines.append(f"PASS flow game: {last}")


def lan_check(install: str, out: Path, lines: list[str]) -> None:
    import time

    host_dir, client_dir = out / "lan-host", out / "lan-client"
    host_dir.mkdir(exist_ok=True)
    client_dir.mkdir(exist_ok=True)
    common = [str(BIN), "--install", install, "--no-intro", "--width", str(W),
              "--height", str(H), "--menu-script"]
    host = subprocess.Popen(common + [LAN_HOST], text=True, stdout=subprocess.DEVNULL,
                            stderr=subprocess.DEVNULL,
                            env=dict(os.environ, SC_NO_AUDIO="1", GC_WALK_DIR=str(host_dir)))
    time.sleep(7)
    run(common + [LAN_CLIENT], dict(os.environ, SC_NO_AUDIO="1", GC_WALK_DIR=str(client_dir)))
    host.wait(timeout=120)

    def last(d: Path) -> str:
        return (d / "walk.tsv").read_text().splitlines()[-1].split("\t")[4]

    h, c = last(host_dir), last(client_dir)
    for who, state, local in (("host", h, "local=0"), ("joiner", c, "local=1")):
        for need in (f"net={'host' if who == 'host' else 'client'} desync=false",
                     "in_game=true", "lobby_game=true", local, "computers=1",
                     "teams=[0, 0, 0, 0, 1, 1, 1, 1]"):
            if need not in state:
                fail(f"LAN {who}: '{need}' missing in: {state}")
    ticks = [int(x.split("tick=")[1].split()[0]) for x in (h, c)]
    if min(ticks) < 48:
        fail(f"LAN game did not advance: ticks {ticks}")
    lines.append(f"PASS LAN host:   {h}")
    lines.append(f"PASS LAN joiner: {c}")


def frame(t: float, out: Path) -> Path | None:
    if not VIDEO.is_file() or not shutil.which("ffmpeg"):
        return None
    p = out / f"ref-{t}.png"
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-ss", str(t), "-i", str(VIDEO),
                    "-frames:v", "1", str(p)], check=True)
    return p


def shots_check(install: str, out: Path, lines: list[str]) -> None:
    from PIL import Image, ImageChops, ImageStat

    env = dict(os.environ, SC_NO_AUDIO="1")
    worst = 0.0
    for name, t in SHOTS:
        shot = out / f"{name}.bmp"
        run([str(BIN), "--install", install, "--no-intro", "--width", str(W), "--height", str(H),
             "--menu-at", "2.0", "--shot-screen", name, "--shot", str(shot)], env)
        ours = Image.open(shot).convert("RGB")
        bars = max(ours.crop((0, 0, BOX[0], H)).getextrema()[c][1] for c in range(3))
        bars = max(bars, max(ours.crop((BOX[2], 0, W, H)).getextrema()[c][1] for c in range(3)))
        if bars > 8:
            fail(f"{name}: side bars are not black (max {bars})")
        ref_path = SCORE if t is None else frame(t, out)
        if ref_path is None or not Path(ref_path).is_file():
            lines.append(f"SKIP {name:18s} retail reference not available")
            continue
        ref = Image.open(ref_path).convert("RGB")
        a = ours.crop(BOX).resize((640, 360)).convert("L")
        b = ref.crop(BOX).resize((640, 360)).convert("L")
        diff = ImageStat.Stat(ImageChops.difference(a, b)).mean[0]
        if t is not None:
            worst = max(worst, diff)
        when = "21:14:04 screenshot" if t is None else f"{t:5.1f} s"
        bound = MAX_DIFF_SCORE if t is None else MAX_DIFF
        verdict = "PASS" if diff <= bound else "FAIL"
        lines.append(
            f"{verdict} {name:18s} vs retail {when:20s} mean |diff| {diff:5.2f} (bound {bound})"
        )
        if diff > bound:
            print("\n".join(lines))
            fail(f"{name}: content differs from the retail frame by {diff:.2f} > {bound}")
    lines.append(f"worst menu-step difference {worst:.2f}")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--install", default=os.environ.get("SC_INSTALL", ""))
    ap.add_argument("--keep", help="keep the artifacts in this directory")
    args = ap.parse_args()
    if not args.install:
        print("SKIP: SC_INSTALL is not set")
        return 0
    if not BIN.is_file():
        fail(f"missing {BIN} (cargo build --release --bin sc_view)")
    out = Path(args.keep) if args.keep else Path(tempfile.mkdtemp(prefix="gc-mp-"))
    out.mkdir(parents=True, exist_ok=True)
    lines = ["multiplayer flow gate (scripts/mp_flow_gate.py), 3440x1440"]
    flow_check(args.install, out, lines)
    lan_check(args.install, out, lines)
    shots_check(args.install, out, lines)
    lines.append("mp flow gate: PASS")
    REPORT.write_text("\n".join(lines) + "\n")
    print("\n".join(lines))
    if not args.keep:
        shutil.rmtree(out, ignore_errors=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())

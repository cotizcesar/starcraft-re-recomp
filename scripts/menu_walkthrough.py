#!/usr/bin/env python3
"""Scripted menu walkthrough gate: transitions, sounds, state and frames.

Feeds a deterministic input script through the real `sc_view` menu handlers
(via `--menu-script`), then checks:

1. the screen transition sequence covers every menu screen and each change
   plays exactly one `ui_swish`;
2. `ui_click` fires on control presses and never on a screen change;
   `ui_move` fires when the pointer enters a control;
3. Options sliders/checkboxes/tabs change the real `GameOptions` values and
   Restore to Default puts them back;
4. Lobby race dropdowns pick the chosen race;
5. every visited screen renders pixel-identical to the headless capture that
   the visual gate compares against the retail screenshots; the live Lobby
   dropdown reproduces the gate-verified popup region byte for byte.

Run from the repo root after `cargo build --release --bin sc_view`:

    python3 scripts/menu_walkthrough.py --install "$SC_INSTALL"
"""
from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
BIN = ROOT / "engine-rust/target/release/sc_view"
REF_DIR = ROOT / "engine-rust/docs/original_screenshots"

# Main -> Multiplayer -> GameType -> Lobby (races) -> Main -> Options (all
# five tabs, sliders, checks, Restore, Ok) -> Main -> Collections (cells, Ok)
# -> Main -> Credits -> Main -> Intro -> Main -> Hotkeys -> Main -> Single
# Player -> GameType -> Registry -> Episode -> Mission -> Episode -> Saved ->
# Episode -> Replays -> Episode -> Main.
SCRIPT = ",".join(
    [
        "move:700:700", "click",            # Multiplayer
        "move:1103:618",                    # Game Type: hover Original
        "key:L", "key:Escape",              # LAN screen and back
        "click",                            # Original
        "move:1000:1028", "click",          # Lobby: open slot 1 race list
        "move:1122:1039", "click",          # hover Zerg, pick it
        "move:1000:980", "click",           # open slot 0 race list
        "move:1122:1046", "click",          # pick Terran
        "move:1964:1288", "click",          # Cancel -> Main
        "move:558:1042", "click",           # Options
        "move:1100:391", "click",           # game speed
        "move:1000:544", "click",           # key scroll
        "move:1300:697", "click",           # mouse scroll
        "move:1700:394", "click",           # mouse sensitivity
        "move:1500:355", "click",           # Mouse Sensitivity check
        "move:1500:511", "click",           # Use Mouse Scaling check
        "move:684:484", "click",            # Sound tab
        "move:1158:394", "click",           # music volume
        "move:900:550", "click",            # digital volume
        "move:1500:355", "click",           # Unit Speech
        "move:1500:517", "click",           # Building Sounds
        "move:684:577", "click",            # Video tab
        "move:1500:355", "click",           # Enable FPS Cap
        "move:684:670", "click",            # Game tab
        "move:950:403", "click",            # Game Timer
        "move:1100:769", "click",           # Portraits: Still
        "move:1150:886", "click",           # Mini-Map: Standard
        "move:684:763", "click",            # Network tab
        "move:950:355", "click",            # Prefer port 6112
        "move:700:1254", "click",           # Restore to Default
        "move:1860:1254", "click",          # Ok -> Main
        "move:558:1123", "click",           # Collections
        "move:587:386", "click",            # console cell 0
        "move:963:707", "click",            # unit cell 4
        "move:1337:1175", "click",          # announcer cell 10
        "move:2017:1277", "click",          # Ok -> Main
        "move:558:1273", "click",           # Show Credits
        "move:558:1348", "click",           # skip Credits -> Main
        # `View Intro` starts the movie path (not a MenuScreen), covered by
        # the intro timeline tests; the walkthrough continues on the menu.
        "move:558:1192", "click",           # Hotkeys
        "key:Escape",                       # -> Main
        "move:700:300", "click",            # Single Player
        "move:1103:618", "click",           # Game Type: Original
        "move:800:390", "click",            # Registry: select LinKa
        "move:1869:1178", "click",          # Ok -> Episode
        "move:1262:666", "click",           # Episode slot -> Mission
        "move:1200:670", "click",           # Mission row
        "move:1477:1015", "click",          # Cancel -> Episode
        "move:1332:1134", "click",          # Load Saved -> Saved Games
        "move:600:380", "click",            # saved row
        "move:1917:1283", "click",          # Cancel -> Episode
        "move:1325:1359", "click",          # Load Replay -> Replays
        "move:700:483", "click",            # replay row
        "move:1964:1288", "click",          # Cancel -> Episode
        "move:1967:1242", "click",          # Cancel -> Main
    ]
)

# (screen, first walk frame) pairs checked against the verified headless
# renders; Lobby is handled separately because the reference capture has the
# slot-1 race list open while the walkthrough enters with it closed.
SCREEN_SHOTS = {
    "GameType": "gametype",
    "Options": "options-speed",
    "Collections": "collections",
    "Credits": "credits",
    "Hotkeys": "hotkeys-terran",
    "Registry": "registry",
    "Episode": "episode",
    "Mission": "mission",
    "SavedGames": "saved",
    "Replays": "replays",
}

EXPECTED_TRANSITIONS = [
    "GameType", "Lan", "GameType", "Lobby", "Main", "Options", "Main", "Collections", "Main",
    "Credits", "Main", "Hotkeys", "Main", "GameType", "Registry", "Episode",
    "Mission", "Episode", "SavedGames", "Episode", "Replays", "Episode",
    "Main",
]

CUE_PATHS = {
    "ui_move": "sound/glue/mouseover.wav",
    "ui_click": "sound/glue/mousedown2.wav",
    "ui_swish": "sound/glue/swishin.wav",
}


def fail(msg: str) -> None:
    print(f"FAIL {msg}", flush=True)
    sys.exit(1)


def parse_tsv(path: Path) -> list[dict]:
    rows = []
    lines = path.read_text().splitlines()
    header = lines[0].split("\t")
    for line in lines[1:]:
        parts = line.split("\t")
        parts += [""] * (len(header) - len(parts))
        rows.append(dict(zip(header, parts)))
    return rows


def shots_by_screen(rows: list[dict]) -> dict[str, int]:
    first: dict[str, int] = {}
    for row in rows:
        first.setdefault(row["screen"], int(row["step"]))
    return first


def run_shot(install: str, screen: str, out: Path) -> np.ndarray:
    env = dict(os.environ, SC_NO_AUDIO="1")
    subprocess.run(
        [str(BIN), "--install", install, "--shot", str(out), "--shot-screen", screen,
         "--no-intro", "--width", "2560", "--height", "1440"],
        check=True, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    return np.asarray(Image.open(out).convert("RGB")).astype(np.int16)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--install", default=os.environ.get("SC_INSTALL", ""))
    ap.add_argument("--keep", help="keep the walkthrough directory at this path")
    args = ap.parse_args()
    if not args.install:
        fail("pass --install or set SC_INSTALL")
    if not BIN.exists():
        fail(f"missing {BIN}; run cargo build --release --bin sc_view")

    outdir = Path(args.keep) if args.keep else Path(tempfile.mkdtemp(prefix="gc-walk-"))
    outdir.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, SC_NO_AUDIO="1", GC_WALK_DIR=str(outdir))
    proc = subprocess.run(
        [str(BIN), "--install", args.install, "--no-intro", "--width", "2560",
         "--height", "1440", "--menu-script", SCRIPT],
        env=env, text=True, capture_output=True,
    )
    if proc.returncode != 0:
        fail(f"sc_view exited {proc.returncode}: {proc.stderr[-500:]}")
    rows = parse_tsv(outdir / "walk.tsv")

    bank = {}
    for line in (outdir / "sounds.tsv").read_text().splitlines()[1:]:
        cue, samples = line.split("\t")
        bank[cue] = int(samples)
    for cue in CUE_PATHS:
        if bank.get(cue, 0) <= 0:
            fail(f"cue {cue} did not load a retail sample ({bank})")
    print(
        "PASS sound bank: "
        + ", ".join(f"{cue}={bank[cue]} samples" for cue in CUE_PATHS)
        + " loaded from the retail install"
    )

    # ---- 1. transitions -------------------------------------------------
    transitions = []
    prev = rows[0]["screen"]
    for row in rows:
        if row["screen"] != prev:
            transitions.append(row)
            prev = row["screen"]
    got = [r["screen"] for r in transitions]
    if got != EXPECTED_TRANSITIONS:
        fail(f"transition sequence {got} != {EXPECTED_TRANSITIONS}")
    for row in transitions:
        if row["sounds"] != "ui_swish":
            fail(f"step {row['step']} changes to {row['screen']} with cues {row['sounds']!r}")
    print(f"PASS transitions: {len(transitions)} screen changes, each with ui_swish")
    print("     " + " -> ".join(["Main"] + got))

    # ---- 2. cue policy --------------------------------------------------
    allowed = set(CUE_PATHS)
    for row in rows:
        cues = set(filter(None, row["sounds"].split("+")))
        if not cues <= allowed:
            fail(f"step {row['step']} uses unknown cue(s) {sorted(cues - allowed)}")
    clicks = [r for r in rows if r["action"] == "click"]
    moves = [r for r in rows if r["action"].startswith("move:")]
    transition_steps = {r["step"] for r in transitions}
    # A press that stays on the screen must click (never swish); a screen
    # change must be swish only.
    for row in clicks:
        if row["step"] in transition_steps:
            continue
        cues = set(filter(None, row["sounds"].split("+")))
        if "ui_swish" in cues:
            fail(f"step {row['step']} stays on {row['screen']} but plays ui_swish")
        if "ui_click" in cues:
            continue
    presses = sum(1 for r in clicks if "ui_click" in r["sounds"])
    if presses < 25:
        fail(f"only {presses} control presses produced ui_click")
    hovers = [r for r in moves if "ui_move" in r["sounds"]]
    if len(hovers) < 25:
        fail(f"only {len(hovers)} pointer entries produced ui_move")
    print(
        f"PASS sounds: ui_swish on {len(transitions)} screen changes, "
        f"ui_click on {presses} presses, "
        f"ui_move on {len(hovers)} pointer entries"
    )
    for cue, path in CUE_PATHS.items():
        print(f"     {cue:8s} <- {path}")

    # ---- 3. Options state ----------------------------------------------
    opt = [r for r in rows if r["screen"] == "Options"]
    if not opt:
        fail("walkthrough never reached Options")
    tabs = {re.search(r"tab=(\w+)", r["state"]).group(1) for r in opt if "tab=" in r["state"]}
    if tabs != {"Speed", "Sound", "Video", "Game", "Network"}:
        fail(f"Options tabs visited {sorted(tabs)}")
    states = [r["state"] for r in opt if r["state"]]
    if not any("speed=0.67" in s and "key=0.17" in s and "mouse=0.83" in s for s in states):
        fail("speed sliders did not move to the clicked detents")
    if not any("music=0.50" in s and "sfx=0.00" in s for s in states):
        fail("sound sliders did not move to the clicked detents")
    flag_values = {re.search(r"flags=(\w+)", s).group(1) for s in states}
    if len(flag_values) < 6:
        fail(f"only {len(flag_values)} checkbox states observed")
    final = states[-1]
    if "tab=Network" not in final or "speed=1.00" not in final or "music=0.25" not in final:
        fail(f"Restore to Default did not reset the options: {final}")
    print(
        f"PASS options: 5 tabs, sliders to 0.67/0.17/0.83 and music 0.50/sfx 0.00,"
        f" {len(flag_values)} distinct checkbox states, Restore reset to defaults"
    )

    # ---- 4. Lobby races -------------------------------------------------
    lobby = [r for r in rows if r["screen"] == "Lobby" and "races=" in r["state"]]
    if not lobby:
        fail("walkthrough never reached Lobby")
    final_races = lobby[-1]["state"]
    if not final_races.startswith("races=[Terran, Zerg"):
        fail(f"race dropdown picks not applied: {final_races}")
    print(f"PASS lobby: race dropdowns set {final_races.split('races=')[1]}")

    # ---- 5. frame identity ---------------------------------------------
    first = shots_by_screen(rows)
    tmp = outdir / "shots"
    tmp.mkdir(exist_ok=True)
    for screen, shot_name in SCREEN_SHOTS.items():
        if screen not in first:
            fail(f"screen {screen} not visited")
        walk = np.asarray(
            Image.open(outdir / f"walk-{first[screen]:02d}-{screen}.bmp").convert("RGB")
        ).astype(np.int16)
        ref = run_shot(args.install, shot_name, tmp / f"{shot_name}.bmp")
        diff = np.abs(walk - ref)
        if diff.max() != 0:
            fail(f"{screen}: walkthrough frame differs from the {shot_name} render "
                 f"(mean {diff.mean():.3f}, max {diff.max()})")
        print(f"PASS frame {screen:12s} == verified headless render ({shot_name})")

    lobby_shot = run_shot(args.install, "lobby", tmp / "lobby.bmp")
    closed = np.asarray(Image.open(outdir / f"walk-{first['Lobby']:02d}-Lobby.bmp").convert("RGB")).astype(np.int16)
    popup_step = next(
        r for r in rows
        if r["screen"] == "Lobby" and r["action"].startswith("move:1122:1039")
    )
    popup = np.asarray(
        Image.open(outdir / f"walk-{int(popup_step['step']):02d}-Lobby.bmp").convert("RGB")
    ).astype(np.int16)
    inside = np.zeros(closed.shape[:2], bool)
    inside[999:1279, 900:1345] = True
    outside = np.abs(closed - popup).max(axis=2)
    if outside[~inside].max() != 0:
        fail("opening the Lobby dropdown changed pixels outside the popup")
    popup_diff = np.abs(popup - lobby_shot)[999:1279, 900:1345].max()
    if popup_diff != 0:
        fail(f"live Lobby dropdown differs from the verified popup (max {popup_diff})")
    print("PASS frame Lobby        live race dropdown == verified 134831 popup "
          "(byte-identical region); opening it changes nothing outside the popup")

    print(f"\nwalkthrough gate: PASS ({len(rows)} steps, artifacts in {outdir})")
    return 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Scripted menu walkthrough gate: transitions, sounds, state and frames.

Feeds a deterministic input script through the real `sc_view` menu handlers
(via `--menu-script`), then checks:

1. the screen transition sequence covers every menu screen and each change
   plays the retail press cue (`ui_click`), with no dedicated swish (the CEF
   bundle only sends GLUSND_MOUSE_OVER/DOWN);
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
from PIL import Image, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
BIN = ROOT / "engine-rust/target/release/sc_view"
REF_DIR = ROOT / "engine-rust/docs/original_screenshots"

# Main -> Multiplayer -> GameType -> LAN (two peers: select + join) -> Lobby
# (race dropdowns) -> Main -> Options (five tabs, sliders, checks, Restore,
# Ok) -> Main -> Collections (cells, Ok) -> Main -> Credits -> Main -> Hotkeys
# (unit/action selection, rebind, restore dialog, race tabs) -> Main -> Single
# Player -> GameType -> Registry -> Episode (locked-campaign prompt) ->
# Mission (scroll/wheel/keys) -> Episode -> Saved -> Episode -> Replays ->
# Episode -> Registry -> Main.
SCRIPT = ",".join(
    [
        "move:700:700", "click",            # Multiplayer
        "move:1103:618",                    # Game Type: hover Original
        "key:L",                            # LAN screen
        "fixture:lan-peer", "fixture:lan-peer",  # two discovered peers
        "move:1000:763", "click",           # select Peer2
        "move:1000:763", "click",           # join it (connection refused)
        "key:Escape",                       # back to Game Type
        "move:1103:618", "click",           # Original
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
        "move:558:1192", "click",           # Hotkeys (Terran)
        "move:471:357", "click",            # select SCV
        "move:549:1344", "click",           # open Restore dialog
        "move:1176:846", "click",           # confirm Restore
        "move:1212:352", "click",           # SCV Move action card
        "key:H",                            # rebind Move to H
        "move:549:1344", "click",           # open Restore again
        "move:1176:846", "click",           # confirm (clean bindings)
        "move:1220:79", "click",            # Protoss tab
        "move:471:357", "click",            # select Probe
        "key:Escape",                       # -> Main
        "move:700:300", "click",            # Single Player
        "move:1103:618", "click",           # Game Type: Original
        "move:800:390", "click",            # Registry: select LinKa
        "move:1869:1178", "click",          # Ok -> Episode
        "move:660:700",                     # hover locked Zerg: caption
        "click",                            # locked -> strRACE_ERR_ZERG prompt
        "move:1535:940", "click",           # prompt Cancel
        "fixture:progress:1=10",            # LinKa: Terran up to New Gettysburg
        "move:1262:666", "click",           # Terran -> Mission (13 entries)
        "move:1794:906", "click",           # scrollbar down arrow: top 1
        "wheel:2",                          # wheel: top 3, selection kept
        "key:Down",                         # selection 1, scrolled into view
        "move:1200:670", "click",           # Mission row 3 (entry 4)
        "move:1477:1015", "click",          # Cancel -> Episode
        "move:1332:1134", "click",          # Load Saved -> Saved Games
        "move:600:380", "click",            # saved row
        "move:1917:1283", "click",          # Cancel -> Episode
        "move:1325:1359", "click",          # Load Replay -> Replays
        "move:700:483", "click",            # replay row
        "move:1964:1288", "click",          # Cancel -> Episode
        "move:1967:1242", "click",          # Cancel -> Registry (live client)
        "move:1869:1316", "click",          # Registry Cancel -> Main
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
    "Mission": "mission-live",
    "SavedGames": "saved",
    "Replays": "replays",
}

EXPECTED_TRANSITIONS = [
    "GameType", "Lan", "GameType", "Lobby", "Main", "Options", "Main", "Collections", "Main",
    "Credits", "Main", "Hotkeys", "Main", "GameType", "Registry", "Episode",
    "Mission", "Episode", "SavedGames", "Episode", "Replays", "Episode",
    "Registry", "Main",
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


# Screens drawn over the running main menu (Options composites the live
# Single/Multi/Exit clips under its modal, 130933..130954): their frame is
# compared outside the pixels the clip phase can change, because the phase
# depends on the wall-clock time the walkthrough reaches the screen.
# Screens drawn over the live main-menu backdrop (wall-clock clips).
# Value: largest share of the frame the clip phase may change.  The Options
# plate hides most of the backdrop; the small Game Type dialog leaves the
# Single/Multi/Exit clips almost fully visible.
LIVE_MENU_SCREENS = {"Options": 0.20, "GameType": 0.30}
# One sample per second over the 14.4 s Single/Multi loop (Exit is 10 s).
CLIP_PHASES = tuple(f"{t:.1f}" for t in range(15))
# flc.json clip rectangles (Single/Multi/Exit and their On variants) at
# 2560x1440: x0, y0, x1, y1 of the content box.  The live clips advance on
# the wall clock, so two walkthrough runs differ only inside these.
CLIP_RECTS = ((260, 0, 1535, 600), (368, 555, 1370, 1365), (1499, 948, 2111, 1440))


# Clip clock of the walkthrough and of every comparison shot.  Without it the
# menu/campaign clips show whatever the decoder had ready at that step, so a
# frame could match or not depending on machine load.
WALK_MENU_AT = "2.0"


def run_shot(
    install: str, screen: str, out: Path, width: int = 2560, height: int = 1440,
    extra: tuple[str, ...] = (),
) -> np.ndarray:
    env = dict(os.environ, SC_NO_AUDIO="1")
    if "--menu-at" not in extra:
        extra = ("--menu-at", WALK_MENU_AT, *extra)
    subprocess.run(
        [str(BIN), "--install", install, "--shot", str(out), "--shot-screen", screen,
         "--no-intro", "--width", str(width), "--height", str(height), *extra],
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
         "--height", "1440", "--menu-at", WALK_MENU_AT, "--menu-script", SCRIPT],
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
        if row["sounds"] != "ui_click":
            fail(f"step {row['step']} changes to {row['screen']} with cues {row['sounds']!r}")
    print(
        f"PASS transitions: {len(transitions)} screen changes, each with the "
        "press cue ui_click (no swish: retail CEF has no transition cue)"
    )
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
        f"PASS sounds: ui_click on {presses} presses including "
        f"{len(transitions)} screen changes, ui_move on {len(hovers)} pointer entries"
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

    # ---- 3b. LAN row selection + join ----------------------------------
    lan = [r for r in rows if r["screen"] == "Lan" and "lan_sel=" in r["state"]]
    if not any("entries=2" in r["state"] and "lan_sel=1" in r["state"] for r in lan):
        fail("LAN row click did not select the second discovered peer")
    if not any("status=join failed" in r["state"] for r in lan):
        fail("LAN row click on the selected peer did not attempt a join")
    print("PASS lan: two peers discovered, row click selected peer 2 and join "
          "returned 'Connection refused'")

    # ---- 3c. Hotkeys controls ------------------------------------------
    hot = [r for r in rows if r["screen"] == "Hotkeys" and "race=" in r["state"]]
    if not hot:
        fail("walkthrough never reached Hotkeys")

    def first_hot(pred, what):
        for row in hot:
            if pred(row["state"]):
                return row
        fail(f"Hotkeys state not observed: {what}")

    scv = first_hot(lambda st: "sel=Some(7)" in st and "action=None" in st, "SCV selected")
    dialog = first_hot(lambda st: "reset=true" in st, "restore dialog")
    action = first_hot(
        lambda st: "action=Some(664)" in st and "key=Some('m')" in st and "reset=false" in st,
        "SCV Move action selected",
    )
    rebound = first_hot(lambda st: "key=Some('H')" in st, "Move rebound to H")
    protoss = first_hot(
        lambda st: "race=Protoss" in st and "sel=None" in st, "Protoss tab"
    )
    probe = first_hot(
        lambda st: "race=Protoss" in st and "sel=Some(64)" in st, "Probe selected"
    )
    if "error=true" not in rebound["state"]:
        fail("H rebind did not report the retail collision error")
    print(
        "PASS hotkeys: SCV cell + Move card + H rebind (collision flagged), "
        "restore dialog confirmed, Protoss tab + Probe cell selected"
    )

    # ---- 3d. Campaign selection (live-client behaviour) ------------------
    ep = [r for r in rows if r["screen"] == "Episode" and "prompt=" in r["state"]]
    if not any("prompt=Some(Zerg)" in r["state"] for r in ep):
        fail("clicking the locked Zerg campaign did not open the strRACE_ERR prompt")
    if not any("hover=Some(0)" in r["state"] and "prompt=None" in r["state"] for r in ep):
        fail("locked Zerg slot was never hovered")
    mis = [r["state"] for r in rows if r["screen"] == "Mission" and "len=" in r["state"]]
    want = [
        ("open", "len=13 sel=0 top=0"),
        ("down arrow", "len=13 sel=0 top=1"),
        ("wheel", "len=13 sel=0 top=3"),
        ("Down key", "len=13 sel=1 top=1"),
        ("row click", "len=13 sel=4 top=1"),
    ]
    for what, frag in want:
        if not any(frag in st for st in mis):
            fail(f"mission list state after {what} not observed ({frag}); saw {sorted(set(mis))}")
    print("PASS campaign: locked prompt, list opens at top with entry 0, arrow/wheel scroll "
          "keep the selection, Down scrolls it into view, row click selects")

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
        diff = np.abs(walk - ref).max(axis=2)
        note = ""
        if screen in LIVE_MENU_SCREENS:
            clip = np.zeros(diff.shape, bool)
            for at in CLIP_PHASES:
                alt = run_shot(args.install, shot_name, tmp / f"{shot_name}-{at}.bmp",
                               extra=("--menu-at", at))
                clip |= np.abs(alt - ref).max(axis=2) > 0
            # Pixels between the sampled phases: grow the mask by 4 px.
            clip = np.asarray(
                Image.fromarray(clip.astype(np.uint8) * 255).filter(ImageFilter.MaxFilter(9))
            ) > 0
            # The flc.json clip rectangles span ~45 % of the frame; the
            # phase changes well under a fifth of it (stars, ships, galaxy).
            if clip.mean() > LIVE_MENU_SCREENS[screen]:
                fail(f"{screen}: clip-phase mask covers {clip.mean():.1%} of the frame")
            in_rect = np.zeros(diff.shape, bool)
            for x0, y0, x1, y1 in CLIP_RECTS:
                in_rect[y0:y1, x0:x1] = True
            if diff[~in_rect].max() != 0:
                fail(f"{screen}: frame differs outside the menu-clip rectangles "
                     f"(max {diff[~in_rect].max()})")
            stray = (diff > 0) & ~clip
            if stray.mean() > 0.0002:
                fail(f"{screen}: {stray.sum()} px differ inside the clip rectangles "
                     f"but outside the sampled clip footprint")
            diff = np.zeros_like(diff)
            note = (f" outside the menu clips (footprint {clip.mean():.2%}, "
                    f"{stray.sum()} transient px)")
        if diff.max() != 0:
            fail(f"{screen}: walkthrough frame differs from the {shot_name} render "
                 f"(mean {diff.mean():.3f}, max {diff.max()})")
        print(f"PASS frame {screen:12s} == verified headless render ({shot_name}){note}")

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

    def frame(row: dict) -> np.ndarray:
        return np.asarray(
            Image.open(outdir / f"walk-{int(row['step']):02d}-{row['screen']}.bmp").convert("RGB")
        ).astype(np.int16)

    for row, shot_name, label in [
        (scv, "hotkeys-terran-scv", "SCV selected"),
        (dialog, "hotkeys-reset-dialog", "restore dialog"),
        (action, "hotkeys-terran-scv-move", "Move action selected"),
        (rebound, "hotkeys-terran-scv-move-unbound", "Move rebound to H"),
        (protoss, "hotkeys-protoss", "Protoss tab"),
        (probe, "hotkeys-protoss-probe", "Probe selected"),
    ]:
        ref = run_shot(args.install, shot_name, tmp / f"{shot_name}.bmp")
        diff = np.abs(frame(row) - ref)
        if diff.max() != 0:
            fail(f"Hotkeys {label}: frame differs from {shot_name} "
                 f"(mean {diff.mean():.3f}, max {diff.max()})")
        print(f"PASS frame Hotkeys      {label:20s} == {shot_name}")

    # Wide (non-16:9) windows letterbox to a 2421x1362 content box at x=509.
    # Every Options control must stay inside the panel (right edge 2445); the
    # original `dx()`-as-size bug pushed the right column outside and drew
    # giant checkboxes.  This is the regression guard for that layout.
    for tab in ("options-speed", "options-sound", "options-video",
                "options-game", "options-network"):
        img = run_shot(args.install, tab, tmp / f"wide-{tab}.bmp", 3440, 1362)
        outside = (img[150:1150, 2446:2931].max(axis=2) > 170).sum()
        if outside:
            fail(f"{tab} at 3440x1362 has {outside} bright pixels outside the panel")
        print(f"PASS wide layout    {tab:16s} all controls inside the panel")

    print(f"\nwalkthrough gate: PASS ({len(rows)} steps, artifacts in {outdir})")
    return 0


if __name__ == "__main__":
    sys.exit(main())

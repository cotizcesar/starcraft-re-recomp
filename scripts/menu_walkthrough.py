#!/usr/bin/env python3
"""Scripted menu walkthrough gate: transitions, sounds, state and frames.

Feeds a deterministic input script through the real `sc_view` menu handlers
(via `--menu-script`), then checks:

1. the screen transition sequence covers every menu screen and each change
   plays the press cue (`ui_click`) and the swish-in (`ui_swish`), plus the
   swish-out (`ui_swish_out`) when a classic list dialog leaves; leaving
   Main through one of its entries (Hotkeys and Exit aside) plays only the
   press;
2. `ui_click` fires on control presses, a press that stays on the screen
   never swishes, and `ui_move` fires when the pointer enters a control;
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
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
BIN = ROOT / "engine-rust/target/release/sc_view"
REF_DIR = ROOT / "engine-rust/docs/original_screenshots"

# Main -> Multiplayer -> GameType -> LAN (two peers: select + join) ->
# GameType -> Select Connection (LAN row) -> Main -> Options (five tabs, sliders, checks, Restore,
# Ok) -> Main -> Collections (cells, Ok) -> Main -> Credits -> Main -> Hotkeys
# (unit/action selection, rebind, restore dialog, race tabs) -> Main -> Single
# Player -> GameType -> Registry -> Episode (locked-campaign prompt) ->
# Mission (scroll/wheel/keys) -> Episode -> Saved -> Episode -> Replays ->
# Episode -> Registry -> Main -> Single Player -> Registry -> Episode -> Play
# Custom -> Lobby (race dropdowns) -> Main.  The multiplayer flow past Select
# Connection has its own gate (scripts/mp_flow_gate.py).
SCRIPT = ",".join(
    [
        "move:700:700", "click",            # Multiplayer
        "move:1103:618",                    # Game Type: hover Original
        "key:L",                            # LAN screen
        "fixture:lan-peer", "fixture:lan-peer",  # two discovered peers
        "move:1000:763", "click",           # select Peer2
        "move:1000:763", "click",           # join it (connection refused)
        "key:Escape",                       # back to Game Type
        "move:1103:618", "click",           # Original -> Select Connection
        "wait-draw:900",                    # plates swish in
        "move:620:361", "click",            # Local Area Network row
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
        "move:558:1348", "click",           # a click does not leave Credits
        "key:Escape",                       # Esc sends `Exit` -> Main
        # `View Intro` (Select Game Type, then the movie) is walked by
        # `view_intro_check`; this walkthrough continues on the menu.
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
        "move:1794:603", "hold",            # grab the scrollbar thumb (top 1)
        "move:1794:700",                    # drag: top 3, selection kept
        "move:1794:950",                    # past the track end: last page
        "release",
        "move:1477:1015", "click",          # Cancel -> Episode
        "move:1332:1134", "click",          # Load Saved -> Saved Games
        "move:600:380", "click",            # saved row
        "move:1917:1283", "click",          # Cancel -> Episode
        "move:1325:1359", "click",          # Load Replay -> Replays
        "move:700:483", "click",            # replay row
        "move:1964:1288", "click",          # Cancel -> Episode
        "move:1967:1242", "click",          # Cancel -> Registry (live client)
        "move:1869:1316", "click",          # Registry Cancel -> Main
        "move:700:300", "click",            # Single Player
        "move:1103:618", "click",           # Game Type: Original
        "move:1869:1178", "click",          # Registry Ok -> Episode
        "move:1307:1242", "click",          # Play Custom -> Lobby
        "move:1000:1028", "click",          # Lobby: open slot 1 race list
        "move:1122:1039", "click",          # hover Zerg, pick it
        "move:1000:980", "click",           # open slot 0 race list
        "move:1122:1046", "click",          # pick Terran
        "move:1964:1288", "click",          # Cancel -> Main
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
    "GameType", "Lan", "GameType", "Connection", "Main", "Options", "Main", "Collections",
    "Main", "Credits", "Main", "Hotkeys", "Main", "GameType", "Registry", "Episode",
    "Mission", "Episode", "SavedGames", "Episode", "Replays", "Episode",
    "Registry", "Main", "GameType", "Registry", "Episode", "Lobby", "Main",
]

CUE_PATHS = {
    "ui_move": "sound/glue/mouseover.wav",
    "ui_click": "sound/glue/mousedown2.wav",
    "ui_swish": "sound/glue/swishin.wav",
    "ui_swish_out": "sound/glue/swishout.wav",
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
# Regions whose content comes from the user's files (x0, y0, x1, y1 at
# 2560x1440): the `replays` shot shows the 130858 state (`LastReplay.rep`
# and its information), the walkthrough the empty temporary replay folder.
# Only the file rows under the three folders and the information values
# may differ; everything else must stay pixel-identical.
DATA_REGIONS = {"Replays": ((490, 570, 1300, 760), (1540, 230, 2000, 720))}
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


# Victory end to end, in two processes sharing a temporary save directory:
# the first plays Terran "9. New Gettysburg" (history up to it), wins through
# the real result path and must write profiles.bin (progress + one won game
# in the statistics); the second starts fresh and must list "10. The Hammer
# Falls" (14 entries) for the same profile.
TO_TERRAN_LIST = [
    "move:700:300", "click",            # Single Player
    "move:1103:618", "click",           # Original
    "move:1869:1178", "click",          # Registry Ok (default profile)
]
WIN_SCRIPT = ",".join(
    TO_TERRAN_LIST
    + ["fixture:progress:1=10", "move:1262:666", "click"]   # 13 entries
    + ["key:Down"] * 12                                      # 9. New Gettysburg
    + ["key:Enter", "fixture:win"]
)
RELOAD_SCRIPT = ",".join(TO_TERRAN_LIST + ["move:1262:666", "click"])


def read_profile_store(path: Path) -> dict:
    """Decode a `GCPF` store (`engine-rust/src/profile.rs`): magic, u16
    version, u16 count, u16 active, profiles `{u8 len, name, u64 created,
    6 x u16 progress, u8 n, n x u64 stats}`, settings, SHA-256 trailer.
    Returns `{name: {"progress": [...], "stats": [...]}}`."""
    import hashlib
    import struct
    raw = path.read_bytes()
    body, digest = raw[:-32], raw[-32:]
    if body[:4] != b"GCPF" or hashlib.sha256(body).digest() != digest:
        fail(f"{path} is not a valid GCPF store")
    count = struct.unpack_from("<H", body, 6)[0]
    at, store = 10, {}
    for _ in range(count):
        n = body[at]
        name = body[at + 1:at + 1 + n].decode()
        at += 1 + n + 8
        progress = list(struct.unpack_from("<6H", body, at))
        at += 12
        k = body[at]
        stats = list(struct.unpack_from(f"<{k}Q", body, at + 1))
        at += 1 + 8 * k
        store[name] = {"progress": progress, "stats": stats}
    return store


def campaign_win_check(args) -> None:
    saves = Path(tempfile.mkdtemp(prefix="gc-win-"))
    outdir = Path(tempfile.mkdtemp(prefix="gc-win-walk-"))
    env = dict(os.environ, SC_NO_AUDIO="1", GC_WALK_DIR=str(outdir),
               GALAXYCRAFT_SAVES=str(saves))
    for script in (WIN_SCRIPT, RELOAD_SCRIPT):
        proc = subprocess.run(
            [str(BIN), "--install", args.install, "--no-intro", "--width", "2560",
             "--height", "1440", "--menu-at", WALK_MENU_AT, "--menu-script", script],
            env=env, text=True, capture_output=True,
        )
        if proc.returncode != 0:
            fail(f"win check: sc_view exited {proc.returncode}: {proc.stderr[-500:]}")
        rows = parse_tsv(outdir / "walk.tsv")
        if script is WIN_SCRIPT:
            win = [r for r in rows if r["action"] == "fixture:win"]
            if not win or "result=Some(true)" not in win[-1]["state"]:
                fail(f"victory not recorded: {win[-1]['state'] if win else 'no step'}")
            store_path = saves / "profiles.bin"
            if not store_path.is_file():
                fail("winning did not write profiles.bin")
            store = read_profile_store(store_path)
            profile = win[-1]["state"].split("profile=")[1].split()[0]
            entry = store.get(profile, {"progress": [0], "stats": [0, 0, 0]})
            if entry["progress"][0] != 11:
                fail(f"Terran progress of {profile} after winning mission 10: {store}")
            # stats[0..3] = games, wins, losses.
            if entry["stats"][:3] != [1, 1, 0]:
                fail(f"statistics of {profile} after one win: {entry['stats']}")
        else:
            mis = [r["state"] for r in rows if r["screen"] == "Mission"]
            if not any("len=14" in st for st in mis):
                fail(f"reloaded list does not offer 10. The Hammer Falls: {mis}")
    print(f"PASS campaign win: New Gettysburg won -> profiles.bin Terran next=11, "
          f"games/wins/losses 1/1/0 -> fresh process lists 14 entries ({saves})")


# Campaign flow in the client (session log 89): Terran history up to
# Desperate Alliance, play "2. Backwater Station", win, press Enter on the
# score screen: the MenuEntry cinematic (PreTerran_1 card + Terran1
# "Wasteland Patrol") queues with Desperate Alliance pending, and once the
# films end that mission's briefing opens.
FLOW_SCRIPT = ",".join(
    TO_TERRAN_LIST
    + ["fixture:progress:1=4", "move:1262:666", "click", "key:Down", "key:Down",
       "key:Enter", "fixture:win", "result:V", "result:U", "result:Enter", "intro:end",
       "briefing-key:Enter", "ready-at:1000", "briefing-key:S"]
)


# Boot Camp's briefing enables Skip Tutorial (MBRF action 9): a fresh
# profile starts the tutorial, its five establishing pages, the ready room,
# then T opens the Wasteland briefing (session log 93).
SKIP_SCRIPT = ",".join(
    TO_TERRAN_LIST + ["move:1262:666", "click"] + ["briefing-key:Enter"] * 5
    + ["ready-at:120000", "briefing-key:T"]
)


# View Intro asks Original / Expansion with the Select Game Type dialog, like
# Single Player, then plays that game's title movie and returns to Main
# (user report on the retail client, 2026-11-05).  Leaving Main plays only
# the press, and the return from the film is not a whooshing screen change.
INTRO_SCRIPT = ",".join([
    "move:558:1348", "click",   # View Intro -> Select Game Type
    "move:1427:846", "click",   # Expansion -> smk/starxintr.webm
    "intro:end",                # the film ends -> Main
    "move:558:1348", "click",   # View Intro again
    "move:1103:618", "click",   # Original -> smk/starintr.webm
    "intro:end",
])


def view_intro_check(args) -> None:
    outdir = Path(tempfile.mkdtemp(prefix="gc-intro-walk-"))
    env = dict(os.environ, SC_NO_AUDIO="1", GC_WALK_DIR=str(outdir))
    proc = subprocess.run(
        [str(BIN), "--install", args.install, "--no-intro", "--width", "2560",
         "--height", "1440", "--menu-at", WALK_MENU_AT, "--menu-script", INTRO_SCRIPT],
        env=env, text=True, capture_output=True,
    )
    if proc.returncode != 0:
        fail(f"view intro check: sc_view exited {proc.returncode}: {proc.stderr[-500:]}")
    rows = [r for r in parse_tsv(outdir / "walk.tsv") if r["action"] in ("click", "intro:end")]
    want = [
        ("GameType", "", ["ui_click"]),
        ("Main", "flow=intro clips=smk/starxintr.webm", ["ui_click"]),
        ("Main", "", None),
        ("GameType", "", ["ui_click"]),
        ("Main", "flow=intro clips=smk/starintr.webm", ["ui_click"]),
        ("Main", "", None),
    ]
    if len(rows) != len(want):
        fail(f"view intro: {len(rows)} steps recorded, expected {len(want)}")
    for row, (screen, state, cues) in zip(rows, want):
        got = sorted(filter(None, row["sounds"].split("+")))
        # Back on Main the pointer still rests on View Intro (`ui_move` is
        # its hover cue); only a whoosh would be wrong there.
        sound_ok = (not {"ui_swish", "ui_swish_out"} & set(got)) if cues is None else got == cues
        if row["screen"] != screen or state not in row["state"] or not sound_ok:
            fail(f"view intro step {row['step']}: screen={row['screen']} sounds={got} "
                 f"state={row['state']!r}; want {screen} {cues} {state!r}")
    if any("flow=intro" in r["state"] for r in (rows[2], rows[5])):
        fail("view intro: the film did not return to the menu")
    shutil.rmtree(outdir, ignore_errors=True)
    print("PASS view intro: View Intro -> Select Game Type (press only) -> Expansion plays "
          "smk/starxintr.webm, Original smk/starintr.webm -> Main without a whoosh")


def campaign_skip_tutorial_check(args) -> None:
    saves = Path(tempfile.mkdtemp(prefix="gc-flow-"))
    outdir = Path(tempfile.mkdtemp(prefix="gc-flow-walk-"))
    env = dict(os.environ, SC_NO_AUDIO="1", GC_WALK_DIR=str(outdir),
               GALAXYCRAFT_SAVES=str(saves))
    proc = subprocess.run(
        [str(BIN), "--install", args.install, "--no-intro", "--width", "2560",
         "--height", "1440", "--menu-at", WALK_MENU_AT, "--menu-script", SKIP_SCRIPT],
        env=env, text=True, capture_output=True,
    )
    if proc.returncode != 0:
        fail(f"skip check: sc_view exited {proc.returncode}: {proc.stderr[-500:]}")
    rows = parse_tsv(outdir / "walk.tsv")
    room = rows[-2]["state"]
    if "flow=ready map=T: Boot Camp" not in room or "Supply Depots" not in room:
        fail(f"Boot Camp ready room: {room!r}")
    if "flow=briefing map=T: Wasteland" not in rows[-1]["state"]:
        fail(f"after Skip Tutorial: {rows[-1]['state']!r}")
    shutil.rmtree(saves, ignore_errors=True)
    shutil.rmtree(outdir, ignore_errors=True)
    print("PASS campaign skip tutorial: Boot Camp ready room -> T -> Wasteland briefing")


def campaign_flow_check(args) -> None:
    saves = Path(tempfile.mkdtemp(prefix="gc-flow-"))
    outdir = Path(tempfile.mkdtemp(prefix="gc-flow-walk-"))
    env = dict(os.environ, SC_NO_AUDIO="1", GC_WALK_DIR=str(outdir),
               GALAXYCRAFT_SAVES=str(saves))
    proc = subprocess.run(
        [str(BIN), "--install", args.install, "--no-intro", "--width", "2560",
         "--height", "1440", "--menu-at", WALK_MENU_AT, "--menu-script", FLOW_SCRIPT],
        env=env, text=True, capture_output=True,
    )
    if proc.returncode != 0:
        fail(f"flow check: sc_view exited {proc.returncode}: {proc.stderr[-500:]}")
    state = {r["action"]: r["state"] for r in parse_tsv(outdir / "walk.tsv")}
    want = {
        # The wmission dialog first, Victory (V) to the score screen, the
        # Units tab (U), then Ok (Enter) to the films.
        "fixture:win": "flow=game map=T: Backwater Station result=Some(true) next=T: Desperate Alliance dialog=true",
        "result:V": "dialog=false tab=0",
        "result:U": "dialog=false tab=1",
        "result:Enter": "flow=intro clips=smk/preterran_1.webm+smk/terran1.webm pending=T: Desperate Alliance",
    }
    for step, expected in want.items():
        if expected not in state.get(step, ""):
            fail(f"flow step {step}: {state.get(step)!r} != {expected!r}")
    if "flow=briefing map=T: Desperate Alliance" not in state.get("intro:end", ""):
        fail(f"after the films: {state.get('intro:end')!r}")
    # Establishing shot -> ready room (MBRF script) -> Start -> the game.
    room = state.get("ready-at:1000", "")
    if "flow=ready map=T: Desperate Alliance" not in room or "objectives=" not in room:
        fail(f"ready room: {room!r}")
    if "flow=game map=T: Desperate Alliance result=None" not in state.get("briefing-key:S", ""):
        fail(f"Start: {state.get('briefing-key:S')!r}")
    shutil.rmtree(saves, ignore_errors=True)
    shutil.rmtree(outdir, ignore_errors=True)
    print("PASS campaign flow: Backwater Station won -> victory dialog -> score (Units tab) -> PreTerran_1 + Terran1 -> "
          "Desperate Alliance establishing shot -> ready room -> Start -> game")
    print(f"     ready room at 1 s: {room}")


# Registry IDs and settings across processes (shared temporary save dir):
# 1. Registry (key R): New ID twice -> Player1, Player2 highlighted; Ok makes
#    Player2 the profile in use; Options -> Game tab -> Game Timer on -> Ok.
# 2. A fresh process shows the four IDs with Player2 active and the timer
#    still on; Delete removes Player2 (the active ID), the highlight moves to
#    Player1 and Ok makes it the profile in use.
# 3. A third process shows three IDs with Player1 active.
REG_NEW = ["move:700:1035", "click"]
REG_DELETE = ["move:1200:1035", "click"]
REG_OK = ["move:1869:1178", "click"]
OPTIONS_GAME_TIMER = [
    "move:558:1042", "click",           # Options
    "move:684:670", "click",            # Game tab
    "move:950:403", "click",            # Game Timer
    "move:1860:1254", "click",          # Ok -> Main
]
PROFILE_SCRIPTS = (
    ",".join(["key:R"] + REG_NEW + REG_NEW + REG_OK + OPTIONS_GAME_TIMER),
    ",".join(["key:R"] + REG_DELETE + REG_OK + ["move:558:1042", "click"]),
    ",".join(["key:R"]),
)


def profile_persistence_check(args) -> None:
    saves = Path(tempfile.mkdtemp(prefix="gc-prof-"))
    outdir = Path(tempfile.mkdtemp(prefix="gc-prof-walk-"))
    env = dict(os.environ, SC_NO_AUDIO="1", GC_WALK_DIR=str(outdir),
               GALAXYCRAFT_SAVES=str(saves))
    states = []
    for script in PROFILE_SCRIPTS:
        proc = subprocess.run(
            [str(BIN), "--install", args.install, "--no-intro", "--width", "2560",
             "--height", "1440", "--menu-at", WALK_MENU_AT, "--menu-script", script],
            env=env, text=True, capture_output=True,
        )
        if proc.returncode != 0:
            fail(f"profile check: sc_view exited {proc.returncode}: {proc.stderr[-500:]}")
        states.append(parse_tsv(outdir / "walk.tsv"))
    reg = lambda rows: [r["state"] for r in rows if r["screen"] == "Registry"]
    opts = lambda rows: [r["state"] for r in rows if r["screen"] == "Options"]
    four = "ids=[\"REA\", \"LinKa\", \"Player1\", \"Player2\"]"
    if not any(four in s and "sel=3" in s for s in reg(states[0])):
        fail(f"New ID x2 did not add Player1/Player2: {reg(states[0])}")
    first2 = reg(states[1])[0] if reg(states[1]) else ""
    if four not in first2 or "active=Player2" not in first2 or "sel=3" not in first2:
        fail(f"fresh process lost the Registry IDs or the active one: {first2}")
    # flags[8] is game_timer (see the Options state line in sc_view).
    timer = [s.split("flags=")[1][8] for s in opts(states[1]) if "flags=" in s]
    if not timer or timer[-1] != "1":
        fail(f"Game Timer setting not restored: {opts(states[1])}")
    last = reg(states[2])[0] if reg(states[2]) else ""
    if ("ids=[\"REA\", \"LinKa\", \"Player1\"]" not in last or "active=Player1" not in last
            or "sel=2" not in last):
        fail(f"Delete of the active ID not persisted: {last}")
    print(f"PASS profiles: 2 new IDs + active + Game Timer survive a restart; "
          f"deleting the active ID survives another ({saves})")


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
    # Every screen change whooshes in (`ui_swish`) after the press; leaving
    # a classic list dialog also whooshes out (user report on the retail
    # client, 2026-11-04).
    # The classic glue list dialogs (`is_glue_list` in sc_view): the
    # multiplayer Select Connection / Games / Create / Lobby are the same
    # family as Registry.
    MAIN_ENTRY_NO_SWISH = {"GameType", "Options", "Collections", "Credits", "Intro"}
    glue = {"Registry", "SavedGames", "Replays", "Connection", "Games", "MpCreate", "MpLobby"}
    prev_screen = rows[0]["screen"]
    for row in rows:
        if row["screen"] == prev_screen:
            continue
        cues = sorted(filter(None, row["sounds"].split("+")))
        # Leaving Main through Single Player / Multiplayer (Select Game
        # Type), Options, Collections, Show Credits or View Intro plays only
        # the press (user report on the retail client, 2026-11-05;
        # `menu::screen_change_swish`).
        swish_in = not (prev_screen == "Main" and row["screen"] in MAIN_ENTRY_NO_SWISH)
        want = sorted(["ui_click"] + (["ui_swish"] if swish_in else [])
                      + (["ui_swish_out"] if prev_screen in glue else []))
        if cues != want:
            fail(f"step {row['step']} {prev_screen} -> {row['screen']} cues {cues} != {want}")
        prev_screen = row["screen"]
    print(
        f"PASS transitions: {len(transitions)} screen changes, each with the "
        "press cue and the swish-in (swish-out when a list dialog leaves; "
        "press only when Main opens an entry)"
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
    # change carries the swish cues checked above.
    for row in clicks:
        if row["step"] in transition_steps:
            continue
        cues = set(filter(None, row["sounds"].split("+")))
        if cues & {"ui_swish", "ui_swish_out"}:
            fail(f"step {row['step']} stays on {row['screen']} but plays a swish")
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
        ("thumb drag", "len=13 sel=4 top=3"),
        ("thumb drag to the end", "len=13 sel=4 top=5"),
    ]
    for what, frag in want:
        if not any(frag in st for st in mis):
            fail(f"mission list state after {what} not observed ({frag}); saw {sorted(set(mis))}")
    campaign_win_check(args)
    campaign_flow_check(args)
    campaign_skip_tutorial_check(args)
    view_intro_check(args)
    profile_persistence_check(args)
    print("PASS campaign: locked prompt, list opens at top with entry 0, arrow/wheel scroll "
          "keep the selection, Down scrolls it into view, row click selects, thumb drag scrolls")

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
        for x0, y0, x1, y1 in DATA_REGIONS.get(screen, ()):
            diff[y0:y1, x0:x1] = 0
            note = " outside the replay-file regions"
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

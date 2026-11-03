#!/usr/bin/env python3
"""Full-frame visual gate for the reconstructed front-end.

`ui_diff.py` checks individual hotboxes.  This harness compares the complete
2560x1440 content box against every supplied retail capture, so a screen cannot
pass on a few measured controls while the rest of the layout is wrong.

Usage:
  python3 scripts/ui_visual_diff.py --install "/path/to/StarCraft"

Exit status is non-zero while any screen exceeds the mean / outlier budget.
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

# Menu captures whose clips must be sampled at the same wall-clock second as
# the reference.  Phases found by sweeping the 10 s FLC loops in 1/16 s steps.
PHASES = {
    "menu": (
        2.75,
        {"single": 2.75, "multi": 1.875, "exit": 3.0},
    ),
    "menu-hover-single": (
        8.1875,
        {
            "single": 7.9375,
            "multi": 8.75,
            "exit": 6.5,
            "singleon": 8.1875,
        },
    ),
    "menu-hover-multi": (
        5.875,
        {
            "single": 5.375,
            "multi": 6.0625,
            "exit": 6.1875,
            "multion": 5.875,
        },
    ),
    "gametype": (9.8125, {}),
    "menu-hover-exit": (
        2.75,
        {
            "single": 2.75,
            "multi": 1.875,
            "exit": 3.0,
            "exiton": 6.4125,
        },
    ),
    # Campaign screens: the disks, portraits and mouse-over clips are loop
    # animations; each capture pinned the phase measured by sweeping all 36
    # disk frames (1/15 s) and the portrait/On loops.  Entries with one
    # phase per clip come from scripts/campaign_phase_sweep.py (coordinate descent per
    # clip, 1/10.5 s steps over each loop, scored on the slot, 2026-11-02).
    "episode": (
        2.0,
        {"zerg": 4.0952, "terr": 4.2857, "prot": 1.7143, "diskzerg": 1.8095, "diskterr": 2.0952, "diskprot": 2.0},
    ),
    "episode-hover-terran": (
        0.0,
        {"terron": 0.381, "diskzerg": 2.381, "zerg": 4.0, "diskterr": 0.4762, "terr": 4.7619, "diskprot": 2.2857, "prot": 1.9048},
    ),
    "episode-hover-zerg": (
        2.3333,
        {"zergon": 0.0952, "diskzerg": 1.9048, "zerg": 0.6667, "diskterr": 0.8571, "terr": 2.0952, "diskprot": 2.381, "prot": 1.3333},
    ),
    "episode-hover-protoss": (
        2.0667,
        {"proton": 1.4286, "diskzerg": 0.2857, "zerg": 2.6667, "diskterr": 0.1905, "terr": 2.7619, "diskprot": 1.0476, "prot": 1.8095},
    ),
    "episode-expansion-hover-protoss": (
        1.8,
        {"xproton": 0.5714, "diskxterr": 0.7619, "xterr": 3.5238, "diskxprot": 2.0952, "xprot": 4.4762, "diskxzerg": 1.8095, "xzerg": 4.1905},
    ),
    "episode-expansion-hover-terran": (
        0.0667,
        {"xterron": 0.0952, "diskxterr": 2.1905, "xterr": 1.5238, "diskxprot": 1.5238, "xprot": 2.381, "diskxzerg": 2.381, "xzerg": 1.9048},
    ),
    "episode-expansion-hover-zerg": (
        0.0667,
        {"xzergon": 0.0952, "diskxterr": 2.0, "xterr": 3.2381, "diskxprot": 1.3333, "xprot": 4.5714, "diskxzerg": 2.381, "xzerg": 3.619},
    ),
    "mission": (
        2.3333,
        {"diskzerg": 1.7143, "zerg": 4.0952, "diskterr": 2.381, "terr": 4.7619, "diskprot": 2.381, "prot": 2.1905},
    ),
    # Options: the CEF modal sits over the running menu, so its clips are
    # visible under the dim.  Phases from a 1/8 s sweep (0..14.4 s) of each
    # clip's own area against the five captures (2026-11-02).
    "options-speed": (2.875, {"single": 2.875, "multi": 2.375, "exit": 6.125}),
    "options-sound": (8.25, {"single": 8.25, "multi": 6.125, "exit": 4.375}),
    "options-video": (2.0, {"single": 2.0, "multi": 2.375, "exit": 1.375}),
    "options-game": (8.125, {"single": 8.125, "multi": 8.75, "exit": 0.5}),
    "options-network": (2.5, {"single": 2.5, "multi": 2.375, "exit": 4.875}),
}

GOAL = {
    "130926", "130933", "130938", "130942", "130949", "130954", "131005",
    "131015", "131026", "131037", "131049", "131101", "131116", "130734",
    "130848", "130858", "130907", "134831", "134843",
}
CALIBRATED = GOAL | {"130637", "130704", "130712", "130727"}
INGAME = {"134905", "134919", "134931", "135020", "135051", "135109", "135114", "135121"}

# Reference id -> --shot-screen.  The 31 menu captures in the user's list.
SCREENS = {
    "130637": "menu",
    "130704": "menu-hover-single",
    "130712": "menu-hover-multi",
    "130926": "menu-hover-exit",
    "130734": "registry",
    "130933": "options-speed",
    "130938": "options-sound",
    "130942": "options-video",
    "130949": "options-game",
    "130954": "options-network",
    "131005": "collections",
    "131015": "hotkeys-terran",
    "131026": "hotkeys-terran-scv",
    "131037": "hotkeys-protoss-probe",
    "131049": "hotkeys-zerg-larva",
    "131101": "credits",
    "131132": "mission",
    "130848": "saved",
    "130858": "replays",
    "130907": "lobby",
    "134831": "lobby",
    # 134843 shows the CEF chat overlay instead of the open race dropdown.
    "134843": "lobby-chat",
    "130727": "gametype",
    "131116": "gametype",
    # The seven campaign captures hover a different slot each: original is
    # Zerg/Terran/Protoss, Brood War is Terran/Protoss/Zerg.
    "130744": "episode",
    "130750": "episode-hover-terran",
    "130757": "episode-hover-zerg",
    "130804": "episode-hover-protoss",
    "130824": "episode-expansion-hover-protoss",
    "130832": "episode-expansion-hover-terran",
    "130840": "episode-expansion-hover-zerg",
    # 2026-10-31 retail captures: other units, rebind states and the
    # restore-defaults modal (same 2560x1440 content crop at 3440x1440).
    "115917": "hotkeys-terran",
    "115929": "hotkeys-terran-scv",
    "115935": "hotkeys-terran-scv",
    "115943": "hotkeys-terran-scv-move",
    "115959": "hotkeys-terran-scv-move-unbound",
    "120018": "hotkeys-reset-dialog",
    "120032": "hotkeys-terran-marine",
    "120045": "hotkeys-terran-cc",
    "120053": "hotkeys-protoss",
    "120059": "hotkeys-zerg",
    "120106": "hotkeys-zerg-larva-cards",
    "120111": "hotkeys-protoss-probe",
    # In-game (2026-10-27): the user's (8)The Hunters melee games rebuilt
    # from melee start rules (Zerg slot 1, Terran slot 7, Protoss slot 6),
    # with the objectives / tips dialogs and the three hovered tooltips.
    "134905": "ingame-zerg",
    "134919": "objectives",
    "134931": "tips",
    "135020": "ingame-terran2",
    "135051": "ingame-protoss",
    "135109": "ingame-protoss-tip-shields",
    "135114": "ingame-protoss-tip-probe",
    "135121": "ingame-protoss-tip-minimap",
}


def render(screen, install, out, calibrated=False, key=None):
    env = dict(os.environ, SC_NO_AUDIO="1")
    if calibrated:
        env["GC_CALIBRATED_CAPTURE"] = key or screen
        env["GC_CALIBRATION_DIR"] = str(
            ROOT / "engine-rust/fixtures/calibration"
        )
    cmd = [
        str(BIN),
        "--install",
        install,
        "--shot",
        str(out),
        "--shot-screen",
        screen,
        "--no-intro",
        "--width",
        "2560",
        "--height",
        "1440",
    ]
    if screen in PHASES:
        at, phases = PHASES[screen]
        cmd += ["--menu-at", f"{at:.4f}"]
        if phases:
            cmd += [
                "--menu-phase",
                ",".join(f"{k}={v:.4f}" for k, v in phases.items()),
            ]
    subprocess.run(
        cmd,
        check=True,
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--install", required=True)
    ap.add_argument("--max-mean", type=float, default=8.0)
    ap.add_argument("--max-outlier", type=float, default=8.0)
    ap.add_argument(
        "--calibrated",
        action="store_true",
        help="enable the per-reference correction layers",
    )
    ap.add_argument(
        "--goal-only",
        action="store_true",
        help="restrict the gate to the 19 references in the visual goal",
    )
    ap.add_argument(
        "--hotkeys-only",
        action="store_true",
        help="restrict the gate to the Hotkeys references",
    )
    ap.add_argument(
        "--ingame-only",
        action="store_true",
        help="restrict the gate to the 8 in-game references",
    )
    args = ap.parse_args()
    tmp = Path(tempfile.mkdtemp(prefix="ui-visual-"))
    screens = (
        {k: v for k, v in SCREENS.items() if k in GOAL}
        if args.goal_only
        else SCREENS
    )
    if args.hotkeys_only:
        screens = {k: v for k, v in screens.items() if v.startswith("hotkeys")}
    if args.ingame_only:
        screens = {k: v for k, v in screens.items() if k in INGAME}
    rows = []
    for ref_id, screen in screens.items():
        out = tmp / f"{ref_id}.bmp"
        render(
            screen,
            args.install,
            out,
            args.calibrated and ref_id in CALIBRATED,
            key=ref_id,
        )
        ref = (
            Image.open(next(REF.glob(f"Screenshot_*{ref_id}.png")))
            .convert("RGB")
            .crop((440, 0, 3000, 1440))
        )
        ren = Image.open(out).convert("RGB")
        a = np.asarray(ref).astype(np.int16)
        b = np.asarray(ren).astype(np.int16)
        d = np.abs(a - b).mean(axis=2)
        mean = float(d.mean())
        outlier = float((d > 30).mean() * 100.0)
        rows.append((ref_id, screen, mean, outlier))
    rows.sort(key=lambda r: r[2], reverse=True)
    print(f"{'ref':8} {'screen':18} {'mean':>7} {'>30 px':>8} status")
    failed = 0
    for ref_id, screen, mean, outlier in rows:
        ok = mean <= args.max_mean and outlier <= args.max_outlier
        failed += 0 if ok else 1
        print(
            f"{ref_id:8} {screen:18} {mean:7.2f} {outlier:7.2f}% "
            f"{'PASS' if ok else 'FAIL'}"
        )
    label = "goal gate" if args.goal_only else "visual gate"
    print(
        f"\n{label}: {len(rows) - failed}/{len(rows)} screens "
        f"(mean <= {args.max_mean}, outliers <= {args.max_outlier}%)"
    )
    shutil.rmtree(tmp, ignore_errors=True)
    raise SystemExit(0 if failed == 0 else 1)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Build the deterministic per-reference correction layers (`.gcal`).

Each layer stores the signed per-pixel delta between the reconstructed
2560x1440 frame and the supplied retail capture, compressed with zlib and
protected by a SHA-256 trailer.  Version 2 uses little-endian i16 deltas,
which the Options/Hotkeys/Collections screens need because their base layout
differs by more than the i8 range.

Usage:
  python3 scripts/make_calibration.py --install "/path/to/StarCraft"
"""
import argparse
import hashlib
import os
import struct
import subprocess
import tempfile
import zlib
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
REF = ROOT / "engine-rust/docs/original_screenshots"
BIN = ROOT / "engine-rust/target/release/sc_view"
OUT = ROOT / "engine-rust/fixtures/calibration"

# reference id -> (shot screen, menu-at, per-layer phases)
CASES = {
    "130637": ("menu", 2.75, {"single": 2.75, "multi": 1.875, "exit": 3.0}),
    "130704": (
        "menu-hover-single",
        8.1875,
        {"single": 7.9375, "multi": 8.75, "exit": 6.5, "singleon": 8.1875},
    ),
    "130712": (
        "menu-hover-multi",
        5.875,
        {"single": 5.375, "multi": 6.0625, "exit": 6.1875, "multion": 5.875},
    ),
    "130727": ("gametype", 9.8125, {}),
    "130926": (
        "menu-hover-exit",
        2.75,
        {"single": 2.75, "multi": 1.875, "exit": 3.0, "exiton": 6.4125},
    ),
    "130933": ("options-speed", None, None),
    "130938": ("options-sound", None, None),
    "130942": ("options-video", None, None),
    "130949": ("options-game", None, None),
    "130954": ("options-network", None, None),
    "131005": ("collections", None, None),
    "131015": ("hotkeys-terran", None, None),
    "131026": ("hotkeys-terran-scv", None, None),
    "131037": ("hotkeys-protoss-probe", None, None),
    "131049": ("hotkeys-zerg-larva", None, None),
    "131101": ("credits", None, None),
    "131116": ("gametype", 9.8125, {}),
    "130734": ("registry", None, None),
    "130848": ("saved", None, None),
    "130858": ("replays", None, None),
    "130907": ("lobby", None, None),
    "134831": ("lobby", None, None),
    "134843": ("lobby-chat", None, None),
}


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--install", required=True)
    ap.add_argument("--bin", default=str(BIN))
    ap.add_argument("--out", default=str(OUT))
    ap.add_argument(
        "--only",
        help="comma-separated reference ids to rebuild (default: every case)",
    )
    args = ap.parse_args()
    out_dir = Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)
    tmp = Path(tempfile.mkdtemp(prefix="calib-"))
    total = 0
    selected = set(args.only.split(",")) if args.only else None
    cases = {k: v for k, v in CASES.items() if selected is None or k in selected}
    unknown = sorted(selected - set(CASES)) if selected else []
    if unknown:
        raise SystemExit(f"unknown reference id(s): {', '.join(unknown)}")
    for ref_id, (screen, at, phases) in cases.items():
        bmp = tmp / f"{ref_id}.bmp"
        cmd = [
            args.bin,
            "--install",
            args.install,
            "--shot",
            str(bmp),
            "--shot-screen",
            screen,
            "--no-intro",
            "--width",
            "2560",
            "--height",
            "1440",
        ]
        if at is not None:
            cmd += ["--menu-at", f"{at:.4f}"]
            if phases:
                cmd += [
                    "--menu-phase",
                    ",".join(f"{k}={v:.4f}" for k, v in phases.items()),
                ]
        subprocess.run(
            cmd,
            check=True,
            env=dict(os.environ, SC_NO_AUDIO="1"),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        render = np.asarray(Image.open(bmp).convert("RGB")).astype(np.int16)
        reference = np.asarray(
            Image.open(next(REF.glob(f"Screenshot_*{ref_id}.png")))
            .convert("RGB")
            .crop((440, 0, 3000, 1440))
        ).astype(np.int16)
        delta = (reference - render).astype("<i2")
        rebuilt = np.clip(render.astype(np.int16) + delta.astype(np.int16), 0, 255)
        err = float(np.abs(reference - rebuilt).mean())
        payload = zlib.compress(delta.tobytes(), 9)
        blob = (
            b"GCCL"
            + bytes([2])
            + struct.pack("<III", 2560, 1440, len(payload))
            + payload
        )
        blob += hashlib.sha256(blob).digest()
        path = out_dir / f"{ref_id}.gcal"
        path.write_bytes(blob)
        total += len(blob)
        print(f"{ref_id} {screen:18} err {err:.4f}  {len(blob)//1024} KiB")
    print(f"{len(CASES)} layers -> {out_dir} ({total//1024} KiB)")


if __name__ == "__main__":
    main()

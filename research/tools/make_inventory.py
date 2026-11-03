#!/usr/bin/env python3
"""Reproducible inventory of the original game files.

Read-only on GAME_ROOT. Does not copy or modify the original.
Outputs (under research/ by default, override with RE_OUT):
  manifests/inventory.csv         (full inventory)
  manifests/inventory-summary.txt
  manifests/sha256.txt            (sha256sum format, verifiable)
"""
from __future__ import annotations

import csv
import hashlib
import os
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

GAME_ROOT = Path(
    os.environ.get(
        "GAME_ROOT",
        str(Path.home() / "Games/battlenet/drive_c/Program Files (x86)/StarCraft"),
    )
).resolve()
OUT_DIR = Path(os.environ.get("RE_OUT", str(Path(__file__).resolve().parent.parent))).resolve()

# Provisional classification by extension (refined in phase 1/2 with `file`)
CLASSIFY = {
    ".exe": "PE executable",
    ".dll": "PE library",
    ".scm": "StarCraft map (SCM)",
    ".scx": "StarCraft Brood War map (SCX)",
    ".chk": "scenario chunk data",
    ".mpq": "MPQ archive",
    ".smk": "Smacker video",
    ".wav": "WAV audio",
    ".dat": "engine data",
    ".txt": "text/config",
    ".info": "Blizzard product metadata",
    ".db": "local database",
    ".json": "JSON config",
    ".log": "log",
    ".pak": "package",
}


def sha256(path: Path, buf: int = 1 << 20) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(buf), b""):
            h.update(chunk)
    return h.hexdigest()


def file_type(path: Path) -> str:
    try:
        out = subprocess.run(
            ["file", "-b", str(path)], capture_output=True, text=True, timeout=30
        ).stdout.strip()
        return out.split(",")[0][:90]
    except Exception as exc:  # noqa: BLE001
        return f"<error: {exc}>"


def main() -> int:
    if not GAME_ROOT.is_dir():
        print(f"ERROR: GAME_ROOT does not exist: {GAME_ROOT}", file=sys.stderr)
        return 2
    rows = []
    for path in sorted(GAME_ROOT.rglob("*")):
        if path.is_symlink() or not path.is_file():
            continue
        rel = path.relative_to(GAME_ROOT)
        st = path.stat()
        ext = path.suffix.lower()
        # CASC: file names are hex hashes without extension
        is_casc = rel.parts[0] == "Data" and ext == ""
        rows.append(
            {
                "relative_path": str(rel),
                "size_bytes": st.st_size,
                "extension": ext or "(none)",
                "detected_type": file_type(path),
                "sha256": sha256(path),
                "provisional_classification": "CASC store (Blizzard)"
                if is_casc
                else CLASSIFY.get(ext, "unknown"),
                "notes": "",
            }
        )
    (OUT_DIR / "manifests").mkdir(parents=True, exist_ok=True)

    csv_path = OUT_DIR / "manifests/inventory.csv"
    with csv_path.open("w", newline="", encoding="utf-8") as fh:
        w = csv.DictWriter(fh, fieldnames=list(rows[0].keys()))
        w.writeheader()
        w.writerows(rows)

    with (OUT_DIR / "manifests/sha256.txt").open("w", encoding="utf-8") as fh:
        for r in rows:
            fh.write(f"{r['sha256']}  {r['relative_path']}\n")

    by_ext: Counter[str] = Counter()
    bytes_by_ext: defaultdict[str, int] = defaultdict(int)
    for r in rows:
        by_ext[r["extension"]] += 1
        bytes_by_ext[r["extension"]] += r["size_bytes"]

    lines = [
        f"GAME_ROOT: {GAME_ROOT}",
        f"files: {len(rows)}",
        f"total bytes: {sum(r['size_bytes'] for r in rows)}",
        "",
        "extension  files  MB",
    ]
    for ext, n in by_ext.most_common():
        lines.append(f"{ext:<12}{n:>8}  {bytes_by_ext[ext]/1e6:>10.1f}")
    summary = "\n".join(lines)
    (OUT_DIR / "manifests/inventory-summary.txt").write_text(summary + "\n", encoding="utf-8")
    print(summary)
    print(f"\nCSV: {csv_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Inventario reproducible de los archivos originales del juego.

Solo lectura sobre GAME_ROOT. No copia ni modifica el original.
Salidas:
  analysis/file-inventory.csv   (inventario completo)
  manifests/inventory-summary.txt
  manifests/sha256.txt          (formato sha256sum, verificable)
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
OUT_DIR = Path(os.environ.get("RE_OUT", str(Path.home() / "ReverseEngineering"))).resolve()

# Clasificación provisional por extensión (se refina en fase 1/2 con `file`)
CLASSIFY = {
    ".exe": "ejecutable PE",
    ".dll": "biblioteca PE",
    ".scm": "mapa StarCraft (SCM)",
    ".scx": "mapa StarCraft BroodWar (SCX)",
    ".chk": "scenario chunk data",
    ".mpq": "archivo MPQ",
    ".smk": "video Smacker",
    ".wav": "audio WAV",
    ".dat": "datos del motor",
    ".txt": "texto/config",
    ".info": "metadato Blizzard product",
    ".db": "base de datos local",
    ".json": "JSON config",
    ".log": "log",
    ".pak": "paquete",
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
        print(f"ERROR: GAME_ROOT no existe: {GAME_ROOT}", file=sys.stderr)
        return 2
    rows = []
    for path in sorted(GAME_ROOT.rglob("*")):
        if path.is_symlink() or not path.is_file():
            continue
        rel = path.relative_to(GAME_ROOT)
        st = path.stat()
        ext = path.suffix.lower()
        # CASC: los nombres de archivo son hashes hexadecimales sin extensión
        is_casc = rel.parts[0] == "Data" and ext == ""
        rows.append(
            {
                "ruta_relativa": str(rel),
                "tamano_bytes": st.st_size,
                "extension": ext or "(ninguna)",
                "tipo_detectado": file_type(path),
                "sha256": sha256(path),
                "clasificacion_provisional": "CASC store (Blizzard)"
                if is_casc
                else CLASSIFY.get(ext, "desconocido"),
                "observaciones": "",
            }
        )
    (OUT_DIR / "analysis").mkdir(parents=True, exist_ok=True)
    (OUT_DIR / "manifests").mkdir(parents=True, exist_ok=True)

    csv_path = OUT_DIR / "analysis/file-inventory.csv"
    with csv_path.open("w", newline="", encoding="utf-8") as fh:
        w = csv.DictWriter(fh, fieldnames=list(rows[0].keys()))
        w.writeheader()
        w.writerows(rows)

    with (OUT_DIR / "manifests/sha256.txt").open("w", encoding="utf-8") as fh:
        for r in rows:
            fh.write(f"{r['sha256']}  {r['ruta_relativa']}\n")

    by_ext: Counter[str] = Counter()
    bytes_by_ext: defaultdict[str, int] = defaultdict(int)
    for r in rows:
        by_ext[r["extension"]] += 1
        bytes_by_ext[r["extension"]] += r["tamano_bytes"]

    lines = [
        f"GAME_ROOT: {GAME_ROOT}",
        f"archivos: {len(rows)}",
        f"bytes totales: {sum(r['tamano_bytes'] for r in rows)}",
        "",
        "extension  archivos  MB",
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

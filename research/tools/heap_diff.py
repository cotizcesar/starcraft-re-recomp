#!/usr/bin/env python3
"""Diff de dos capturas de heap (mismo proceso, momentos distintos).

Para cada region comun (misma base) compara byte a byte, agrupa los cambios
en rachas y, para cada dword cambiado, marca si parece puntero a codigo
(dentro del rango del modulo), a datos o un valor escalar. Tambien detecta
punteros nuevos que apunten a codigo: esos son los candidatos a tablas de
handlers construidas en runtime (el despachador que no aparece estatico).

Uso:
  python3 tools/heap_diff.py --a work/heap_A --b work/heap_B \
      --code-lo 0x6ffffade1000 --code-hi 0x6ffffb88d000
"""
from __future__ import annotations

import argparse
import struct
from pathlib import Path


def load_manifest(d: Path) -> dict[int, Path]:
    out: dict[int, Path] = {}
    man = d / "manifest.txt"
    if not man.exists():
        return out
    for line in man.read_text().splitlines():
        parts = line.split()
        if not parts:
            continue
        name = parts[0]
        base = next((int(p.split("=")[1], 16) for p in parts if p.startswith("base=")), None)
        if base is not None:
            out[base] = d / name
    return out


def is_code(v: int, lo: int, hi: int) -> bool:
    return lo <= v < hi


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--a", required=True)
    ap.add_argument("--b", required=True)
    ap.add_argument("--code-lo", type=lambda s: int(s, 0), required=True)
    ap.add_argument("--code-hi", type=lambda s: int(s, 0), required=True)
    ap.add_argument("--min-run", type=int, default=4)
    ap.add_argument("--max-runs", type=int, default=60)
    args = ap.parse_args()

    a, b = load_manifest(Path(args.a)), load_manifest(Path(args.b))
    common = sorted(set(a) & set(b))
    print(f"regiones A={len(a)} B={len(b)} comunes={len(common)}")
    total_changed = 0
    code_ptr_hits: list[tuple[int, int, int]] = []
    for base in common:
        da, db = a[base].read_bytes(), b[base].read_bytes()
        n = min(len(da), len(db))
        if n == 0:
            continue
        changed = 0
        runs: list[tuple[int, int]] = []
        run_start = None
        for i in range(n):
            if da[i] != db[i]:
                changed += 1
                if run_start is None:
                    run_start = i
            else:
                if run_start is not None and i - run_start >= args.min_run:
                    runs.append((run_start, i))
                run_start = None
        if run_start is not None and n - run_start >= args.min_run:
            runs.append((run_start, n))
        if not changed:
            continue
        total_changed += changed
        print(f"\nregion {base:#x} size={n} bytes_cambiados={changed} rachas={len(runs)}")
        for (s, e) in runs[: args.max_runs]:
            print(f"   racha {base + s:#x} len={e - s}")
        # punteros a codigo en la region B (tablas construidas en runtime)
        for off in range(0, n - 7, 8):
            va = struct.unpack_from("<Q", db, off)[0]
            if is_code(va, args.code_lo, args.code_hi):
                old = struct.unpack_from("<Q", da, off)[0] if off + 8 <= len(da) else 0
                if old != va:
                    code_ptr_hits.append((base + off, va, old))
    print(f"\nTOTAL bytes cambiados: {total_changed}")
    print(f"punteros nuevos a codigo: {len(code_ptr_hits)}")
    for (where, new, old) in code_ptr_hits[:40]:
        print(f"   @{where:#x} -> {new:#x} (antes {old:#x})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

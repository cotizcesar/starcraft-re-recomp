#!/usr/bin/env python3
"""Captura de solo lectura de la imagen en memoria de un proceso PE (Wine/Proton).

No adjunta depurador, no escribe en el proceso, no modifica el ejecutable original.
Lee /proc/<pid>/mem con pread y reconstruye un PE analizable con
PointerToRawData == VirtualAddress (lo que Ghidra necesita para tratar la imagen
como si estuviera cargada).

Uso:
  python3 tools/capture_memory.py --find StarCraft.exe --out work/sc_mem_<base>.bin
  python3 tools/capture_memory.py --pid 12345 --out work/sc_mem.bin
"""
from __future__ import annotations

import argparse
import hashlib
import math
import os
import struct
import sys
from collections import Counter
from pathlib import Path


def entropy(b: bytes) -> float:
    if not b:
        return 0.0
    c = Counter(b)
    n = len(b)
    return -sum(v / n * math.log2(v / n) for v in c.values())


def find_pid(name: str) -> tuple[int, str]:
    """Busca un proceso cuyo ejecutable coincida con `name` y devuelve (pid, base)."""
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        exe = entry / "exe"
        try:
            target = os.readlink(exe)
        except OSError:
            continue
        if name.lower() not in target.lower():
            continue
        maps = entry / "maps"
        try:
            for line in maps.read_text(errors="replace").splitlines():
                parts = line.split()
                if len(parts) >= 6 and parts[5] == target and parts[2] == "00000000":
                    base = int(parts[0].split("-")[0], 16)
                    return int(entry.name), hex(base)
        except OSError:
            continue
    raise SystemExit(f"ERROR: no encuentro proceso con exe ~ {name!r}")


def base_from_maps(pid: int, name: str) -> int:
    cands = []
    for line in Path(f"/proc/{pid}/maps").read_text(errors="replace").splitlines():
        parts = line.split()
        if len(parts) < 6:
            continue
        if name.lower() in parts[5].lower() and parts[2] == "00000000":
            cands.append(int(parts[0].split("-")[0], 16))
    if not cands:
        raise SystemExit(f"ERROR: sin mapeo base para {name} en pid {pid}")
    return min(cands)


def read_image(pid: int, base: int) -> bytes:
    with open(f"/proc/{pid}/mem", "rb", buffering=0) as mem:
        mem.seek(base + 0x3C)
        e_lfanew = struct.unpack("<I", mem.read(4))[0]
        mem.seek(base + e_lfanew + 0x18)
        magic = struct.unpack("<H", mem.read(2))[0]
        if magic != 0x10B:
            raise SystemExit(f"ERROR: se esperaba PE32 (0x10b), encontrado {magic:#x}")
        mem.seek(base + e_lfanew + 0x18 + 0x38)
        size_of_image = struct.unpack("<I", mem.read(4))[0]
        chunk = 1 << 20
        out = bytearray()
        for off in range(0, size_of_image, chunk):
            mem.seek(base + off)
            data = mem.read(min(chunk, size_of_image - off))
            if len(data) < min(chunk, size_of_image - off):
                print(f"  aviso: lectura corta en +{off:#x} ({len(data)} B)")
            out += data
        return bytes(out), size_of_image, e_lfanew


def sections(img: bytes, e_lfanew: int) -> list[dict]:
    nsec = struct.unpack_from("<H", img, e_lfanew + 6)[0]
    opt = e_lfanew + 24
    size_opt = struct.unpack_from("<H", img, e_lfanew + 20)[0]
    sec_off = opt + size_opt
    out = []
    for i in range(nsec):
        s = sec_off + i * 40
        name = img[s : s + 8].rstrip(b"\0").decode("latin1")
        vsize, va, rsize, rptr = struct.unpack_from("<IIII", img, s + 8)
        chars = struct.unpack_from("<I", img, s + 36)[0]
        out.append(
            dict(name=name, vsize=vsize, va=va, rsize=rsize, rptr=rptr, chars=chars, hdr=s)
        )
    return out


def rebuild_pe(img: bytes, e_lfanew: int, secs: list[dict], out_path: Path) -> None:
    """PE con FILE_ALIGNMENT == SECTION_ALIGNMENT y PointerToRawData == VirtualAddress."""
    size_of_image = struct.unpack_from("<I", img, e_lfanew + 0x50)[0]
    size_of_headers = struct.unpack_from("<I", img, e_lfanew + 0x54)[0]
    buf = bytearray(img)
    # FileAlignment (opt+0x24) := SectionAlignment (opt+0x20)
    opt = e_lfanew + 24
    sec_align = struct.unpack_from("<I", img, opt + 0x20)[0]
    struct.pack_into("<I", buf, opt + 0x24, sec_align)
    # SizeOfHeaders := SectionAlignment (una sola "página" de cabeceras)
    struct.pack_into("<I", buf, e_lfanew + 0x54, sec_align)
    for s in secs:
        # PointerToRawData := VirtualAddress ; SizeOfRawData := SizeOfImage de sección
        struct.pack_into("<I", buf, s["hdr"] + 20, s["va"])
        struct.pack_into("<I", buf, s["hdr"] + 16, s["vsize"] or s["rsize"])
    # El propio fichero debe empezar en la alineación: los datos ya están en offset==VA
    out_path.write_bytes(bytes(buf[:size_of_image]))
    print(f"  PE reconstruido: {out_path} ({size_of_image} B, base alineada {sec_align:#x})")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--pid", type=int)
    ap.add_argument("--find", default="StarCraft.exe")
    ap.add_argument("--out", required=True)
    ap.add_argument("--name", default="StarCraft.exe")
    ap.add_argument("--base", default=None, help="base de carga en hex (p.ej. 0x400000)")
    args = ap.parse_args()

    pid = args.pid
    if not pid:
        pid, base_s = find_pid(args.find)
        print(f"pid={pid} base={base_s}")
    if args.base:
        base = int(args.base, 16)
    else:
        base = base_from_maps(pid, args.name)
    print(f"base de imagen: {base:#x}")

    img, size_of_image, e_lfanew = read_image(pid, base)
    print(f"imagen leída: {len(img)} B (SizeOfImage {size_of_image:#x})")
    print(f"sha256 imagen: {hashlib.sha256(img).hexdigest()}")

    secs = sections(img, e_lfanew)
    for s in secs:
        start = s["va"]
        data = img[start : start + (s["vsize"] or s["rsize"])]
        print(
            f"  {s['name']:<8} va={s['va']:#08x} vsize={s['vsize']:#08x} "
            f"entropía={entropy(data):.3f}"
        )

    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(img)
    print(f"  imagen cruda: {out}")

    rebuild_pe(img, e_lfanew, secs, out.with_name(out.stem + "_pe.exe"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

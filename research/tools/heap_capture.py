#!/usr/bin/env python3
"""Read-only capture of RW (heap) regions of a Wine/Proton process.

Does not attach a debugger, does not write to the process. Reads /proc/<pid>/mem with
pread and dumps each anonymous or memfd RW region to work/heap_<ts>/<base>.bin,
with a manifest (base, size, perms, origin). By default it skips
system libraries (file-backed with a .so/.dll path) and caps each
region at --cap-miB (truncating it and noting so).

Usage:
  python3 tools/heap_capture.py --pid 12345 --out work/heap_ingame
  python3 tools/heap_capture.py --find StarCraft.exe --cap-miB 256
"""
from __future__ import annotations

import argparse
import hashlib
import os
import struct
from pathlib import Path


def find_pid(name: str) -> int:
    for entry in Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        try:
            cmd = (entry / "cmdline").read_bytes().replace(b"\0", b" ").decode("latin1")
        except OSError:
            continue
        if name.lower() in cmd.lower() and "heap_capture" not in cmd:
            return int(entry.name)
    raise SystemExit(f"ERROR: no process found with ~ {name!r}")


def regions(pid: int, cap: int) -> list[dict]:
    out = []
    for line in Path(f"/proc/{pid}/maps").read_text(errors="replace").splitlines():
        parts = line.split()
        if len(parts) < 5:
            continue
        addr, perms = parts[0], parts[1]
        if not perms.startswith("rw"):
            continue
        path = parts[5] if len(parts) >= 6 else ""
        start_s, end_s = addr.split("-")
        start, end = int(start_s, 16), int(end_s, 16)
        size = end - start
        # Potential heap only: anonymous or memfd (Wine maps the game there).
        # File-backed regions with a system path are skipped unless --all.
        if path and not path.startswith(("/memfd:", "[stack", "[heap")):
            kind = "file"
        elif path.startswith("/memfd:"):
            kind = "memfd"
        else:
            kind = "anon"
        out.append({"base": start, "size": size, "perms": perms, "path": path,
                    "kind": kind, "capped": size > cap,
                    "take": min(size, cap)})
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--pid", type=int)
    ap.add_argument("--find", default="StarCraft.exe")
    ap.add_argument("--out", required=True)
    ap.add_argument("--cap-miB", type=int, default=512)
    ap.add_argument("--kinds", default="anon,memfd",
                    help="kinds to capture, comma-separated")
    args = ap.parse_args()
    pid = args.pid or find_pid(args.find)
    cap = args.cap_miB * 1024 * 1024
    kinds = set(args.kinds.split(","))
    regs = [r for r in regions(pid, cap) if r["kind"] in kinds]
    total = sum(r["take"] for r in regs)
    print(f"pid={pid} regions={len(regs)} bytes~{total} ({total/1048576:.0f} MB)")
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    man = out / "manifest.txt"
    with open(f"/proc/{pid}/mem", "rb", buffering=0) as mem, open(man, "w") as mf:
        for r in regs:
            name = f"{r['base']:012x}.bin"
            data = bytearray()
            chunk = 1 << 20
            left = r["take"]
            off = r["base"]
            try:
                while left > 0:
                    mem.seek(off)
                    blk = mem.read(min(chunk, left))
                    if not blk:
                        break
                    data += blk
                    off += len(blk)
                    left -= len(blk)
            except OSError as ex:
                print(f"  warning {name}: {ex} (partial)")
            p = out / name
            p.write_bytes(bytes(data))
            h = hashlib.sha256(bytes(data)).hexdigest()
            mf.write(f"{name} base={r['base']:#x} size={len(data)} "
                     f"full={r['size']} capped={int(r['capped'])} kind={r['kind']} "
                     f"sha256={h} path={r['path']}\n")
            print(f"  {name} {len(data)} B sha={h[:12]}")
    print(f"manifest: {man}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

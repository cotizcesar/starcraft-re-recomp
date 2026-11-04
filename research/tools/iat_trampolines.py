#!/usr/bin/env python3
"""Name the protected client's imports in a running process (read-only).

The 1.23 client's import directory is zeroed and every IAT slot points to a
private trampoline: `movabs rax, KEY; jmp ...` followed by a chain of
`xor/add/sub/imul rax` steps that ends in `jmp rax`.  This script reads
`/proc/<pid>/mem` with pread (no debugger, no writes), emulates only those
instructions to recover each slot's target, and names targets from the
export tables of the DLLs mapped in the process (read from their files).

Usage:
  python3 research/tools/iat_trampolines.py --pid PID --image-base 0x6ffffb1d0000 \
      [--iat-rva 0xaad000 --slots 512] [--grep Message]
Find the image base by its PE header (TimeDateStamp of the dump); the 64-bit
1.23.10.13515 client's IAT is `.rdata` start, RVA 0xaad000.
"""
import argparse
import json
import os
import struct

M = (1 << 64) - 1


def s32(b):
    return struct.unpack("<i", b)[0]


def resolve(fd, pc, limit=400):
    """Emulate one trampoline; returns (target, None) or (partial, error)."""
    rax, r10 = 0, 0
    for _ in range(limit):
        c = os.pread(fd, 16, pc)
        if c[:2] == b"\x48\xb8":
            rax = struct.unpack("<Q", c[2:10])[0]; pc += 10
        elif c[:2] == b"\x49\xba":
            r10 = struct.unpack("<Q", c[2:10])[0]; pc += 10
        elif c[:2] == b"\x48\x35":
            rax = (rax ^ (s32(c[2:6]) & M)) & M; pc += 6
        elif c[:2] == b"\x48\x05":
            rax = (rax + s32(c[2:6])) & M; pc += 6
        elif c[:2] == b"\x48\x2d":
            rax = (rax - s32(c[2:6])) & M; pc += 6
        elif c[:4] == b"\x49\x0f\xaf\xc2":
            rax = (rax * r10) & M; pc += 4
        elif c[:3] == b"\x4c\x01\xd0":
            rax = (rax + r10) & M; pc += 3
        elif c[:3] == b"\x4c\x29\xd0":
            rax = (rax - r10) & M; pc += 3
        elif c[:3] == b"\x4c\x31\xd0":
            rax = (rax ^ r10) & M; pc += 3
        elif c[:3] == b"\x48\xf7\xd0":
            rax = (~rax) & M; pc += 3
        elif c[:3] == b"\x48\xf7\xd8":
            rax = (-rax) & M; pc += 3
        elif c[:1] == b"\xe9":
            pc = pc + 5 + s32(c[1:5])
        elif c[:1] == b"\xeb":
            pc = pc + 2 + struct.unpack("<b", c[1:2])[0]
        elif c[:2] in (b"\xff\xe0", b"\x50\xc3"):
            return rax, None
        else:
            return rax, c.hex()
    return rax, "limit"


def exports(path, base):
    b = open(path, "rb").read()
    pe = struct.unpack_from("<I", b, 0x3C)[0]
    opt = pe + 24
    magic = struct.unpack_from("<H", b, opt)[0]
    erva, _ = struct.unpack_from("<II", b, opt + (112 if magic == 0x20B else 96))
    if not erva:
        return {}
    nsec = struct.unpack_from("<H", b, pe + 6)[0]
    so = struct.unpack_from("<H", b, pe + 20)[0]
    secs = []
    for i in range(nsec):
        vs, va, rs, ro = struct.unpack_from("<IIII", b, opt + so + 40 * i + 8)
        secs.append((va, max(vs, rs), ro))

    def off(rva):
        for va, sz, ro in secs:
            if va <= rva < va + sz:
                return rva - va + ro
        raise ValueError(rva)

    e = off(erva)
    nfun, nnames, af, an, ao = struct.unpack_from("<IIIII", b, e + 20)
    funcs = struct.unpack_from("<%dI" % nfun, b, off(af))
    names = struct.unpack_from("<%dI" % nnames, b, off(an))
    ords = struct.unpack_from("<%dH" % nnames, b, off(ao))
    out = {}
    for n, o in zip(names, ords):
        no = off(n)
        out[base + funcs[o]] = b[no:b.index(b"\0", no)].decode()
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--pid", type=int, required=True)
    ap.add_argument("--image-base", type=lambda s: int(s, 0), required=True)
    ap.add_argument("--iat-rva", type=lambda s: int(s, 0), default=0xAAD000)
    ap.add_argument("--slots", type=int, default=512)
    ap.add_argument("--grep", default="")
    ap.add_argument("--json")
    a = ap.parse_args()
    fd = os.open(f"/proc/{a.pid}/mem", os.O_RDONLY)
    mods = {}
    for line in open(f"/proc/{a.pid}/maps"):
        p = line.split()
        if len(p) > 5 and p[5].lower().endswith(".dll") and p[2] == "00000000":
            mods.setdefault(p[5], int(p[0].split("-")[0], 16))
    table = {}
    for path, base in mods.items():
        try:
            for addr, name in exports(path, base).items():
                table[addr] = path.split("/")[-1] + "!" + name
        except (OSError, ValueError, struct.error):
            pass
    raw = os.pread(fd, 8 * a.slots, a.image_base + a.iat_rva)
    slots = struct.unpack("<%dQ" % a.slots, raw)
    named, total, unresolved = {}, 0, 0
    for i, v in enumerate(slots):
        if not v:
            continue
        total += 1
        target, err = resolve(fd, v)
        if err:
            target = v  # a direct pointer, not a trampoline
        name = table.get(target)
        if name:
            named[a.iat_rva + 8 * i] = name
        else:
            unresolved += 1
    print(f"slots {total}, named {len(named)}, unnamed {unresolved}, modules {len(mods)}")
    for rva, name in sorted(named.items()):
        if a.grep and a.grep not in name:
            continue
        print(f"rva {rva:#x}  {name}")
    if a.json:
        json.dump({hex(k): v for k, v in named.items()}, open(a.json, "w"), indent=1)


if __name__ == "__main__":
    main()

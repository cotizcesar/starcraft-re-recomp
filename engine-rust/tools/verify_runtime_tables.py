#!/usr/bin/env python3
"""Read-only cross-check of GalaxyCraft's tables against the running game.

This never attaches a debugger, writes target memory, or launches the game.
It reads /proc/<pid>/mem for an already-running 32-bit StarCraft process and
compares the expanded per-field arrays the game built in .data with the
fixtures in fixtures/.

Usage:
  python3 tools/verify_runtime_tables.py [pid]
  python3 tools/verify_runtime_tables.py sc_dump.exe   # rebuilt runtime image

Exit code 0 means every checked array matched byte-for-byte.
"""
import os
import struct
import sys

BASE = 0x79FA0000
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def find_pid():
    for entry in os.listdir("/proc"):
        if not entry.isdigit():
            continue
        try:
            cmd = open(f"/proc/{entry}/cmdline", "rb").read().replace(b"\0", b" ")
        except OSError:
            continue
        if b"StarCraft.exe" in cmd and b"-launch" in cmd:
            return int(entry)
    return None


IMAGE = None


def read(pid, va, size):
    if IMAGE is not None:
        with open(IMAGE, "rb") as f:
            f.seek(va - BASE)
            return f.read(size)
    with open(f"/proc/{pid}/mem", "rb", 0) as f:
        f.seek(va)
        return f.read(size)


def u8_col(data, off, n=228):
    return data[off : off + n]


def u16_col(data, off, n=228):
    return b"".join(data[off + 2 * i : off + 2 * i + 2] for i in range(n))


def u32_col(data, off, n=228):
    return b"".join(data[off + 4 * i : off + 4 * i + 4] for i in range(n))


def main():
    global IMAGE
    pid = None
    if len(sys.argv) > 1 and os.path.isfile(sys.argv[1]):
        IMAGE = sys.argv[1]
    else:
        pid = int(sys.argv[1]) if len(sys.argv) > 1 else find_pid()
    if not pid and IMAGE is None:
        print("no running StarCraft.exe -launch process found", file=sys.stderr)
        return 2
    units = open(os.path.join(ROOT, "fixtures/units.dat"), "rb").read()
    weapons = open(os.path.join(ROOT, "fixtures/weapons.dat"), "rb").read()
    flingy = open(os.path.join(ROOT, "fixtures/flingy.dat"), "rb").read()

    checks = []
    # (name, runtime VA, fixture bytes)
    checks.append(("unit flingy", 0x7AB2DE50, units[0:228]))
    checks.append(("unit turret", 0x7AB2DF38, u16_col(units, 228)))
    checks.append(("unit has_shield", 0x7AB2E800, units[2472:2700]))
    checks.append(("unit shield", 0x7AB2E8E8, u16_col(units, 2700)))
    checks.append(("unit hp", 0x7AB2EAB0, u32_col(units, 3156)))
    checks.append(("unit ground weapon", 0x7AB2F580, units[5892:6120]))
    checks.append(("unit air weapon", 0x7AB2F750, units[6348:6576]))
    checks.append(("unit flags", 0x7AB2FA08, u32_col(units, 7032)))
    checks.append(("unit sight", 0x7AB2FE80, units[8172:8400]))
    checks.append(("unit armor upgrade", 0x7AB2FF68, units[8400:8628]))
    checks.append(("unit size", 0x7AB30050, units[8628:8856]))
    checks.append(("unit armor", 0x7AB30138, units[8856:9084]))
    checks.append(("unit mineral", 0x7AB318C8, u16_col(units, 14860)))
    checks.append(("unit gas", 0x7AB31A90, u16_col(units, 15316)))
    checks.append(("unit build time", 0x7AB31C58, u16_col(units, 15772)))
    checks.append(("unit supply provided", 0x7AB320D0, units[16912:17140]))
    checks.append(("unit supply required", 0x7AB321B8, units[17140:17368]))
    checks.append(("weapon label", 0x7AB332A0, u16_col(weapons, 0, 130)))
    checks.append(("weapon flingy", 0x7AB333A8, u32_col(weapons, 260, 130)))
    checks.append(("weapon target flags", 0x7AB33638, u16_col(weapons, 910, 130)))
    checks.append(("weapon min range", 0x7AB33740, u32_col(weapons, 1170, 130)))
    checks.append(("weapon max range", 0x7AB33948, u32_col(weapons, 1690, 130)))
    checks.append(("weapon upgrade", 0x7AB33B50, weapons[2210:2340]))
    checks.append(("weapon type", 0x7AB33BD8, weapons[2340:2470]))
    checks.append(("weapon cooldown", 0x7AB34320, weapons[4160:4290]))
    checks.append(("weapon damage", 0x7AB34110, u16_col(weapons, 3640, 130)))
    checks.append(("weapon damage bonus", 0x7AB34218, u16_col(weapons, 3900, 130)))
    checks.append(("flingy sprite", 0x7AB1FFA0, u16_col(flingy, 0, 209)))
    checks.append(("flingy top speed", 0x7AB20148, u32_col(flingy, 418, 209)))
    checks.append(("flingy acceleration", 0x7AB20490, u16_col(flingy, 1254, 209)))
    checks.append(("flingy halt", 0x7AB20638, u32_col(flingy, 1672, 209)))
    checks.append(("flingy turn rate", 0x7AB20980, flingy[2508:2717]))
    checks.append(("flingy movement type", 0x7AB20B30, flingy[2926:3135]))

    # orders.dat expansion.  The runtime loader pads each per-field byte
    # array to 0xc0 bytes (192 entries) and each u16 column to 378 bytes.
    orders = open(os.path.join(ROOT, "fixtures/orders.dat"), "rb").read()
    on = 189
    checks.append(("order labels", 0x7AB2A090, orders[0 : on * 2]))
    for field in range(15):
        off = on * 2 + field * on
        checks.append(
            (
                f"order flag {field}",
                0x7AB2A210 + field * 0xC0,
                orders[off : off + on],
            )
        )
    highlight = on * 2 + 15 * on
    checks.append(("order highlight", 0x7AB2AD50, orders[highlight : highlight + on * 2]))
    dep = highlight + on * 2
    checks.append(("order dependency", 0x7AB2AED0, orders[dep : dep + on * 2]))
    target = dep + on * 2
    checks.append(("order target-order", 0x7AB2B050, orders[target : target + on]))

    # techdata.dat expansion: 44 entries, u16 columns with an 88-byte stride,
    # the u32 `unknown` column and the trailing byte columns.
    tech = open(os.path.join(ROOT, "fixtures/techdata.dat"), "rb").read()
    tn = 44
    checks.append(("tech mineral", 0x7AB2DB08, u16_col(tech, 0, tn)))
    checks.append(("tech gas", 0x7AB2DB60, u16_col(tech, tn * 2, tn)))
    checks.append(("tech time", 0x7AB2DBB8, u16_col(tech, tn * 4, tn)))
    checks.append(("tech energy", 0x7AB2DC10, u16_col(tech, tn * 6, tn)))
    checks.append(("tech unknown", 0x7AB2DC68, u32_col(tech, tn * 8, tn)))
    checks.append(("tech icon", 0x7AB2DD18, u16_col(tech, tn * 12, tn)))
    checks.append(("tech label", 0x7AB2DD70, u16_col(tech, tn * 14, tn)))
    checks.append(("tech race", 0x7AB2DDC8, tech[tn * 16 : tn * 17]))
    checks.append(("tech flags", 0x7AB2DDF4, u16_col(tech, tn * 17, tn)))

    # upgrades.dat expansion: 61 entries.  u16 fields use an 0x80-byte stride;
    # the three trailing byte columns are 61 bytes each, 0x40 apart.
    upgrades = open(os.path.join(ROOT, "fixtures/upgrades.dat"), "rb").read()
    un = 61
    for index, name in enumerate(
        [
            "mineral base",
            "mineral factor",
            "gas base",
            "gas factor",
            "time base",
            "time factor",
            "unknown",
            "icon",
            "label",
        ]
    ):
        checks.append(
            (
                f"upgrade {name}",
                0x7AB32D60 + index * 0x80,
                u16_col(upgrades, index * un * 2, un),
            )
        )
    trailer = un * 2 * 9
    checks.append(("upgrade race", 0x7AB331DC, upgrades[trailer : trailer + un]))
    checks.append(
        ("upgrade max level", 0x7AB3321C, upgrades[trailer + un : trailer + 2 * un])
    )
    checks.append(
        ("upgrade broodwar", 0x7AB3325C, upgrades[trailer + 2 * un : trailer + 3 * un])
    )

    failures = 0
    for name, va, expected in checks:
        actual = read(pid, va, len(expected))
        ok = actual == expected
        print(f"[{'ok' if ok else 'MISMATCH'}] {name:26s} {len(expected):5d} bytes @ {va:#x}")
        if not ok:
            failures += 1
            for i in range(0, min(len(actual), len(expected))):
                if actual[i] != expected[i]:
                    print(f"    first difference at byte {i}: {actual[i]:#x} != {expected[i]:#x}")
                    break
    print(f"\n{len(checks) - failures}/{len(checks)} arrays match the running game")
    return 0 if failures == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())

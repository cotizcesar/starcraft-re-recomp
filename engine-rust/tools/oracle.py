#!/usr/bin/env python3
"""Independent oracle for the StarCraft 1.23 column-major DAT tables.

This script is intentionally separate from the Rust implementation.  It is
used to produce golden files consumed by the Rust test-suite, so a mistake in
one implementation is caught by the other.

Usage:
    python3 tools/oracle.py fixtures
"""

from __future__ import annotations

import hashlib
import struct
import sys
from pathlib import Path

UNIT_COUNT = 228
WEAPON_COUNT = 130

# units.dat field offsets (column-major, 228 entries unless noted).
U = {
    "flingy": (0, 1),
    "turret": (228, 2),
    "has_shield": (2472, 1),
    "shield": (2700, 2),
    "hp": (3156, 4),
    "elevation": (4068, 1),
    "ground_weapon": (5892, 1),
    "max_ground_hits": (6120, 1),
    "air_weapon": (6348, 1),
    "max_air_hits": (6576, 1),
    "flags": (7032, 4),
    "target_acq": (7944, 1),
    "sight": (8172, 1),
    "armor_upgrade": (8400, 1),
    "size": (8628, 1),
    "armor": (8856, 1),
    "portrait": (14404, 2),
    "mineral": (14860, 2),
    "gas": (15316, 2),
    "build_time": (15772, 2),
    "group_flags": (16684, 1),
    "supply_provided": (16912, 1),
    "supply_required": (17140, 1),
    "space_required": (17368, 1),
    "space_provided": (17596, 1),
    "build_score": (17824, 2),
    "destroy_score": (18280, 2),
}

W = {
    "label": (0, 2),
    "flingy": (260, 4),
    "target_flags": (910, 2),
    "min_range": (1170, 4),
    "max_range": (1690, 4),
    "damage_upgrade": (2210, 1),
    "damage_type": (2340, 1),
    "bullet_type": (2470, 1),
    "lifetime": (2600, 1),
    "explosion": (2730, 1),
    "inner_splash": (2860, 2),
    "middle_splash": (3120, 2),
    "outer_splash": (3380, 2),
    "damage": (3640, 2),
    "damage_bonus": (3900, 2),
    "cooldown": (4160, 1),
    "bullets": (4290, 1),
    "attack_angle": (4420, 1),
    "launch_spin": (4550, 1),
    "forward_offset": (4680, 1),
    "vertical_offset": (4810, 1),
    "target_error": (4940, 2),
    "icon": (5200, 2),
}

# flingy.dat field offsets (column-major, 209 entries).
F = {
    "sprite": (0, 2),
    "top_speed": (418, 4),
    "acceleration": (1254, 2),
    "halt_distance": (1672, 4),
    "turn_rate": (2508, 1),
    "movement_type": (2926, 1),
}


def read_col(data: bytes, base: int, size: int, index: int) -> int:
    off = base + size * index
    if size == 1:
        return data[off]
    if size == 2:
        return struct.unpack_from("<H", data, off)[0]
    if size == 4:
        return struct.unpack_from("<I", data, off)[0]
    raise ValueError(size)


def read_signed(data: bytes, base: int, size: int, index: int) -> int:
    off = base + size * index
    if size == 1:
        return struct.unpack_from("<b", data, off)[0]
    if size == 2:
        return struct.unpack_from("<h", data, off)[0]
    if size == 4:
        return struct.unpack_from("<i", data, off)[0]
    raise ValueError(size)


def parse_units(path: Path):
    data = path.read_bytes()
    rows = []
    for i in range(UNIT_COUNT):
        row = {name: read_col(data, base, size, i) for name, (base, size) in U.items()}
        row["id"] = i
        rows.append(row)
    return rows


def parse_weapons(path: Path):
    data = path.read_bytes()
    rows = []
    for i in range(WEAPON_COUNT):
        row = {name: read_col(data, base, size, i) for name, (base, size) in W.items()}
        row["id"] = i
        rows.append(row)
    return rows


FLINGY_COUNT = 209
ORDER_COUNT = 189


def parse_orders(path: Path):
    data = path.read_bytes()
    labels = 0
    flags = ORDER_COUNT * 2
    highlight = flags + 15 * ORDER_COUNT
    dep = highlight + ORDER_COUNT * 2
    target = dep + ORDER_COUNT * 2
    rows = []
    for i in range(ORDER_COUNT):
        row = {
            "id": i,
            "label": read_col(data, labels, 2, i),
            "highlight": read_signed(data, highlight, 2, i),
            "dependency": read_col(data, dep, 2, i),
            "target_order": data[target + i],
        }
        for f in range(15):
            row[f"flag{f}"] = data[flags + f * ORDER_COUNT + i]
        rows.append(row)
    return rows


def parse_flingies(path: Path):
    data = path.read_bytes()
    rows = []
    signed = {"acceleration", "top_speed", "halt_distance"}
    for i in range(FLINGY_COUNT):
        row = {}
        for name, (base, size) in F.items():
            row[name] = (
                read_signed(data, base, size, i)
                if name in signed
                else read_col(data, base, size, i)
            )
        row["id"] = i
        rows.append(row)
    return rows


def size_multiplier(damage_type: int, size: int) -> int:
    """Numerator over 256.  Size 0 takes no damage."""
    if damage_type == 0:
        return 0
    if size == 0:
        return 0
    if damage_type == 1:  # explosive
        return {1: 128, 2: 192, 3: 256}[size]
    if damage_type == 2:  # concussive
        return {1: 256, 2: 128, 3: 64}[size]
    return 256  # normal / ignore armour


def resolve_damage(
    weapon: dict,
    upgrade: int,
    size: int,
    armor: int,
    has_shield: bool,
    shield_points: int,
    shield_upgrade: int,
):
    """Reference implementation of the recovered weapon-damage routine."""
    damage = (weapon["damage"] + weapon["damage_bonus"] * upgrade) * 256
    damage = max(damage, 128)
    shield_damage = 0
    if has_shield and shield_points >= 256:
        if weapon["damage_type"] != 4:
            shield_armor = shield_upgrade * 256
            if damage > shield_armor:
                damage -= shield_armor
            else:
                damage = 128
        shield_damage = min(damage, shield_points)
        damage -= shield_damage
    if weapon["damage_type"] != 4:
        armor = armor * 256
        if damage > armor:
            damage -= armor
        else:
            damage = 0
    damage = damage * size_multiplier(weapon["damage_type"], size) // 256
    if shield_damage == 0 and damage < 128:
        damage = 128
    return shield_damage, max(damage, 0)


def write_tsv(path: Path, columns, rows):
    with path.open("w", encoding="ascii", newline="\n") as f:
        f.write("\t".join(columns) + "\n")
        for row in rows:
            f.write("\t".join(str(row.get(c, "")) for c in columns) + "\n")


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    root = Path(sys.argv[1])
    units = parse_units(root / "units.dat")
    weapons = parse_weapons(root / "weapons.dat")
    flingies = parse_flingies(root / "flingy.dat")
    orders = parse_orders(root / "orders.dat")

    unit_cols = [
        "id", "flingy", "turret", "has_shield", "shield", "hp", "elevation",
        "ground_weapon", "max_ground_hits", "air_weapon", "max_air_hits",
        "flags", "target_acq", "sight", "armor_upgrade", "size", "armor",
        "portrait", "mineral", "gas", "build_time", "group_flags",
        "supply_provided", "supply_required", "space_required",
        "space_provided", "build_score", "destroy_score",
    ]
    weapon_cols = [
        "id", "label", "flingy", "target_flags", "min_range", "max_range",
        "damage_upgrade", "damage_type", "bullet_type", "lifetime", "explosion",
        "inner_splash", "middle_splash", "outer_splash", "damage",
        "damage_bonus", "cooldown", "bullets", "attack_angle", "launch_spin",
        "forward_offset", "vertical_offset", "target_error", "icon",
    ]
    write_tsv(root / "oracle_units.tsv", unit_cols, units)
    write_tsv(root / "oracle_weapons.tsv", weapon_cols, weapons)
    flingy_cols = [
        "id", "sprite", "top_speed", "acceleration", "halt_distance",
        "turn_rate", "movement_type",
    ]
    write_tsv(root / "oracle_flingy.tsv", flingy_cols, flingies)
    order_cols = ["id", "label", "highlight", "dependency", "target_order"] + [
        f"flag{i}" for i in range(15)
    ]
    write_tsv(root / "oracle_orders.tsv", order_cols, orders)

    # A deterministic cross-product over every shipped weapon.  The first
    # pass sampled ten representative weapons; this version covers all 130 so
    # a weapon-specific damage/type row cannot hide behind the sample.
    damage_cols = [
        "weapon", "upgrade", "size", "armor", "shield_points",
        "shield_upgrade", "shield_damage", "hp_damage",
    ]
    damage_rows = []
    for wid in range(WEAPON_COUNT):
        for upgrade in range(4):
            for size in (1, 2, 3):
                for armor in range(4):
                    for shield_points in (0, 60 * 256, 4 * 256):
                        for shield_upgrade in range(4):
                            sd, hd = resolve_damage(
                                weapons[wid], upgrade, size, armor,
                                True, shield_points, shield_upgrade,
                            )
                            damage_rows.append({
                                "weapon": wid,
                                "upgrade": upgrade,
                                "size": size,
                                "armor": armor,
                                "shield_points": shield_points,
                                "shield_upgrade": shield_upgrade,
                                "shield_damage": sd,
                                "hp_damage": hd,
                            })
    write_tsv(root / "oracle_damage.tsv", damage_cols, damage_rows)

    with (root / "SHA256SUMS").open("w", encoding="ascii", newline="\n") as f:
        for name in ("units.dat", "weapons.dat", "flingy.dat", "damage_multipliers.bin"):
            f.write(f"{hashlib.sha256((root / name).read_bytes()).hexdigest()}  {name}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

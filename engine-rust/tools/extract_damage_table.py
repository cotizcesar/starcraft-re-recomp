#!/usr/bin/env python3
"""Extract the 5x5 damage-type/size multiplier table from StarCraft.exe.

The table lives at file offset 0xb05d7c in StarCraft 1.23.10.13515 (x86).
Usage:
    python3 tools/extract_damage_table.py /path/to/StarCraft.exe fixtures
"""

from __future__ import annotations

import hashlib
import sys
from pathlib import Path

OFFSET = 0xB05D7C
LENGTH = 100  # 25 little-endian int32 values


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2
    exe = Path(sys.argv[1])
    out_dir = Path(sys.argv[2])
    data = exe.read_bytes()
    table = data[OFFSET : OFFSET + LENGTH]
    if len(table) != LENGTH:
        print("executable is too short", file=sys.stderr)
        return 1
    out = out_dir / "damage_multipliers.bin"
    out.write_bytes(table)
    print(f"{out}: {LENGTH} bytes, sha256={hashlib.sha256(table).hexdigest()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

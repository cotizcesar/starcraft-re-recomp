#!/usr/bin/env python3
"""Inventory the trigger condition/action ids used by the shipped maps.

The 1.23 trigger VM has classic ids (conditions 0..=23, actions 1..=59) and
Remastered/EUD-only extensions.  This script parses every extracted
`scenario.chk`, resynchronises on a valid `TRIG` section (length a multiple of
2400), and reports the ids actually present, so the reconstruction can state
which paths the retail content exercises.

Usage:
    python3 tools/trigger_ids.py /tmp/sc_maps
"""

import collections
import os
import struct
import sys


def trig_payload(data: bytes):
    """Return the first structurally valid TRIG payload, or None."""
    i = 0
    while True:
        j = data.find(b"TRIG", i)
        if j < 0:
            return None
        if j + 8 <= len(data):
            size = struct.unpack_from("<I", data, j + 4)[0]
            if size and size % 2400 == 0 and j + 8 + size <= len(data):
                return data[j + 8 : j + 8 + size]
        i = j + 4


def main(root: str) -> int:
    conditions = collections.Counter()
    actions = collections.Counter()
    maps = 0
    with_trig = 0
    for dirpath, _dirs, files in os.walk(root):
        for name in files:
            if not name.lower().endswith(".chk"):
                continue
            path = os.path.join(dirpath, name)
            with open(path, "rb") as f:
                data = f.read()
            maps += 1
            payload = trig_payload(data)
            if payload is None:
                continue
            with_trig += 1
            for record in range(len(payload) // 2400):
                rec = payload[record * 2400 : (record + 1) * 2400]
                for c in range(16):
                    kind = rec[c * 20 + 15]
                    if kind:
                        conditions[kind] += 1
                for a in range(64):
                    kind = rec[320 + a * 32 + 26]
                    if kind:
                        actions[kind] += 1
    print(f"maps: {maps}, with TRIG: {with_trig}")
    print("conditions:", sorted(conditions.items()))
    print("actions:", sorted(actions.items()))
    if conditions:
        print(
            "range: conditions {}..{}, actions {}..{}".format(
                min(conditions), max(conditions), min(actions), max(actions)
            )
        )
    return 0


if __name__ == "__main__":
    if len(sys.argv) != 2:
        print(__doc__)
        raise SystemExit(2)
    raise SystemExit(main(sys.argv[1]))

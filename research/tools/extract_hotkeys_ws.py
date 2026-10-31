#!/usr/bin/env python3
"""Dump the retail Hotkeys bindings from the game's CEF WebSocket.

The shipped `webui` only contains action ids/icons; the key table arrives from
the native client as `OnBindingsChanged`.  This tool finds the live
`HotkeyPanel.html?port=...&guid=...` URL in the Wine browser cache, opens the
panel route, and saves every message (read-only; never touches the install).

Usage:
  SC_PREFIX="$HOME/Games/battlenet" python3 research/tools/extract_hotkeys_ws.py \
      --out research/analysis/hotkeys-bindings-retail.json

Requires a running game with the Hotkeys panel open and `websockets`.
"""
from __future__ import annotations

import argparse
import asyncio
import json
import re
import sys
from pathlib import Path

try:
    import websockets
except ImportError:  # pragma: no cover
    sys.exit("pip install websockets")


def find_panel_url(prefix: Path) -> tuple[int, int]:
    cache = (
        prefix
        / "drive_c/users/steamuser/AppData/Local/Temp/blizzard_browser_cache"
    )
    candidates: list[tuple[int, int]] = []
    for path in cache.glob("data_*"):
        data = path.read_bytes()
        for match in re.finditer(
            rb"http://127\.0\.0\.1:(\d+)/gamedata/webui/dist/HotkeyPanel/"
            rb"HotkeyPanel\.html[^\x00\r\n]*guid=(\d+)",
            data,
        ):
            candidates.append((int(match.group(1)), int(match.group(2))))
    if not candidates:
        sys.exit(f"no HotkeyPanel URL in {cache}")
    return candidates[-1]


async def dump(port: int, guid: int, out: Path) -> None:
    uri = f"ws://127.0.0.1:{port}/panel/HotkeyPanel/{guid}"
    messages = []
    async with websockets.connect(uri, max_size=None) as ws:
        while True:
            try:
                raw = await asyncio.wait_for(ws.recv(), timeout=3)
            except asyncio.TimeoutError:
                break
            messages.append(json.loads(raw))
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(messages, indent=1))
    for message in messages:
        data = message.get("data", {})
        print(data.get("endpoint"), len(json.dumps(data.get("data"))))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--prefix",
        type=Path,
        default=Path.home() / "Games/battlenet",
        help="Wine prefix containing the retail install",
    )
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    port, guid = find_panel_url(args.prefix)
    print(f"HotkeyPanel ws://127.0.0.1:{port}/panel/HotkeyPanel/{guid}")
    asyncio.run(dump(port, guid, args.out))


if __name__ == "__main__":
    main()

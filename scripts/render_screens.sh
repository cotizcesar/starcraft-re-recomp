#!/usr/bin/env bash
# Render every client screen headlessly at the reference resolution (21:9).
# Output: /tmp/shots/<screen>.png  (BMP intermediate is deleted)
set -u
cd "$(dirname "${BASH_SOURCE[0]}")/../engine-rust" || exit 1
INSTALL="${SC_INSTALL:-$HOME/Games/battlenet/drive_c/Program Files (x86)/StarCraft}"
OUT="${1:-/tmp/shots}"
W="${W:-3440}"
H="${H:-1440}"
mkdir -p "$OUT"
BIN=./target/release/sc_view
[ -x "$BIN" ] || { echo "build sc_view first"; exit 1; }

for s in intro menu registry gametype episode mission lobby options \
         options-speed options-sound options-video options-game options-network hotkeys hotkeys-terran hotkeys-protoss hotkeys-zerg ingame \
         collections credits saved replays lan gamemenu objectives tips \
         ingame-terran ingame-terran2 ingame-zerg ingame-protoss \
         briefing pause victory defeat score leaderboard stats; do
  SC_NO_AUDIO=1 timeout 200 "$BIN" --install "$INSTALL" --width "$W" --height "$H" \
      --shot "$OUT/$s.bmp" --shot-screen "$s" >/dev/null 2>&1
  rc=$?
  if [ -f "$OUT/$s.bmp" ]; then
    python3 - "$OUT/$s.bmp" "$OUT/$s.png" <<'PY'
import sys
from PIL import Image
Image.open(sys.argv[1]).convert("RGB").save(sys.argv[2])
PY
    rm -f "$OUT/$s.bmp"
    echo "OK   $s (rc=$rc)"
  else
    echo "FAIL $s (rc=$rc)"
  fi
done

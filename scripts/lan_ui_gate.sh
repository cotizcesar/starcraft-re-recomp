#!/usr/bin/env bash
# LAN UI gate: the real `sc_view` client (menu script) against independent
# peers on loopback.  Three scenarios, one report:
#   1. hosted lobby on a (4) map takes two joins (lan_probe_client.py), the
#      panel lists the seats, ENTER starts a 3-player match, each client is
#      told its own peer index;
#   2. both clients drop after the start and rejoin: the host pauses, accepts
#      the Rejoin and sends a Resync;
#   3. sc_view joins an sc_lan host that is then killed: the client retries on
#      the exponential backoff schedule.
# Usage: scripts/lan_ui_gate.sh "$SC_INSTALL" [report]
set -euo pipefail
INSTALL="${1:?usage: lan_ui_gate.sh INSTALL [report]}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPORT="${2:-$ROOT/engine-rust/docs/gates/lan-lobby-reconnect.txt}"
BIN="$ROOT/engine-rust/target/release"
PROBE="$ROOT/scripts/lan_probe_client.py"
(cd "$ROOT/engine-rust" && cargo build --release --quiet --bin sc_view --bin sc_lan)
CACHE="${GALAXYCRAFT_CACHE:-$(ls -d "${XDG_CACHE_HOME:-$HOME/.cache}"/galaxycraft/retail-* | head -1)}"
MAP="$(ls "$CACHE"/maps/ladder/*/'(4)Allegro_1.1b.chk' | head -1)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
NAV="move:700:700,click,move:1103:618,key:L,fixture:lan-map:4"
run_view() { # $1 = out dir, $2 = script
  mkdir -p "$1"
  SC_NO_AUDIO=1 GC_WALK_DIR="$1" "$BIN/sc_view" --install "$INSTALL" --no-intro \
    --width 2560 --height 1440 --menu-at 2.0 --menu-script "$2" >/dev/null 2>&1
}
state() { cut -f1,2,5 "$1/walk.tsv" | sed 's| map=[^\t]*status=| status=|'; }
{
  echo "# LAN UI gate ($(date -u +%Y-%m-%dT%H:%MZ)), map: $(basename "$MAP")"
  echo
  echo "## 1. Lobby with more than two players"
  python3 -I "$PROBE" 127.0.0.1 7777 "$MAP" 1 4 > "$TMP/p1" &
  (sleep 0.7; python3 -I "$PROBE" 127.0.0.1 7777 "$MAP" 2 4 > "$TMP/p2") &
  run_view "$TMP/w1" "$NAV,key:H,wait:3500,key:Enter,wait:300"
  wait
  state "$TMP/w1" | tail -4
  cat "$TMP/p1" "$TMP/p2"
  echo
  echo "## 2. Host: dropped peers rejoin"
  python3 -I "$PROBE" 127.0.0.1 7777 "$MAP" 1 4.5 1.5 > "$TMP/r1" &
  (sleep 0.7; python3 -I "$PROBE" 127.0.0.1 7777 "$MAP" 2 4.5 3.0 > "$TMP/r2") &
  run_view "$TMP/w2" "$NAV,key:H,wait:3500,key:Enter$(printf ',wait:1000%.0s' 1 2 3 4 5 6)"
  wait
  state "$TMP/w2" | tail -7
  cat "$TMP/r1" "$TMP/r2"
  echo
  echo "## 3. Client: reconnect with backoff after the host dies"
  "$BIN/sc_lan" --host --port 7791 --races terran,protoss --chk "$MAP" \
    --vf4 "$CACHE/TileSet/badlands.vf4" --ticks 24000 > "$TMP/host" 2>&1 &
  HOST=$!
  (while ! grep -q "all peers joined" "$TMP/host"; do sleep 0.1; done; sleep 2; kill "$HOST") &
  run_view "$TMP/w3" "$NAV,fixture:lan-peer:7791,key:Enter$(printf ',wait:1000%.0s' 1 2 3 4 5 6 7 8 9)"
  wait || true
  head -2 "$TMP/host"
  state "$TMP/w3" | tail -10
} > "$REPORT"
cat "$REPORT"
grep -q "lobby=3/4" "$REPORT" && grep -q "started with 3 players" "$REPORT" &&
  grep -q "you=1" "$REPORT" && grep -q "you=2" "$REPORT" &&
  grep -q "rejoined" "$REPORT" && grep -q "RESYNC kind=9" "$REPORT" &&
  grep -q "reconnect:Some(6)" "$REPORT" && echo "lan ui gate: PASS"

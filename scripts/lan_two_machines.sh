#!/usr/bin/env bash
# LAN acceptance harness: run the same command on two machines and compare
# the printed LAN_RESULT line. Both must show the same tick and hash.
#
#   PC 1:  scripts/lan_two_machines.sh host  192.168.1.20 terran  "/path/map.chk" "/path/platform.vf4"
#   PC 2:  scripts/lan_two_machines.sh join  192.168.1.20 protoss "/path/map.chk" "/path/platform.vf4"
#
# Optional 6th argument: tcp (default) or udp.
set -euo pipefail

ROLE="${1:?role: host|join}"
HOST_IP="${2:?host ip or address}"
RACE="${3:-terran}"
MAP="${4:?path to scenario.chk}"
VF4="${5:?path to <tileset>.vf4}"
TRANSPORT="${6:-tcp}"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/engine-rust"

PORT="${LAN_PORT:-7777}"
TICKS="${LAN_TICKS:-28800}"   # 20 minutes of game time at 24 Hz

echo "building (release)..."
cargo build --release --quiet --bin sc_lan

UDP_FLAG=""
if [ "$TRANSPORT" = "udp" ]; then UDP_FLAG="--udp"; fi

common=(--chk "$MAP" --vf4 "$VF4" --ticks "$TICKS" $UDP_FLAG)

if [ "$ROLE" = "host" ]; then
  echo "hosting on :$PORT (transport=$TRANSPORT, ticks=$TICKS)"
  ./target/release/sc_lan --host --port "$PORT" --races "$RACE,protoss" "${common[@]}" \
    | tee /tmp/lan_host.log
  grep -h "final hash" /tmp/lan_host.log | tail -1 | sed 's/^/LAN_RESULT /'
else
  echo "joining $HOST_IP:$PORT (transport=$TRANSPORT, race=$RACE, ticks=$TICKS)"
  ./target/release/sc_lan --join "$HOST_IP:$PORT" --race "$RACE" "${common[@]}" \
    | tee /tmp/lan_join.log
  grep -h "final hash" /tmp/lan_join.log | tail -1 | sed 's/^/LAN_RESULT /'
fi

cat <<'EOF'

PASS criteria (compare both machines):
  * both exit 0
  * no "DESYNC" line in either log
  * the two LAN_RESULT lines end with the same "final hash <hex>" value
EOF

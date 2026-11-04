#!/usr/bin/env python3
"""Minimal LAN protocol-2 client, independent of the Rust code (test probe).

Joins a host, prints the `Setup` it answers with, and optionally drops the
link and comes back with `Rejoin`, printing the `Resync` header.

  lan_probe_client.py HOST PORT MAP.chk RACE [HOLD_S] [BACK_AFTER_S]

RACE: 0 Zerg, 1 Terran, 2 Protoss.  Without BACK_AFTER_S it holds the
socket HOLD_S seconds and exits.  Wire format: `engine-rust/src/net.rs`
(`u32 length` + `u16 protocol, u8 kind, ...`).
"""
import hashlib
import socket
import struct
import sys
import time

PROTOCOL = 2
SEED = 0x1234_5678_9ABC_DEF0


def connect(host, port):
    deadline = time.time() + 30
    while True:
        try:
            return socket.create_connection((host, port), timeout=30)
        except ConnectionRefusedError:
            if time.time() > deadline:
                raise
            time.sleep(0.1)


def send(sock, payload):
    sock.sendall(struct.pack("<I", len(payload)) + payload)


def recv(sock):
    def exact(n):
        buf = b""
        while len(buf) < n:
            part = sock.recv(n - len(buf))
            if not part:
                raise SystemExit("host closed")
            buf += part
        return buf

    return exact(struct.unpack("<I", exact(4))[0])


def main():
    host, port, chk, race = sys.argv[1], int(sys.argv[2]), sys.argv[3], int(sys.argv[4])
    hold = float(sys.argv[5]) if len(sys.argv) > 5 else 1.0
    back_after = float(sys.argv[6]) if len(sys.argv) > 6 else None
    map_hash = hashlib.sha256(open(chk, "rb").read()).digest()

    sock = connect(host, port)
    # Join: kind 1, u16 protocol, 32 B map hash, u64 seed, u8 race.
    send(sock, struct.pack("<HBH", PROTOCOL, 1, PROTOCOL) + map_hash + struct.pack("<QB", SEED, race))
    msg = recv(sock)
    proto, kind, peers, you = struct.unpack_from("<HBBB", msg, 0)
    count = struct.unpack_from("<I", msg, 5 + 32 + 8)[0]
    races = list(msg[5 + 32 + 8 + 4: 5 + 32 + 8 + 4 + count])
    print(f"SETUP proto={proto} kind={kind} peers={peers} you={you} races={races}", flush=True)
    time.sleep(hold)
    sock.close()
    if back_after is None:
        return
    print("DROPPED", flush=True)
    time.sleep(back_after)
    sock = connect(host, port)
    # Rejoin: kind 10, u8 peer, 32 B map hash, u64 seed.
    send(sock, struct.pack("<HBB", PROTOCOL, 10, you) + map_hash + struct.pack("<Q", SEED))
    msg = recv(sock)
    kind = msg[2]
    turn, size = struct.unpack_from("<II", msg, 3)
    print(f"RESYNC kind={kind} turn={turn} log_bytes={size}", flush=True)
    time.sleep(1.0)
    sock.close()


if __name__ == "__main__":
    main()

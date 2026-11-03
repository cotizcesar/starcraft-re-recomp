# starcraft-re-recomp

[![CI](https://github.com/cotizcesar/starcraft-re-recomp/actions/workflows/ci.yml/badge.svg)](https://github.com/cotizcesar/starcraft-re-recomp/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Progress](https://img.shields.io/badge/progress-see%20PROGRESS.md-orange.svg)](PROGRESS.md)

A **verifiable, reproducible reconstruction** of StarCraft 1.23.10.13515
(Remastered client build), written in Rust and driven by evidence recovered
from the shipped executable and its CASC/MPQ data.

The repository contains **no game assets**. The playable client reads the
player's own legal StarCraft installation at runtime and never modifies it
(see [NOTICE.md](NOTICE.md)).

> **Where are we?** See **[PROGRESS.md](PROGRESS.md)** for what already
> works, what is missing and the completion percentage of each area. The
> table there is generated from checklists and checked by CI.

## Highlights

- **Full-fidelity rules engine**: 228 units, 130 weapons, 189 orders and 137
  order handlers transcribed from the binary. Damage, projectiles, A*
  movement, economy, tech tree, spells, triggers, `iscript` and campaign AI
  are all deterministic.
- **Data verified against the live game**: 73/73 runtime tables identical.
  All 323 shipped melee maps and 68 campaign missions load and simulate
  (391/391, 0 failures).
- **Playable client `sc_view`**: Remastered menus, campaign, briefings, HUD
  per race, minimap, hotkeys, fog of war, saves, replays, 16:9 to 4K and
  ultrawide.
- **LAN multiplayer**: command-only lockstep over TCP or UDP, with discovery,
  a lobby and resync. Two processes stay hash-identical over 2400 ticks.
- **Reverse-engineering notes** with FACT vs INFERENCE discipline: memory
  dumps, Ghidra projects, an architecture map and a dated log
  ([research/docs](research/docs/README.md)).

## Repository layout

| Path | Contents |
|---|---|
| [`engine-rust/`](engine-rust/README.md) | The engine and every binary (`sc_view`, `sc_lan`, `sc_assets`, …). Crate rules are in [`engine-rust/AGENTS.md`](engine-rust/AGENTS.md) |
| [`scripts/`](scripts/README.md) | Harnesses: visual gates, headless renders, LAN acceptance, `progress.py` |
| [`research/`](research/docs/README.md) | Reverse engineering: docs, install manifests, Ghidra/capture tools, history of the removed C engine |
| [`PROGRESS.md`](PROGRESS.md) | What we have and what is missing, with percentages |
| [`AGENTS.md`](AGENTS.md) | The project contract: principles, rules, Definition of Done |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | How to build, test and submit changes |
| [`CHANGELOG.md`](CHANGELOG.md) | Notable changes |

## Quick start

Requirements: Rust (stable, 2021 edition), a C compiler (for the vendored
PKWARE DCL decoder) and, to play, your own StarCraft: Remastered
installation. On Linux the client uses X11 and ALSA; both are loaded at
runtime.

```bash
git clone https://github.com/cotizcesar/starcraft-re-recomp.git
cd starcraft-re-recomp/engine-rust

# Build, lint and run the client from your own install
cargo build --release
cargo clippy --all-targets -- -D warnings
cargo run --release --bin sc_view -- --install "/path/to/StarCraft"

# Headless screenshot (no window, no audio)
SC_NO_AUDIO=1 cargo run --release --bin sc_view -- \
  --install "/path/to/StarCraft" --shot /tmp/pause.bmp --shot-screen pause
```

The full test suite needs developer fixtures, which are Blizzard data. They
are not in the repository, so you generate them once from your own install
([`engine-rust/fixtures/README.md`](engine-rust/fixtures/README.md)):

```bash
export SC_INSTALL="/path/to/StarCraft"
cargo run --release --bin sc_assets -- --install "$SC_INSTALL" fixtures --out fixtures
python3 tools/extract_damage_table.py "$SC_INSTALL/x86/StarCraft.exe" fixtures
python3 tools/oracle.py fixtures
cargo test
```

### LAN

```bash
# PC 1 (host) and PC 2 (client), same map on both
cargo run --release --bin sc_lan -- --host --chk map.chk --vf4 platform.vf4 \
  --races terran,protoss --ticks 28800
cargo run --release --bin sc_lan -- --join 192.168.1.20:7777 \
  --chk map.chk --vf4 platform.vf4 --race protoss
# add --udp on both sides for the UDP transport

# Two-machine acceptance test (compares the printed LAN_RESULT lines)
scripts/lan_two_machines.sh host 192.168.1.20 terran  map.chk platform.vf4 tcp
scripts/lan_two_machines.sh join 192.168.1.20 protoss map.chk platform.vf4 tcp
```

In the client: main menu → game type → **L** opens the LAN lobby (**S** scan,
**H** host, **Enter** join).

### Keys

| Key | Action |
|---|---|
| F5 / F6 | Save game / save replay |
| Space | Pause / play replay |
| `+` / `-` | Replay speed |
| R | Restart / retry |
| Esc | Menu / back |
| Enter / Del | Load or play / delete (Saved Games, Replays) |

## How the project works

- **Evidence before assumption.** Every finding records the file, address,
  confidence and the experiment that would refute it. Falsifications are
  published too ([session log](research/docs/session-log.md)).
- **Determinism.** The simulation is a fixed-seed LCG. Saves, replays and
  LAN are all a command log plus deterministic fast-forward, and tests
  prove hash equality.
- **Definition of Done.** `cargo test` 0 failed, clippy 0 warnings,
  `cargo fmt --check` clean, `scripts/progress.py --check` OK, and the docs
  updated. Details are in [AGENTS.md §7](AGENTS.md#7-definition-of-done-verifiable-no-exceptions).

## Scope

In scope: offline single player, the full campaign, skirmish against the AI,
saves and replays, and LAN multiplayer. **Out of scope:** Battle.net and
online services, the map/campaign editor, and other game patches.

## Contributing

Issues and pull requests are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md)
and the [Code of Conduct](CODE_OF_CONDUCT.md) first. Never attach retail game
files, screenshots or decompiled Blizzard code to an issue or PR. Security
reports: [SECURITY.md](SECURITY.md).

## License

Code is dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. The vendored PKWARE DCL decoder keeps its own license
([`engine-rust/third_party/pklib/LICENSE`](engine-rust/third_party/pklib/LICENSE)).

StarCraft, Brood War and Blizzard Entertainment are trademarks or registered
trademarks of Blizzard Entertainment, Inc. This project is not affiliated
with or endorsed by Blizzard. See [NOTICE.md](NOTICE.md).

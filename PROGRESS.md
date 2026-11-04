# Progress — what we have and what is missing

This page tells the community what already works, what is still missing and
what we are working on. It is the **single source of truth for completion
percentages**; other documents link here instead of restating numbers.

**How the numbers work (the progress directive):**

- Every area is a checklist. An item is `[x]` only when it is backed by
  observable evidence: a passing test, a gate output or a dated
  `research/docs/session-log.md` entry. The *Evidence* note says where.
- An area's percentage is `checked / total` items. The overall figure is
  the average of the areas weighted by their `weight`. Nothing is estimated
  by hand.
- The summary table between the `progress:summary` markers is **generated**:
  run `python3 scripts/progress.py --write` after editing a checklist. CI
  runs `python3 scripts/progress.py --check` and fails if the table is stale.
- This file is updated when a phase closes (see `AGENTS.md` §6), in the same
  commit as the work it reports.

<!-- progress:summary:start -->
| Area | Weight | Done | Total | Progress |
|---|---:|---:|---:|---:|
| [Simulation rules](#simulation-rules) | 30 | 13 | 13 | 100 % |
| [Data and file formats](#data-and-file-formats) | 15 | 10 | 10 | 100 % |
| [Playable client (`sc_view`)](#playable-client-sc_view) | 20 | 16 | 18 | 89 % |
| [Persistence: saves, replays, profiles](#persistence-saves-replays-profiles) | 10 | 5 | 6 | 83 % |
| [LAN multiplayer](#lan-multiplayer) | 15 | 6 | 9 | 67 % |
| [Reverse engineering](#reverse-engineering) | 10 | 8 | 14 | 57 % |
| **Overall (weighted)** | **100** | **58** | **70** | **87 %** |

_Last updated: 2026-11-03_
<!-- progress:summary:end -->

**Currently working on:** in-game visual parity against the retail captures,
then the physical two-machine LAN acceptance test.

**Out of scope (not counted):** Battle.net / online services, ladder, the map
or campaign editor, and patches other than 1.23.10.13515.

---

## Simulation rules

<!-- area: weight=30 -->

- [x] Damage pipeline: matrix, shields, armour, sizes, splash, status effects — `tests/original_parity.rs`, `src/sim.rs` unit tests
- [x] Projectiles: all 10 bullet behaviours and 25 hit types — `src/sim.rs` tests
- [x] Movement: octile A* with the recovered heap order, region graph, `iscript` walking cycle — `src/path.rs`, `tests/reconstruction.rs`
- [x] Unit collision and separation — `src/sim.rs` tests
- [x] Economy: 8 minerals per trip, gas, half-point supply up to 200 — `tests/economy_original.rs`
- [x] Tech tree: recovered producer / prerequisite switch — `tests/reconstruction.rs`
- [x] 25+ spells and abilities — `src/sim.rs` tests
- [x] Transports, bunkers, Nydus Canal, Shield Battery — `src/sim.rs` tests
- [x] Vision, fog of war, cloak, burrow and detectors — `src/sim.rs` tests
- [x] Classic triggers: conditions 0–23, actions 1–59 — `src/trigger.rs` tests
- [x] Campaign AI (`aiscript.bin` / `bwscript.bin`) and skirmish AI — `src/ai.rs` tests, `tests/replay.rs` (Boxer + AI)
- [x] `iscript` animation VM, opcodes 0x00–0x44 — `src/iscript.rs` tests
- [x] 228 units, 189 orders and 137 order handlers transcribed and diffed against the recovery evidence — `tests/handlers.rs`, `sc_orders`

## Data and file formats

<!-- area: weight=15 -->

- [x] 73/73 runtime tables identical to the live game — `engine-rust/tools/verify_runtime_tables.py`, `research/docs/session-log.md`
- [x] CASC reader for the Remastered data store — `src/retail.rs`, `tests/retail.rs`
- [x] MPQ archives with PKWARE DCL explode — `src/retail.rs`, `third_party/pklib`
- [x] CHK scenarios: all 323 shipped maps and 68 campaign missions load and simulate (391/391, 0 failed) — `sc_campaign --install`
- [x] Tilesets: SD CV5/VX4EX/VR4/WPE chain and HD megatiles — `tests/terrain_layers.rs`, `tests/hd_tileset.rs`
- [x] Sprites: GRP, HD `.anim` containers and DDS — `src/render.rs`, `src/hd_anim.rs`, `src/dds.rs`
- [x] Audio: WAV, Ogg music and unit voices — `tests/audio.rs`
- [x] Video: WebM portraits and the 28-film intro/cinematic table — `tests/portraits.rs`, `src/video.rs` (28/28)
- [x] Retail `.rep` container reader — `src/retail_replay.rs`, `tests/retail_replay.rs`
- [x] Rules tables (`*.dat`, `stat_txt.tbl`) and UI layouts (`*.ui.json`, `flc.json`) — `engine-rust/fixtures/README.md`, `tools/oracle.py`

## Playable client (`sc_view`)

<!-- area: weight=20 -->

- [x] Remastered main menu, Select Game Type and Registry, visual gate 43/43 — `engine-rust/docs/gates/ui-visual-diff.txt`, `scripts/ui_visual_diff.py`
- [x] Options, Hotkeys (220 actions, 88 units), Collections and Credits screens — `engine-rust/docs/gates/ui-hotkeys-16.txt`, `research/docs/hotkeys-data.md`
- [x] Campaign episode screen, mission modal and briefings — `engine-rust/docs/proof-sheets/REPORT.md`
- [x] Menu walkthrough gate: transitions, retail UI sounds, mouse on every screen — `engine-rust/docs/gates/menu-walkthrough.txt`
- [x] In-game HUD per race, minimap, selection, command card and hotkeys — `tests/render.rs`
- [x] Pause, victory, defeat and score screens — `sc_view --shot-screen pause|score`
- [x] Saved Games and Replays screens, replay player (pause, speed, restart) — `engine-rust/docs/replays.md`
- [x] Resolutions from 16:9 up to 4K and ultrawide — `research/docs/graphics-remastered.md`
- [x] Full retail campaign playable in retail order (68 missions) — `engine-rust/README.md`
- [x] Plays from the player's own install, with no retail files in the repository — `NOTICE.md`, `sc_assets`
- [ ] In-game frame parity with the retail captures (raw mean difference currently 2.46–3.27 on the eight (8)The Hunters references; open: building/mineral lighting, which needs the client's point-light inputs) — `engine-rust/docs/gates/ui-visual-diff-raw.txt`, session log 2026-11-03 (75)
- [x] Pixel-exact creep from the retail art and rule: cv5 group-1 tiles picked per tile by the client's CRT `rand` (64-bit `0x64fb30`), `tileset/<era>.grp` edge frames at their offsets, SD and HD (variants are per-session in retail) — `tests/render.rs`, `src/render.rs` creep tests, session log 2026-11-03 (76)
- [x] Full-motion WebM outside the title and briefing screens: HUD portraits (weighted idle clips, talking while the unit or a trigger transmission speaks), campaign act movies — `src/portrait.rs`, `tests/portraits.rs`, `engine-rust/docs/gates/package-parity.txt`
- [ ] Trigger transmissions show their text and play the map WAV (the portrait already talks)
- [x] Windows build (mingw cross build; 407 tests pass under Wine 10, same frames as Linux) — `scripts/package_windows.sh`, `engine-rust/docs/gates/package-parity.txt`
- [x] Linux packages: AppImage and Flatpak, built from a clean tree, pixel-identical to the developer build — `scripts/package_appimage.sh`, `scripts/package_flatpak.sh`, `engine-rust/docs/gates/package-parity.txt`
- [x] macOS universal app (x86_64 + arm64) built and tested on CI — `cross` job in `ci.yml` (run 41761417870), `scripts/package_macos.sh` (release run 41761682900)
- [x] Published GitHub release with all packages and SHA-256 sums — [v0.1.0](https://github.com/cotizcesar/starcraft-re-recomp/releases/tag/v0.1.0), `.github/workflows/release.yml`

## Persistence: saves, replays, profiles

<!-- area: weight=10 -->

- [x] Save/load (`.gcsave`): command log + deterministic fast-forward, magic + version + SHA-256 — `tests/replay.rs`, `engine-rust/docs/save-format.md`
- [x] Replays (`.gcrep`): same log gives the same `state_hash` (Boxer + AI, 300 ticks) — `tests/replay.rs`
- [x] Hotkey profile on disk (`GCHK` v1 with SHA-256) — `src/bin/sc_view.rs`
- [x] Per-profile campaign progress on disk (`GCPR`) — `src/campaign_progress.rs` (10 tests)
- [x] Victory unlocks the next mission, verified end to end — `research/docs/session-log.md` 2026-11-03 (72)
- [ ] Full player profile persistence (Registry IDs, settings, statistics)

## LAN multiplayer

<!-- area: weight=15 -->

- [x] Command-only lockstep core with a per-turn state checksum — `src/net.rs`, `tests/lan.rs`
- [x] TCP transport: 2400 ticks between two processes on a retail map, identical hash — `research/docs/lan-protocol.md`
- [x] UDP transport: 600 ticks, identical hash — `research/docs/lan-protocol.md`
- [x] LAN discovery by UDP broadcast (`:7778`) — `tests/lan.rs`
- [x] Lobby inside `sc_view` (key **L**) — `scripts/menu_walkthrough.py`
- [x] Reconnect by resyncing the command log (no state transfer) — `tests/lan.rs`
- [ ] Physical test on two machines (`scripts/lan_two_machines.sh`)
- [ ] Lobby with more than two players in the UI
- [ ] Automatic reconnect with backoff

## Reverse engineering

<!-- area: weight=10 -->

- [x] Environment and toolchain: REA + Ghidra 12.1.4 + JDK 21, end-to-end pass — `research/docs/toolchain.md`, `research/docs/rea-opencode-integration.md`
- [x] Install inventory: 724 files, 5.88 GB, SHA-256 manifest — `research/manifests/`
- [x] Engine identification: native Blizzard C/C++, x86 and x86-64, CASC + MPQ — `research/docs/engine-identification.md`
- [x] Decrypted memory images: 32-bit menu and 64-bit in-game dumps — `research/docs/architecture.md`
- [x] Ghidra projects of both dumps (56,948 + 59,706 functions) — `research/docs/architecture.md`
- [x] Architecture map: gate `0x70e810`, WndProc `0x945050`, simulation step `0x969f0d`, nuke handler `0x5d3823` — `research/docs/architecture.md`
- [x] CHK parser and TRIG handlers decompiled — `research/docs/architecture.md`
- [x] Same engine behind both binaries confirmed (hypothesis H2) — `research/docs/architecture.md`
- [ ] In-game map heap captured (needs a re-dump with the game running)
- [ ] Function `0x8aee30` analysed (parked after two timeouts)
- [ ] `GetMessage` message pump located
- [ ] Manifest from every extracted file to its origin in CASC/MPQ
- [ ] Capability matrix A–F completed
- [ ] CEF UI sound ID to file resolution (`GLUSND_*` → WAV)

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
| [Playable client (`sc_view`)](#playable-client-sc_view) | 20 | 20 | 21 | 95 % |
| [Persistence: saves, replays, profiles](#persistence-saves-replays-profiles) | 10 | 6 | 7 | 86 % |
| [LAN multiplayer](#lan-multiplayer) | 15 | 8 | 9 | 89 % |
| [Reverse engineering](#reverse-engineering) | 10 | 14 | 14 | 100 % |
| **Overall (weighted)** | **100** | **71** | **74** | **96 %** |

_Last updated: 2026-11-04_
<!-- progress:summary:end -->

**Currently working on:** the physical two-machine LAN acceptance test (needs
two machines), then the INFERENCES that need the retail client running
(`research/docs/unresolved-questions.md`).

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
- [x] Resolutions from 16:9 up to 4K and ultrawide: every screen (front end, briefing, game, score) is the 16:9 picture between black bars, pixel-identical to 16:9 — `engine-rust/docs/gates/ultrawide-diff.txt` (31/31), `scripts/ultrawide_diff.py`, `research/docs/graphics-remastered.md`
- [x] Main menu details: the panel's animated `loadingcircle.gif` spinner, the build footer in the shipped `blizzard-regular` face, no Battle.net tag offline, mouse-over clips that start on hover without decoder starvation (142 → 0 skipped frames), per-button clip order, footer labels that keep their glyphs on hover — `engine-rust/docs/gates/menu-time-diff.txt`, `engine-rust/docs/gates/menu-hover-text.txt`, session log 2026-11-04 (82)
- [x] Registry and Load Saved Game drawn from the shipped `glulogin`/`gluload` dialogs and `pList` plates (retail mean 3.19 → 1.32 and 6.99 → 1.18), Load Replay information from the selected replay's metadata — `engine-rust/docs/gates/ui-visual-diff.txt`, `menu::LOGIN_DIALOG`/`LOAD_DIALOG` tests
- [ ] Glue screen transitions: the retail panels build in when a classic dialog opens (Registry, Saved Games, Replays); timing and direction need a retail capture — session log 2026-11-04 (82)
- [x] Full retail campaign playable in retail order (68 missions) — `engine-rust/README.md`
- [x] Plays from the player's own install, with no retail files in the repository — `NOTICE.md`, `sc_assets`
- [x] In-game frame parity with the retail captures: the eight (8)The Hunters references at raw mean 2.44–3.25, all under the gate; building/mineral lighting from the retail point-light table (`anim/main.lit`) checked against a live capture — `engine-rust/docs/gates/ui-visual-diff-raw.txt`, `research/docs/hd-lighting.md`, retail test `shipped_point_light_table_matches_the_live_client_lights`, session log 2026-11-04 (80)
- [x] Pixel-exact creep from the retail art and rule: cv5 group-1 tiles picked per tile by the client's CRT `rand` (64-bit `0x64fb30`), `tileset/<era>.grp` edge frames at their offsets, SD and HD (variants are per-session in retail) — `tests/render.rs`, `src/render.rs` creep tests, session log 2026-11-03 (76)
- [x] Full-motion WebM outside the title and briefing screens: HUD portraits (weighted idle clips, talking while the unit or a trigger transmission speaks), campaign act movies — `src/portrait.rs`, `tests/portraits.rs`, `engine-rust/docs/gates/package-parity.txt`
- [x] Trigger transmissions show their text and play the map WAV (the portrait already talks) — `src/transmission.rs`, `TriggerCue` in `src/trigger.rs`, retail test `campaign_transmissions_resolve_to_text_and_playable_map_audio`, `engine-rust/docs/gates/transmission.txt`, session log 2026-11-04 (78)
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
- [ ] Locked-campaign prompt follows retail: progress is recorded when a mission starts, and the prompt only shows while the campaign was never started (`0x006eaac0`, live `.spc` diff 2026-11-04) — `research/docs/profile-format.md` #9
- [x] Full player profile persistence (Registry IDs, settings, statistics) — `GCPF` `profiles.bin`, retail `.spc` import — `src/profile.rs` (11 tests), `scripts/menu_walkthrough.py` (`profile_persistence_check`, `campaign_win_check`), `research/docs/profile-format.md`, `research/docs/session-log.md` 2026-11-03 (77)

## LAN multiplayer

<!-- area: weight=15 -->

- [x] Command-only lockstep core with a per-turn state checksum — `src/net.rs`, `tests/lan.rs`
- [x] TCP transport: 2400 ticks between two processes on a retail map, identical hash — `research/docs/lan-protocol.md`
- [x] UDP transport: 600 ticks, identical hash — `research/docs/lan-protocol.md`
- [x] LAN discovery by UDP broadcast (`:7778`) — `tests/lan.rs`
- [x] Lobby inside `sc_view` (key **L**) — `scripts/menu_walkthrough.py`
- [x] Reconnect by resyncing the command log (no state transfer) — `tests/lan.rs`
- [ ] Physical test on two machines (`scripts/lan_two_machines.sh`)
- [x] Lobby with more than two players in the UI (up to 8, one seat per start location) — `JoinQueue` in `src/net.rs`, `tests/lan.rs::three_peers_join_a_polled_lobby_and_stay_in_sync`, `engine-rust/docs/gates/lan-lobby-reconnect.txt`
- [x] Automatic reconnect with backoff (host waits and accepts rejoins, clients retry on `Backoff::lan`) — `tests/lan.rs::a_dropped_peer_rejoins_with_backoff_while_the_others_wait`, `rejoin_backoff_retries_until_the_host_listens`, `engine-rust/docs/gates/lan-lobby-reconnect.txt`

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
- [x] In-game map heap captured (RW re-dump during Terran 8; map text and objectives found) — `research/docs/heap-ingame-findings.md` (re-verified 2026-11-04)
- [x] Function `0x8aee30` analysed: protector-obfuscated body (junk, overlapping branches), why it never decompiles, what it touches — `research/docs/architecture.md` §1, `research/tools/FunctionProfile.java`
- [x] Message pump located: `FUN_00946500` (`PeekMessage`/Translate/Dispatch, `WM_QUIT`), called per frame at `0x6bbdfa` — `research/docs/architecture.md` §2
- [x] Manifest from every extracted file to its origin in CASC/MPQ (2 923 files by CKey = MD5, 0 without origin) — `research/manifests/extracted-origin.tsv`, `sc_assets manifest`, retail test `materialised_assets_load_without_the_bundled_asset_directory`
- [x] Capability matrix A–F completed (15 rows × 6 columns, new rows observed 2026-11-04) — `research/docs/rea-capabilities.md`
- [x] CEF UI sound ID to file resolution (`GLUSND_*` → WAV): sound id = `sfxdata.dat` row, 1-based `.tbl` string — `src/sfx.rs` (`SfxData`, `glusnd_path`, tests)

# Progress — every feature, done or not

The single list of what the game needs, what works and what is planned.
Work through it top to bottom; other documents link here instead of
restating status.

- An item is `[x]` only with observable evidence (a test, a gate output in
  `engine-rust/docs/gates/` or a dated `research/docs/session-log.md`
  entry), named after the dash.
- An area's percentage is `checked / total`; the overall figure weights the
  areas. The table is generated: run `python3 scripts/progress.py --write`
  after editing a checklist; CI runs `--check`.
- New scope is a new `[ ]` item, which lowers the percentage honestly.

<!-- progress:summary:start -->
| Area | Weight | Done | Total | Progress |
|---|---:|---:|---:|---:|
| [Simulation rules](#simulation-rules) | 25 | 14 | 16 | 88 % |
| [Data and file formats](#data-and-file-formats) | 10 | 8 | 8 | 100 % |
| [Front end (menus)](#front-end-menus) | 10 | 6 | 7 | 86 % |
| [Campaign](#campaign) | 15 | 6 | 10 | 60 % |
| [In-game client](#in-game-client) | 10 | 6 | 8 | 75 % |
| [Persistence](#persistence) | 5 | 4 | 5 | 80 % |
| [LAN multiplayer](#lan-multiplayer) | 10 | 3 | 6 | 50 % |
| [Packaging and platforms](#packaging-and-platforms) | 5 | 4 | 5 | 80 % |
| [Reverse engineering](#reverse-engineering) | 5 | 6 | 6 | 100 % |
| [Controller support (planned)](#controller-support-planned) | 5 | 0 | 4 | 0 % |
| **Overall (weighted)** | **100** | **57** | **75** | **75 %** |

_Last updated: 2026-11-05_
<!-- progress:summary:end -->

**Out of scope (not listed):** Battle.net and online services, ladder, the
map/campaign editor, patches other than 1.23.10.13515.

---

## Simulation rules

<!-- area: weight=25 -->

- [x] Damage pipeline: matrix, shields, armour, sizes, splash, status effects (74,880 differential cases) — `tests/original_parity.rs`
- [x] Projectiles: 10 bullet behaviours, 25 hit types — `src/sim.rs` tests
- [x] Movement: octile A* in the recovered heap order, region graph, `iscript` walking cycle, unit separation — `src/path.rs`, `tests/reconstruction.rs`
- [x] Economy: 8 minerals per trip, gas, half-point supply up to 200, queues, refunds — `tests/economy_original.rs`
- [x] Tech tree: recovered producer / prerequisite switch — `tests/reconstruction.rs`
- [x] Spells and abilities (every shipped family) — `src/sim.rs` tests
- [x] Transports, bunkers, Nydus Canal, Shield Battery, lift/land, add-ons, burning, repair — `src/sim.rs` tests
- [x] Vision, fog of war, cloak, burrow, detectors — `src/sim.rs` tests
- [x] 228 units, 189 orders, 137 order handlers transcribed and diffed — `tests/handlers.rs`, `sc_orders`
- [x] `iscript` animation VM, opcodes 0x00–0x44 — `src/iscript.rs` tests
- [x] Trigger VM: conditions 0–23, actions 1–59, `UPRP`; force triggers, tick-1 pass, pixel locations, wait-blocking, Victory/Defeat owner checked against retail — `src/trigger.rs` tests, `tests/campaign_episode1.rs`
- [x] Campaign AI (`aiscript.bin` 293 / `bwscript.bin` 156 scripts) and skirmish AI — `src/aiscript.rs`, `src/ai.rs`, `tests/replay.rs`
- [x] Rescuable players (Rescue Passive) — `sim::tests::rescuable_units_are_nobodys_target_and_join_the_first_comer`
- [x] Score accounting (build/destroy scores, produced/lost, spending) — `sim::tests::score_tallies_follow_production_spending_and_kills`
- [ ] Alliances: allied players must not fight (`World::hostile` ignores `Player.team`, which trigger action 57 and the AI already use)
- [ ] Campaign AI opcodes that only advance today (`if_dif`, `if_towns`, `get_oldpeons`, `wait_upgrades`, `set_attacks`, `quick_attack`, …; only `get_oldpeons` is used by an Episode I map) — `research/docs/session-log.md` 2026-11-05 (93)

## Data and file formats

<!-- area: weight=10 -->

- [x] 73/73 runtime tables identical to the live game — `engine-rust/tools/verify_runtime_tables.py`
- [x] CASC store and MPQ maps (PKWARE DCL), read-only from the player's install — `src/retail.rs`, `tests/retail.rs`
- [x] CHK scenarios: 323 melee maps + 68 campaign missions load and simulate (391/391) — `sc_campaign --install`
- [x] Tilesets (SD chain and HD megatiles), GRP, HD `.anim`, DDS — `tests/terrain_layers.rs`, `tests/hd_tileset.rs`, `tests/render.rs`
- [x] Audio: WAV effects, 23 Ogg music tracks, 1,122 voice files — `tests/audio.rs`
- [x] Video: WebM portraits, menu clips and the 28-film table — `tests/portraits.rs`, `src/video.rs`
- [x] Rules tables (`*.dat`, `stat_txt.tbl`) and UI layouts (`*.ui.json`, `flc.json`) — `engine-rust/fixtures/README.md`, `tools/oracle.py`
- [x] Retail `.rep` replay container reader — `tests/retail_replay.rs`

## Front end (menus)

<!-- area: weight=10 -->

- [x] Main menu, Select Game Type, Registry, Options, Hotkeys, Collections, Credits as retail (visual gate 51/51) — `engine-rust/docs/gates/ui-visual-diff.txt`
- [x] Menu clips, hover, swish transitions and UI sounds measured on retail recordings — `engine-rust/docs/gates/menu-time-diff.txt`, `glue-swish.txt`, `menu-hover-text.txt`
- [x] Mouse and keyboard on every screen (walkthrough, 146 steps) — `engine-rust/docs/gates/menu-walkthrough-run.txt`
- [x] Saved Games and Replays browsers — `engine-rust/docs/replays.md`
- [x] 16:9 to 4K and ultrawide: every screen pillarboxed, pixel-identical to 16:9 (31/31) — `engine-rust/docs/gates/ultrawide-diff.txt`
- [x] Menu frame time under 20 ms at 3440x1440 — `engine-rust/docs/gates/menu-draw-time.txt`
- [ ] Create-game screen with selectable game types and their rules (Top vs Bottom, Team Melee, …); today the row is a fixed "Free For All" label

## Campaign

<!-- area: weight=15 -->

- [x] All 68 missions in retail order (`arr/mapdata.tbl`), `gluhist` titles, films from the `MenuEntry` table — `campaign_progress::tests`
- [x] Establishing shots with per-page `DISPLAYTIME` (video pages wait for a click, `VIDEOAUTOPROCEED`) — `campaign::tests::pages_leave_after_their_display_time_or_video`
- [x] Terran ready room (`MBRF` + `glurdyt`): talking portraits, scrolling transmission, objectives, Start/Replay/Cancel, Skip Tutorial — `ready_room::tests`, `ready_text_scroll_follows_the_retail_recording`
- [x] Victory/defeat dialog and score screen with slide-in and count-up — `score_count_up_follows_the_retail_curve`, `--shot-screen win-dialog|victory`
- [x] Episode I audited against the shipped maps (every trigger, briefing action and AI script used) — `every_episode_one_ready_room_runs_to_the_end_with_real_portraits`
- [x] Boot Camp, Wasteland and Backwater Station match retail recordings (openings, rescues, endings, victory timing) — `tests/campaign_episode1.rs` (11 tests)
- [ ] Episode I missions 4–11 compared with retail recordings
- [ ] Audio timing (briefing voices, transmissions, rescue and victory sounds) against a recording with sound
- [ ] Zerg and Protoss ready rooms (`glurdyz` / `glurdyp`)
- [ ] Episodes II–VI (Zerg, Protoss, Brood War) compared with retail recordings

## In-game client

<!-- area: weight=10 -->

- [x] Race consoles, HUD, minimap, selection, wireframes, command card, hotkeys — `tests/render.rs`
- [x] Full-motion portraits, talking during transmissions — `tests/portraits.rs`
- [x] Trigger text (62 ms per character, 11 lines), trigger leaderboard, `stat_txt` error lines — `transmission::tests`, `--shot-screen trigger-text|leaderboard-marines|game-error`
- [x] Pause menu, mission objectives, tips, mission statistics — `--shot-screen pause|objectives|tips|stats`
- [x] Creep from the retail art and rule; building lights from `anim/main.lit` — `src/render.rs` tests, `research/docs/hd-lighting.md`
- [x] In-game frames within the visual gate on the eight (8)The Hunters references — `engine-rust/docs/gates/ui-visual-diff-raw.txt`
- [ ] Single-player cheat codes (retail feature)
- [ ] Save Replay from the score screen (button shown disabled; F6 saves during play)

## Persistence

<!-- area: weight=5 -->

- [x] Save/load (`.gcsave`) and replays (`.gcrep`): command log + fast-forward, identical `state_hash` — `tests/replay.rs`, `engine-rust/docs/save-format.md`
- [x] Player profiles (`profiles.bin`): Registry IDs, settings, statistics, campaign progress; retail `.spc` import — `src/profile.rs`, `research/docs/profile-format.md`
- [x] Victory unlocks the next mission across restarts — walkthrough `campaign win`
- [x] Hotkey profile on disk — `src/bin/sc_view.rs`
- [ ] Locked-campaign prompt as retail (progress recorded at mission start; prompt only while never started) — `research/docs/profile-format.md` #9

## LAN multiplayer

<!-- area: weight=10 -->

- [x] Command-only lockstep with per-turn state checksum over TCP (2400 ticks) and UDP — `tests/lan.rs`, `research/docs/lan-protocol.md`
- [x] Discovery by UDP broadcast, lobby in `sc_view` (up to 8 players) — `tests/lan.rs`, `engine-rust/docs/gates/lan-lobby-reconnect.txt`
- [x] Reconnect by resyncing the command log, with backoff — `tests/lan.rs`
- [ ] Physical test on two machines — `scripts/lan_two_machines.sh`
- [ ] Lobby and in-game chat (the protocol carries `Msg::Chat`; no UI)
- [ ] Observer / "Become Observer" after defeat

## Packaging and platforms

<!-- area: weight=5 -->

- [x] Rust 1.99.1 pinned, edition 2024, latest crates — `rust-toolchain.toml`, `engine-rust/Cargo.toml`
- [x] Linux AppImage and Flatpak, pixel-identical to the developer build — `engine-rust/docs/gates/package-parity.txt`
- [x] Windows (mingw) and macOS universal builds tested in CI — `.github/workflows/ci.yml`
- [x] Published release with SHA-256 sums — [v0.1.0](https://github.com/cotizcesar/starcraft-re-recomp/releases/tag/v0.1.0)
- [ ] Localised text (the client reads only the `enUS` locale)

## Reverse engineering

<!-- area: weight=5 -->

- [x] Toolchain (REA, Ghidra 12.1.4, JDK 21), install inventory with SHA-256 manifest — `research/docs/toolchain.md`, `research/manifests/`
- [x] Engine identified; 32-bit menu and 64-bit in-game dumps; Ghidra projects — `research/docs/engine-identification.md`, `research/docs/architecture.md`
- [x] Architecture map: gate, WndProc, message pump, simulation step, CHK/TRIG handlers — `research/docs/architecture.md`
- [x] In-game map heap capture; `0x8aee30` explained (protector obfuscation) — `research/docs/heap-ingame-findings.md`, `architecture.md` §1
- [x] Every extracted file traced to its CASC/MPQ origin (2,923 files) — `research/manifests/extracted-origin.tsv`
- [x] CEF UI sound ids resolved to files — `src/sfx.rs` tests

## Controller support (planned)

<!-- area: weight=5 -->

Design: [`engine-rust/docs/controller-support.md`](engine-rust/docs/controller-support.md) (phases in §7).

- [ ] Unified `InputFrame` for keyboard, mouse, script and gamepad
- [ ] Gamepad in game: virtual cursor, camera pan, select/command, control groups
- [ ] Gamepad in menus, briefings, pause, score and replays (focus navigation)
- [ ] Gamepad command card (radial) and settings (dead zone, speed, swap A/B)

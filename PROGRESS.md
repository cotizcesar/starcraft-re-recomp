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
| [Simulation rules](#simulation-rules) | 25 | 16 | 17 | 94 % |
| [Data and file formats](#data-and-file-formats) | 10 | 8 | 8 | 100 % |
| [Front end (menus)](#front-end-menus) | 10 | 8 | 9 | 89 % |
| [Campaign](#campaign) | 15 | 9 | 13 | 69 % |
| [In-game client](#in-game-client) | 10 | 6 | 8 | 75 % |
| [Persistence](#persistence) | 5 | 4 | 5 | 80 % |
| [LAN multiplayer](#lan-multiplayer) | 10 | 6 | 9 | 67 % |
| [Packaging and platforms](#packaging-and-platforms) | 5 | 4 | 5 | 80 % |
| [Reverse engineering](#reverse-engineering) | 5 | 6 | 6 | 100 % |
| [Controller support (planned)](#controller-support-planned) | 5 | 0 | 4 | 0 % |
| **Overall (weighted)** | **100** | **67** | **84** | **80 %** |

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
- [x] Score accounting as OpenBW (build score on completion incl. starting units, kill/razing destroy scores, kills credited to the attacker; losses and spending) — `tests/multiplayer.rs::a_melee_start_scores_like_the_retail_score_screen`, `sim::tests::score_tallies_follow_production_spending_and_kills`
- [x] Mining as OpenBW: 75 frames per mineral trip, 37 in a refinery, one worker per field (SimpleAi income on Boxer 1,448 → 520 in 1,857 ticks; retail sheet ~376) — `tests/economy_original.rs::mining_takes_75_frames_and_a_field_serves_one_worker_at_a_time`
- [x] Alliances: allied players (lobby teams, trigger action 57) neither fight nor target each other and share vision in team games — `trigger::tests::set_alliance_status_stops_two_players_fighting`, `tests/multiplayer.rs::a_team_wins_together_and_never_targets_itself`, `teammates_share_vision`
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
- [x] View Intro asks Original / Expansion and plays that title movie; leaving the main menu plays only the click — `scripts/menu_walkthrough.py` (`view_intro_check`), `engine-rust/docs/gates/menu-walkthrough-run.txt`, session log 2026-11-05 (102)
- [x] Saved Games and Replays browsers — `engine-rust/docs/replays.md`
- [x] 16:9 to 4K and ultrawide: every screen pillarboxed, pixel-identical to 16:9 (31/31) — `engine-rust/docs/gates/ultrawide-diff.txt`
- [x] Menu frame time under 20 ms at 3440x1440 — `engine-rust/docs/gates/menu-draw-time.txt`
- [x] Retail multiplayer front end: Select Connection → Registry → Games → Create (map browser, 13 game types, teams, speed) → 8-slot lobby → countdown → game, each step within 2.0–3.6 grey levels of the recording — `scripts/mp_flow_gate.py`, `engine-rust/docs/gates/mp-flow.txt`
- [ ] Special rules of Greed, Slaughter, Capture the Flag and Sudden Death (the types are selectable; their rules are not applied)

## Campaign

<!-- area: weight=15 -->

- [x] All 68 missions in retail order (`arr/mapdata.tbl`), `gluhist` titles, films from the `MenuEntry` table — `campaign_progress::tests`
- [x] Establishing shots with per-page `DISPLAYTIME` (video pages wait for a click, `VIDEOAUTOPROCEED`) — `campaign::tests::pages_leave_after_their_display_time_or_video`
- [x] Terran ready room (`MBRF` + `glurdyt`): talking portraits, scrolling transmission, objectives, Start/Replay/Cancel, Skip Tutorial — `ready_room::tests`, `ready_text_scroll_follows_the_retail_recording`
- [x] Victory/defeat dialog and score screen with slide-in and count-up — `score_count_up_follows_the_retail_curve`, `--shot-screen win-dialog|victory`
- [x] Episode I audited against the shipped maps (every trigger, briefing action and AI script used) — `every_episode_one_ready_room_runs_to_the_end_with_real_portraits`
- [x] Boot Camp, Wasteland and Backwater Station match retail recordings (openings, rescues, endings, victory timing) — `tests/campaign_episode1.rs` (11 tests)
- [x] Desperate Alliance matches its retail recording: ready room (six lines within 0.5 s), scripted VULTURE/MARINE lines, the 30-minute countdown at 16 ticks per game second, the "One minute" / "Sons of Korhal" / pilot / victory ending — `desperate_alliance_follows_the_retail_recording`, `the_countdown_and_elapsed_time_run_on_16_tick_game_seconds`
- [x] Countdown timer drawn as retail (Eurostile, centred at x 1418, `m:ss`) — `countdown_reads_minutes_and_seconds`, `--shot-screen countdown`
- [x] Real-time pace of 42 ms per tick (Fastest); score Elapsed Time in real seconds — `sim::tests::ticks_are_42_ms_of_real_time`
- [ ] Episode I missions 5–11 compared with retail recordings
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
- [x] Lobby protocol 3: named joins, live lobby for joiners, race and colour picks, lobby chat, the host's countdown shown to joiners, resolved setup at start; two `sc_view` processes end on the same state hash — `tests/lan.rs::a_joiner_sits_in_the_lobby_and_starts_with_the_setup`, `engine-rust/docs/gates/mp-flow.txt`
- [x] Lobby games: colours drawn in game, teams ally and win together, computers run in the lockstep on every peer, saves keep the setup — `tests/multiplayer.rs`, `tests/replay.rs::saves_carry_the_lobby_setup_and_still_read_version_1`
- [x] Retail score screen for lobby games — `sc_view --shot-screen score-defeat`, `engine-rust/docs/gates/mp-flow.txt`
- [x] Reconnect by resyncing the command log, with backoff — `tests/lan.rs`
- [ ] Physical test on two machines — `scripts/lan_two_machines.sh`
- [ ] In-game chat (lobby chat exists)
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

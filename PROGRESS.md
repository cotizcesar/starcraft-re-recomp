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
| [Simulation rules](#simulation-rules) | 25 | 19 | 20 | 95 % |
| [Data and file formats](#data-and-file-formats) | 10 | 8 | 8 | 100 % |
| [Front end (menus)](#front-end-menus) | 10 | 9 | 10 | 90 % |
| [Campaign](#campaign) | 15 | 9 | 13 | 69 % |
| [In-game client](#in-game-client) | 10 | 22 | 27 | 81 % |
| [Persistence](#persistence) | 5 | 5 | 6 | 83 % |
| [LAN multiplayer](#lan-multiplayer) | 10 | 6 | 9 | 67 % |
| [Packaging and platforms](#packaging-and-platforms) | 5 | 4 | 5 | 80 % |
| [Reverse engineering](#reverse-engineering) | 5 | 6 | 6 | 100 % |
| [Controller support (planned)](#controller-support-planned) | 5 | 0 | 4 | 0 % |
| **Overall (weighted)** | **100** | **88** | **108** | **81 %** |

_Last updated: 2026-11-06_
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
- [x] Build, research and upgrade times are game frames (SCV 300 frames = 12.6 s on the Backwater Station recording) — `tests/reconstruction.rs::time_units_are_game_frames`, session log 2026-11-05 (105)
- [x] A Terran SCV stays on its building (orders.dat 33); construction pauses without it and resumes when an SCV is sent back; hit points grow with the progress — `tests/reconstruction.rs::a_terran_scv_stays_on_its_building_and_construction_pauses_without_it`
- [x] Terran construction as on the Boot Camp recording: the `units.dat` scaffold (`TBldLrg`/`tBldMed`/`TBldSml`) frames 0/1/2, then the AlmostBuilt frame, switching at progress 1/9, 1/3, 5/9; the SCV works around the building's edges with Fusion Cutter frames and `scvspark` — `construction_look_tests::scaffold_frames_switch_at_the_recorded_progress_marks`, session log 2026-11-06 (109)
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
- [x] Every screen keeps its 1440p layout from 1280x720 up (16:9, ultrawide); windows narrower than 16:9 are letterboxed (245/245) — `scripts/resolution_diff.py`, `engine-rust/docs/gates/resolution-diff.txt`
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
- [x] Mouse and keyboard act on the world at every UI scale: click and box selection (own units, units over buildings, 12 max), retail right click (attack enemies, workers gather, others move, buildings set rally), the retail command card and its tooltip hotkeys, build cards, placement, training, target buttons — `scripts/game_input_gate.py`, `engine-rust/docs/gates/game-input.txt`
- [x] In-game frame under 8 ms at 3440x1440 (was 181 ms): baked sprite shading, row-sliced terrain / fog / stretch on parallel bands, cached console plates and minimap — `engine-rust/docs/gates/ingame-draw-time.txt`, `hd_sprites::tests::baked_frames_match_the_per_pixel_shader`
- [x] Selection panel as retail: rank (`units.dat` sublabel), kills, armour / weapon upgrade boxes, the production queue with its progress bar (`pbrempt` / `pbrfull`), Under Construction, research and upgrade progress — `statdata.ui.json` controls, Backwater Station 126 s
- [x] Multi-selection panel as on the Wasteland recording: `statdata` 33-44 (6 x 2, column-major), `cmdbtns` box 13 (19 for heroes), wireframes; box selection ordered left to right; click keeps one unit, Shift+click drops it — `scripts/game_input_gate.py`, session log 2026-11-06 (110)
- [x] Bunker cargo in the selection panel (`statdata` 27-30, as the recording at 272 s) and loading by right click on an own Bunker / transport — `scripts/game_input_gate.py`
- [x] Unloading one unit by clicking its cargo slot (recorded, tag 11); units enter a Bunker / transport from any side (box gap) — `tests/reconstruction.rs::one_marine_leaves_the_bunker_and_the_act_is_logged`, `scripts/game_input_gate.py`
- [x] Mission end as recorded: the world runs on after Victory / Defeat, the box shows ~1 s (24 frames) after the result, Elapsed Time stays at the result (Boot Camp 05:12) — `scripts/mission_gate.py`, session log 2026-11-06 (113)
- [ ] Two- and four-space cargo slots (18-24) for Dropships / Shuttles / Overlords (none in Episode I: Bunkers take one-space infantry only)
- [x] Cancel construction (Esc / card, 75 % refund, recorded tag 10), targeted spells through `Order::Cast` (Spider Mines: one mine per order from the Vulture's three, heroes without research), placement exactly where clicked with the retail preview (green / red box over a half-blended building) — `tests/reconstruction.rs`, `scripts/game_input_gate.py`, session log 2026-11-06 (111)
- [ ] Land placement preview for lifted buildings (no lift-off in the four Episode I recordings)
- [x] The four Episode I missions are won through input alone at 720p / 1080p / 3440x1440: Boot Camp (depots, Refinery, 100 gas; tick 6,542), Wasteland (Raynor, Barracks, 10 Marines; 7,320), Backwater Station (Infested CC destroyed; 9,728), Desperate Alliance (countdown survived, calm opening as recorded; 29,025) — `scripts/mission_gate.py`, `engine-rust/docs/gates/mission-gate.txt`
- [x] Units no longer leave the map while chasing, idle units fight back, groups arrive through shared path corners, SCVs walk to build sites at full speed — `tests/reconstruction.rs` (`a_chasing_unit_walks_to_its_target_and_stays_on_the_map`, `an_idle_unit_that_is_shot_fights_back`, `a_group_move_through_one_corner_arrives_and_goes_idle`, `an_scv_walks_to_its_build_site_at_full_speed`)
- [x] Mined-out fields disappear and their workers move to the next field — `tests/reconstruction.rs::a_mined_out_field_disappears_and_its_worker_moves_to_the_next`
- [x] Campaign AI attacks are the script's waves: `attack_do` takes the requested count per type, no invented periodic all-in — `aiscript::tests::attack_do_sends_the_requested_count_and_no_periodic_attack`
- [x] Trigger timelines of the four mission openings match the recordings within 0.5 s (10 transmissions / objectives, onsets measured after each recording's first game frame) — `scripts/timeline_gate.py`, `engine-rust/docs/gates/timeline.txt`
- [x] Flingy movement as retail: SCVs, Vultures, Drones, Overlords and lifted buildings accelerate by flingy.dat `acceleration` and brake on their stopping distance; Boot Camp gas with 3 SCVs every 2.05 s against the recording's 1.97 s (was 1.80 s) — `flingy_units_accelerate_from_rest_and_walkers_do_not`, `scripts/mission_gate.py`
- [x] Worker income against the recordings: mining starts on contact with the field and lasts the recorded 93 frames (Wasteland, 5 SCVs: 0.1667 minerals/frame vs recorded 0.1696, was +14 %); Boot Camp gas 1.89 s vs 1.97 s with the recording's 1.5 / 1.6 / 2.8 s three-SCV rhythm (harvest not restarted when nudged, Refinery entry and hand-in on contact, flingy turning) — `a_worker_starts_mining_only_when_it_touches_the_field`, `scripts/mission_gate.py`, HUD digit reader `research/tools/hud_read_number.py`
- [x] Production queue as retail: a queue is paid at once, each unit takes its supply when it starts training and waits at the front without room (Backwater Station: supply rises at 83.5 / 96.3 / 109.1 / 121.9 s vs recorded 83.5 / 96.0 / 109.0 / 121.5 s) — `queued_units_take_supply_when_they_start_training`, `scripts/mission_gate.py`
- [ ] Player-driven event times (each recording's own build / attack order replayed) against the recordings; anchors so far: Boot Camp supply 18 -> 26 -> 34, depot 600 frames, gas and mineral rates
- [ ] Single-player cheat codes (retail feature)
- [ ] Save Replay from the score screen (button shown disabled; F6 saves during play)

## Persistence

<!-- area: weight=5 -->

- [x] Save/load (`.gcsave`) and replays (`.gcrep`): command log + fast-forward, identical `state_hash` — `tests/replay.rs`, `engine-rust/docs/save-format.md`
- [x] Player profiles (`profiles.bin`): Registry IDs, settings, statistics, campaign progress; retail `.spc` import — `src/profile.rs`, `research/docs/profile-format.md`
- [x] Victory unlocks the next mission across restarts — walkthrough `campaign win`
- [x] Hotkey profile on disk — `src/bin/sc_view.rs`
- [x] Command log v3: research, upgrade, cancel, rally, add-on and spell acts are recorded, so saves and replays keep them (v2 files still load) — `replay::tests::rally_points_enter_the_log_and_replay`
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

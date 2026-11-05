# Changelog

Notable changes to this project. The detailed, dated engineering record is
[research/docs/session-log.md](research/docs/session-log.md). Completion
status is in [PROGRESS.md](PROGRESS.md).

## 2026-11-05 — Episode I (Rebel Yell) campaign flow

### Added
- The retail ready room after each Terran establishing shot: the map's
  `MBRF` briefing script (portraits that talk, the transmission box that
  scrolls and clears, objectives, "End of Briefing."), `glurdyt` art,
  Start / Replay / Cancel, the projector and the Start hologram; timeline
  measured on a retail Wasteland recording.
- The retail end of a mission: "Congratulations! You are victorious!"
  (`wmission`) / "You failed to achieve victory!" (`lmission`) over the
  game, then the `gluscore` score screen with Overview / Units /
  Structures / Resources tabs, per-player bars (`iscore`), force and
  computer nation names ("Garm Brood"), APM and Elapsed Time.
- Score accounting in the simulation: build scores of produced units and
  structures, destroy scores of kills and razings, units/structures lost
  and resources spent (also what trigger Score conditions read).

### Fixed
- Rescuable units are first checked on the first tick, so Backwater
  Station's Raynor speaks on the first frame (was 1.25 s late).
- Trigger locations were scaled x32 (MRGN is already in pixels), so every
  location condition and action covered the whole map: Boot Camp jumped
  to its Marine lesson 18 s in.
- A Wait or Transmission now holds the player's later triggers in the
  same cycle (BW wait-blocking), not only from the next one.
- Trigger text stays 62 ms per character (at least 4 s, at least the
  transmission); up to 11 lines on screen.
- Briefing pages honour their own `DISPLAYTIME`: video pages with 0 wait
  for a click (they flashed past), `VIDEOAUTOPROCEED` leaves when its
  video ends; the ready-room text holds 4 s, then scrolls linearly.

### Added
- Boot Camp's "Skip Tutorial" in the ready room (MBRF action 9): marks
  the tutorial done and opens the Wasteland briefing.

### Fixed
- In-game messages as retail: refused production / orders show the
  `stat_txt` error line centred over the console (was a "Cannot train:
  NotEnoughMinerals" debug status; "Building X" / "Mission started" are
  gone), Terran town halls say "Supplies ..." (was "Control ..."), and the
  trigger leaderboard is drawn ("9 of 10 Marines" in Wasteland; it was
  never shown).
- Victory and Defeat trigger actions apply to the player running the
  trigger (they read the unused `group` operand, so Wasteland's victory
  went to player 0 and could not be won).
- A Transmission shown under a trigger's Pause Game keeps its text only
  while it talks (Raynor's rescue line, 4.0 s on the retail recording).
- Score screen motion: the empty frame slides in from the left, then rows
  and numbers appear and count up (again on every tab change), as on the
  retail recording.
- In-game trigger text in the retail face and place (was the 8x8 bitmap
  font), map padding removed, blank lines kept, at most 6 lines, lifetime
  by line count; Waits and Transmissions resume on the tick they end
  instead of the next 30-tick trigger cycle.
- Triggers owned by a force (`FORC`) run for its players and groups 18-21
  count the force: Boot Camp, Desperate Alliance, New Gettysburg and The
  Hammer Falls ran none of their scripts.  Triggers first run at game
  start ("Elapsed Time at most 0" in The Jacobs Installation).
- Mission computers run only their `Run AI Script` scripts (the melee AI
  made The Trump Card lose at 4 minutes); rescue range 2 tiles, from the
  retail recording.
- The campaign advances through `arr/mapdata.tbl`: Norad II is followed by
  The Trump Card and The Big Push by New Gettysburg; the unused shipped
  maps `terran07` ("Biting the Bullet") and `terran10` ("Operation Silent
  Scream") are no longer played.
- Mission titles come from the retail `gluhist` list (`terran11` was named
  "Kerrigan has been killed.", now "New Gettysburg"; the tutorial is
  "Boot Camp").
- Cinematics follow the retail `MenuEntry` table with their location card:
  `PreTerran_1` + Wasteland Patrol after Backwater Station, Norad II after
  Revolution, Open Rebellion after The Big Push, The Inauguration after
  The Hammer Falls (the old three-mission "acts" put them one mission
  late); Brood War `x<Race>` films close their campaign instead of opening
  it.
- Rescue Passive players (`OWNR` 3): nobody targets their units and a human
  player's unit within reach takes them over, with the retail rescue sound.
  Raynor is no longer shot by the player's own Marines, so The Jacobs
  Installation no longer ends in defeat after 12 s.
- Missions end only through their triggers (no melee "last player
  standing" rule freezing the closing transmissions), and a Transmission
  holds its trigger for the line, so conversations play in sequence (The
  Trump Card).

## 2026-11-05 — Menu playback, movies and frame time

### Fixed
- Menu animations no longer speed up after visiting another screen: clip
  clocks pause while a clip is not drawn and never fast-forward through
  queued frames (98 skipped-frame events -> 0 on a menu/campaign/menu
  round trip).  Mouse-over loops (the Single Player hologram grid) play
  their whole loop on one clock, as on the retail recording.
- Movie soundtracks played twice as fast (decoded at 22 050 Hz, played at
  44 100 Hz); intros now decode at their authored 15 fps; the SD Blizzard
  logo fills the 16:9 box like the HD films.
- Menu frame time at 3440x1440: Options 101 -> 15 ms, Load Replay 45 -> 18,
  Episode 40 -> 8, Collections 40 -> 16, Registry/Saved Games 37 -> 11,
  Hotkeys 25 -> 13 (cached static layers, exact fast paths in the rounded
  CEF primitives, the moon backdrop no longer cloned every frame).
- Collections is a translucent modal over the live main menu, unselected
  cells unframed (reference 131005: 6.39 -> 4.38).

### Changed
- The title-frame panel introduces the game instead of showing the
  Battle.net loading spinner (no Battle.net here); the GIF decoder that
  only drew that spinner is removed.
- Screen changes play the glue swish: `swishin` when a screen appears,
  `swishout` when a classic list dialog leaves (cache marker v18).

## 2026-11-04 — Front-end fidelity and ultrawide

### Added
- Credits behave like the retail client (recording + shipped
  `creditspanel.js`): hard cut in, a 2 s hold, then the list scrolls at
  1 CSS px per frame (89 px/s at 1440p); Up/Down accelerate, the wheel
  pauses the auto scroll, only Esc/Enter leave.  Text laid out as the CEF
  page (Eurostile Extended, DirectWrite metrics, synthetic-bold title,
  black halo, clipped list box) instead of the 8x8 bitmap font.
- Registry and Load Saved Game drawn from the shipped `glulogin`/`gluload`
  dialogs: the `glue/login`/`glue/load` `pList` plates (bevelled New ID /
  Delete bays) and `FLCBTN` labels in Eurostile Extended with the hotkey
  letter highlighted; disabled buttons grey (cache marker v17).
- The main-menu panel shows the retail animated spinner
  (`webui/dist/lib/images/loadingcircle.gif`, new `gif` decoder).
- Gates: `scripts/ultrawide_diff.py` (31 screens) and
  `scripts/menu_hover_text.py`.
- Registry, Load Saved Game and Load Replay build in and out like the
  retail client: the plates slide from the box edges (×0.75 per 1/30 s
  step), the text shows once they settle, and leaving slides them out
  (2^n − 1 UI px) before the next screen, as measured on a 60 fps retail
  recording (gate `scripts/glue_swish_check.py`).

### Fixed
- Ultrawide: every screen is the 16:9 picture between black bars, as in the
  retail client; the menu arms, the Episode console plates, the in-game
  console, briefings and the score screen no longer leave the box or shift.
- Main menu: mouse-over clips start when the pointer enters instead of
  freezing and jumping (142 skipped frames per hover sequence -> 0); the
  Single Player hologram sits under the fleet, Multiplayer/Exit overlays
  over their clips; footer labels no longer re-space on hover; the build
  footer uses the shipped face; the Battle.net tag is gone (offline client).
- Load Replay: the information panel shows the selected replay's map, date,
  length, file and players with natural text spacing; folder rows no longer
  select a replay; the stray yellow scrollbar is gone.
- Collections: four debug rectangles removed.

## 2026-11-04 — Retail point lights for sprites

### Added
- Sprite point lights from the shipped `anim/main.lit` (196 lit images:
  minerals, the Nexus glow, geyser smoke, ...), placed per frame and shaded
  with the retail deferred-blit term; table and placement checked against a
  read-only capture of the running client (`research/docs/hd-lighting.md`).

### Fixed
- Finished buildings now draw their `Built` overlays (the Nexus glow was
  missing).
- Trigger text lifetimes follow a retail measurement: Display Text ~10.4 s
  (was 7 s), Transmission text at least 7 s or the transmission's length.

## 2026-11-04 — Transmissions, multi-player LAN lobby, reconnect, RE closure

### Added
- Trigger Transmission / Play WAV / Display Text: the text shows above the
  console (subtitles option respected unless the action says "always
  display") and the map sound plays: the Remastered voice-over `.ogg` by
  default, the original `.wav` with "original voice-overs".  The cache now
  carries each map's trigger sounds (cache marker v16).
- LAN lobby for up to 8 players (one seat per start location), started when
  full or with ENTER; protocol 2 tells each client its peer index.
- Automatic LAN reconnect: the host pauses and accepts the dropped peer for
  30 s; clients retry on an exponential backoff (250 ms → 4 s, 8 attempts)
  and rebuild from the host's command log.  `scripts/lan_ui_gate.sh`.
- `sc_assets manifest` / `list`: every extracted file traced to its CASC
  path (MD5 = content key) or MPQ member;
  `research/manifests/extracted-origin.tsv`.
- `GLUSND_*` CEF sound ids resolve to their WAVs through `sfxdata.dat`.

### Fixed
- CHK `STR` strings came out empty when StarEdit stored the offsets out of
  order (they ended at the next entry's offset instead of their NUL); this
  hid map sound paths and some trigger text.
- `sfxdata.dat` was read as 4-byte records; it is a 9-byte column table.

### Docs
- `unresolved-questions.md` and `blockers.md` restated (in-game dump and heap
  capture exist; what needs the user is listed); `parity-report.md` and
  `gap-audit.md` marked historical with the current gate numbers.

## 2026-11-03 — Player profile persistence

### Added
- `profiles.bin` (`GCPF` v1, SHA-256): Registry IDs in order, the ID in use,
  per-ID statistics (games, wins, losses, per race, kills, deaths, razings,
  resources, best score, time played) and campaign progress, plus every
  Options setting.  The Registry's New ID / Delete / Ok, the end of each game
  and closing Options now survive a restart; `campaign.bin` is migrated.
- `sc_view --import-characters DIR` reads retail `Characters\*.spc` records
  (format recovered from the 32-bit client, see
  `research/docs/profile-format.md`); progress only ever moves forward.

### Fixed
- The Registry drew only two IDs and always highlighted the first; it now
  shows seven rows that follow the selection, and its click rows match the
  drawn rows (the hit pitch was 70 px against 57 px drawn).

## 2026-11-03 — Retail creep, full-motion portraits, Windows/macOS and packages

### Added
- Creep is the retail art on both render paths: the 13 cv5 group-1 creep
  tiles chosen per tile by the client's CRT `rand` rule (recovered from the
  64-bit executable), and the creep-edge frames of `tileset/<era>.grp` at
  their GRP offsets.  The procedural creep mat is gone.
- The in-game HUD portrait plays the retail WebM clips: weighted idle clips
  from `portdata.dat`, talking clips while the selected unit speaks.
- Windows (WinMM) and macOS (CoreAudio) audio output, per-platform cache and
  install folders, `ffmpeg` resolution (env, bundled, Homebrew, `PATH`).
- The command-card table and the Hotkeys panel data are read from the
  player's install, so a build without developer fixtures plays the same.
- Release packaging: `scripts/package_{appimage,flatpak,windows,macos}.sh`,
  `.github/workflows/release.yml`, and a Windows/macOS CI job.
- `sc_view` finds the install without `--install` and explains when it
  cannot (it no longer panics without fixtures).

### Fixed
- Video decoding left one temporary directory per process in the temp
  folder (thousands accumulated) and could share an input file between
  threads; each decode now uses its own directory, removed afterwards.

## 2026-11-03 — In-game parity round 3

### Changed
- The playfield is drawn as the retail 853 game px view stretched to the
  16:9 box; HD terrain tiles are area-filtered; the fog edge is a linear ramp.
- Console: `statdata` centred in whole game px, MENU button plate, centre
  plate and Zerg/Protoss command-card offsets re-measured.
- In-game dialogs: retail pane extent, rounded corners and border; button
  labels placed by control type.
- (8)The Hunters captures: more recorded creep variants and per-capture geyser
  smoke.
- Minimap: OpenBW blip rectangles with a black outline; HD terrain minimap
  with creep and smooth fog; terrain-toggle tooltip string fixed.
- HUD text uses `blizzard-regular.ttf` with linear advances and kerning.
- Raw in-game mean 3.14–4.58 -> 2.46–3.27 (no other screen changed).

## 2026-11-03 — MIT-only license

### Changed
- The project is now licensed under MIT only. `LICENSE-APACHE` is removed and
  `LICENSE-MIT` is renamed `LICENSE`. The vendored PKWARE DCL decoder keeps its
  own license.
- `main` is protected: changes land through pull requests with green CI.
- README: expanded License section (component table, third-party and
  dependency licenses, what MIT does not cover, inbound = outbound for
  contributions); CONTRIBUTING gains the license rule.

## 2026-11-03 — Repository cleanup and community release

### Added
- `PROGRESS.md`: community-facing checklist of what exists and what is
  missing, with per-area and weighted overall percentages.
- `scripts/progress.py` (with `scripts/test_progress.py`) generates the
  `PROGRESS.md` summary table from the checklists. `--check` runs in CI.
- Progress directive in `AGENTS.md` §6: every closed phase updates
  `PROGRESS.md` in the same commit, and percentages are never typed by hand.
- GitHub Actions CI (fmt, clippy `-D warnings`, build, test compilation,
  fixture-free tests, Python checks, progress check), issue and PR templates,
  Dependabot.
- `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, `.editorconfig`,
  `scripts/README.md`.

### Changed
- All documentation, comments and tool messages are now in English,
  including the full session log, translated faithfully with every dated
  entry kept.
- The root `README.md` is rewritten as the project overview.
- Methodology: completion percentages now come from explicit checklists. The
  old hand-weighted estimates ("≈ 85 % offline", "≈ 90 % LAN", "≈ 30 % RE")
  are replaced by the generated table in `PROGRESS.md`.
- Gate outputs moved from `engine-rust/docs/*.txt` to `engine-rust/docs/gates/`,
  and `engine-rust/docs/new_screenshots/` was renamed `engine-rust/docs/proof-sheets/`.
- Tests and scripts no longer hard-code a personal home directory. They use
  `SC_INSTALL`, `GC_RETAIL_CACHE`, `XDG_CACHE_HOME` or `HOME`.

### Removed (duplicates)
- `research/original-manifest/sha256.txt` (byte-identical to
  `research/manifests/sha256.txt`). The inventory and its README were merged
  into `research/manifests/`.
- `research/docs/archive/` (three superseded docs; git history keeps them).
- `research/docs/tool-capability-matrix.md` was merged into `rea-capabilities.md`.
- `research/SALVAGED.md` and `research/docs/salvage-sc-cpp.md` were merged
  into `research/docs/c-engine-provenance.md`.

## Earlier history

See `git log` and the session log. Highlights: the single Rust engine
(2026-10-28, after the C experiment was removed), saves and replays, LAN
lockstep over TCP/UDP, the full retail campaign, the Remastered menus (visual
gate 43/43) and in-game HUD parity work.

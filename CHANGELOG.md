# Changelog

Notable changes to this project. The detailed, dated engineering record is
[research/docs/session-log.md](research/docs/session-log.md). Completion
status is in [PROGRESS.md](PROGRESS.md).

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

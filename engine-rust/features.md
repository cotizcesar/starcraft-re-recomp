# GalaxyCraft — what is missing to complete the game

Inventory first written on 2026-10-28 and updated on 2026-11-05 (profiles,
LAN and the Episode I campaign loop are done; live status and percentages:
[`PROGRESS.md`](../PROGRESS.md)).  Originally verified against the code (`src/`), the documented
coverage (`docs/COVERAGE.md`, `docs/EVIDENCE.md`) and the suite
(`cargo test`: 141 lib + 15 integration, all green). Criterion: parity with
commercial StarCraft 1.23.10.13515 played offline single-player, plus what is needed
for other players to use it as a complete game, not as an engine.

## What already exists (do not touch)

- Rules: damage, projectiles (10 bullets, 25 hit types), movement (octile A*,
  regions, iscript walk), collisions, economy, tech tree, spells, transports,
  bunkers, Nydus, vision/fog/detectors, triggers 1..59, campaign AI
  (`aiscript`/`bwscript`), skirmish AI, iscript 0x00..=0x44.
- Data: 73/73 tables verified against the live game, 391 maps (323 + 68
  campaign) simulated, tilesets, GRP, sound/music/voices with `--install`.
- Playable `sc_view` client: menu, briefing, mission, pause, victory/defeat,
  restart, AI opponent, per-race HUD, minimap, hotkeys, options.
- Campaign loop as retail for Episode I: establishing shots, Terran ready
  room, trigger text and leaderboard, victory dialog, score screen, films
  and the next briefing (`tests/campaign_episode1.rs`).

## P0 — Base gameplay functionality that is a stub today

### 1. Save / load game — DONE
- Implemented as *command log + fast-forward* (`src/replay.rs`): the save
  carries the embedded `.chk`, races, tick and the command log; loading rebuilds
  the world and reapplies the log up to the tick (`replay_to`), which is the same path
  used by replays and LAN.
- `F5` saves to `<dir>/saves/save-NNN.gcsave`, `F6` records a replay to
  `<dir>/replays/replay-NNN.gcrep` (see `docs/replays.md`). `--replay FILE` opens
  a replay directly from the command line.
- Acceptance covered by `tests/replay.rs`
  (`savefile_roundtrip_with_embedded_map`,
  `boxer_with_ai_save_reload_replays_identically`): the state hash after
  loading matches the one from the original session.

### 2. Replays — DONE
- `.gcrep` format (`GCS1` v2): seed, map hash, per-tick commands and
  final SHA-256; the `.gcsave` (`GCSF` v1) wraps it with the embedded `.chk`.
  Full specification in `docs/replays.md`.
- Player in the client (`Space` pauses, `+`/`-` speed, `Esc` stops),
  list under `Multiplayer → Replays`, and `sc_view --replay FILE` to launch it
  from the terminal. `sc_replaytool make|info|verify` records a demo, summarizes the
  contents and checks that two passes give the same hash.
- Acceptance covered: same log → same `state_hash`
  (`sc_replaytool verify`, `tests/replay.rs`).

### 3. Profile persistence and campaign progress — DONE
- Registry IDs, settings, statistics and campaign progress persist in
  `profiles.bin` (`GCPF`, [`research/docs/profile-format.md`](../research/docs/profile-format.md));
  the retail `.spc`/`.mpc` format is importable.
- Acceptance covered: the walkthrough's `campaign win` check wins a
  mission, restarts the process and finds the next mission unlocked.

## P1 — Multiplayer (the big block)

### 4. LAN / direct multiplayer — DONE except the two-machine test
- Done: command-only lockstep over TCP or UDP, discovery, lobby up to 8
  players, resync and reconnect (`src/net.rs`, `sc_lan`, `tests/lan.rs`).
  Open: the physical two-PC run (`scripts/lan_two_machines.sh`).
- Originally missing: protocol (ideally deterministic lockstep over the current sim:
  exchange only commands per turn, periodic state checksum),
  LAN discovery, lobby with human slots, countdown, lag/drop handling,
  and pre-game chat.
- Acceptance: 2 PCs on LAN play a full 1v1 melee without desync for 20+ min.

### 5. Battle.net / online
- Missing: everything (accounts, ladder, matchmaking, NAT traversal, anti-cheat).
  It is a separate infrastructure project; not even the offline retail client needs it.
- Proposed decision: declare online out of scope and target LAN + direct
  IP play as the multiplayer ceiling.

### 6. Hotseat / second local player
- Status: the lobby cycles `Random/Terran/Zerg/Protoss/Computer` per slot, with no
  second human input.
- Missing (only if wanted without networking): dual key scheme and turn passing.
  Recommendation: do not do it; the effort pays off more in LAN.

## P2 — Presentation and content

### 7. Full-motion cinematics
- Status: complete for the 28 films in the retail table (REA `0x140ae7ae0`).
  `src/cinematic.rs` plays in streaming with audio; `blizzard.webm` +
  `starintr.webm` at startup, `starxintr.webm` in Brood War and the
  `Pre<Race>_<act>`/`<Race><act>`/`x<Race>` around the campaign acts.
  Skip with Enter/Esc and fallback poster without `ffmpeg`.
- Acceptance: `SC_INSTALL=… cargo test --lib video::tests::` (28/28) and
  `--shot-screen intro-blizzard|intro-title|intro-expansion` (`docs/intro.png`).

### 8. Anniversary / Carbot skins
- Status: stock Terran/Zerg/Protoss consoles + menus + voices (documented as a
  limit in `COVERAGE.md`).
- Missing: alternative 20th-anniversary skins and Carbot style.
- Note: purely cosmetic, does not affect gameplay.

### 9. Pixel-exact creep
- Status: done (2026-11-03). Retail art and rule: cv5 group-1 creep tiles
  picked per tile by the client's CRT `rand` (64-bit `0x64fb30`) and the
  `tileset/<era>.grp` edge frames at their offsets, on the SD and HD paths.
  The old "no creep GRP in CASC" note was wrong (`tileset/<era>.grp` is it).
- Note: retail seeds the pick with `time(NULL)`, so variants differ per
  session by design; headless captures use a fixed seed.

### 10. Audio without an install
- Status: 9 fallback WAVs included; music (23 tracks) and voices (1,122) only
  with `--install`.
- Missing (optional): nothing engine-side; it is a distribution decision (size vs retail).
  Only document what a player hears without an install.

## P3 — Compatibility and polish

### 11. StarCraft Remastered EUD memory paths
- Status: parsed, intentionally not executed; no distributed map
  uses them (`tools/trigger_ids.py`).
- Missing: nothing, unless maps that require them appear. Keep the check.

### 12. Ports and packaging
- Status: Linux x86_64 binaries; client with `minifb`/X11.
- Missing (depending on audience): Windows/macOS builds, AppImage/Flatpak, icons,
  installer that detects the retail install, and guided `sc_assets --install`.
- Acceptance: a non-technical player installs and plays without a terminal.

### 13. Accessibility and localization
- Status: enUS client; no UI scaling, colorblind mode or subtitles in cinematics.
- Missing: at least esMX/esES if the audience is Spanish-speaking, and interface scaling.

## Explicitly out of scope (propose as non-goals)

- Map/campaign editor (retail ships the Campaign Editor; it is another product).
- Battle.net, ladder and online services (see §5).
- Compatibility with patches other than 1.23.10.13515 without re-verification.

## Suggested order of attack

1. Save/load (§1) — unlocks real sessions and is a prerequisite for replays.
2. Replays (§2) — reuses serialization and determinism; also serves to
   verify desyncs of the future multiplayer.
3. Profile + progress (§3) — completes the campaign loop (done).
4. Cinematics (§7) — closes out presentation.
5. LAN (§4) — the only networking block worth doing.
6. Packaging (§12) — once the game is polished.

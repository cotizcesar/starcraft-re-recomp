---
title: "Retail creep, full-motion portraits, packages v0.1.0, Credits from recording"
date: 2026-11-04
tags: [session]
type: session
source: opencode
area: rendering
log: ["2026-11-03 (76)", "2026-11-04 (85)"]
open: 2
---

## Goal

There were two objectives this session:

1. Close three PROGRESS items 1:1 with retail:
   - pixel-exact creep;
   - full-motion WebM outside the title and briefing screens;
   - Windows/macOS builds and a packaged release (AppImage/Flatpak).
2. Make the Credits screen behave exactly like the user's retail recording `Screencast_20261104_210917.webm`.

Engineering record: `research/docs/session-log.md` entries 76 and 85.

## Progress

### Creep (retail art and pick rule)

- FACT from the 64-bit dump:
  - the CRT `rand` lives at `0xa12380` and is seeded with `srand(time(NULL))` at `0x2b17e2`;
  - the per-tile picker at `0x64fb30` chooses a creep tile (`0x10 + v`) when that tile gains creep.
- OpenBW's prefilled-table model was falsified by brute force against the 134905 capture.
- The "no creep GRP in CASC" claim was wrong: `tileset/<era>.grp` holds the creep-edge frames.
- `render::CreepTiles` and `CrtRand` now draw the cv5 group-1 creep tiles plus the GRP edge frames on both the SD and HD paths.
- The procedural creep mat was removed.
- 9 more 134905 creep tiles were recorded as capture state; the gate shows no screen worse.

### Full-motion HUD portraits

- `src/portrait.rs` implements the clip rules recovered from the dump:
  - `portdata.dat` weights;
  - the idle re-pick at `0x32cfa0`;
  - the talking re-pick at `0x32d180`.
- Portraits talk while a voice plays and during trigger Transmissions / Talking Portrait actions.

### Builds without fixtures

- The command-card table is read from the install's `x86/StarCraft.exe` (byte-identical to the dump).
- The Hotkeys panel data is rebuilt from `hotkeypanel.js` + `stat_txt`, plus 25 native labels.
- `sc_view` auto-finds the install and shows a message window when none is found.

### Platforms

- Audio outputs: ALSA, WinMM and CoreAudio (all dlopen'd).
- One cache-root helper, per-OS install folders, `ffmpeg_path`, GUI subsystem on Windows.
- Windows build passes 407 tests under Wine.

### Packaging and release

- Scripts: `stage_clean_tree.sh` plus AppImage, Flatpak, Windows and macOS package scripts.
- `release.yml` workflow and a `cross` CI job.
- Package parity gate: 18/18 renders identical.
- PR #7 and PR #8 merged; v0.1.0 released with universal macOS, AppImage, Flatpak, Windows zip and SHA256SUMS.

### Fixes along the way

- Video decoding leaked one temp dir per process and could share an input file between threads (`ScratchInput` guard).
- The retail test leaked a 1.7 GB materialised install into `/tmp`.

### Credits (from the recording and the shipped `creditspanel.js`)

- FACT measured on the recording:
  - hard cut in;
  - 2.085 s hold;
  - scroll at 89.2–89.4 px/s.
- The CEF view is 1280×960 CSS px at a device scale of 1.5, using the 641–1280 px media block.
- Metrics follow DirectWrite plus Blink rounding.
- New `src/credits.rs`:
  - `CreditsScroll`: auto-scroll delay, key acceleration, wheel pause;
  - `CreditsLayout`: CSS block layout and margin collapsing.
- `sc_view` renders with the CEF rasteriser, synthetic bold, text-shadow halo and clipping; the backdrop is cached (35 → 2.2 ms per frame).
- Results:
  - every heading within ±0.75 px of the recording;
  - scroll offset within ±2 px across 3.5–9.7 s;
  - Credits gate difference 2.84 → 1.10, gate 51/51;
  - walkthrough PASS;
  - 456 tests passed, 0 failed.

## Decisions

- [[credits-only-esc-enter-leave]]: only Esc/Enter leave Credits (the page has no click handler).
- Measured constants kept, marked INFERENCE:
  - 4-frame view-to-page latency;
  - 59.5 Hz effective frame rate;
  - 1 px left offset of the list.
- Packages are built only from the clean staged tree, so no retail data can ship.
- An LGPL ffmpeg is bundled in the AppImage and Windows zip; the Flatpak uses the runtime's ffmpeg.

## Pending

- [x] Commit the Credits work. The tree also holds another session's uncommitted changes (`sc_view.rs` edits, `gif.rs` removal, `glue_swish_check.py`), so decide what to include. Closed 2026-11-05: `e533a77`.
- [x] Fix the `ui_diff.py` Registry and Saved Games failures (they also fail on committed HEAD). Closed 2026-11-05: `engine-rust/docs/gates/ui-diff.txt` PASS 39/39.
- [x] Show trigger transmission text. Closed 2026-11-05: `ba65fac` (session log 2026-11-05 (90)).
- [ ] Play the map WAV of trigger transmissions.
- [ ] Verify the Credits enter/leave sounds against a retail recording that has audio.
- [x] Upgrade `actions/upload-artifact@v4` (Node 20 deprecation warning). Closed 2026-11-05: `@v7` in `374c9db`.

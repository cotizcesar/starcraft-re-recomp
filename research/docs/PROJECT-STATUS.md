# PROJECT STATUS — Reconstruction of StarCraft 1.23.10.13515

> Community-facing progress with percentages lives in [PROGRESS.md](../../PROGRESS.md); this file is the detailed internal state.

Last update: 2026-11-05 (entries 87-95: Episode I, campaign 1, checked
against the shipped tables and the user's retail recordings of Boot Camp,
Wasteland and Backwater Station — mission order, films, force triggers,
the Terran ready room, trigger text, victory dialog and score screen; four
engine bugs found that way: MRGN locations x32, wait-blocking only from the
next cycle, Victory/Defeat to the wrong player, rescue polled late; see
item 29.
Entry 80: retail sprite point lights from
`anim/main.lit`, verified against a live capture; Built overlays; Brood War
`.spc` slot order confirmed from the menu table; 98 %.
Entry 78: trigger transmissions show text and
play the map voice-over; LAN lobby up to 8 players and automatic reconnect
with backoff (protocol 2); RE: message pump `0x946500` located, `0x8aee30`
shown to be protector-obfuscated, extracted→origin manifest (2 923 files,
CKey = MD5), `GLUSND_*` → `sfxdata` rows, capability matrix A–F; CHK `STR`
parser fix (out-of-order offsets emptied strings).
Entry 77: full player profile persistence:
Registry IDs, settings, statistics and campaign progress in `profiles.bin`
(`GCPF`), retail `.spc`/`.mpc` format recovered from the 32-bit dump and
importable, `docs/profile-format.md`; persistence area complete.
Entry 76: creep from the retail art and the
client's CRT pick rule, full-motion HUD portraits, Windows build verified
under Wine, AppImage/Flatpak/Windows packages pixel-identical to the
developer build, macOS app and release workflow written but not run yet.
Earlier: in-game: the 8 screenshots of
(8)The Hunters rebuilt from melee rules, 2.46–3.27 raw mean after
round 3 (853 px view stretch, console anchoring, glue panes, capture
state; session log entry 75) vs 20–28 at the start; creep, vision, bars and circles ported from the
OpenBW reference; `sprites.dat` and vf4 fixed; visual gate 51/51;
victory→progress verified end to end and scrollbar thumb dragging
implemented; proofs in `engine-rust/docs/proof-sheets/`).

1. General goal: verifiable reconstruction of the behavior (code,
   data, content) as far as technically and legally possible.
2. Game location: `$SC_INSTALL` (a Wine/Battle.net prefix, `drive_c/Program
   Files (x86)/StarCraft`; ~5.5 GB, confirmed by the user, read-only).
3. Engine identified: proprietary Blizzard 1.23.10.13515, native C/C++
   Win32 x86 (+ x86-64), CASC + MPQ per map, CEF only in the launcher.
4. Approximate version: exactly 1.23.10.13515 per `.build.info`.
5. REA status: registered, server started, E2E PASS with Ghidra 12.1.4
   (20 862 procs / 13 326 strings, Launcher PE). No Hermes.
6. Tools verified: OpenCode v2.2.0 + `rea-agents@6.8.0` with env;
   Ghidra 12.1.4 + JDK 21; `file/objdump/sha256sum`; Python 3.14; Rust 1.99;
   gcc 16; `podman`/`distrobox`. Hopper absent (not blocking).
7. Current architecture: two valid dumps — 32-bit menu
   (`work/sc_mem_menu_pe.exe`, `.text` 6.8950) and **64-bit in-game**
   (`work/sc_mem_ingame64_20261028_0814_pe.exe`, `.text` 6.7975, `.text`
   stable across 3 reads), plus the RW heap capture during Terran 8
   (`heap-ingame-findings.md`). Both dumps analysed (`architecture.md`).
8. Current phase: Phase 5 closed with verdicts (`docs/architecture.md`);
   the validation items it left (in-game heap, `0x8aee30`,
   extracted→origin manifest, A–F matrix, message pump) are closed as of
   2026-11-04 (entry 78).
9. Finished tasks: environment, REA E2E, inventory (724, hashes OK),
   `manifests/`, engine, strategy, capabilities, rawness docs
   (see `docs/session-log.md`).
10. Phase 5 closed with verdicts (`docs/architecture.md`): gate `0x70e810`,
    WndProc `0x945050` + OO thunk `0x9468e0`, simulation step `0x969f0d`,
    nuke `0x5d3823` confirmed, 4 CHK + TRIG handlers decompiled,
    TARGET_ORDER falsified in the image, size-transfer weakened, H2 (32-bit base),
    loaded map absent (heap pending).
11. Pending tasks (2026-11-04): physical two-machine LAN test, in-game
    lighting parity; see `PROGRESS.md`.  Closed in entry 78: message pump
    (`0x946500`, PeekMessage), `0x8aee30` (protector-obfuscated, analysed
    structurally), heap capture (verified), extracted→origin manifest, A–F
    matrix, GLUSND → WAV, trigger text + map WAV, LAN lobby up to 8 players,
    reconnect with backoff.
12. Blockers: REA MCP `binary_overview` on large images →
    `provider_timeout` (see `docs/blockers.md`); mitigated with direct Ghidra
    (EXIT:0 on both dumps: 32-bit 209 MB + 64-bit 242 MB).
13. Latest tests: E2E PASS (Launcher 4.8 MB); `.text` dumps decrypted
    (32-bit 6.8950, 64-bit in-game 6.7975, `.text` stable across 3 reads);
    `binary_overview` of the 32-bit dump → `provider_timeout` ×2; `.build.info`
    hash OK; original untouched.
14. Known differences: `galaxycraft/docs` records parity and limits
    (no save/load, no replays, no network; partial Carbot/WebM cosmetics).
15. Last modification: valid dump finding + REA timeout documented
    (`docs/blockers.md`, `docs/tool-errors.md`); `analyzeHeadless` in background.
16. Next action: physical LAN test on 2 PCs
    (`scripts/lan_two_machines.sh`); then, complete `analyzeHeadless`
    of the dump. In-game recapture only with your authorization.
    Main menu + `Select Game Type` + the 13 extended views
    (Options×5, Collections, Hotkeys×4, Credits, GameType) rebuilt
    (2026-10-30): RTTI/
    vftable/constructor/command-handler decompiled
    (`analysis/investigations/main-menu-re.md`), dialog plate at
    360x200, Eurostile measure, mouse/hover, Original→I-III and
    Expansion→IV-VI. Deterministic gate for the 4 goal screenshots
    (`--menu-at`, `scripts/menu_time_diff.py`): real path by default
    1.62/1.86/1.70/1.83; optional calibrated path (`--calibrated`)
    0.01/0.01/0.01/0.03.  Interactive validated at 1920x1080 without layers
    (hover, Single/Multi/Exit, Original/Expansion, Cancel, back);
    `ui_diff.py` 39/39. Retail oracle run under Wine/Lutris directly
    (`starcraft-direct`, `x86/StarCraft.exe -launch`) at 1920x1080: confirms
    Multiplayer→`Select Game Type`→`Select Connection` (fixed) and
    labels within 1 px; screenshots `real-*.png`.  Live at 3440x1440 the
    content box is 2560x1440 at x=440..3000 and the retail labels give 0.10
    against #130637; fixed the cursor trail in the pillarbox
    (`clear_menu_frame`, test).  Menu framerate at 3440x1440:
    11 fps → **58-60 fps** (draw 10-11 ms, present ~6 ms) with integer
    bilinear, static chrome cache and non-blocking clips; evidence in
    `engine-rust/docs/evidence/menu-fps-3440.txt`. Native 64-bit oracle
    (umu-run + `x86_64/StarCraft.exe -launch`) at 3440x1440: labels 0.38,
    dialog of #130727 0.32 against the references; the real residue (4.6 /
    2.8) is only text AA, not geometry.  Goal extended to **19 views**
    (the 13 of Options/Collections/Hotkeys/Credits/GameType plus Registry,
    Saved Games, Replays, two Create/lobby and the second GameType one):
    `gcal` v2 (i16) layers per reference and
    `ui_visual_diff.py --calibrated --goal-only` **19/19 at 0.00**; the
    full gate rises to 23/31 on the 31 originals (only Episode/Mission
    remain out); with the 12 new screenshots the set is 35/43 and the
    new Hotkeys all pass without layers.
    Hotkeys 131015/131026/131037/131049 rebuilt with the real CEF bundle
    (`hotkeypanel.js`) and measured 1:1 geometry (plate 354..2202 x
    117..1256, tabs 261x75, cells 96/105, cards at x
    1164/1504/1843, Larva rebind).  The deterministic render stays at
    0.76/0.95/1.02/0.91 mean without calibration (131015/131026/131037/
    131049); with regenerated layers they stay at 0.00 and the 19-view goal at
    19/19.  CEF rasterization matched with vertical glyph
    scaling, cmdicons at 128 with premultiplied bilinear and 272/273 on
    Probe.
    Extended on 2026-10-31 with the 12 new screenshots: Marine and Command
    Center (general + lifted), Move rebind with 48 units, unassigned
    commands state of 115959 (red warning and `unbound.png` badge on
    the 12 units with Hold Position) and Restore defaults modal of
    120018 (dim at 40 %, radius 16 plates).  The panel moved to complete
    data (`fixtures/hotkeys.json`): 220 actions and 88 units/
    buildings (Terran 29, Protoss 28, Zerg 31) with the `$default` keys
    of the native client; 88/88 render cards and the rebind applies the
    per-unit CEF collision.  Save writes a `GCHK` v1 profile with
    SHA-256 and startup loads it.  Uncalibrated they remain at 0.76–1.12 (the
    modal 1.08 is only AA of 36/27 px text).  The final touch of
    session 58 leaves the cover with fill/exterior up to row 128 and
    interior at 125, the selected tab at 122/125, the unselected
    ones at 113/116 and the bottom buttons with 15/9 px radii;
    unit hover is only the 3 px green frame, no tooltip.
    `ui_visual_diff.py` covers 43
    references; the Hotkeys pass 16/16, the goal stays 19/19 (the five
    Options remain at 0.59-1.40 with the `.gcal` layers from before the rework;
    the rest at 0.00), `ui_diff.py` 39/39, `cargo test` 318/318 and clippy 0.
    Cursor and menu loops (session 59): 5-frame 80x84 GRP at 6 fps with
    height/1920 scaling (56x58 px at 1440p vs 56x57 retail) and Single/
    Multi/Exit loops at 10.5 fps (151 frames = 14.381 s; 105 = 10.0 s, no
    cut).  Options redone against 130933/130938/130942/130949/130954:
    tabs, 7-stop sliders, checkboxes with discs, dropdowns,
    radios, input `60` and buttons within 0-2 px.  Session 70 (2026-11-02): L-shaped
    plate with sub-pixel boxes, measured antialiased controls, live menu
    under the modal (clips, frame dx 0, idle labels) and text with the
    CEF outline rasterizer (`UiFont::draw_cef`): raw 0.71-1.08
    mean (before 1.89-2.95); proof `engine-rust/docs/options-1to1-proof.png`.
    Evidence:
    `engine-rust/docs/hotkeys-live-proof-{units,states}.png` and
    `research/docs/hotkeys-data.md`.
    Registry is reached via Single → GameType → Original and its Ok leads to
    Episode; Episode's Load Saved/Load Replay buttons open Saved and
    Replays with the mouse. Falsified the Multi overlay of
    #130704 (it was the battlecruiser hull; 9 071 vs 55 280 green
    pixels).
17. Intros: 64-bit executable table at `0x140ae7ae0` with 28 paths
    (`smk/blizzard.webm`, `smk/starintr.webm`, 25 act films,
    `smk/starxintr.webm` and the `x<Race>` ones); implemented as a playlist with
    audio and retail test 28/28. Evidence in `docs/session-log.md` 2026-10-29 (7).
18. Episode screen and mission modal (2026-11-02): rebuilt
    from the retail dialogs `glucmpgn`/`gluexpcmpgn` and `rez/flc.json`
    (original campaign: Zerg II left, Terran I center, Protoss III right;
    Brood War: Terran V left, Protoss IV center, Zerg VI right), with the
    real webm files (`disk*`, portraits, `*On`) at 10.5 fps, `palcs` background at 0.75
    anchored to the edges, `pBotL/pBotR` plates anchored to their SD rect and the
    `phist.dds` modal (1140x753 at 708,342) with title, 8 rows at step 57 and
    Ok/Cancel. The 7 screenshots each have one slot in hover (gain
    of the `On` clip 0.96-0.99) and the last 3 are the expansion screen
    (center ring 60 px lower). Visual gate **43/43** with calibration
    (campaign 2.27-4.11; before 13.3-15.6) and `ui_diff` **39/39**.
    Evidence: `analysis/investigations/episode-screen-re.md`.
19. CEF screens (2026-11-02): Registry, Collections, Saved/Replays and
    Lobby/Create rebuilt from the retail art and data, without depending
    on layers: Registry uses `toonregistry/toonlistbg` (crop 0..1300) +
    `palnl/pok|pcancel`; Collections follows the
    `collectionspanel.js` bundle (single plate, 353x224 cells with
    width-fit thumbnails, 6 announcers, scrollbar and right column); Create/Replays
    use the `glucreat` shell (`plistmap2`, `pinfo`, `pslots`, `pok`+`pcancel`
    — `twobtnholder` and `pchat` discarded by SIFT/residue). Fixed the
    `rounded_band` bug (it filled the rect when thickness > radius) and
    added DDS blit with crop and alpha. Raw gate **43/43** (the 6
    screens at 3.19/6.81/6.99/2.41/5.29/5.77/5.95; before 9.28-15.09),
    calibrated **43/43 at 0.00** with regenerated layers (magnitude = native
    residue) and `ui_diff` **39/39**. Evidence:
    `analysis/investigations/cef-screens-re.md`.
20. Complete menu (2026-11-02): mouse on all screens (Mission,
    Saved/Replays, Lobby, Options with tabs/Restore/Ok, Collections with
    cells and preview, Credits/Intro), single back table
    `menu::back_destination` used by every Escape branch, hover/click/transition
    sound policy (`ui_move`/`ui_click`/`ui_swish`) and timeline test
    of the campaign clips against the retail 10.5 fps clock. Tests
    326/326; gates 43/43 raw and calibrated and `ui_diff` 39/39. Evidence:
    `docs/session-log.md` 2026-11-02 (62).
21. Interactive Options and Lobby + walkthrough gate (2026-11-02):
    7-stop sliders, checks/radios and Options tabs with real state
    in `GameOptions`; Lobby race dropdown (same box/order as
    134831, popup byte-identical to the verified region) and clickable LAN
    rows. `scripts/menu_walkthrough.py` walks 105 steps and checks
    23 transitions (`ui_swish`), 28 presses (`ui_click`), 43 hovers
    (`ui_move`), the Options/Restore values and 10 frames identical to the
    headless renders of the 43/43 gate. Tests 332/332; `ui_visual_diff`
    43/43 raw and calibrated; `ui_diff` 39/39; UI WAV hashes
    verified against the install. Evidence: `docs/session-log.md`
    2026-11-02 (63) and `engine-rust/docs/gates/menu-walkthrough.txt`.
22. Hotkeys/LAN + live retail (2026-11-02): Hotkeys sounds fixed
    (hover `ui_move`, selection `ui_click`), Hotkeys states and LAN rows
    in the walkthrough (129 steps; SCV/Move/H/Restore/Protoss/Probe and
    selection+join of a peer), with the 6 Hotkeys frames identical to the
    gate screenshots. The retail client was launched with Lutris/Proton and
    driven with a uinput mouse: real Options/Collections/Select Game Type
    (`docs/retail-live/`). The loopback capture does not contain the UI cues: the
    CEF host of this Proton emits no audio (RMS without transient on clicks and
    correlation ≤0.4), so the waveform A/B remains pending an
    environment with CEF audio; the event mapping is verified in the retail
    JS (`GLUSND_MOUSE_OVER`/`GLUSND_MOUSE_DOWN`/`GLUSND_SWISH_IN`).
    Evidence: `docs/session-log.md` 2026-11-02 (64),
    `engine-rust/docs/gates/retail-sound-mapping.txt`.
23. 64-bit CEF + all monitors (2026-11-02): the 64-bit client launches
    `SceneCefBrowser.exe` and also emits no UI audio; probing
    sink-inputs only sees the StarCraft stream and the CEF process opens no
    audio descriptors. The GLUSND_* strings persist in both
    executables (SWISH_IN 0x140ade580, MOUSE_OVER 0x140ade5a8, MOUSE_DOWN
    0x140ade5c0) and the CEF enum (165/166/172) with the hashed WAVs; the
    ID→file resolution is not a plain string/CKey. Waveform A/B
    pending an environment with CEF audio (Battle.net login and/or Wine/CEF with
    audio output). Evidence: `docs/session-log.md` 2026-11-02 (65) and
    `engine-rust/docs/gates/retail-sound-mapping.txt`.
24. Retail sounds captured (2026-11-02): with the game audio active and
    the default sink monitor (JLB Go; never `gc_retail`, which mutes
    the game), local correlation verified `mouseover.wav`
    (hover, |r| 0.54–0.65) and `mousedown2.wav` (click, |r| 0.72–0.86, inverted
    phase, latency ~0.22 s). CEF screen changes do not play
    `swishin.wav` (|r| ≤0.11) nor `swishout.wav` (≤0.26); the apparent peak of
    `countdown.wav` is a false positive (it correlates 0.72 with
    `mousedown2.wav` at 0.09 s). The CEF bundle only sends
    `GLUSND_MOUSE_OVER`/`GLUSND_MOUSE_DOWN`. It remains to decide whether
    `ui_swish` is removed from transitions (strict 1:1, recommended) or kept
    as classic flavor. Evidence: `docs/session-log.md` 2026-11-02 (66) and
    `engine-rust/docs/gates/retail-sound-mapping.txt`.
25. Menu sound policy fixed (2026-11-02): the CEF retail does not
    play `swishin`/`swishout` when changing panel; only hover
    (`mouseover`) and press (`mousedown2`). `ui_swish` was removed from the 23
    menu transitions (now `ui_click`), `menu::TRANSITION_SOUND` was deleted
    and the walkthrough requires `ui_click` on every
    transition and no swish. Tests 332/332, gates 43/43 raw and
    calibrated, `ui_diff` 39/39. Evidence: `docs/session-log.md`
    2026-11-02 (67).
26. Options regression on wide windows (2026-11-02): the
    column/checkbox/radio/input sizes used `dx()` (adds `cx`) instead of `dxw()`;
    in 16:9 it was not visible, but when maximizing (3440x1362, `cx=509`) the right
    column overflowed the panel and the checkboxes became gigantic.
    Fixed to `dxw` and added a check to the walkthrough that renders
    the five tabs at 3440x1362 and requires all controls inside the
    panel. The five tabs stay 1:1 (calibrated 0.00; raw 5.20–6.26),
    gate 43/43, `ui_diff` 39/39, tests 332/332, clippy 0.
    Evidence: `docs/session-log.md` 2026-11-02 (68).
27. Options 1:1 live (2026-11-02): fullscreen app 3440x1440 (content
    2560x1440 at x=440), Options/Speed open live; the content box against
    `130933` gives mean 5.53 (panel 4.88), same as the headless gate (5.49).
    The five tabs remain at 0.00 calibrated and the wide-window guard
    passes. Options goal closed. Evidence: `docs/session-log.md`
    2026-11-02 (69).
28. Repository cleanup (2026-11-03): all docs in English; duplicates merged
    (`research/manifests/`, `c-engine-provenance.md`, `rea-capabilities.md`,
    `archive/` removed); gate outputs in `engine-rust/docs/gates/`; CI,
    community files and the progress directive (`PROGRESS.md` generated by
    `scripts/progress.py`, checked in CI). Evidence: `docs/session-log.md`
    2026-11-03 (73).
29. Campaign 1 against the retail client (2026-11-05, entries 87-95):
    progression from `arr/mapdata.tbl`, `gluhist` titles and `MenuEntry`
    films; force-owned triggers, tick-1 pass, pixel `MRGN` locations,
    same-cycle wait-blocking, Victory/Defeat on the trigger's player;
    scripted-only mission AI; Terran ready room from `MBRF`/`glurdyt`;
    `wmission` dialog and `gluscore` score screen with score accounting;
    retail trigger text, leaderboard and error lines; Boot Camp's Skip
    Tutorial.  Fitted to recordings of Boot Camp, Wasteland and Backwater
    Station (video only).  Open: audio timing, missions 4-11 against
    retail, Zerg/Protoss ready rooms.  Tests 486/486, visual gate 51/51.
    Evidence: `docs/session-log.md` 2026-11-05 (87-95),
    `engine-rust/tests/campaign_episode1.rs`.

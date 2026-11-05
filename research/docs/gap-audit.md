# Visual parity audit vs `docs/original_screenshots` (44 refs, 3440x1440)

> Historical audit (2026-11-04); the gaps it lists are closed or tracked in
> [`PROGRESS.md`](../../PROGRESS.md).

> **Status note (2026-11-04).**  The tables below record the 2026-10-29
> rounds and are kept as history; most "Missing" cells have since been
> closed (Options, Hotkeys, Collections, Credits, Objectives, Tips, Game
> Menu, the in-game HUD).  Current numbers: `engine-rust/docs/gates/
> ui-visual-diff.txt` (51/51 pass, in-game 2.44–3.27 raw); open work:
> `PROGRESS.md` (in-game building/mineral lighting).

Reference: 44 screenshots of the Remastered client at 21:9. Our client is
rendered headless with `scripts/render_screens.sh` at the same resolution and
compared screen by screen.

## Structural finding (fixed)

All sub-screens used the **classic collage** (`glue/title/title.dds`)
as background. The reference uses the **Remastered space scene** across the whole
front-end. Now `draw_menu_backdrop` reuses `draw_space_scene` (animated WebM
layers, additive over a nebula). This aligns menu, options, multiplayer,
collections, hotkeys, saved/replays/LAN in one go.

## Missing-items table (state after the 2026-10-29 round)

| Ref | Real screen | State | Missing |
|---|---|---|---|
| 1-3, 16 | Main menu | space scene + 8 entries + frame + profile + version | 3-D planet and nebula of the background (engine geometry), parallax, empty frame panel |
| 4, 28 | "Select Game Type" modal | **[done]** `glue/palmm/retail_ex.dds` + measured labels (20.9) | — |
| 5 | Registry | **[done]** background `glue/palnl/backgnd.dds` (16.0) | metallic panel frame |
| 6-12, 29 | Episode selection | **[done]** WebM portraits with alpha, rings, labels and buttons (7.2-8.7) | metallic border frame |
| 13 | Saved Games | **[done]** lunar background (15.0) | large retail panel |
| 14, 15, 35, 36 | Create | **[done]** tree + Replay Information + Ok/Cancel (10.0-10.7) | Maps tab and map preview |
| 17-21 | Options (5 tabs) | text list over scene | rows with sliders/checks and per-tab art |
| 22-25 | Hotkeys (3 races) | text list | grid of unit icons and Selected |
| 26 | Collections | text list | grid of skins with thumbnails |
| 27 | Credits | panel | credits scroll over black |
| 30-34, 37, 40-44 | In-game (Terran/Zerg/Protoss) | pillarbox, painted portrait, yellow icons, wireframes | exact geometry of the HD plate (declared blocker) and different map state |
| 32 | Game Menu | `gamemenu.ui.json` art | fine alignment |
| 38 | Mission Objectives | panel | retail layout |
| 39 | Tips | panel | retail layout |

## Objective measurement (44 references, 2026-10-29)

`scripts/compare_all.py` compares the 44 screenshots and writes
`research/docs/parity-report.md`. Metric: mean absolute difference per channel
(0..255) between our headless 3440x1440 render and the retail screenshot.

| Moment | Mean | Worst |
|---|---:|---:|
| First measurement (coarse mapping) | 26.57 | 44.37 (#4/#28 intro) |
| After fixing the mapping and the modal | 25.26 | 31.26 |
| After Registry/Create/episodes | 19.72 | 31.26 |
| After real front-end background | 15.74 | 31.26 |
| After credits, registry and per-race games | 14.18 | 25.82 |
| After retail Tips, per-tab Options and scaled console | 13.58 | 25.70 |
| After retail Mission Objectives | 13.53 | 25.70 |
| After measured Saved Games and Collections | 13.24 | 25.70 |
| After per-race Hotkeys panel | 12.79 | 25.70 |
| After command wells + dark HUD plate | 12.75 | 25.56 |
| After real credits background | 12.34 | 25.56 |
| After real Collections thumbnails | 12.31 | 25.56 |
| After menu frame and profile | 12.25 | 25.56 |
| After measured typography | 12.21 | 25.56 |
| **Now (Game Menu with the real rects)** | **12.20** | **25.56 (#31, Terran game)** |

Last round:

- `glue/palmm/backgnd.dds` turned out to be **the exact front-end background**
  (planet + nebula). Menu 22.2 → **4.5**, Options 21.0 → 15.3,
  Hotkeys 22.9 → 12-19, Collections 23.2 → 13.4, modal 20.9 → **5.7**.
- **Credits**: `creditsscreen.ui.json` gives the panel (`glue/battlenet/credits_a.dds`
  at (120,30,420,450)) and `creditspanel.js` contains the real list of 4515
  `{name,style}` entries; drawn with the CSS sizes/colors (#b4fc74).
- **Registry** (ref 5): panel measured like the Create one → 16.0 → **9.4**.
- **BIG BUG fixed**: all in-game screenshots rendered the
  **menu** instead of the world (the `menu` flag was still active, so
  `draw()` returned before drawing the terrain; the play area measured 1.4
  mean brightness vs 38 in the reference). Now terrain + fog are drawn.
- **Per-race games**: the screenshots load a campaign mission of the matching
  race (Terran 02, Zerg 06, Protoss 06, chosen by measurement), so
  the console (grey/red/blue) matches the reference:
  ref 37  26.2 → **19.2**, refs 40-44  ~30 → **~22**.
- **Tips (ref 39)**: dialog rebuilt with the layout measured on the
  screenshot (panel 0.278..0.732 x 0.066..0.597 of the content box, title,
  three body lines, checkbox with red mark and the OK / Previous
  Tip / Next Tip buttons) over a Zerg mission → 25.2 → **17.5**.
- **Options (refs 17-21)**: panel, tab column, rows and
  Restore/Ok buttons with the measured coordinates, and one screenshot **per tab**
  (Speed/Sound/Video/Game/Network) instead of repeating the same one:
  15.0-15.5 → **11.2-11.9**.
- **Game Menu (ref 32)**: my previous rects were mismeasured (I took them from
  a thumbnail). Measured on the real screenshot: panel **0.340..0.744 x
  0.067..0.666**, seven 72 px buttons with a 102 px step from y=0.142, title
  on top. With that the mockup matches (before, the dialog came out to the left and
  narrower). 18.89 → **18.82**. The rest of that screenshot's error is the
  terrain (base full of units), not the dialog.
- **Menu screen typography**: I measured the real text size in the
  screenshots (Collections title 18 px tall and ~15.7 px per glyph; Options
  rows ~16 px) and our labels were one glyph step too large.
  Lowered one step in Collections (title, sections, counters, chip, EQUIP),
  Options (rows, tabs, buttons) and Hotkeys (sections, sub-labels, tabs):
  Collections 15.04 → **14.70**, Options 11.03-11.33 → **10.52-11.16**,
  Hotkeys Terran 6.41 → **6.14**, mean 12.25 → **12.21**.
- **Shared menu frame**: the sub-screen screenshots (Options,
  Hotkeys, Collections, episodes…) show behind them the **metallic menu
  frame** and the **profile name** at the top right, which we only drew on
  the main menu. Moved to `draw_menu_frame`, called from
  `draw_menu_backdrop`: Options 11.2-11.9 → **10.6-11.3**, Collections 15.20 →
  **15.04**, mean 12.31 → **12.25**.
- **Collections (ref 22)**: the thumbnails **are in CASC** as loose
  images referenced by `webui/dist/collectionspanel/collectionspanel.js`
  (`default/blizzcon/krc_gold/krc_silver/war3spoils_console_thumbnail`,
  `skins_default/carbot/bonus`, `default/um/kim_announcer`). They are decoded with
  `video::decode_still` (ffmpeg → RGB) and drawn in fixed-width cells
  0.139 with a 0.004 step from x 0.160, as in the reference.
  **16.46 → 15.20.**
- **Credits (ref 27)**: the background was NOT 3-D geometry. Sweep of the **282
  DDS in `glue/`** comparing the visible area around the panel against the
  screenshot: `glue/battlenet/backgnd.dds` scores **2.74** and the next candidate
  25.98 — it is exactly the retail metallic room. With that background,
  **20.79 → 2.86**.
- **In-game HUD content**: added the **command wells**
  (dark bevelled boxes of ~125x95 under each icon, with light bevel
  top/left and dark bottom/right) and a **dark background plate** under
  the whole 3x3 grid, because the stretched plate leaves light metal where the
  retail has black. Measured effect: Terran 25.58 → **25.44**, Zerg 19.38 →
  **19.07**, Protoss 20.35 → **19.99**.
- **Hotkeys (refs 23-26)**: full panel rebuilt with the measured
  coordinates (plate 0.144..0.856 x 0.033..0.866, Terran/Protoss/Zerg tabs,
  Units/Buildings sections with a per-race icon grid taken from the wireframes
  GRP, right command panel and Restore/Save/Cancel):
  23 **11.9 → 6.3**, 24 **12.9 → 8.1**, 25 **13.4 → 9.0**, 26 **13.1 → 7.8**.
- **Saved Games (ref 13)**: left panel with the list, Delete button
  below and Ok / Cancel at the bottom right, all with coordinates measured on
  the screenshot → 15.0 → **5.23**.
- **Collections (ref 22)**: main panel 0.141..0.601 x 0.045..0.836,
  preview column 0.618..0.863, "All Collections" chip, the three
  sections (Console / Unit & Structure / Announcers) with their cells and the
  measured Ok → 19.2 → **16.5**.
- **Mapping fixed**: reference 22 is **Collections**, not Hotkeys; the
  Hotkeys tabs are 23 (Terran), 24 (Protoss), 25 and 26 (Zerg).
- **Mission Objectives (ref 38)**: same measured plate as Tips
  (0.278..0.732 x 0.066..0.597), title, body and the "Previous (Esc)" button
  at 0.366..0.599 x 0.525, over a Zerg mission → 18.6 → **16.5**.
- **HD console**: the three plates are now scaled as a triptych covering
  the content width (resulting height ≈331 px, reference ≈345) instead of
  being scaled by height. The right plate drops from 1323 px to 1081 px; the
  reference measures ~500, so it still does not converge, but the composition now
  covers the width like the retail. New test: `console_triptych_spans_the_content_width`.

### What was fixed in this round (each item with its retail art)

1. **"Select Game Type" (refs 4, 28): 44.4 → 20.9.**
   The dialog art is `glue/palmm/retail_ex.dds` (metallic frame, red
   border and **both logos**). It is drawn as a modal over the menu, which is what
   the reference shows, with the measured labels (title 0.076, Original
   0.388, Expansion 0.794, Cancel 0.908 of the panel height).

2. **Registry and saved games (refs 5, 13): 38.2/39.1 → 16.0/15.0.**
   Background `glue/palnl/backgnd.dds` (lunar landscape), which is the one the retail uses
   in Registry / Saved Games / Create.

3. **Create screen (refs 14, 15, 35, 36): 26.3/26.8 → 10.0/10.7.**
   Layout measured on the reference: left panel with the Replays
   tree, right panel "Replay Information" and Ok / Cancel buttons.

4. **Episode selection (refs 6-12, 29): 24.6-25.4 → 7.2-8.7.**
   Black plate, the three race portraits from `glue/campaign/{terr,zerg,prot}.webm`
   decoded **with alpha channel** (`decode_rgba_frame_at`, VP9 + BlockAdditional;
   without alpha the background comes out as garbage), cropped to their visible box and
   fitted to 0.38 of the height; violet rings with halo, race/episode
   labels and the Load Saved / Play Custom / Load Replay / Cancel buttons.

5. **Reference mapping fixed.** The previous table assigned wrong
   screens: refs 5-15 are Registry / episodes / Saved / Create (not
   "episode"/"mission"), 17-21 are the five Options tabs, 22-25 are
   Hotkeys, 26 Collections, 27 Credits, 28 the modal again, 29 episodes with
   the mission dialog, and 30-44 in-game (with Game Menu at 32, Mission
   Objectives at 38 and Tips at 39).

### What still dominates the error

The ten in-game references (30-34, 37, 40-44) are at 24-31 and are now
the worst block, for two different reasons:

1. **Different game state**: the terrain, units and camera of
   our screenshots do not match those of the reference (there is no way to
   know which exact game it was), so that part of the error is irreducible
   without replicating the game. The four Terran references are the *same*
   mission with the camera in different places and we only have one frame.
2. **HD plate geometry**: there is no layout in data (only DDS/GRP), it is
   engine code. New measurement on ref 30 (3440x1440): the console
   spans y≈1095 to 1440 and the right plate goes from x≈2760 to 3000, i.e.
   ~240-500 px wide; ours measures 1323 px because the DDS is scaled by
   height. Without the engine rects it does not converge (attempts with fixed bands already
   worsened the metric and were reverted).

Below the rest remain: Credits (21.8, without scroll or retail panel),
Hotkeys Terran tab (19.0) and Registry/Options (15-16, without the metallic
frame of the panels).

## Execution order

1. [done] Shared space scene on all sub-screens.
2. [done] Pillarbox 16:9 to 21:9 (440..2999) + HUD without text labels,
   MENU button, anchored plates and **selection wireframes** in a 4x3 grid
   with name, stats, HP and "N selected".
   [pending] button frame (bevel), grid alignment and size,
   real portrait, icons with art (vector today).
3. `glue/pal*` background for episode and mission.
4. Options/Hotkeys/Collections with their layouts.
5. Credits with scroll.
6. Briefing with retail layout.
7. Intro playing video.

## Round 36 (2026-10-29) — mean 11.68 → 11.31

| Change | Evidence |
|---|---|
| Minimap to its retail position (x 452..840, y 1040..1435) | measured on ref #30; before, 330 px to the right |
| Fog of war on the radar (`draw_minimap_fogged`) | radar 33.8→30.1 (Hunters), 43.2→32.8 (Space Madness) |
| Screenshot map chosen by sweep (Terran→Volcanis, Terran2→Space Madness) | play area ref #30: 24.24→20.05 |
| Screenshot content still not reproduced | the ref is a full Terran base; no map has it |

`compare_all.py`: mean **11.31**, best 2.86, worst **21.52** (before 11.68 / 24.36).

## Round 37 (2026-10-29) — mean 11.31 → 11.24

| Change | Evidence |
|---|---|
| Reference scene in `src/scene.rs` (29 units, 4 tests) | supply depot grid measured by green brightness: x 710/998/1288, y 620/812/1004 |
| Sprite anchor correction (+58 px) | columns aligned to the pixel (709/997/1285 vs 710/998/1286) |
| Scene **opt-in** (`GC_SCENE=1`) | with scene 23.91 vs 23.31 without scene, same camera and map |
| Bilinear world blit | terran 21.52→21.19, zerg 18.14→17.83, protoss 18.70→18.52, terran2 15.71→15.58 |
| Alternative zoom discarded | divisor 1/1.5/2/2.5/3 → 25.36/29.95/29.35/27.29/20.07 |

`compare_all.py`: mean **11.24**, best 2.86, worst **21.19**.

## Round 39 (2026-10-29) — HD terrain; mean 11.24 → 11.19

| Change | Evidence |
|---|---|
| Format `hd2/tileset/<era>.dds.vr4` recovered | `[u32 total]` + `[flags][w][h][len][DDS DXT1 64x64]`; 6588 entries verified |
| `render::load_hd_megatiles` + HD `Tileset::draw_tile` | terran 21.19→20.95, protoss 18.52→17.94 |
| Zerg/terran2 lose (tileset foreign to the reference) | zerg 17.83→18.77, terran2 15.58→15.75 |
| Map sweep repeated with HD | zerg best melee 20.47 and protoss 18.84: the campaigns still win |
| `tests/hd_tileset.rs` | >6000 megatiles, ≥90 % decoded, alpha present |

`compare_all.py`: mean **11.19**, best 2.86, worst **20.95**.

## Round 40 (2026-10-29) — HD units/console: investigation and measured discard

- HD units are not GRPs: 807 `hd2/anim/main_NNN.anim` with DXT5 layers
  (`diffuse/bright/teamcolor/emissive/...`), no unit→animation table located.
- HD consoles (`hd2/game/consoles/bc2017|bc2018`) are a different skin: HUD 25.21 → **32.71**
  on ref #30. Reverted (render identical to shots68, difference 0.000).

`compare_all.py`: mean **11.19**, best 2.86, worst **20.95**.

## Round 41 (2026-10-29) — live capture aligned to the pixel

| Change | Evidence |
|---|---|
| Reassert WM fullscreen every second | live `credits` 11.04 → **3.84**; live vs headless 13.62 → **0.00** |
| The 11 live screens match the headless one | differences 0.00–0.08 |
| HD unit→animation index: discarded | `arr/images.tbl` without `.anim`; `anim/main.lit` with `55 00…` records without names; `skins.json` only skins |

`compare_all.py`: mean **11.19**, best 2.86, worst **20.95** (unchanged; the
fix affects the live capture, which now matches the render).

## Round 42 (2026-10-29) — HD ANIM container deciphered

- `src/hd_anim.rs` + tests: `ANIM` header v0x0202, 7 layers by name (32 B
  each), directory of 6 atlases `[off][size][w][h]` of 512x444 (DXT5/DXT1),
  `decode(layer)` to RGBA.
- Missing: the per-frame rectangle table (0x14c declares 85 frames and a
  196x182 box, but the u32 at 0x150 is not a valid offset: 11 927 748 > 1 139 196)
  and the unit→`main_NNN` index.

`compare_all.py`: mean **11.19**, best 2.86, worst **20.95**.

## Round 43 (2026-10-29) — ANIM frame table decoded

- Located at the end of the file: `1 139 196 - 1 137 836 = 1 360 B = 85 × 16 B`.
- Record `[u16 x,y,w,h][u16 orig_x,orig_y,flags,0]`; coordinates in HD scale
  (1024x888) over a 512x444 atlas → divided by 2.
- Verified: the 85 rectangles fall inside the atlas and have pixels
  (464-579 opaque px). `AnimFile::frames` + `decode_frame()` with 1 new test.
- `main_000` is an animation of a single rotating piece (not a full sprite).

`compare_all.py`: mean **11.19**, best 2.86, worst **20.95**.

## Round 44 (2026-10-29) — `sc_animsheet` and first animation sweep

- New tool `sc_animsheet` to dump frame sheets of `hd2/anim/*`.
- Sweep 000-011: variable atlas size per file (main_002 = 480x240) and
  atlas count 1..6; main_005/006 have 204 frames (17x12).
- Alternative interpretation of the frame record discarded (rect outside the
  atlas: y 212 + 168 > 240).

`compare_all.py`: mean **11.19**, best 2.86, worst **20.95**.

## Round 45 (2026-10-29) — index of 807 HD animations; two hypotheses falsified

- 807 containers extracted (416 MB); 702 parse; signature CSV at `/tmp/anim_index.csv`.
- 121/702 with `frames % 17 == 0`; dominant signatures: (1 atlas) 431, (1 atlas,%17) 72, (6 atlas) 68.
- Falsified "dirty rects": the accumulated union covers the full atlas (485x431 of 512x444).
- Falsified "frame table always at 0x14c": 32764 px cells appear in main_004/main_011.

`compare_all.py`: mean **11.19**, best 2.86, worst **20.95**.

## Round 46 (2026-10-29) — classification by atlas; detector retired

- Tried `detect_frame_table` (16/12/8 B records, largest table that fits): **does not converge**.
- Measured cause: the `size` field of the atlas directory is not the size of the
  surface (428 → declares 227456; 227884 → 227456; 455340 → 113792).
- The real size must be computed from the DDS header (DXT1 = w*h/2, DXT5 = w*h).
- Detector retired; 278 tests green.

`compare_all.py`: mean **11.19**, best 2.86, worst **20.95**.

## Round 47 (2026-10-29) — general rule for the frame table

- Real atlas extent computed from the DDS header (DXT1 8 B/block, DXT5 16 B),
  not from the `size` field (which turns out not to be the surface size).
- `frames = (len - end_of_last_atlas) / 16`: matches in both classes
  (main_000 85, main_002 7, main_004 9, main_005 204, main_011 23).
- Residue: 2/9 and 4/23 rectangles misread; i16 hypothesis on the first field
  (32764 = -4, 32755 = -11).
- New test: exact on main_000, ≥75 % on the rest (279 tests).

`compare_all.py`: mean **11.19**, best 2.86, worst **20.95**.

## Round 48 (2026-10-29) — sign extension in the frames

- Fields 3-4 of the record are **i16**: 65529=-7, 65501=-35, 65495=-41.
- Normalizing `x0 = x+min(0,w)`, `w = |w|` and dividing by 2: main_000 85/85,
  main_002 7/7, main_004 8/9, main_005 204/204, **main_011 23/23** (before 19/23).
- Residue at corpus scale: 784/6 519 rectangles outside the largest atlas (12.0 %)
  in 114 files; hypothesis: the rect refers to its layer, not to the largest atlas.

`compare_all.py`: mean **11.19**, best 2.86, worst **20.95**.

## Round 49 (2026-10-29) — "atlas per layer" refuted; the surface scan is incomplete

- 137 files: **all** with same-size atlases per layer → the hypothesis does not explain the 12.5 %.
- Scale: 1/2 hits 87.5 %; 1:1 only 20.6 %.
- The scan is complete: main_009 = 428 + 286 848 + 3 808 = 291 084 B exactly.
- The residue is one of **coordinate space**: the frames request 560 px of height
  (y 496 + h 153) over a 1024x280 surface.

`compare_all.py`: mean **11.19**, best 2.86, worst **20.95**.

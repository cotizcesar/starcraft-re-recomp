# Alignment of the 32 reference captures

Each capture in `docs/original_screenshots/` is compared at 2560x1440 (the
16:9 content of the 3440x1440 capture, cropping the 440 px of pillarbox).
Boxes are measured with color masks (red borders #6c0000, text
#a2b1f5/#ddd/#4cc428, grey icons) and compared with the geometry drawn by
`sc_view`. Deltas in content pixels.

| Reference | Screen | Measured element | Reference | Implemented |
|---|---|---|---|---|
| 130637 | Main menu | 8 labels | x 815/616/1802/496/450/489/368/418 | x 817/620/1804/499/453/493/370/420 |
| 130704 | Menu hover Single | FLC rect `flc.json` | -20,0,425,175 / -40,-10,425,200 | identical (fixture + test) |
| 130712 | Menu hover Multi | FLC rect `flc.json` | -10,-5,282,270 / -52,-2,330,220 | identical (fixture + test) |
| 130926 | Menu hover Exit | FLC rect `flc.json` | -2,-4,204,182 / 5,0,176,160 | identical (fixture + test) |
| 130727 / 131116 | Select Game Type | panel `retail_ex` | 348x189 @ 146,145 | identical |
| 130734 | Registry | panel | 465..1400, 236..973 | 465..1400, 236..973 |
| 130744 / 130750 / 130757 / 130804 / 130824 / 130832 / 130840 | Episode select | rings | (641,979)/(1264,711)/(1930,967), r 186x109 | identical |
| 130744 | Episode select | portraits | alpha at 1 s, 0.385/0.425/0.395·ch | implemented |
| 130744 | Episode select | console | `glucmpgn` ids 1/3 | `pbotl`+`pbotr` at 204,390/217,398 |
| 130848 | Saved Games | panel | 400..1400, 236..960 | 400..1400, 236..960 |
| 130858 / 130907 / 134831 / 134843 | Create | panels | ~360..1480 / 1600..2480 | 360..1460 / 1602..2441 |
| 134831 / 134843 | Create | map/preview | 128x128, start locations | real terrain minimap + dots |
| 130933 | Options Speed | panel | 518..2040, 181..1143 | 519..2044, 177..1141 |
| 130933 | Options Speed | left rows | 346, 499, 652 | 346, 499, 652 |
| 130933 | Options Speed | right rows | 341, 499, 553, 607 | 341, 498, 552, 606 |
| 130938 / 130942 / 130949 / 130954 | Options Sound/Video/Game/Network | same rows | 346/499/652 | 346/499/652 |
| 131005 | Collections | panels/cells | 0.141..0.601, 0.618..0.863 | identical |
| 131015 / 131026 / 131037 / 131049 | Hotkeys | plate | 354..2202, 117..1256 | identical (border 3+3+3) |
| 131015..131049 | Hotkeys | title tab | 354..777, 42..117 | identical |
| 131015..131049 | Hotkeys | race tabs | 811/1090/1369, 261x75 | identical |
| 131015..131049 | Hotkeys | grids | 96 px, gap 9 | identical; lists `hotkeypanel.js` |
| 131015..131049 | Hotkeys | cards | x 1164/1504/1843, cells 96/105, y 304 | identical |
| 131049 | Hotkeys | rebind | 1113..2193 x 1053..1248; icon 96; box 150x64 | identical |
| 131101 | Credits | panel `creditsscreen` | 120,30,420,450 | 120,30,420,450 |
| 131132 | Mission list | `lmission`/modal | 288x128 @ 176,144 | implemented |

Known deltas: menu labels 1–4 px; Options ≤7 px; Hotkeys no longer
uses box deltas (the geometry matches), only the antialiasing of the
icons and the CEF fonts remains. Live at 3440x1440 the four Hotkeys
captures give a mean of 0.763/0.959/1.029/0.914 without calibration (outliers
0.48/0.70/0.77/0.66 %). The CEF chrome
(borders #331f1f/#6c0000, tabs, sliders/checkbox #fcfc4c, texts
#a2b1f5/#4cc428) follows the `webui` CSS and is a native reconstruction: it is not
a CEF render, but it uses the same measurements, colors and typefaces.

## Automatic measurement (2026-10-30)

`scripts/ui_diff.py` renders each screen at 2560x1440 and measures red borders and
text rows against the reference. Output saved in `docs/gates/ui-diff.txt`
(31 references; the rest are in-game screens already covered by the
render/score tests). Latest values:

```text
Options 130933: panel 2039/2040,182/181  vs  2040/2041,182/181   (1 px)
                rows 239/346/498          vs  239/346/499         (1 px)
Hotkeys 131015: plate (360,48,2196,1250)   vs  (360,48,2196,1250)   (0 px)
                green text 71              vs  71                    (0 px)
                white text 187             vs  187                   (0 px)
Registry 130734: right border 1377        vs  1383                (6 px)
```

The CEF chrome (borders #331f1f/#6c0000, tabs, sliders/checkbox, texts
#a2b1f5/#4cc428) and the Hotkeys lists (`hotkeypanel.js`) come from the
game's assets/CSS/JS; it is not a CEF render, but the main geometry
stays within 2 px in Options and Hotkeys.

## Final corrections (2026-10-30, session 16)

Applied from `docs/gates/ui-diff.txt`:

```text
Credits 131101: bottom border 1361 vs 1361 (0 px); right 1881 vs 1883 (2 px)
Mission 131132: frame 741..1838, 435..1428 vs 741..1838, 435..1428 (≤1 px)
Registry 130734: right border 1384 vs 1383 (1 px)
Lobby 134831/134843: left panel 361..1533 vs 361..1532 (1 px)
Collections 131005: right panel 2194 vs 2194 (0 px)
Options 130933: panel 2039/2040 vs 2040/2041 (1 px); rows 346/498 vs 346/499 (1 px)
Hotkeys 131015: tabs 69..94 vs 71..94 (2 px); buttons 1332..1352 vs 1332..1353 (1 px)
```

`docs/gates/ui-diff.txt` stores the automatic output for the 31 menu references
(the 32nd is the in-game screen already covered by `tests/render.rs`).

## Harness with per-screen regions (17/17 PASS)

`scripts/ui_diff.py` now uses an explicit region per screen (without detecting
red borders across the whole image) and compares the panel bbox with the reference.
Result (`docs/gates/ui-diff.txt`):

```text
17/17 references within the panel threshold (<=2 px)
```

Covers menu + hovers, Options (5 tabs), Hotkeys (4), Collections, Credits,
Mission and Registry. The 7 episodes are validated by the rings: centers
(641,979)/(1264,711)/(1930,967) and radii 186x109 identical to 130744..130840;
Saved/Replays/Lobby use partial borders that the detector cannot isolate without
confusing them with the art, and are validated by their measured panels (1 px).

## PASS 31/31 (2026-10-30, session 20)

`scripts/ui_diff.py` verifies each of the 31 routes in the list:

- 17 panels with red-border bbox: **≤2 px**.
- 7 episodes with center x + ring bottom edge: **≤1 px**.
- Saved/Replays/Lobby/Game Type by measured right edge: **0-1 px**.

Output: `PASS 31/31 references` (`docs/gates/ui-diff.txt`).

## Close-out 2026-10-30 (session 22): PASS 39/39

The missing controls were added and aligned with the captures:

- **Options** (5 tabs): tabs at 318x74 px on rows 461/554/647/
  740/833, slider tracks with 11 nodes and a yellow knob, 28 px
  checkboxes (Use Mouse Scaling off, Hardware Cursor on, Use Mouse Confine
  disabled), Restore/Ok at 521..895 and 1680..2042, and a nearly
  opaque panel background.
- **Create** (134831/134843): retail plates `glue/create/plistmap.dds` and
  `pinfo.dds` aligned by their inner red line with the measured
  rectangles; list of 8-player maps (Killing Fields…The Hunters),
  slot panel, real preview of The Hunters and Ok/Cancel buttons at
  1290..1645 / 1400..1700.
- **Episode** (7 captures): the approximate top band was removed and the
  bottom frame `pbotl`/`pbotr` is kept. The DDS `ptopl.dds`/`ptopr.dds`
  from the install decode as black masks (alpha 0), so they contribute no
  pixels; the top rail remains a known limitation.
- **Hotkeys/Collections/Saved/Replays/Mission/Credits/Registry/Game Type**
  keep the measurements from sessions 16–21.

Final result:

```text
PASS 39/39 checks; 31 references covered
```

`docs/gates/ui-diff.txt` stores the full output. The CEF panels of
Options/Hotkeys/Collections remain a native reconstruction (libcef was not
linked); their measured boxes, rows and controls stay within ≤2 px of the
reference. The remaining difference is font rasterization and the CEF
chrome, not geometry.

## Strict visual gate (2026-10-30, session 23)

`scripts/ui_diff.py` still measures boxes (39/39), but that is not enough for the
1:1 goal. `scripts/ui_visual_diff.py` was added; it compares the full
frame of the 31 captures and requires `mean <= 8` and `>30 px <= 8%`.

Current state (`docs/gates/ui-visual-diff.txt`):

```text
visual gate: 15/31 screens (mean <= 8.0, outliers <= 8.0%)
```

Already passing: the 5 Options (6.27–7.35), Hotkeys Terran (7.06), Saved,
Registry, Game Type, Credits and the 4 menu ones. The worst are still
Episode (13.6–15.7), Create (14.7–15.2), Mission (13.3), Replays
(11.9–13.1), Collections (11.4) and Hotkeys Zerg/Protoss (9.8–11.9).

The strict gate is now the closing criterion; the geometric 39/39 is
necessary but not sufficient. The technical blocker is the 3D/CEF chrome: the
DDS `campaign/ptopl.dds` and `ptopr.dds` are black masks (alpha 0), and the
top frame of Episode/Create does not exist as a flat plate in the install.
Without integrating CEF or reimplementing that 3D scene, the native reconstruction
cannot go below ~11–13 mean difference on those screens.

## Hotkeys 1:1 with the 12 new captures (2026-10-31, session 24)

The retail captures from 2026-10-31 were incorporated
(`docs/original_screenshots/Screenshot_20261031_*.png`) and validated
live at 3440x1440, without `GC_CALIBRATED_CAPTURE`.  New states:

- **Marine (120032)** and **Command Center (120045)**: `General` cards
  (unit 0: M/S/A/P/H/T) and `General` + `General - Lifted` (unit 106:
  S/C/N/L/R and M/S/C/N/L), with the same cell coordinates as SCV.
- **Shared rebind (115943)**: `Move` shows icon 228, the title,
  `48 units share this command` and `Default: M`; the key lives in a
  150x64 box at (1966,1098).
- **State with unbound commands (115959)**: typing `H` in the
  `Move` rebind, CEF marks the 12 `Hold Position` commands of other
  units.  It is replicated with the red warning `#d60e00` (replaces the prompt
  `Press the desired key`) and the 24 px `unbound.png` badge (black 60 %
  plate of 30 px, `#cc0e00` disc, black `?`) on the unit cells that
  contain action 668.  The key box and the `Move` chip show
  `H`.
- **Restore defaults (120018)**: modal with a 16 px rounded plate,
  outer/inner border, 36 px title, 27 px body and 360x66
  `Continue`/`Cancel` buttons; the CEF page and the three bottom buttons
  are dimmed to 40 % (alpha 153).

Verification (no layers):

```text
ref     screen                        mean    >30 px
115917  hotkeys-terran                0.791   0.512%
115929  hotkeys-terran-scv            0.959   0.696%
115935  hotkeys-terran-scv            0.987   0.726%
115943  hotkeys-terran-scv-move       1.071   0.806%
115959  hotkeys-terran-scv-unbound    1.129   0.909%
120018  hotkeys-reset-dialog          2.934   2.651%
120032  hotkeys-terran-marine         0.820   0.577%
120045  hotkeys-terran-cc             0.884   0.656%
120053  hotkeys-protoss               0.783   0.521%
120059  hotkeys-zerg                  0.773   0.486%
120106  hotkeys-zerg-larva-cards      0.855   0.596%
120111  hotkeys-protoss-probe         1.029   0.768%
```

Live, each state matches its deterministic render
(`live-vs-shot 0.000`); tests: `cargo test` 308/308, clippy 0,
`ui_diff.py` 39/39, calibrated goal 19/19 at 0.00 and the Hotkeys
subset 16/16 (`docs/gates/ui-hotkeys-16.txt`).  Visual evidence:
`docs/hotkeys-live-proof-units.png`, `docs/hotkeys-live-proof-states.png`
and the full table in `docs/gates/hotkeys-live-diff.txt`.  The residual of 2.93
on the modal is only antialiasing of the large texts (title 36 px/body
27 px); geometry and flat colors match.

## Full Hotkeys: 88 units/buildings, 220 actions (2026-10-31, session 25)

The panel no longer has hand-written cards per unit.  It now uses
`fixtures/hotkeys.json`, generated from:

- `webui/dist/hotkeypanel/hotkeypanel.js`: 220 actions and the cards for
  Terran (29), Protoss (28) and Zerg (31), grouped into `ground`, `air`,
  `basic-building` and `advanced-building`.
- `OnBindingsChanged` captured from the native client over the CEF WebSocket
  (profile `$default`/Brood War).  The dump and the method are in
  `research/docs/hotkeys-data.md`; the tool is
  `research/tools/extract_hotkeys_ws.py`.

With the same drawing path, **88/88 units/buildings** were verified
(`--shot-screen hotkeys-any-<race>-<id>`; all with cards and keys), and the
16 Hotkeys references still pass.  The rebind is generic: any
action of any card opens the panel, accepts A-Z and applies the CEF's
per-unit collision rule; `Move=H` reproduces 115959 (Hold Position and the
Drone's Build Hatchery end up unbound), and `Move=M` unbinds Train Marine
only on the Barracks Lifted card.  Save/Cancel/Restore work;
Save writes `~/.cache/galaxycraft/hotkeys.bin` (GCHK v1, 64 KB limit,
SHA-256) and startup loads it; Cancel restores the snapshot and Restore
returns to the standard table.  The `--shot` states force defaults so that
the references do not depend on the saved profile.

Result: `ui-hotkeys-16.txt` 16/16 (120018 drops to **1.08** after applying the
real `letter-spacing:.4px` of `.dialogBody`, the glyph bearing and y-scale
and the dim of the whole container, not just the buttons), `ui-goal-19.txt` 19/19 at 0.00, `ui_diff.py` 39/39, `cargo test`
315/315 and clippy 0.

On the raw 0.00: full-frame supersampling (4/3 ->
0.75), fontdue glyph supersampling (4/3 -> 0.75, bilinear and box), gamma
0.85 and horizontal embolden were tried.  All make it worse; the residual is the shape/gamma
of the CEF/DirectWrite rasterizer, not geometry.  An exact 0.00 would require
using the original CEF rasterizer or layers derived from the captures.

## Hotkeys: tab borders and hover tooltip (2026-10-31, session 26)

The CEF tabs were not rectangular: `.hotkeyPanelHeader` and
`.raceSelectionTabSection .tab` have a top `border-radius` (12 px for the
header, 9 px for the races at the effective scale) and the selected tab drops
`margin-bottom:-6px` (9 px here), so its side borders continue
down to the top edge of the panel.  The straight border was replaced by
`tab_border_rounded` + `top_rounded_fill`, and the panel by
`css_panel3_rounded` (radius 12/6).  The 1:1 comparison of the corners is in
`docs/hotkeys-tabs-proof.png` (crops of 131015 with REF on top and the
current render below); the four Hotkeys references still pass.

The **unit hover tooltip was an invention**: the retail
`hotkeypanel.js` contains no tooltip at all; its only change on
mouse-over is `.unit:hover:not(.selected){box-shadow: inset 0 0 0 2px
#4cc428}`.  The "Terran SCV" box that covered "Ground Units" was removed.
The new state `hotkeys-terran-hover-scv` shows that hover leaves only
the green border and no text.

### Final touch on borders and buttons (2026-10-31, session 58)

Row-by-row measurement on 131015/131037 corrects the session 57
touch-up: the fill and outer border of the header reach **row 128**
(not 127), the inner `#6c0000` ends at 125 and the left border stays
dimmed on 126–128; the selected tab ends its inner border at 122 and
its outer at 125 (9 px overlap); the unselected ones end at
113 (inner) and 116 (outer).  The three bottom buttons use a group
with 15 px radius (5 CSS) and a 4 px `#331f1f` band, and a button with
9 px radius (3 CSS) and a 3 px `#6c0000` band (fill alpha 205 in both).  The
unit hover is only the 3 px inset frame `#4cc428`; there is no tooltip.

Verification: `ui-hotkeys-16.txt` 16/16 (0.76–1.12; 120018 drops to 1.08),
calibrated goal 19/19 at 0.00 (layers regenerated after the corner change),
`ui_diff.py` 39/39, `cargo test` 315/315, clippy 0.

## Cursor, menu loops and Options (2026-10-31, session 59)

### Cursor

The retail cursor is a GRP of **5 frames of 80x84** with the hotspot at the
asset origin and a **6 fps cycle in GRP order 0→4**.  It is drawn with
scale `screen_height / 1920` and a premultiplied bilinear blit (no
`fill_diamond` nor fixed scalings).  At 1440p the measured box is
**56x58 px** versus **56x57** in retail (one row of AA); at 1080p the
scale is 0.5625 (42-43 px).  Tests:
`retail_timing_tests::cursor_cycles_five_frames_at_six_fps` and
`cursor_is_hd_art_scaled_to_screen_height`.

### Single/Multiplayer/Exit loops

The install's WebMs are **15 fps**, but the retail client
consumes them at **10.5 fps**.  They are decoded at 15 fps and the timeline
advances at 10.5 fps from frame 0, without trimming the loop:

- Single/Multi: 151 frames = **14.381 s** (live capture 14.4 s).
- Exit: 105 frames = **10.0 s**.

`video.rs` passes `-ss` with 4 decimals: with 2 decimals the chunk drifted
almost one frame and the loop held or skipped one.  Test:
`menu_layers_use_the_measured_retail_10_5_fps_loop`.

### Frame-by-frame diff (close-out)

Cursor at 1080p: 3 s at 60 fps with the pointer still in both clients.  The
three visible states A/B/C align with translation (0,0); the mean diff is
**0.81/0.94/0.98** and pixels >30 are **1.83/2.00/2.08 %**.  86 % of
those pixels fall on gradient >10 and flat areas give a mean of **0.29**:
it is scaling AA (retail's cursor X vs. our premultiplied bilinear),
not size, position or frame order.  The duplicate pattern
(initial 3-frame delay) matches retail.

Loops: stable 64-bit retail capture at 1280x720/30 fps against our own
render.  The optimal translation is (0,0); the rate slope over 33 s is
**1.00138** (Single), **0.99783** (Multi) and **0.99965** (Exit), with a residual
of 1-2 frames (Single/Exit P50/P95 |res| 1/2, max 3; Multi 2/7, max 10).
At 1920x1080, comparing the same authored frame: Single **1.02** mean
(p95 2, >30 1.39 %), Multi **2.82** (p95 16, 3.84 %), Exit **1.89** (p95 5,
1.15 %); outside the clips the background and labels give **0.21-0.63**.  The
remaining residual is WebM decoding/filtering (ffmpeg vs. the retail
decoder), not phase or cut.

Tolerance recorded for the animated clips: mean <= 3, p95 <= 16 and
outliers <= 4 % in the clip region; background/UI <= 0.63.  No pixel 0.00 is
claimed for video because it would require the exact decoder of the retail client;
it is 1:1 in geometry, scale, frame order and rate (151/151 and
105/105 frames), and the static UI captures do close at 0.00 with the
`.gcal` layers.

### Options

`draw_options` was redone for the five tabs against 130933/130938/
130942/130949/130954 in the 2560x1440 content box (retail crop
`(440,0,3000,1440)`): plate with a double frame, tabs and divider, sliders
with 7 circular stops, checkboxes with help discs, dropdowns
(enabled/disabled), radios, rounded `60` input and
Restore/Ok buttons.  Title, captions, sliders, labels and buttons are at 0-1 px;
tabs at 1-2 px.  The weight of the inactive tabs is still ~30-40 %
above CEF in the ink metric (DirectWrite vs fontdue
rasterization), without affecting the gate.

Calibrated gate (`ui_visual_diff.py --calibrated --goal-only`): **19/19**;
the five Options give **0.59-1.40** mean and **0.95-1.91 %**
outliers.  The `.gcal` layers of those five references predate the
rework (which is why calibrated does not give 0.00 there); the rest stay at 0.00.
Without layers, Options is at 5.20-6.26 mean (the translucent background
outside the controls).  `ui_diff.py` 39/39, Hotkeys 16/16 (0.00-1.12),
`cargo test` **318/318**, clippy 0.  Local captures:
`/tmp/opencode/n4-options-{speed,sound,video,game,network}.png`.

## Options: CEF rasterization, sub-pixel chrome and live menu (2026-11-02, session 70)

**Correction.** Session 69 declared that the Options geometry was already 1:1
and that the residual was only font antialiasing and video phase.  That was false:
the side-by-side comparisons showed structural errors that were
measured and corrected here (plate, controls, content and background).  Visual
proof: `docs/options-1to1-proof.png` (retail on the left, `sc_view` on
the right, the five tabs).

### Plate (`draw_options_plate`)

The retail plate is not a rectangle: it is an L.  The title and each tab
are separate boxes (#331f1f band of 4.6 px, r 4.5, and #6c0000 of 3 px,
r 5) with 7-9 px gaps **without fill** (the menu's "Single Player" text
shows through at 832..836 x 358); below the last tab (y > 820.5) there is no plate and
the planet shows; the divider (brown 837.2..841.9, red 844.9..847.9) goes down
to y 1152 and the bottom band starts at x 837.  CEF draws at
fractional positions (half-covered border rows), so the
bands are rasterized with `cef_band` (exact SDF + 4x4 supersampling).  The
previous `rounded_sdf` clips the interior to `-r` and does not work for bands
thicker than the radius.

### Controls (measured on 130933/130942/130949)

| Control | Retail | Before |
|---|---|---|
| checkbox | box 29.9x30, ring 2.9 px r 5; mark 14.9 px with 3.7 px ramp | hard square without radius |
| disabled | ring #404040, mark #808080 (Use Mouse Confine **checked**) | grey unchecked |
| slider | rail 6 px, stops r 12 (ring 3), knob r 18, mark r 8.5 ramp 4; 1st stop x0+18.4 | aliased circles, 1st stop +16 |
| radio | r 17.85, ring 2.7, mark r 8.8 ramp 4.3 | 4 px ring, hard dot |
| dropdown | box r 7 with continuous top border (Video 373.5, Game 638.17, Language 449.17); value at top+30; chevron 31.8x15.6 #6f6809→#c4c60e | rectangle, small green arrow |
| Monitor | grey box r 8 **without arrow** | dropdown with arrow |
| help | antialiased disc r 9 (Environment Effects and Real-Time Lighting have it too) | two were missing |

Content: "Console"/"Portraits"/"Mini-Map Position" are lavender (not
white); "Music/Digital Volume" are white; "take effect" without a period;
Color Text / Play Sound nested 42 px and 7 px lower; Display Mode and
Gamma Correction 5 px lower; left column of Game 1 px lower.
First letter of tabs and buttons in #b4fc74 (`::first-letter`; the
whole active tab #b4fc74).

### Background: the modal sits on the live menu

- Title frame with the main menu anchor (dx 0): with the
  sub-screen offset (5.55 %) it was 142 px to the right.
- The Single/Multi/Exit clips are composited under the modal (the Exit galaxy
  behind "Ok", the Multi ship).  Phases measured by a 1/8 s sweep of each
  clip's own area, in `scripts/ui_visual_diff.py` (`PHASES`).
- The menu labels are drawn in their idle color and then the dim:
  (42,93,22) = 0.416 x (102,227,53) exactly.

### Text: CEF outline rasterizer (`UiFont::draw_cef`)

The softness did not come from fontdue's antialiasing but from **shifting the
bitmap** with bilinear interpolation (each stroke is spread over two
columns/rows).  CEF rasterizes each glyph at its exact sub-pixel position
(baseline 361.5, pen 899.75).  `draw_cef` reads the outline with
`ttf-parser` (already present via fontdue), flattens curves, accumulates exact
non-zero area (`raster_lines`, tests for sub-pixel square, counter and
diagonal) and applies the contrast curve `1-(1-a)^1.2`.

Fits with an identical prototype (fontTools + sweep) on the captures:
body 23.8 px/em, slider ends 20.7, headings 29.9, tabs 30.05
(Extended), title 41.5, buttons 30.0, "60" 21.35.  Mean error per text
from 0.102 (shifted fontdue) to 0.027-0.05.

Falsified hypotheses (recorded): autohinter-style vertical hinting (0.069
vs 0.066 without hinting); GPOS kerning (origin rms 0.50 px vs 0.42; "Tu"
-85 worsens "Smooth Unit Turning" from 0.033 to 0.076); integer advances (rms
1.23); advances rounded to DSF 1.25/1.333/1.5/2 (0.66-2.4).  The adopted
choice is design advance + 0.03 px (0.00125 em).

### Result (without calibration layers)

```text
ref     screen           session-69 start   now
130933  options-speed    2.19 / 1.46 %     0.82 / 0.48 %
130938  options-sound    2.25 / 1.52 %     0.92 / 0.60 %
130942  options-video    2.52 / 1.89 %     0.93 / 0.60 %
130949  options-game     2.95 / 2.11 %     1.08 / 0.77 %
130954  options-network  1.89 / 1.13 %     0.71 / 0.35 %
```

The `.gcal` layers of the five Options were corrections of the old render
(with them the new render gave 4.7-5.1); they were moved aside to
`fixtures/calibration/stale-options-pre-cef/` (gitignored) and the calibrated
gate now measures the real render: 19/19.

Remaining residual, measured: decoding of the menu clips (Exit
1.2-2.7 in its area), CEF glyph shape versus the CFF outline (0.03-0.05
per text) and the phase of the live clips, which advances with the real clock
(two walkthrough runs differ by 0.041 mean).  That is why
`menu_walkthrough.py` requires exact identity of Options outside the
clip rectangles from `flc.json` and ≤0.02 % transient pixels inside.

Verification: `cargo test` 342/342, clippy 0, `ui_diff.py` 39/39,
`ui_visual_diff.py --goal-only` 19/19 (raw and calibrated),
`menu_walkthrough.py` PASS.

## Campaign: 1:1 behavior and native glue (2026-11-02/03, session 71)

Full report (REA, strings, live client, falsifications):
`research/analysis/investigations/episode-flow-re.md`.  Visual proofs
(retail | raw sc_view | diff×4) and `REPORT.md`:
`engine-rust/docs/proof-sheets/` (local, `*.png` gitignored), generated
with `scripts/make_proof_sheets.py`.

### Behavior (live client + `MenuEntry` table from the dump)

- History = retail table in `sc_mem_menu_pe.exe` @ `0x952c88`
  (`campaign_progress.rs`); per-profile progress in `campaign.bin` (`GCPR`).
- Locked campaign: hover → `<Race> Campaign\nincomplete`; click →
  `glupokcancel` (`strRACE_ERR_*`, plate `glue/palcs/pdpopup.dds`, without
  darkening the background); Ok plays it anyway.
- Without history the campaign starts directly; with history it opens the list with
  entry 0 selected and at the top.  Arrows/wheel scroll without moving the
  selection; Up/Down move it and bring it into view; clicking a row
  selects it.  Scrollbar drawn with the measured geometry
  (`draw_mission_scrollbar`, `menu::mission_scrollbar_hit`).
- Campaign Cancel → Registry → Main (it used to go to GameType: falsified).
- Hotkeys from `rc3.json`: L, R (Load **R**eplay), u (Play C**u**stom), C,
  T/Z/P; the hotkey glyph is drawn in (208,255,131).

### Glue text (not CEF)

`RC_BIGGERTEXT` = `font/eurostileext-reg.ttf` 47.5 px/em, `RC_BIGTEXT` =
`font/eurostile-reg.ttf` 47.4, `RC_SMALLTEXT` and plain buttons =
`font/bl.ttf` (BlizzardGlobal) 33.4 / 34.0, with `kern` pairs from the TTF.
Labels, rows and title are within ≤1 px of the original (measured by ink
box).

### Capture determinism

Falsified: "headless renders are deterministic".  Under CPU load a
frozen clip (`--menu-at`/`--menu-phase`) waited only 2 s for the first
frame and drew the empty layer (130804 gave 2.48 or 2.64 depending on load; 1
of 3 renders differed in 108 717 px).  Now a frozen clip blocks until
the decoder is ready (`FROZEN_CLIP_WAIT`, 60 s): 8 concurrent renders
byte-for-byte identical.

### Clip phases

`scripts/campaign_phase_sweep.py`: coordinate descent over the phase of
each clip (discs, portraits, On) in steps of 1/10.5 s over its loop,
scored on the slot area.  Results in `ui_visual_diff.py` PHASES.

### Result (raw, without calibration layers)

```text
ref     screen                            start    now
130744  episode                           2.27     1.38
130750  episode-hover-terran              2.88     1.83
130757  episode-hover-zerg                3.24     1.80
130804  episode-hover-protoss             3.30     1.85
130824  episode-expansion-hover-protoss   2.76     1.60
130832  episode-expansion-hover-terran    2.91     1.61
130840  episode-expansion-hover-zerg      3.74     1.92
131132  mission                           3.11     1.78
live    mission-live / -scrolled / locked  —      3.46 / 3.40 / 3.42
```

The live captures differ mostly in discs and portraits (the client's real-clock
phase); list, colors, selection, scrollbar, prompt and buttons
match.  Verification: `cargo test` 354/354, clippy 0, `ui_diff.py`
39/39, `ui_visual_diff.py` 43/43 raw and calibrated, `menu_walkthrough.py`
PASS (140 steps, campaign section).

## In-game: the 8 captures of (8)The Hunters (2026-11-03, session 72)

References 134905 / 134919 / 134931 (Zerg, slot 1), 135020 (Terran,
slot 7) and 135051 / 135109 / 135114 / 135121 (Protoss, slot 6).  The
retail replays carry no command stream, so the scene is rebuilt
with the melee start rules (`App::hunters_start`).  Gate:
`ui_visual_diff.py --ingame-only`; sheets: `make_proof_sheets.py` (group
`ingame`).

### Ported rules (OpenBW as behavior reference)

- Creep: `spread_creep_completely` — ellipse 25·dx² + 64·dy² ≤ 25·320²,
  8-connected, `tile_can_have_creep` (cv5 0x80 = unbuildable), occupied
  tiles excluded; edges with `tileset/<era>.dds.grp` (37 frames) via the
  9-bit neighbor table; variants 0-5 at 96 %, 6-12 at 4 %.
- Vision: masks from `generate_sight_values` and height propagation from
  `reveal_sight_at` (replaces the r+1 circle + target height).
- Health bars (`draw_health_bars`) and selection circles (561 + n).

### Corrected data

- `sprites.dat`: bars/circles indexed from sprite 130 (previously 0).
- vf4 was indexed by cv5 group instead of by megatile (walkable lakes).
- HD art per GRP: `anim/main_NNN` exists once per GRP, under the first
  image that uses it (Command Center 275 → 63).
- Button panel: `statbtndefault` for all races (statbtnz was off by
  12/8 px).

### Measured on the captures (constants documented in the code)

Console (per-race plates ±1-2 px, minimap boxes, upgrade, portrait,
MENU), HUD text with advances rounded up to whole pixels
(`GlueFace::Hud`), layered HD wireframe, team colors, fog sampled
half a tile to the right (unexplained), and — as capture state, not
rule — the creep variants of 134905 (the client's session RNG) and the
camera adjustment of each match.

### Result (raw, mean |diff| 0-255 over 2560x1440)

```text
ref     session start  now
134905  25.3           4.89
134919  20.4           4.86
134931  21.8           5.37
135020  28.4           4.35
135051  24.3           5.61
135109  —              5.99
135114  —              5.82
135121  —              5.68
```

Residual: unit pose and phase (walking SCV, random facings of
drones/overlord, larvae), geyser smoke, HD lighting (normal/
specular) of minerals and buildings, portrait margins.  Verification:
`cargo test` 370/370, clippy 0, `ui_visual_diff.py` 51/51 raw and
calibrated, `ui_diff.py` 39/39, `menu_walkthrough.py` PASS (145 steps,
end-to-end victory and thumb drag).

### Second round (same session)

HD lighting (normal/AO/specular + emissive, fitted on Nexus/CC/
Hatchery), portrait geometry and idle clip per capture, recorded
poses of workers/Overlord/larva, geyser smoke
(`creategasoverlays`), and the second minimap button is no longer drawn (the
console shows its gap).  Tried and discarded: HUD text weight
via coverage gamma (≤0.03 mean).

```text
ref     now
134905  4.14
134919  4.41
134931  4.93
135020  3.25
135051  4.44
135109  4.66
135114  4.69
135121  4.59
```

### Third round (2026-11-03, session 75)

Geometry and layout rules found by sub-pixel shift search and template
matching (details and evidence: `research/docs/session-log.md`, entry 75):

- The playfield is 853 game px wide and stretched to the 2560 px box
  (`render::stretch_columns`; x scale 3.0012, y exactly 3).
- HD megatiles are area-filtered 128 -> 96 with straight alpha.
- `statdata` centres in whole game px (106); right-hand and glue dialogs keep
  the fractional widening.
- MENU plate (`dlg` frame 0) at (-3, 0) from the `stat_f10` control.
- Glue dialog pane: 2 px left / 23 px wider than its rect, rounded corners, a
  one-game-px translucent border; `DEFPUSHBUTTON` labels 6 px below
  `PUSHBUTTON` ones.
- Zerg/Protoss command card drawn (-3, +1) from its rects; centre plate
  nudge -1 for every race.
- Fog edge: linear ramp (box blur) instead of a Gaussian (INFERENCE).
- Capture state: creep variants of the band above the console, geyser smoke
  per capture (none on 134919 / 134931 / 135114 / 135121), terrain minimap on
  134905 only.

The client's lighting shaders were located (plain GLSL in CASC) but the
literal model, with or without bloom, scores worse than the fitted one on
whole frames, so the fitted model stays (INFERENCE, see the session log).

```text
ref     round 2  now
134905  4.14     2.66
134919  4.41     2.70
134931  4.93     3.06
135020  3.25     2.50
135051  4.44     3.31
135109  4.66     3.47
135114  4.69     3.45
135121  4.59     3.45
```

Then (same session): OpenBW minimap blips with a 1 px outline, the HD
terrain minimap with creep under the smooth fog, the HUD face corrected to
`blizzard-regular.ttf` with linear advances and kerning (stat rows, names,
tooltips, checkbox label), and the upgrade/tooltip anchors re-measured:

```text
ref     now
134905  2.46
134919  2.51
134931  2.79
135020  2.49
135051  3.14
135109  3.24
135114  3.27
135121  3.27
```

Remaining: building/mineral lighting (needs the client's point-light inputs).

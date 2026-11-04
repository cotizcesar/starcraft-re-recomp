# Remastered sprite lighting: point lights (`anim/main.lit`)

Date: 2026-11-04 (session log entry 80).  Implementation:
`engine-rust/src/hd_lights.rs`, `hd_sprites::draw_frame_lit`,
`sc_view` (`hd_unit_parts`, `hd_light_env`).

## Sources

- Shaders, plain-text GLSL in CASC (`shadersglsl/deferred_blit.glsl`,
  `util_lighting.glsl`, `util_scale.glsl`, `deferred_sprite.glsl`; read into
  `/tmp`, not committed).
- Live client, 64-bit, (8)The Hunters, the user playing Protoss from the
  bottom-left start (player 6), camera untouched after the start: screenshot
  plus a read-only capture of the 485 RW regions (905 MB,
  `research/tools/heap_capture.py`, local `research/work/heap_lights_20261104/`).
- `anim/main.lit` from CASC (462 180 B, SHA-256 `d19ce599…2324`).

## Findings

| # | Finding | Evidence | Confidence |
|---|---|---|---|
| 1 | The deferred blit adds, per sprite, the lights listed for it in a `samplerBuffer`: two `vec4` per light, `(x, y, z, radius)` and `(r, g, b, intensity)`; colour × 2, intensity × 0.6, `z = 125 S`, falloff `clamp(R − d, 0, R)/R · 0.9`, Lambert + `pow(spec, 16)` × AO. Sprites only (terrain is not in the sprite buffer). | `deferred_blit.glsl` lines 55–86. | FACT |
| 2 | The GPU staging buffer holds those records with positions in 4x px (multiples of 4) and radius in 4x px. | Region `0x7ffff43a0000` +0x10: 131 035 records; unique values: 10 mineral lights (radius 176, intensity 0.15), one cyan (radius 240, 0.28) and flickering green ones (radius 160, 0.12–0.24). | FACT |
| 3 | A CPU list of the frame's lights: 36-byte records `{x, y, z, r, g, b, radius, intensity, 0}`. | Region `0xa572000` +0x59f2ed0: 10 minerals + the Nexus glow. | FACT |
| 4 | Positions map to the units by `4 × map position − camera`: the 10 mineral lights fit player 6's 10 mineral fields at scale exactly 4.0, camera (0, 3488) game px; residuals depend on the mineral type only. | Least-squares match over the CHK `UNIT` section (cost 44 px for 10 points at scale 4). | FACT |
| 5 | `anim/main.lit` is the light table: `u32 999, u32 0, u32 0`, 999 offsets by `images.dat` id, entries `u32 frames` + 20-byte `{u32 x, u32 y, u8 r, g, b, a, u32 radius, u32 intensity}` per frame. | The table starts at byte 12 (first entry at `0xfa8 = 12 + 999 × 4`); with that index the frame counts match the GRP frame counts of all 130 images checked (shifted indices: ≤ 51/130); 866 of 867 entries end where the next begins. | FACT |
| 6 | Values: radius in game px (× 4 = HD px), intensity in percent, colour in bytes. | `min01/02/03` (images 347/349/351) frame 0 = (12,43,58)/(13,45,59)/(12,44,60), radius 44, intensity 15 = the three mineral colours, 176 and 0.15 measured live; `pneGlow` (181) = (0,80,128), 60, 28 = the live cyan 240 / 0.28; `GeySmok1..5` (430..434) green, radius 40 = the live 160. | FACT |
| 7 | Placement: `sprite centre + frame.(x, y) + record.(x, y) − box / 2` (4x px, `box` = the `.anim` frame box). | The camera recovered from each of the 11 live lights agrees within ±2 / ±2.25 px. | FACT; the client snaps to whole game px (≤ 1 game px, 9/11 exact with the best snapping rule tried) — the snapping rule itself is INFERENCE. |
| 8 | 196 images carry lights; 4 more (380–382, 998) store positions only. | Count over the table. | FACT |
| 9 | The Nexus glow (and its light) is attached by the `Built` animation (iscript entry 16, `imgol 181`), not `Init`; the client drew only `Init` overlays. | Overlays per Nexus animation: Init → 182 (shadow), Built → 181. | FACT |
| 10 | The sprite's lighting mix (`data.z`, per-sprite "lighting") and the lighting scale `S` are uniforms; this client uses 1 and screen px per game px. | Not in the captured CPU data. | INFERENCE (falsifier: a live capture of the uniform values, e.g. via a GL trace) |

## Result

The client now places every lit image's light (main image, `Init` and
`Built` overlays, geyser smoke) and adds the shader term on top of the fitted
directional model.  `ui_visual_diff.py` (raw): 135020 2.49 → 2.47, 135051
3.14 → 3.12, 135109 3.24 → 3.21, 135114 3.27 → 3.24, 135121 3.27 → 3.25;
the other 46 screens unchanged (`engine-rust/docs/gates/ui-visual-diff*.txt`).
The gain is small because the references are daylight captures where the
directional term dominates; the lights matter most on minerals and glows.

Also confirmed in the same session: a finished custom game does not change
`Characters\*.spc`/`.mpc` (no statistics stored; `profile-format.md` #10).

# Graphics — actual status (2026-10-28)

Client: `engine-rust/src/bin/sc_view.rs` + `src/render.rs`, `src/dds.rs`,
`src/ui_art.rs`, `src/video.rs`. All art comes from the user's
install (`--install`); the repo contains no retail art.

## 1. Resolution support

| Requirement | Status | Proof |
|---|---|---|
| 16:9 up to 4K | **done** | renders 1280x720 / 1920x1080 / 2560x1440 / 3840x2160, exit 0 |
| Ultrawide expands (no bars) | **done** | render 3440x1440, exit 0 |

Model: **the scale is derived from the height** (`scale = height / 480`), the 4:3
HUD is centered and the **world fills the full width**. So 16:9 and 21:9 show
more world at the same sprite proportion, instead of letterboxing.

The world is drawn into a logical framebuffer (640x360 at 1x) and scaled with
`blit_scaled` to the window size; the HUD is drawn afterwards in screen
space. The mouse is converted with `to_logical` before resolving the world.

## 2. Fixed bugs

### 2.1 Black holes in the terrain (fixed)

`draw_tile` painted only the MTXM megatile. CV5 additionally stores a
**background megatile per group** (`+10`), and the per-tile megatiles are largely
transparent (cliffs, edges, space). Without that layer black showed
through.

- Before: 2 of 698 distinct tiles of `Boxer.chk` painted nothing and dozens
  covered only 30–90%.
- After: **0 empty, 0 partial**.
- Regression covered by `tests/terrain_layers.rs`
  (`every_shipped_map_tile_is_fully_opaque`).

### 2.2 Tiny sprites relative to the HUD (fixed)

The world was drawn 1:1 while the HUD was scaled: at 720p sprites
came out at half relative size. Now both share `scale`.

### 2.3 Misaligned clicks when scaling (fixed)

`to_logical` was not wired: with scale ≠ 1 the wrong unit was selected.
Now both mouse→world conversion points go through it.

## 3. Remastered main menu

Composition (final image: `menu Remastered`, 1920x1080):

| Layer | Source | Treatment |
|---|---|---|
| Space background | gradient + deterministic star field | the original is a **3D scene** of the engine, not a texture |
| Fleet + distant cruiser | `glue/mainmenu/single.webm` | frame at 1.5 s, **additive blending** |
| Near cruiser | `glue/mainmenu/multi.webm` | same |
| Exit portal | `glue/mainmenu/exit.webm` | same |
| Metal frame | `glue/mainmenu/titleframe_overlay.dds` | DDS with alpha, top right |
| Entries | green text (Single/Multi/Options/Collections/Hotkeys/Credits/Intro/Exit) | positions from the reference layout |
| Profile | local profile + `#1532` | top right |
| Version | `1.23.10.13515 \| enUS \| 64-bit` | bottom right |

Key finding: those WebMs **are not text buttons**, they are the animated
layers of the menu (over black, `yuv420p` without alpha) and the original engine
composites them additively. That is what was implemented (`blit_additive`).

## 4. Animation and full-width HUD

- The three menu layers are decoded **in sequence** (6 frames at 6 fps,
  a single ffmpeg process per layer, `decode_frames`) and cycled with
  `MenuLayers::pick`. Frames with a full-screen explosion flash
  are discarded (`drop_flash`, mean luminance > 96) because in
  additive mode they washed out the menu.
  Proof: `tests/menu_layers.rs::menu_layer_yields_distinct_frames`.
- The HUD already anchored the console plates to the edges (`draw_plates`:
  left at x=0, right at `w - width`, center centered), so at
  21:9 it fills the full width; what is centered is the interactive content.
- Fixed a duplicate: the `gamemenu` shot drew the procedural panel
  on top of the retail art.

## 5. Known limits (honest)

- No parallax: the layers animate but do not shift with the mouse as
  in the real client.
- Menu frames are sampled at 6 fps (not 30) to avoid decoding
  hundreds of images at startup.
- **The planet is missing** in the bottom-left corner: it does not appear as a
  texture in the CASC we have mapped (it is part of the 3D scene).
- The frame panel comes out empty: in retail it shows the rank/portrait.
- The HUD is 4:3 art (640x480) centered; it is not stretched to 21:9 as in SC:R,
  which recomposes the consoles to fill the width.
- No bilinear filtering: nearest scaling, same as the original pixel art.

## 6. How to see it

```bash
# Menu (1920x1080)
sc_view --install "$SC_INSTALL" --width 1920 --height 1080 \
        --shot /tmp/menu.bmp --shot-screen menu
# Game (4K)
sc_view --install "$SC_INSTALL" --width 3840 --height 2160 \
        --shot /tmp/game.bmp --shot-screen objectives
# Interactive
sc_view --install "$SC_INSTALL"
```

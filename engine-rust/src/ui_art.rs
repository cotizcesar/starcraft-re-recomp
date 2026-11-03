//! Retail UI art bindings: console plates, command buttons, wireframes,
//! main-menu glue and score panels.
//!
//! The paths are the CASC names materialised by
//! [`crate::retail::RetailInstall::materialize`]; no art is compiled into the
//! crate.  When the client runs without `--install` the loaders return `None`
//! and the view falls back to its text HUD.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::dds::{self, DdsImage};
use crate::render::{Grp, Palette};
use crate::sim::Race;
use crate::ui::UiDialog;

pub fn race_dir(race: Race) -> &'static str {
    match race {
        Race::Zerg => "zerg",
        Race::Protoss => "protoss",
        _ => "terran",
    }
}

fn load_dds(root: &Path, rel: &str) -> Option<DdsImage> {
    let bytes = std::fs::read(root.join(rel)).ok()?;
    dds::parse(&bytes)
}

fn load_grp(root: &Path, rel: &str) -> Option<Grp> {
    let bytes = std::fs::read(root.join(rel)).ok()?;
    Grp::parse(&bytes)
}

fn load_dds_grp(root: &Path, rel: &str) -> Option<dds::DdsGrp> {
    let bytes = std::fs::read(root.join(rel)).ok()?;
    dds::DdsGrp::parse(&bytes)
}

fn load_ui(root: &Path, name: &str) -> Option<UiDialog> {
    UiDialog::from_path(&root.join("ui").join(format!("{name}.ui.json")))
}

/// The three-part bottom console plus its overlays for one race.
pub struct ConsoleArt {
    pub race: &'static str,
    pub left: DdsImage,
    pub center: DdsImage,
    pub right: DdsImage,
    pub con_over: Option<DdsImage>,
    pub clock: Option<DdsImage>,
    pub apm: Option<DdsImage>,
    pub command_buttons: Option<Grp>,
    pub command_buttons_hd: Option<dds::DdsGrp>,
    /// Remastered command glyphs (`hd2/unit/cmdicons/cmdicons.dds.grp`, 390
    /// frames, frame = button id): the retail classic console tints these
    /// yellow, which the 27x28 SD sprites cannot reproduce at 1440p.
    pub command_icons_hd: Option<dds::DdsGrp>,
    pub command_icons: Option<Grp>,
    pub wireframe: Option<Grp>,
    /// Source sub-rectangle of the right plate that carries the 3x3 command
    /// wells (measured on the art: Terran 1283..1730 x 185..530 of 2286x700,
    /// scaled per race from its own proportions).
    pub well_src: (f32, f32, f32, f32),
    /// Source sub-rectangle of the right plate that carries its button wells
    /// (fractions of the art), calibrated per race because the three arts lay
    /// their wells out differently.
    pub right_src: (f32, f32, f32, f32),
    pub palette: Palette,
    pub layouts: HashMap<String, UiDialog>,
}

impl ConsoleArt {
    pub fn load(root: &Path, race: Race) -> Option<Self> {
        let dir = format!("game/consoles/{}", race_dir(race));
        let left = load_dds(root, &format!("{dir}/console_left.dds"))?;
        let center = load_dds(root, &format!("{dir}/console_center.dds"))?;
        let right = load_dds(root, &format!("{dir}/console_right.dds"))?;
        // Crop of `console_right.dds` that carries the command card.  Swept
        // per race with `cv2`-matched renders scored on the card region
        // (x 2250..3000, y 1040..1440 of the 3440x1440 reference).  Raw NCC
        // before -> after: Terran 0.207 -> 0.285, Zerg 0.168 -> 0.174,
        // Protoss 0.134 -> 0.176.
        let right_src = match race {
            Race::Zerg => (0.36, 0.057, 0.95, 0.943),
            Race::Protoss => (0.30, 0.057, 0.72, 0.943),
            _ => (0.30, 0.057, 0.72, 0.943),
        };
        let well_src = match race {
            // Fractions of console_right.dds measured on the art itself.
            Race::Zerg => (0.5496, 0.2604, 0.7591, 0.7589),
            Race::Protoss => (0.5571, 0.2639, 0.7624, 0.7542),
            _ => (0.5612, 0.2643, 0.7568, 0.7571),
        };
        let palette = std::fs::read(root.join("unit.pal"))
            .ok()
            .map(|b| Palette::from_pal(&b))
            .unwrap_or_else(|| Palette {
                colors: [crate::render::Rgb::new(160, 160, 160); 256],
            });
        let mut layouts = HashMap::new();
        for name in crate::ui::UI_JSON {
            if let Some(dialog) = load_ui(root, name) {
                layouts.insert((*name).to_string(), dialog);
            }
        }
        Some(Self {
            race: race_dir(race),
            left,
            center,
            right,
            well_src,
            right_src,
            con_over: load_dds(root, &format!("{dir}/conover.dds")),
            clock: load_dds(root, &format!("{dir}/console_time.dds")),
            apm: load_dds(root, &format!("{dir}/console_apm.dds")),
            command_buttons: load_grp(root, &format!("{dir}/cmdbtns.grp")),
            command_buttons_hd: load_dds_grp(root, &format!("{dir}/cmdbtns.dds.grp")),
            command_icons_hd: load_dds_grp(root, "hd2/unit/cmdicons/cmdicons.dds.grp"),
            command_icons: load_grp(root, "unit/cmdicons/cmdicons.grp"),
            wireframe: load_grp(root, "unit/wirefram/wirefram.grp"),
            palette,
            layouts,
        })
    }

    pub fn layout(&self, name: &str) -> Option<&UiDialog> {
        self.layouts.get(name)
    }

    /// Draw the plate triptych across the bottom `hud_h` pixels.
    pub fn draw_plates(&self, fb: &mut crate::render::Framebuffer, hud_top: i32, hud_h: i32) {
        self.draw_plates_in(fb, hud_top, hud_h, 0, fb.width);
    }

    /// Draw the 3x3 command-well block of the right plate into `dest`, scaled
    /// to fit and multiplied by `gain` (retail draws the console darker than
    /// the raw DDS; measured best gain 0.79 on the reference).
    pub fn draw_wells(
        &self,
        fb: &mut crate::render::Framebuffer,
        dest: (i32, i32, i32, i32),
        gain: f32,
    ) {
        let (dw, dh) = (dest.2.max(1), dest.3.max(1));
        let (sw, sh) = (self.right.width as f32, self.right.height as f32);
        if sw <= 0.0 || sh <= 0.0 {
            return;
        }
        let (u0, v0, u1, v1) = self.well_src;
        let (sx, sy) = ((sw * u0) as i32, (sh * v0) as i32);
        let (sww, shh) = (
            ((sw * (u1 - u0)) as i32).max(1),
            ((sh * (v1 - v0)) as i32).max(1),
        );
        for y in 0..dh {
            let v = sy + (y * shh / dh);
            for x in 0..dw {
                let u = sx + (x * sww / dw);
                let p = self.right.pixel(u.max(0) as usize, v.max(0) as usize);
                let r = (((p >> 16) & 0xff) as f32 * gain).round() as u32;
                let g = (((p >> 8) & 0xff) as f32 * gain).round() as u32;
                let b = ((p & 0xff) as f32 * gain).round() as u32;
                let c =
                    crate::render::Rgb::new(r.min(255) as u8, g.min(255) as u8, b.min(255) as u8);
                fb.put(dest.0 + x, dest.1 + y, c);
            }
        }
    }

    /// True when the right plate is drawn cropped to its well region, so the
    /// wells come from the art and must not be repainted.
    pub fn right_is_cropped(&self) -> bool {
        self.right_src != (0.0, 0.0, 1.0, 1.0)
    }

    /// Scaled size of the right console plate for a given console height.
    pub fn right_plate_width(&self, hud_h: i32) -> i32 {
        if self.right.height == 0 {
            return hud_h;
        }
        ((self.right.width as f32 * hud_h as f32) / self.right.height as f32).round() as i32
    }

    /// Same, but confined to the 16:9 content box `[x0, x0 + w)` so the
    /// console never bleeds into the pillarbox area.
    pub fn draw_plates_in(
        &self,
        fb: &mut crate::render::Framebuffer,
        hud_top: i32,
        hud_h: i32,
        x0: i32,
        w: i32,
    ) {
        // The three plates are drawn as one triptych scaled to the content
        // width, with a per-race vertical calibration.
        //
        // A landmark layout (radar interior 458,1056 384x384, panel interior
        // 1361,1146 582x292, right plate filling the remainder) was implemented
        // and measured: band raw 0.3435 / 0.1160 / 0.1352 and card raw 0.298 /
        // 0.088 / 0.033 for Terran / Zerg / Protoss, i.e. worse than this
        // triptych (0.3649 / 0.1299 / 0.1511 and 0.295 / 0.174 / 0.138), so it
        // was reverted.  The triptych keeps the plates' native aspect, which is
        // what the NcC on the band rewards.
        let Some((sizes, height)) = plate_layout(
            [
                (self.left.width, self.left.height),
                (self.center.width, self.center.height),
                (self.right.width, self.right.height),
            ],
            w.max(1) as usize,
        ) else {
            return;
        };
        // Per-race calibration measured by sweeping stretch/voff against each
        // race's own reference band and keeping the best raw NCC: Terran
        // 1.20/0 (0.365), Zerg 1.00/+20 (0.130), Protoss 1.40/-20 (0.151).
        let (default_stretch, default_voff) = match self.race {
            "zerg" => (1.00f32, 20i32),
            "protoss" => (1.40, -20),
            _ => (1.20, 0),
        };
        let stretch: f32 = std::env::var("GC_HUD_STRETCH")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default_stretch);
        let voff: i32 = std::env::var("GC_HUD_VOFF")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default_voff);
        let height = ((height as f32 * stretch).round() as i32).max(1);
        let top = hud_top + hud_h - height + voff;
        let mut x = x0;
        for (i, (img, (dw, dh))) in [&self.left, &self.center, &self.right]
            .into_iter()
            .zip(sizes.iter().copied())
            .enumerate()
        {
            let mut dh = ((dh as f32 * stretch).round() as i32).max(1);
            let mut px = x;
            let mut py = top;
            if i == 1 {
                // Centre plate (info panel): per-race scale/offset measured on
                // the panel region (x 1000..2100, y 1100..1440).  Panel raw NC:
                // Terran 0.326, Protoss 0.170.  Zerg's organic frame does not
                // correlate (best 0.002) and moving it covers the command card,
                // so it keeps the identity transform.
                let (cs, cdx, cdy) = match self.race {
                    "protoss" => (1.06f32, 0.010f32, 0.0f32),
                    "zerg" => (1.0, 0.0, 0.0),
                    _ => (1.06, -0.010, 0.0),
                };
                let (cs, cdx, cdy) = std::env::var("GC_HUD_CENTRE")
                    .ok()
                    .and_then(|raw| {
                        let v: Vec<f32> = raw
                            .split(',')
                            .filter_map(|t| t.trim().parse().ok())
                            .collect();
                        (v.len() == 3).then(|| (v[0], v[1], v[2]))
                    })
                    .unwrap_or((cs, cdx, cdy));
                dh = ((dh as f32 * cs).round() as i32).max(1);
                px += (w as f32 * cdx).round() as i32;
                py += (hud_h as f32 * cdy).round() as i32;
            }
            if i == 2 {
                // The right plate is drawn cropped to the region that carries
                // the command card and the MENU/portrait block (swept per race
                // on the card region: Terran 0.207 -> 0.295, Zerg 0.168 ->
                // 0.174, Protoss 0.134 -> 0.138).
                let (sw, sh) = (img.width as f32, img.height as f32);
                let (u0, v0, u1, v1) = std::env::var("GC_HUD_RIGHT_SRC")
                    .ok()
                    .and_then(|raw| {
                        let v: Vec<f32> = raw
                            .split(',')
                            .filter_map(|t| t.trim().parse().ok())
                            .collect();
                        (v.len() == 4).then(|| (v[0], v[1], v[2], v[3]))
                    })
                    .unwrap_or(self.right_src);
                let src = (
                    (sw * u0) as i32,
                    (sh * v0) as i32,
                    (sw * (u1 - u0)) as i32,
                    (sh * (v1 - v0)) as i32,
                );
                img.draw_sub(fb, src, (px, py, dw, dh));
            } else {
                img.draw_scaled(fb, px, py, dw, dh);
            }
            x += dw;
        }
    }

    /// Interior rectangle of the centre plate (the info panel), in window
    /// pixels.  Mirrors `draw_plates_in` exactly so text and slots can never
    /// drift from the frame they belong to.
    pub fn center_interior(
        &self,
        hud_top: i32,
        hud_h: i32,
        x0: i32,
        w: i32,
    ) -> (i32, i32, i32, i32) {
        let Some((sizes, height)) = plate_layout(
            [
                (self.left.width, self.left.height),
                (self.center.width, self.center.height),
                (self.right.width, self.right.height),
            ],
            w.max(1) as usize,
        ) else {
            return (x0, hud_top, w.max(1), hud_h.max(1));
        };
        let (default_stretch, default_voff) = match self.race {
            "zerg" => (1.00f32, 20i32),
            "protoss" => (1.40, -20),
            _ => (1.20, 0),
        };
        let stretch: f32 = std::env::var("GC_HUD_STRETCH")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default_stretch);
        let voff: i32 = std::env::var("GC_HUD_VOFF")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default_voff);
        let h = ((height as f32 * stretch).round() as i32).max(1);
        let top = hud_top + hud_h - h + voff;
        let cx = x0 + sizes[0].0;
        let (dw, dh) = sizes[1];
        let dh = ((dh as f32 * stretch).round() as i32).max(1);
        // Interior of console_center.dds: x 52..1086, y 87..509 of 1156x512.
        let (nw, nh) = (self.center.width as f32, self.center.height as f32);
        let ix = cx + (52.0 / nw * dw as f32).round() as i32;
        let iy = top + (87.0 / nh * dh as f32).round() as i32;
        let iw = ((1086.0 - 52.0) / nw * dw as f32).round() as i32;
        let ih = ((509.0 - 87.0) / nh * dh as f32).round() as i32;
        (ix, iy, iw, ih)
    }

    /// 64x64 wireframe for a unit type (frame index is the unit id).
    pub fn draw_wireframe(
        &self,
        fb: &mut crate::render::Framebuffer,
        unit_id: u16,
        dest: (i32, i32, i32, i32),
        team: Option<crate::render::Rgb>,
    ) -> bool {
        let Some(grp) = &self.wireframe else {
            return false;
        };
        if grp.frames.is_empty() {
            return false;
        }
        let Some(frame) = grp.frame((unit_id as usize).min(grp.frames.len() - 1)) else {
            return false;
        };
        let (dx, dy, dw, dh) = dest;
        if dw <= 0 || dh <= 0 || frame.w <= 0 || frame.h <= 0 {
            return false;
        }
        let green = |idx: u8| -> crate::render::Rgb {
            // `wirefram.grp` keeps its shading in the 192..=211 page of
            // unit.pal, which that palette fills with flat blue placeholders
            // (measured: indices 208..=211 are all #2323FF).  The retail
            // blueprint is green, so that page is mapped onto a green ramp.
            if (192..=239).contains(&idx) {
                // The page runs 192..=239 across unit ids (a Marine wireframe
                // uses 216..=219, a Supply Depot 208..=211), so the whole
                // range is mapped, not just the first block.
                let k = ((idx as u32 - 191) as f32 / 28.0).clamp(0.0, 1.0); // dark -> bright
                crate::render::Rgb::new(
                    (30.0 * k) as u8 + 6,
                    (200.0 * k) as u8 + 30,
                    (40.0 * k) as u8 + 6,
                )
            } else if (3..=15).contains(&idx) {
                let k = (16 - idx as u32) as f32 / 9.0;
                crate::render::Rgb::new(
                    (40.0 * k) as u8 + 8,
                    (215.0 * k) as u8 + 20,
                    (60.0 * k) as u8 + 8,
                )
            } else {
                self.palette.color(idx, team)
            }
        };
        for y in 0..dh {
            let sy = (y as i64 * frame.h as i64 / dh as i64) as i32;
            for x in 0..dw {
                let sx = (x as i64 * frame.w as i64 / dw as i64) as i32;
                let c = frame.indices[(sy * frame.w + sx) as usize];
                if c != 0 {
                    fb.put(dx + x, dy + y, green(c));
                }
            }
        }
        true
    }

    /// Grayscale wireframe for the Remastered Hotkeys browser (the retail
    /// panel shows white unit silhouettes, not the green in-game blueprint).
    pub fn draw_wireframe_gray(
        &self,
        fb: &mut crate::render::Framebuffer,
        unit_id: u16,
        dest: (i32, i32, i32, i32),
    ) -> bool {
        let Some(grp) = &self.wireframe else {
            return false;
        };
        let Some(frame) = grp.frame((unit_id as usize).min(grp.frames.len() - 1)) else {
            return false;
        };
        let (dx, dy, dw, dh) = dest;
        if dw <= 0 || dh <= 0 || frame.w <= 0 || frame.h <= 0 {
            return false;
        }
        for y in 0..dh {
            let sy = (y as i64 * frame.h as i64 / dh as i64) as i32;
            for x in 0..dw {
                let sx = (x as i64 * frame.w as i64 / dw as i64) as i32;
                let c = frame.indices[(sy * frame.w + sx) as usize];
                if c != 0 {
                    let lum = self.palette.color(c, None);
                    let r = (lum.0 >> 16) & 0xff;
                    let g = (lum.0 >> 8) & 0xff;
                    let b = lum.0 & 0xff;
                    let v = ((r + g + b) / 3).clamp(40, 235) as u8;
                    fb.put(dx + x, dy + y, crate::render::Rgb::new(v, v, v));
                }
            }
        }
        true
    }

    /// Draw a unit's command-card icon (`cmdicons.grp`, frame = unit id).
    /// Command icon in retail ink: every lit pixel becomes the icon yellow and
    /// the dark interior detail stays dark, so the glyph keeps its cut-outs
    /// instead of being a solid silhouette.
    pub fn draw_command_icon_ink(
        &self,
        fb: &mut crate::render::Framebuffer,
        frame: usize,
        dest: (i32, i32, i32, i32),
    ) -> bool {
        let Some(grp) = &self.command_icons else {
            return false;
        };
        let Some(f) = grp.frame(frame) else {
            return false;
        };
        if f.w <= 0 || f.h <= 0 || dest.2 <= 0 || dest.3 <= 0 {
            return false;
        }
        let yellow = crate::render::Rgb::new(252, 226, 40);
        let dark = crate::render::Rgb::new(12, 12, 14);
        for y in 0..dest.3 {
            let sy = (y as i64 * f.h as i64 / dest.3 as i64) as i32;
            for x in 0..dest.2 {
                let sx = (x as i64 * f.w as i64 / dest.2 as i64) as i32;
                let idx = f.indices[(sy * f.w + sx) as usize];
                if idx == 0 {
                    continue;
                }
                let c = self.palette.color(idx, None);
                let lum = ((c.0 >> 16) & 0xff) + ((c.0 >> 8) & 0xff) + (c.0 & 0xff);
                fb.put(
                    dest.0 + x,
                    dest.1 + y,
                    if lum < 170 { dark } else { yellow },
                );
            }
        }
        true
    }

    /// Command icon as a flat yellow glyph (retail look).
    pub fn draw_command_glyph(
        &self,
        fb: &mut crate::render::Framebuffer,
        unit_id: u16,
        dest: (i32, i32, i32, i32),
        color: crate::render::Rgb,
    ) -> bool {
        let Some(grp) = &self.command_icons else {
            return false;
        };
        if grp.frames.is_empty() {
            return false;
        }
        crate::render::draw_grp_silhouette(fb, grp, unit_id as usize, dest, color);
        true
    }

    pub fn draw_unit_icon(
        &self,
        fb: &mut crate::render::Framebuffer,
        unit_id: u16,
        dest: (i32, i32, i32, i32),
        team: Option<crate::render::Rgb>,
    ) -> bool {
        let Some(grp) = &self.command_icons else {
            return false;
        };
        if grp.frames.is_empty() {
            return false;
        }
        // Coloured hero frames share the stock id, exactly like `wirefram`.
        crate::render::draw_grp_frame(
            fb,
            grp,
            (unit_id as usize).min(grp.frames.len() - 1),
            &self.palette,
            dest,
            team,
        );
        true
    }

    /// Draw a command glyph from the Remastered atlas, tinted `color`.
    ///
    /// The retail classic console paints its command card yellow on a near
    /// black inset (measured on the 3440x1440 reference: glyph #FCFC38,
    /// inset #0A0A0C).  The HD atlas carries the shape at 144px with grey
    /// shading, so the tint keeps that shading instead of flattening the
    /// glyph into a silhouette.
    pub fn draw_command_icon_tinted(
        &self,
        fb: &mut crate::render::Framebuffer,
        frame: usize,
        dest: (i32, i32, i32, i32),
        color: crate::render::Rgb,
    ) -> bool {
        let Some(grp) = &self.command_icons_hd else {
            return false;
        };
        let Some(img) = grp.frame(frame) else {
            return false;
        };
        if dest.2 <= 0 || dest.3 <= 0 || img.width == 0 || img.height == 0 {
            return false;
        }
        let base = color.0;
        for y in 0..dest.3 {
            let sy = (y as i64 * img.height as i64 / dest.3 as i64) as usize;
            for x in 0..dest.2 {
                let sx = (x as i64 * img.width as i64 / dest.2 as i64) as usize;
                let p = img.pixels[sy.min(img.height - 1) * img.width + sx.min(img.width - 1)];
                let a = (p >> 24) & 0xff;
                if a < 16 {
                    continue;
                }
                let lum = (((p >> 16) & 0xff) + ((p >> 8) & 0xff) + (p & 0xff)) / 3;
                // 78%..100% of the tint: retail glyphs are solid yellow with
                // engraved shading, not a dark silhouette.
                let k = 200 + (55 * lum) / 255;
                let scale = |c: u32| ((c * k) / 255).min(255) as u8;
                let c = crate::render::Rgb::new(
                    scale((base >> 16) & 0xff),
                    scale((base >> 8) & 0xff),
                    scale(base & 0xff),
                );
                let dst = fb.get(dest.0 + x, dest.1 + y);
                fb.put(dest.0 + x, dest.1 + y, dst.blend(c, a as u8));
            }
        }
        true
    }

    /// Draw a classic command-button icon (archive frame order).
    pub fn draw_command_icon(
        &self,
        fb: &mut crate::render::Framebuffer,
        frame: usize,
        dest: (i32, i32, i32, i32),
    ) -> bool {
        if let Some(grp) = &self.command_buttons_hd {
            if let Some(image) = grp.frame(frame) {
                image.draw_scaled(fb, dest.0, dest.1, dest.2, dest.3);
                return true;
            }
        }
        let Some(grp) = &self.command_buttons else {
            return false;
        };
        if grp.frames.is_empty() {
            return false;
        }
        crate::render::draw_grp_frame(
            fb,
            grp,
            frame.min(grp.frames.len() - 1),
            &self.palette,
            dest,
            None,
        );
        true
    }
}

/// Main-menu / title / score surfaces.
/// Scale the console triptych so the three plates span `w` pixels.
///
/// Returns the three destination widths and the common height, or `None` when
/// the art has no size. The retail console spans the full content width, and
/// the height follows from the plates' native aspect: driving the scale from
/// the height instead overruns the screen (each wing is ~3x wider than tall).
pub fn plate_layout(native: [(usize, usize); 3], w: usize) -> Option<([(i32, i32); 3], i32)> {
    let total: f32 = native.iter().map(|(nw, _)| *nw as f32).sum();
    if total <= 0.0 || w == 0 {
        return None;
    }
    let k = w as f32 / total;
    let mut out = [(1, 1); 3];
    for (i, (nw, nh)) in native.iter().enumerate() {
        out[i] = (
            ((*nw as f32 * k).round() as i32).max(1),
            ((*nh as f32 * k).round() as i32).max(1),
        );
    }
    let height = out[0].1;
    Some((out, height))
}

pub struct GlueArt {
    pub title: Option<DdsImage>,
    pub title_bg: Option<DdsImage>,
    pub title_overlay: Option<DdsImage>,
    pub intro_credit: Option<DdsImage>,
    pub credit: Option<DdsImage>,
    pub etail: Option<DdsImage>,
    pub lock: Option<DdsImage>,
    /// "Select Game Type" modal plate (`glue/palmm/retail_ex.dds`): the metal
    /// frame, red hairline and the StarCraft / Brood War wordmarks.
    pub gametype: Option<DdsImage>,
    /// Moon-landscape backdrop (`glue/palnl/backgnd.dds`) shared by the
    /// Registry, Saved Games and Create screens.
    pub palnl_backdrop: Option<DdsImage>,
    /// Space backdrop behind the menu and its sub-screens
    /// (`glue/palmm/backgnd.dds`: planet, nebula, stars).
    pub palmm_backdrop: Option<DdsImage>,
    /// Episode-select console plates (`glucmpgn.ui.json` ids 1 and 3).
    pub campaign_botl: Option<DdsImage>,
    pub campaign_botr: Option<DdsImage>,
    /// Edge-anchored `palcs` backdrop halves behind the episode screen
    /// (each 320x480 SD, shipped as 1280x1920 HD art).
    pub campaign_backl: Option<DdsImage>,
    pub campaign_backr: Option<DdsImage>,
    /// Mission-history modal (`glue/campaign/phist.dds`): the whole frame,
    /// list box and Ok/Cancel buttons in one shipped plate.
    pub campaign_hist: Option<DdsImage>,
    /// Two-button glue popup plate (`glue/palcs/pdpopup.dds`, 4x the
    /// 360x200 `glupokcancel` dialog: frame and both button bays).
    pub campaign_prompt: Option<DdsImage>,
    /// Campaign side rails framing the episode-select screen.
    pub campaign_topl: Option<DdsImage>,
    pub campaign_topr: Option<DdsImage>,
    /// Create-game chrome plates (map list, slots, map info, buttons).
    pub create_map: Option<DdsImage>,
    pub create_map2: Option<DdsImage>,
    pub create_map_tall: Option<DdsImage>,
    pub create_slots: Option<DdsImage>,
    pub create_info: Option<DdsImage>,
    pub create_buttons: Option<DdsImage>,
    /// Chat plate under the Create/Replay list (`glue/chatroom/pchat.dds`).
    pub create_chat: Option<DdsImage>,
    /// Toon Registry backdrop, profile list plate and popup.
    pub registry_backdrop: Option<DdsImage>,
    pub registry_list: Option<DdsImage>,
    pub registry_popup: Option<DdsImage>,
    /// Shared `palnl` Ok/Cancel plates.
    pub palnl_ok: Option<DdsImage>,
    pub palnl_cancel: Option<DdsImage>,
    /// Credits panel frame (`glue/battlenet/credits_a.dds`).
    pub credits_frame: Option<DdsImage>,
    /// Metal-room backdrop behind the credits (`glue/battlenet/backgnd.dds`).
    pub credits_backdrop: Option<DdsImage>,
    pub menu: Option<UiDialog>,
    pub score: Option<UiDialog>,
    pub mission: Option<UiDialog>,
}

impl GlueArt {
    pub fn load(root: &Path) -> Self {
        Self {
            title: load_dds(root, "glue/title/title.dds"),
            title_bg: load_dds(root, "glue/mainmenu/titleframe_bg.dds"),
            title_overlay: load_dds(root, "glue/mainmenu/titleframe_overlay.dds"),
            intro_credit: load_dds(root, "glue/mainmenu/pintro.dds"),
            credit: load_dds(root, "glue/mainmenu/pcredit.dds"),
            etail: load_dds(root, "glue/mainmenu/etail.dds"),
            lock: load_dds(root, "glue/mainmenu/lock.dds"),
            gametype: load_dds(root, "glue/palmm/retail_ex.dds"),
            palnl_backdrop: load_dds(root, "glue/palnl/backgnd.dds"),
            palmm_backdrop: load_dds(root, "glue/palmm/backgnd.dds"),
            campaign_botl: load_dds(root, "glue/campaign/pbotl.dds"),
            campaign_botr: load_dds(root, "glue/campaign/pbotr.dds"),
            campaign_backl: load_dds(root, "glue/palcs/backgndl.dds"),
            campaign_backr: load_dds(root, "glue/palcs/backgndr.dds"),
            campaign_hist: load_dds(root, "glue/campaign/phist.dds"),
            campaign_prompt: load_dds(root, "glue/palcs/pdpopup.dds"),
            campaign_topl: load_dds(root, "glue/campaign/ptopl.dds"),
            campaign_topr: load_dds(root, "glue/campaign/ptopr.dds"),
            create_map: load_dds(root, "glue/create/plistmap.dds"),
            create_map2: load_dds(root, "glue/create/plistmap2.dds"),
            create_map_tall: load_dds(root, "glue/create/plistmaptall.dds"),
            create_slots: load_dds(root, "glue/create/pslots.dds"),
            create_info: load_dds(root, "glue/create/pinfo.dds"),
            create_buttons: load_dds(root, "glue/create/twobtnholder.dds"),
            create_chat: load_dds(root, "glue/chatroom/pchat.dds"),
            registry_backdrop: load_dds(root, "glue/battlenet/toonregistry/backgnd.dds"),
            registry_list: load_dds(root, "glue/battlenet/toonregistry/toonlistbg.dds"),
            registry_popup: load_dds(root, "glue/battlenet/toonregistry/popopup.dds"),
            palnl_ok: load_dds(root, "glue/palnl/pok.dds"),
            palnl_cancel: load_dds(root, "glue/palnl/pcancel.dds"),
            credits_frame: load_dds(root, "glue/battlenet/credits_a.dds"),
            credits_backdrop: load_dds(root, "glue/battlenet/backgnd.dds"),
            menu: load_ui(root, "glumain"),
            score: load_ui(root, "gluscore"),
            mission: load_ui(root, "msnstats"),
        }
    }

    pub fn has_art(&self) -> bool {
        self.title.is_some() || self.title_bg.is_some() || self.title_overlay.is_some()
    }

    /// Load the race/victory score backdrop on demand.  `pmain` is ~6 MB on
    /// disk and 23 MB decoded, so it is not kept resident until needed.
    pub fn load_score_panel(root: &Path, race: Race, victory: bool) -> Option<DdsImage> {
        let suffix = match race {
            Race::Zerg => "z",
            Race::Protoss => "p",
            _ => "t",
        };
        let variant = if victory { "v" } else { "d" };
        load_dds(root, &format!("glue/score{suffix}{variant}/pmain.dds"))
    }

    pub fn score_panel_path(root: &Path, race: Race, victory: bool) -> PathBuf {
        let suffix = match race {
            Race::Zerg => "z",
            Race::Protoss => "p",
            _ => "t",
        };
        let variant = if victory { "v" } else { "d" };
        root.join(format!("glue/score{suffix}{variant}/pmain.dds"))
    }
}

/// Draw a UI rectangle after scaling the classic 640x480 coordinate space.
pub fn scaled_rect(
    rect: crate::ui::UiRect,
    scale: f32,
    safe_x: i32,
    safe_y: i32,
) -> crate::ui::UiRect {
    rect.scaled(scale, safe_x, safe_y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_triptych_spans_the_content_width() {
        // Native sizes of the shipped Terran console plates.
        let native = [(1970, 700), (1156, 512), (2286, 700)];
        let (sizes, height) = plate_layout(native, 2560).expect("layout");
        let total: i32 = sizes.iter().map(|(w, _)| *w).sum();
        // The three plates must tile the whole width (rounding aside).
        assert!((total - 2560).abs() <= 2, "plates span {total}");
        // Every plate keeps its native aspect.
        for (i, (dw, dh)) in sizes.iter().enumerate() {
            let (nw, nh) = native[i];
            let want = nw as f32 / nh as f32;
            let got = *dw as f32 / *dh as f32;
            assert!(
                (want - got).abs() < 0.05,
                "plate {i}: aspect {got:.2} vs {want:.2}"
            );
        }
        // The common height follows from the aspect, not from the HUD band.
        assert!(height > 200 && height < 400, "height {height}");
        assert_eq!(plate_layout([(0, 0); 3], 2560), None);
    }

    #[test]
    fn race_directories_are_the_shipped_console_names() {
        assert_eq!(race_dir(Race::Terran), "terran");
        assert_eq!(race_dir(Race::Zerg), "zerg");
        assert_eq!(race_dir(Race::Protoss), "protoss");
    }

    #[test]
    fn score_panel_paths_follow_the_race_and_result() {
        let root = Path::new("/cache");
        assert_eq!(
            GlueArt::score_panel_path(root, Race::Terran, true),
            Path::new("/cache/glue/scoretv/pmain.dds")
        );
        assert_eq!(
            GlueArt::score_panel_path(root, Race::Zerg, false),
            Path::new("/cache/glue/scorezd/pmain.dds")
        );
        assert_eq!(
            GlueArt::score_panel_path(root, Race::Protoss, true),
            Path::new("/cache/glue/scorepv/pmain.dds")
        );
    }
}

#[cfg(test)]
mod wireframe_tests {
    use super::*;
    use crate::render::{Framebuffer, Grp, GrpFrame, Palette};

    fn one_pixel_grp(index: u8) -> Grp {
        Grp {
            width: 1,
            height: 1,
            frames: vec![GrpFrame {
                x: 0,
                y: 0,
                w: 1,
                h: 1,
                indices: vec![index],
            }],
        }
    }

    #[test]
    fn wireframes_are_green_blueprints_not_palette_placeholders() {
        // unit.pal fills indices 208..=211 with flat blue; the wireframe page
        // must be remapped onto green or the info panel shows blue blobs.
        let mut console = ConsoleArt {
            race: "terran",
            left: crate::dds::DdsImage {
                width: 1,
                height: 1,
                pixels: vec![0],
            },
            center: crate::dds::DdsImage {
                width: 1,
                height: 1,
                pixels: vec![0],
            },
            right: crate::dds::DdsImage {
                width: 1,
                height: 1,
                pixels: vec![0],
            },
            well_src: (0.0, 0.0, 1.0, 1.0),
            right_src: (0.0, 0.0, 1.0, 1.0),
            palette: Palette {
                colors: [crate::render::Rgb::new(35, 35, 255); 256],
            },
            layouts: HashMap::new(),
            con_over: None,
            clock: None,
            apm: None,
            command_buttons: None,
            command_buttons_hd: None,
            command_icons_hd: None,
            command_icons: None,
            wireframe: Some(one_pixel_grp(208)),
        };
        let mut fb = Framebuffer::new(1, 1);
        assert!(console.draw_wireframe(&mut fb, 0, (0, 0, 1, 1), None));
        let c = fb.get(0, 0);
        let (r, g, b) = ((c.0 >> 16) & 0xff, (c.0 >> 8) & 0xff, c.0 & 0xff);
        assert!(g > r && g > b, "wireframe pixel {r},{g},{b} is not green");
        console.wireframe = None;
    }
}

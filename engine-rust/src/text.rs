//! Remastered front-end text: the shipped TrueType faces.
//!
//! The classic in-game HUD uses the bundled 8x8 bitmap face (`render::Framebuffer::text`).
//! The Remastered menus on the other hand draw with the Eurostile faces that
//! ship inside the installation (`webui/dist/lib/fonts/*.otf`, `font/*.ttf`).
//! This module rasterises those faces at runtime with `fontdue`, so the menu
//! labels use the same letterforms, sizes and tracking as the retail client
//! instead of a bitmap approximation.
//!
//! No font is vendored: [`UiFont::load`] reads the player's own file, and the
//! client falls back to the bitmap face when it is absent (no-install runs).

use std::collections::HashMap;

use crate::render::{Framebuffer, Rgb};

/// A rasterised UI face.  Glyph bitmaps are cached per pixel size, because a
/// menu screen re-renders the same labels every frame.
pub struct UiFont {
    font: fontdue::Font,
    cache: HashMap<(u16, u32, i32, i32), CachedGlyph>,
    pub name: String,
    /// The face file, kept for the outline rasteriser ([`Self::draw_cef`]).
    bytes: Vec<u8>,
    cef_cache: HashMap<(u16, u32, u8, u8), CefGlyph>,
}

/// Coverage of one glyph rasterised from its outline at a sub-pixel pen
/// and baseline phase (see [`UiFont::draw_cef`]).
struct CefGlyph {
    left: i32,
    top: i32,
    w: usize,
    h: usize,
    cov: Vec<u8>,
}

/// Sub-pixel phases per pixel cached for [`UiFont::draw_cef`].
const CEF_PHASES: f32 = 16.0;

/// Chromium's grayscale glyph masks are lifted by `1 - (1 - a)^k`.  Fitted
/// on the retail Options captures (130933 "Game Speed": mean coverage
/// error 0.027 against 0.102 for the shifted fontdue bitmap).
const CEF_CONTRAST: f32 = 1.2;

struct CachedGlyph {
    metrics: fontdue::Metrics,
    bitmap: Vec<u8>,
}

impl UiFont {
    /// Load a TrueType/OpenType face from an on-disk file.
    pub fn load(path: &std::path::Path) -> Option<Self> {
        let bytes = std::fs::read(path).ok()?;
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("font")
            .to_string();
        Self::from_bytes(bytes, name)
    }

    /// A face from its file bytes (e.g. read straight from the install).
    pub fn from_bytes(bytes: Vec<u8>, name: String) -> Option<Self> {
        let font =
            fontdue::Font::from_bytes(bytes.clone(), fontdue::FontSettings::default()).ok()?;
        Some(Self {
            font,
            cache: HashMap::new(),
            name,
            bytes,
            cef_cache: HashMap::new(),
        })
    }

    /// The face file.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Whether the face maps `ch` to a real glyph (not `.notdef`).
    pub fn has_glyph(&self, ch: char) -> bool {
        self.font.lookup_glyph_index(ch) != 0
    }

    /// Load the regular retail menu face from a materialised cache root.
    /// The wide Extended face used by lit main-menu labels is loaded
    /// separately by [`Self::load_retail_extended`].
    pub fn load_retail(cache: &std::path::Path) -> Option<Self> {
        for rel in [
            // The Remastered menu labels use the regular-width Eurostile
            // face with generous tracking; Eurostile Extended is the panel
            // heading face and is only a fallback.
            "webui/dist/lib/fonts/eurostile-reg.otf",
            "font/eurostile-reg.ttf",
            "webui/dist/lib/fonts/eurostileext-reg.otf",
            "font/eurostileext-reg.ttf",
            "font/blizzard-regular.ttf",
        ] {
            if let Some(font) = Self::load(&cache.join(rel)) {
                return Some(font);
            }
        }
        None
    }

    /// The wide face used by the lit main-menu labels.  It is a separate
    /// file from the regular face used by idle labels and panels.
    pub fn load_retail_extended(cache: &std::path::Path) -> Option<Self> {
        for rel in [
            "webui/dist/lib/fonts/eurostileext-reg.otf",
            "font/eurostileext-reg.ttf",
        ] {
            if let Some(font) = Self::load(&cache.join(rel)) {
                return Some(font);
            }
        }
        None
    }

    fn glyph(&mut self, ch: char, px: f32, x_scale: f32, y_scale: f32) -> &CachedGlyph {
        let index = self.font.lookup_glyph_index(ch);
        self.glyph_index(index, px, x_scale, y_scale)
    }

    fn glyph_index(&mut self, index: u16, px: f32, x_scale: f32, y_scale: f32) -> &CachedGlyph {
        let key = (
            index,
            (px * 4.0).round() as u32,
            (x_scale * 100.0).round() as i32,
            (y_scale * 100.0).round() as i32,
        );
        if !self.cache.contains_key(&key) {
            let (mut metrics, bitmap) = self.font.rasterize_indexed(index, px);
            // The retail face is condensed relative to Eurostile Extended;
            // scale the glyph bitmap horizontally so the label reproduces
            // the width measured on the reference without changing cap
            // height or weight.
            let new_w = (metrics.width as f32 * x_scale).round().max(0.0) as usize;
            let bitmap = if (x_scale - 1.0).abs() < 0.01 || metrics.width == 0 {
                bitmap
            } else {
                let mut out = vec![0u8; new_w * metrics.height];
                for row in 0..metrics.height {
                    for col in 0..new_w {
                        let sx = ((col as f32 + 0.5) / x_scale) as usize;
                        let sx = sx.min(metrics.width.saturating_sub(1));
                        out[row * new_w + col] = bitmap[row * metrics.width + sx];
                    }
                }
                metrics.xmin = (metrics.xmin as f32 * x_scale).round() as i32;
                metrics.advance_width *= x_scale;
                metrics.width = new_w;
                out
            };
            // Vertical stretch (CEF's x-height/cap height is a little taller
            // than fontdue's at the same px size).  Rows are resampled around
            // the glyph top; the advance is untouched.
            let bitmap = if (y_scale - 1.0).abs() < 0.01 || metrics.height == 0 {
                bitmap
            } else {
                let new_h = (metrics.height as f32 * y_scale).round().max(1.0) as usize;
                let mut out = vec![0u8; metrics.width * new_h];
                let src_h = metrics.height;
                for row in 0..new_h {
                    let syf = (((row as f32 + 0.5) / y_scale) - 0.5)
                        .clamp(0.0, src_h.saturating_sub(1) as f32);
                    let y0 = syf.floor() as usize;
                    let y1 = (y0 + 1).min(src_h.saturating_sub(1));
                    let wy = syf - y0 as f32;
                    for col in 0..metrics.width {
                        let a = bitmap[y0 * metrics.width + col] as f32;
                        let b = bitmap[y1 * metrics.width + col] as f32;
                        out[row * metrics.width + col] = (a * (1.0 - wy) + b * wy).round() as u8;
                    }
                }
                metrics.ymin = (metrics.ymin as f32 * y_scale).round() as i32;
                metrics.height = new_h;
                out
            };
            self.cache.insert(key, CachedGlyph { metrics, bitmap });
        }
        &self.cache[&key]
    }

    /// Width of `text` in pixels with `tracking` extra pixels between glyphs.
    pub fn measure(&mut self, text: &str, px: f32, tracking: f32) -> i32 {
        self.measure_scaled(text, px, tracking, 1.0)
    }

    /// Width with a horizontal glyph scale (see [`Self::draw_at_scaled`]).
    pub fn measure_scaled(&mut self, text: &str, px: f32, tracking: f32, x_scale: f32) -> i32 {
        let mut width = 0.0f32;
        let mut count = 0usize;
        for ch in text.chars() {
            let g = self.glyph(ch, px, x_scale, 1.0);
            width += g.metrics.advance_width;
            count += 1;
        }
        if count > 1 {
            width += tracking * (count - 1) as f32;
        }
        width.round() as i32
    }

    /// Baseline-relative draw.  `baseline_y` is the text baseline in screen
    /// pixels.  The optional `shadow` is a dark offset copy used by the retail
    /// menu labels (they carry a 1px dark edge).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_at(
        &mut self,
        fb: &mut Framebuffer,
        x: i32,
        baseline_y: i32,
        text: &str,
        px: f32,
        tracking: f32,
        color: Rgb,
        shadow: Option<Rgb>,
    ) -> i32 {
        self.draw_at_scaled(fb, x, baseline_y, text, px, tracking, 1.0, color, shadow)
    }

    /// [`Self::draw_at`] with a horizontal glyph scale.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_at_scaled(
        &mut self,
        fb: &mut Framebuffer,
        x: i32,
        baseline_y: i32,
        text: &str,
        px: f32,
        tracking: f32,
        x_scale: f32,
        color: Rgb,
        shadow: Option<Rgb>,
    ) -> i32 {
        self.draw_at_scaled_weighted(
            fb,
            x,
            baseline_y as f32,
            text,
            px,
            tracking,
            x_scale,
            color,
            shadow,
            1.0,
        )
    }

    /// [`Self::draw_at_scaled`] with an alpha-coverage gamma (values below
    /// 1 thicken glyph edges, above 1 thin them).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_at_scaled_weighted(
        &mut self,
        fb: &mut Framebuffer,
        x: i32,
        baseline_y: f32,
        text: &str,
        px: f32,
        tracking: f32,
        x_scale: f32,
        color: Rgb,
        shadow: Option<Rgb>,
        gamma: f32,
    ) -> i32 {
        self.draw_at_scaled_xy(
            fb, x, baseline_y, text, px, tracking, x_scale, 1.0, color, shadow, gamma,
        )
    }

    /// Baseline-relative draw with independent horizontal and vertical glyph
    /// scales and an alpha-coverage gamma.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_at_scaled_xy(
        &mut self,
        fb: &mut Framebuffer,
        x: i32,
        baseline_y: f32,
        text: &str,
        px: f32,
        tracking: f32,
        x_scale: f32,
        y_scale: f32,
        color: Rgb,
        shadow: Option<Rgb>,
        gamma: f32,
    ) -> i32 {
        self.draw_at_scaled_xy_phase(
            fb, x, 0.0, baseline_y, text, px, tracking, x_scale, y_scale, color, shadow, gamma,
        )
    }

    /// [`Self::draw_at_scaled_xy`] with a fractional pen phase (the CEF text
    /// origin can sit between device pixels).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_at_scaled_xy_phase(
        &mut self,
        fb: &mut Framebuffer,
        x: i32,
        phase: f32,
        baseline_y: f32,
        text: &str,
        px: f32,
        tracking: f32,
        x_scale: f32,
        y_scale: f32,
        color: Rgb,
        shadow: Option<Rgb>,
        gamma: f32,
    ) -> i32 {
        if px <= 0.0 {
            return x;
        }
        let mut pen = x as f32 + phase;
        for ch in text.chars() {
            let index = self.font.lookup_glyph_index(ch);
            let adv = self.draw_index(
                fb, index, pen, baseline_y, px, x_scale, y_scale, color, shadow, gamma,
            );
            pen += adv + tracking;
        }
        pen.round() as i32
    }

    /// Blit one already-rasterised glyph at a fractional pen position.  The
    /// coverage is split between the two neighbouring columns/rows.
    #[allow(clippy::too_many_arguments)]
    fn draw_index(
        &mut self,
        fb: &mut Framebuffer,
        index: u16,
        pen: f32,
        baseline_y: f32,
        px: f32,
        x_scale: f32,
        y_scale: f32,
        color: Rgb,
        shadow: Option<Rgb>,
        gamma: f32,
    ) -> f32 {
        let g = self.glyph_index(index, px, x_scale, y_scale);
        let gxf = pen + g.metrics.xmin as f32;
        let gx = gxf.floor() as i32;
        let fx = gxf - gx as f32;
        let gyf = baseline_y - g.metrics.ymin as f32 - g.metrics.height as f32;
        let gy = gyf.floor() as i32;
        let fy = gyf - gy as f32;
        if let Some(shadow) = shadow {
            blit_coverage(fb, g, gx + 1, gy + 1, shadow, 200, gamma);
        }
        let left = ((1.0 - fx) * 255.0).round() as u8;
        let right = (fx * 255.0).round() as u8;
        let top = ((1.0 - fy) * 255.0).round() as u8;
        let bottom = (fy * 255.0).round() as u8;
        for (yy, wy) in [(gy, top), (gy + 1, bottom)] {
            if wy == 0 {
                continue;
            }
            if left > 0 {
                let a = ((left as u16 * wy as u16) / 255) as u8;
                if a > 0 {
                    blit_coverage(fb, g, gx, yy, color, a, gamma);
                }
            }
            if right > 0 {
                let a = ((right as u16 * wy as u16) / 255) as u8;
                if a > 0 {
                    blit_coverage(fb, g, gx + 1, yy, color, a, gamma);
                }
            }
        }
        g.metrics.advance_width
    }

    /// Draw one glyph with its bitmap left edge at `left` and a measured
    /// target width.  Used for the profile line calibrated against the
    /// retail captures (per-glyph advance/width table).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_glyph_left(
        &mut self,
        fb: &mut Framebuffer,
        ch: char,
        left: i32,
        top: i32,
        px: f32,
        target_width: i32,
        color: Rgb,
        gamma: f32,
        bold_alpha: u8,
    ) {
        let base = self.glyph(ch, px, 1.0, 1.0);
        let x_scale = if target_width > 0 && base.metrics.width > 0 {
            (target_width as f32 / base.metrics.width as f32).max(0.1)
        } else {
            1.0
        };
        let g = self.glyph(ch, px, x_scale, 1.0);
        let baseline = top + (px * 0.67).round() as i32;
        let gy = baseline - g.metrics.ymin - g.metrics.height as i32;
        blit_coverage(fb, g, left, gy, color, 255, gamma);
        if bold_alpha > 0 {
            blit_coverage(fb, g, left - 1, gy, color, bold_alpha, gamma);
            blit_coverage(fb, g, left + 1, gy, color, bold_alpha, gamma);
            blit_coverage(fb, g, left, gy - 1, color, bold_alpha, gamma);
            blit_coverage(fb, g, left, gy + 1, color, bold_alpha, gamma);
        }
    }

    /// Draw with the top of the cap height at `top` (the menu uses top
    /// anchors read from the shipped 640x480 dialogs).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_top(
        &mut self,
        fb: &mut Framebuffer,
        x: i32,
        top: i32,
        text: &str,
        px: f32,
        tracking: f32,
        color: Rgb,
        shadow: Option<Rgb>,
    ) -> i32 {
        self.draw_top_scaled(fb, x, top, text, px, tracking, 1.0, color, shadow)
    }

    /// [`Self::draw_top`] with a horizontal glyph scale.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_top_scaled(
        &mut self,
        fb: &mut Framebuffer,
        x: i32,
        top: i32,
        text: &str,
        px: f32,
        tracking: f32,
        x_scale: f32,
        color: Rgb,
        shadow: Option<Rgb>,
    ) -> i32 {
        // Eurostile's cap height is ~0.70 em and the ascender ~0.75 em; the
        // reference labels are top-anchored on the cap line.
        let baseline = top + (px * 0.67).round() as i32;
        self.draw_at_scaled(fb, x, baseline, text, px, tracking, x_scale, color, shadow)
    }

    /// Draw centred horizontally on `cx`, top-anchored at `top`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_centered(
        &mut self,
        fb: &mut Framebuffer,
        cx: i32,
        top: i32,
        text: &str,
        px: f32,
        tracking: f32,
        color: Rgb,
        shadow: Option<Rgb>,
    ) {
        let w = self.measure(text, px, tracking);
        self.draw_top(fb, cx - w / 2, top, text, px, tracking, color, shadow);
    }

    /// Centred with an alpha-coverage gamma (values below 1 thicken the
    /// glyph edges, matching CEF's slightly heavier antialiasing).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_centered_weighted(
        &mut self,
        fb: &mut Framebuffer,
        cx: i32,
        top: i32,
        text: &str,
        px: f32,
        tracking: f32,
        color: Rgb,
        shadow: Option<Rgb>,
        gamma: f32,
    ) {
        let w = self.measure(text, px, tracking);
        let baseline = top + (px * 0.67).round() as i32;
        self.draw_at_scaled_weighted(
            fb,
            cx - w / 2,
            baseline as f32,
            text,
            px,
            tracking,
            1.0,
            color,
            shadow,
            gamma,
        );
    }

    /// Centred with an alpha-coverage gamma and a fractional pen phase.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_centered_weighted_phase(
        &mut self,
        fb: &mut Framebuffer,
        cx: i32,
        phase: f32,
        top: i32,
        text: &str,
        px: f32,
        tracking: f32,
        color: Rgb,
        shadow: Option<Rgb>,
        gamma: f32,
    ) {
        let w = self.measure(text, px, tracking);
        let baseline = top + (px * 0.67).round() as i32;
        self.draw_at_scaled_xy_phase(
            fb,
            cx - w / 2,
            phase,
            baseline as f32,
            text,
            px,
            tracking,
            1.0,
            1.0,
            color,
            shadow,
            gamma,
        );
    }

    /// Top-anchored draw with independent x/y glyph scaling and a gamma.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_top_stretched(
        &mut self,
        fb: &mut Framebuffer,
        x: i32,
        top: i32,
        text: &str,
        px: f32,
        tracking: f32,
        x_scale: f32,
        y_scale: f32,
        color: Rgb,
        shadow: Option<Rgb>,
        gamma: f32,
    ) -> i32 {
        let baseline = top + (px * y_scale * 0.67).round() as i32;
        self.draw_at_scaled_xy(
            fb,
            x,
            baseline as f32,
            text,
            px,
            tracking,
            x_scale,
            y_scale,
            color,
            shadow,
            gamma,
        )
    }

    /// Centred [`Self::draw_top_stretched`].
    #[allow(clippy::too_many_arguments)]
    pub fn draw_centered_stretched(
        &mut self,
        fb: &mut Framebuffer,
        cx: i32,
        top: i32,
        text: &str,
        px: f32,
        tracking: f32,
        x_scale: f32,
        y_scale: f32,
        color: Rgb,
        shadow: Option<Rgb>,
        gamma: f32,
    ) {
        let w = self.measure_scaled(text, px, tracking, x_scale);
        self.draw_top_stretched(
            fb,
            cx - w / 2,
            top,
            text,
            px,
            tracking,
            x_scale,
            y_scale,
            color,
            shadow,
            gamma,
        );
    }

    /// Centred with a horizontal glyph scale.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_centered_scaled(
        &mut self,
        fb: &mut Framebuffer,
        cx: i32,
        top: i32,
        text: &str,
        px: f32,
        tracking: f32,
        x_scale: f32,
        color: Rgb,
        shadow: Option<Rgb>,
    ) {
        let w = self.measure_scaled(text, px, tracking, x_scale);
        self.draw_top_scaled(
            fb,
            cx - w / 2,
            top,
            text,
            px,
            tracking,
            x_scale,
            color,
            shadow,
        );
    }

    /// Right-aligned with a horizontal glyph scale.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_right_scaled(
        &mut self,
        fb: &mut Framebuffer,
        right: i32,
        top: i32,
        text: &str,
        px: f32,
        tracking: f32,
        x_scale: f32,
        color: Rgb,
        shadow: Option<Rgb>,
    ) {
        let w = self.measure_scaled(text, px, tracking, x_scale);
        self.draw_top_scaled(
            fb,
            right - w,
            top,
            text,
            px,
            tracking,
            x_scale,
            color,
            shadow,
        );
    }

    /// Right-aligned with a horizontal scale and a coverage gamma.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_right_scaled_weighted(
        &mut self,
        fb: &mut Framebuffer,
        right: i32,
        top: i32,
        text: &str,
        px: f32,
        tracking: f32,
        x_scale: f32,
        color: Rgb,
        shadow: Option<Rgb>,
        gamma: f32,
    ) {
        let w = self.measure_scaled(text, px, tracking, x_scale);
        let baseline = top + (px * 0.67).round() as i32;
        self.draw_at_scaled_weighted(
            fb,
            right - w,
            baseline as f32,
            text,
            px,
            tracking,
            x_scale,
            color,
            shadow,
            gamma,
        );
    }

    /// f32-baseline alias used by the fractional info-text path.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_at_scaled_weighted_f(
        &mut self,
        fb: &mut Framebuffer,
        x: i32,
        baseline_y: f32,
        text: &str,
        px: f32,
        tracking: f32,
        x_scale: f32,
        color: Rgb,
        shadow: Option<Rgb>,
        gamma: f32,
    ) -> i32 {
        self.draw_at_scaled_weighted(
            fb, x, baseline_y, text, px, tracking, x_scale, color, shadow, gamma,
        )
    }

    /// Right-aligned with fractional top/baseline and a coverage gamma.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_right_scaled_weighted_f(
        &mut self,
        fb: &mut Framebuffer,
        right: i32,
        top: f32,
        text: &str,
        px: f32,
        tracking: f32,
        x_scale: f32,
        color: Rgb,
        shadow: Option<Rgb>,
        gamma: f32,
    ) {
        let w = self.measure_scaled(text, px, tracking, x_scale);
        let baseline = top + px * 0.67;
        self.draw_at_scaled_weighted_f(
            fb,
            right - w,
            baseline,
            text,
            px,
            tracking,
            x_scale,
            color,
            shadow,
            gamma,
        );
    }

    /// Draw right-aligned at `right`, top-anchored at `top`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_right(
        &mut self,
        fb: &mut Framebuffer,
        right: i32,
        top: i32,
        text: &str,
        px: f32,
        tracking: f32,
        color: Rgb,
        shadow: Option<Rgb>,
    ) {
        let w = self.measure(text, px, tracking);
        self.draw_top(fb, right - w, top, text, px, tracking, color, shadow);
    }
}

impl UiFont {
    /// Width of `text` laid out by [`Self::draw_cef`] (unrounded advances,
    /// as the CEF lays text out with sub-pixel positioning).
    pub fn measure_cef(&self, text: &str, px: f32, tracking: f32) -> f32 {
        let Ok(face) = ttf_parser::Face::parse(&self.bytes, 0) else {
            return 0.0;
        };
        let s = px / face.units_per_em() as f32;
        let mut w = 0.0;
        for (i, ch) in text.chars().enumerate() {
            let id = face.glyph_index(ch).unwrap_or(ttf_parser::GlyphId(0));
            w += face.glyph_hor_advance(id).unwrap_or(0) as f32 * s;
            if i > 0 {
                w += tracking;
            }
        }
        w
    }

    /// Horizontal `kern` table adjustment between two characters, in pixels
    /// at `px`.  The retail glue (native, not CEF) lays its labels out with
    /// the `font/*.ttf` kern pairs: `Terran` is 4.6 px narrower and
    /// `Protoss` 2.7 px wider at 47.5 px/em on 130744.
    pub fn kerning(&self, a: char, b: char, px: f32) -> f32 {
        let Ok(face) = ttf_parser::Face::parse(&self.bytes, 0) else {
            return 0.0;
        };
        let (Some(l), Some(r)) = (face.glyph_index(a), face.glyph_index(b)) else {
            return 0.0;
        };
        let s = px / face.units_per_em() as f32;
        if let Some(kern) = face.tables().kern {
            for sub in kern.subtables {
                if sub.horizontal
                    && !sub.variable
                    && let Some(v) = sub.glyphs_kerning(l, r)
                {
                    return v as f32 * s;
                }
            }
        }
        0.0
    }

    /// Ink box of `text` laid out like [`Self::draw_cef`] from pen 0 on
    /// baseline 0: `(x0, x1, top, bottom)` with y growing downwards.  The
    /// retail glue centres button labels on this box (`FB_HTEXTCENTER |
    /// FB_VTEXTCENTER`), so descenders lift `Play Custom` and `Load Replay`.
    pub fn ink_bounds_cef(
        &self,
        text: &str,
        px: f32,
        tracking: f32,
    ) -> Option<(f32, f32, f32, f32)> {
        let face = ttf_parser::Face::parse(&self.bytes, 0).ok()?;
        let s = px / face.units_per_em() as f32;
        let mut pen = 0.0f32;
        let mut bounds: Option<(f32, f32, f32, f32)> = None;
        for ch in text.chars() {
            let id = face.glyph_index(ch).unwrap_or(ttf_parser::GlyphId(0));
            if let Some(bb) = face.glyph_bounding_box(id) {
                let g = (
                    pen + bb.x_min as f32 * s,
                    pen + bb.x_max as f32 * s,
                    -(bb.y_max as f32) * s,
                    -(bb.y_min as f32) * s,
                );
                bounds = Some(match bounds {
                    None => g,
                    Some(b) => (b.0.min(g.0), b.1.max(g.1), b.2.min(g.2), b.3.max(g.3)),
                });
            }
            pen += face.glyph_hor_advance(id).unwrap_or(0) as f32 * s + tracking;
        }
        bounds
    }

    /// Draw `text` the way the retail Chromium (CEF) front-end does: every
    /// glyph is rasterised from its outline at its exact sub-pixel pen and
    /// baseline (no bitmap shifting, which blurs stems), with exact-area
    /// non-zero coverage and the CEF contrast curve.  `x` is the pen origin
    /// and `baseline` the baseline, both in fractional window pixels.  The
    /// first glyph uses `first` (CSS `::first-letter`), the rest `rest`.
    /// Returns the pen position after the text.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_cef(
        &mut self,
        fb: &mut Framebuffer,
        x: f32,
        baseline: f32,
        text: &str,
        px: f32,
        tracking: f32,
        first: Rgb,
        rest: Rgb,
    ) -> f32 {
        if px <= 0.0 {
            return x;
        }
        let Self {
            bytes, cef_cache, ..
        } = self;
        let Ok(face) = ttf_parser::Face::parse(bytes, 0) else {
            return x;
        };
        let s = px / face.units_per_em() as f32;
        let iy = baseline.floor();
        let mut fy = ((baseline - iy) * CEF_PHASES).round() as u32;
        let iy = iy as i32 + (fy / CEF_PHASES as u32) as i32;
        fy %= CEF_PHASES as u32;
        let mut pen = x;
        for (i, ch) in text.chars().enumerate() {
            let id = face.glyph_index(ch).unwrap_or(ttf_parser::GlyphId(0));
            let ix = pen.floor();
            let mut fx = ((pen - ix) * CEF_PHASES).round() as u32;
            let ix = ix as i32 + (fx / CEF_PHASES as u32) as i32;
            fx %= CEF_PHASES as u32;
            let key = (id.0, (px * 64.0).round() as u32, fx as u8, fy as u8);
            let g = cef_cache.entry(key).or_insert_with(|| {
                cef_rasterise(&face, id, s, fx as f32 / CEF_PHASES, fy as f32 / CEF_PHASES)
            });
            let color = if i == 0 { first } else { rest };
            for row in 0..g.h {
                for col in 0..g.w {
                    let a = g.cov[row * g.w + col];
                    if a == 0 {
                        continue;
                    }
                    let (px_, py_) = (ix + g.left + col as i32, iy + g.top + row as i32);
                    fb.put(px_, py_, fb.get(px_, py_).blend(color, a));
                }
            }
            pen += face.glyph_hor_advance(id).unwrap_or(0) as f32 * s + tracking;
        }
        pen
    }
}

/// Outline -> line segments in glyph-bitmap pixels (y down).
struct Flatten {
    s: f32,
    ox: f32,
    oy: f32,
    start: (f32, f32),
    last: (f32, f32),
    lines: Vec<[f32; 4]>,
}

impl Flatten {
    fn map(&self, x: f32, y: f32) -> (f32, f32) {
        (self.ox + x * self.s, self.oy - y * self.s)
    }
    fn push(&mut self, p: (f32, f32)) {
        self.lines.push([self.last.0, self.last.1, p.0, p.1]);
        self.last = p;
    }
}

impl ttf_parser::OutlineBuilder for Flatten {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.map(x, y);
        self.start = p;
        self.last = p;
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.map(x, y);
        self.push(p);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let p0 = self.last;
        let p1 = self.map(x1, y1);
        let p2 = self.map(x, y);
        for k in 1..=8 {
            let t = k as f32 / 8.0;
            let mt = 1.0 - t;
            self.push((
                mt * mt * p0.0 + 2.0 * mt * t * p1.0 + t * t * p2.0,
                mt * mt * p0.1 + 2.0 * mt * t * p1.1 + t * t * p2.1,
            ));
        }
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let p0 = self.last;
        let p1 = self.map(x1, y1);
        let p2 = self.map(x2, y2);
        let p3 = self.map(x, y);
        for k in 1..=12 {
            let t = k as f32 / 12.0;
            let mt = 1.0 - t;
            let (a, b, c, d) = (mt * mt * mt, 3.0 * mt * mt * t, 3.0 * mt * t * t, t * t * t);
            self.push((
                a * p0.0 + b * p1.0 + c * p2.0 + d * p3.0,
                a * p0.1 + b * p1.1 + c * p2.1 + d * p3.1,
            ));
        }
    }
    fn close(&mut self) {
        if self.last != self.start {
            let start = self.start;
            self.push(start);
        }
    }
}

/// Rasterise glyph `id` at scale `s` (px per font unit) with the pen at
/// sub-pixel phase `fx` and the baseline at phase `fy`.
fn cef_rasterise(
    face: &ttf_parser::Face,
    id: ttf_parser::GlyphId,
    s: f32,
    fx: f32,
    fy: f32,
) -> CefGlyph {
    let empty = CefGlyph {
        left: 0,
        top: 0,
        w: 0,
        h: 0,
        cov: Vec::new(),
    };
    let Some(bb) = face.glyph_bounding_box(id) else {
        return empty;
    };
    let left = (fx + bb.x_min as f32 * s).floor() as i32;
    let right = (fx + bb.x_max as f32 * s).ceil() as i32;
    let top = (fy - bb.y_max as f32 * s).floor() as i32;
    let bottom = (fy - bb.y_min as f32 * s).ceil() as i32;
    let (w, h) = (
        (right - left).max(0) as usize + 1,
        (bottom - top).max(0) as usize + 1,
    );
    if w * h > 1 << 20 {
        return empty;
    }
    let mut fl = Flatten {
        s,
        ox: fx - left as f32,
        oy: fy - top as f32,
        start: (0.0, 0.0),
        last: (0.0, 0.0),
        lines: Vec::new(),
    };
    face.outline_glyph(id, &mut fl);
    let cov = raster_lines(&fl.lines, w, h)
        .into_iter()
        .map(|c| ((1.0 - (1.0 - c).powf(CEF_CONTRAST)) * 255.0).round() as u8)
        .collect();
    CefGlyph {
        left,
        top,
        w,
        h,
        cov,
    }
}

/// Exact-area coverage of closed polylines (non-zero, non-overlapping
/// contours) on a `w x h` grid: each segment deposits its signed area into
/// the cells it crosses, and a running sum along every row integrates it.
fn raster_lines(lines: &[[f32; 4]], w: usize, h: usize) -> Vec<f32> {
    let stride = w + 2;
    let mut acc = vec![0.0f32; stride * h];
    for l in lines {
        let (mut x0, mut y0, mut x1, mut y1) = (l[0], l[1], l[2], l[3]);
        if (y1 - y0).abs() <= f32::EPSILON {
            continue;
        }
        let dir = if y0 < y1 {
            1.0
        } else {
            std::mem::swap(&mut x0, &mut x1);
            std::mem::swap(&mut y0, &mut y1);
            -1.0
        };
        let dxdy = (x1 - x0) / (y1 - y0);
        let ystart = y0.max(0.0);
        let mut x = x0 + (ystart - y0) * dxdy;
        let row_end = (y1.ceil() as usize).min(h);
        for row in (ystart as usize)..row_end {
            let dy = ((row + 1) as f32).min(y1) - (row as f32).max(y0);
            if dy <= 0.0 {
                continue;
            }
            let xnext = x + dxdy * dy;
            let d = dy * dir;
            let (xa, xb) = if x < xnext { (x, xnext) } else { (xnext, x) };
            let base = row * stride;
            let put = |acc: &mut Vec<f32>, col: i64, v: f32| {
                let c = col.clamp(0, stride as i64 - 1) as usize;
                acc[base + c] += v;
            };
            let xaf = xa.floor();
            let xai = xaf as i64;
            let xbc = xb.ceil();
            let xbi = xbc as i64;
            if xbi <= xai + 1 {
                // The segment stays inside one column on this row.
                let xm = 0.5 * (x + xnext) - xaf;
                put(&mut acc, xai, d * (1.0 - xm));
                put(&mut acc, xai + 1, d * xm);
            } else {
                let inv = 1.0 / (xb - xa);
                let fa = xa - xaf;
                let a0 = 0.5 * inv * (1.0 - fa) * (1.0 - fa);
                let fb = xb - xbc + 1.0;
                let am = 0.5 * inv * fb * fb;
                put(&mut acc, xai, d * a0);
                if xbi == xai + 2 {
                    put(&mut acc, xai + 1, d * (1.0 - a0 - am));
                } else {
                    let a1 = inv * (1.5 - fa);
                    put(&mut acc, xai + 1, d * (a1 - a0));
                    for col in xai + 2..xbi - 1 {
                        put(&mut acc, col, d * inv);
                    }
                    let a2 = a1 + (xbi - xai - 3) as f32 * inv;
                    put(&mut acc, xbi - 1, d * (1.0 - a2 - am));
                }
                put(&mut acc, xbi, d * am);
            }
            x = xnext;
        }
    }
    let mut out = vec![0.0f32; w * h];
    for row in 0..h {
        let mut sum = 0.0f32;
        for col in 0..w {
            sum += acc[row * stride + col];
            out[row * w + col] = sum.abs().min(1.0);
        }
    }
    out
}

fn blit_coverage(
    fb: &mut Framebuffer,
    g: &CachedGlyph,
    x: i32,
    y: i32,
    color: Rgb,
    scale: u8,
    gamma: f32,
) {
    for row in 0..g.metrics.height {
        for col in 0..g.metrics.width {
            let a = g.bitmap[row * g.metrics.width + col];
            if a == 0 {
                continue;
            }
            let a = ((a as f32 / 255.0).powf(gamma) * scale as f32).round() as u8;
            let dst = fb.get(x + col as i32, y + row as i32);
            fb.put(x + col as i32, y + row as i32, dst.blend(color, a));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face() -> Option<UiFont> {
        let root = std::env::var("SC_INSTALL").ok()?;
        let install = crate::retail::RetailInstall::open(root).ok()?;
        let cache = install.default_cache_dir();
        install.ensure_cache(&cache).ok()?;
        UiFont::load_retail(&cache)
    }

    #[test]
    fn retail_face_rasterises_and_measures() {
        let Some(mut font) = face() else {
            eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
            return;
        };
        let w = font.measure("Single Player", 48.0, 1.0);
        assert!(w > 200, "unexpected width {w}");
        let mut fb = Framebuffer::new(800, 120);
        font.draw_top(
            &mut fb,
            10,
            10,
            "Single Player",
            48.0,
            1.0,
            Rgb::new(105, 255, 105),
            None,
        );
        assert!(
            fb.pixels.iter().any(|p| *p != 0),
            "the face must put pixels on the framebuffer"
        );
    }

    #[test]
    fn cef_text_splits_first_letter_and_places_subpixel() {
        let Some(mut font) = face() else {
            eprintln!("skipping: set SC_INSTALL to a StarCraft installation");
            return;
        };
        let (first, rest) = (Rgb::new(255, 0, 0), Rgb::new(0, 255, 0));
        let mut fb = Framebuffer::new(200, 40);
        font.draw_cef(&mut fb, 10.0, 30.0, "Speed", 23.8, 0.0, first, rest);
        let cols = |want: fn(u32) -> bool| -> Vec<i32> {
            (0..200)
                .filter(|&x| (0..40).any(|y| want(fb.get(x, y).0)))
                .collect()
        };
        let red = cols(|p| (p >> 16) & 0xff > 128 && (p >> 8) & 0xff < 64);
        let green = cols(|p| (p >> 8) & 0xff > 128 && (p >> 16) & 0xff < 64);
        assert!(!red.is_empty() && !green.is_empty());
        assert!(red.iter().max() < green.iter().max(), "only the S is red");
        // A half-pixel pen shift changes the coverage (no integer snapping).
        let mut a = Framebuffer::new(200, 40);
        let mut b = Framebuffer::new(200, 40);
        font.draw_cef(&mut a, 10.0, 30.0, "l", 23.8, 0.0, rest, rest);
        font.draw_cef(&mut b, 10.5, 30.0, "l", 23.8, 0.0, rest, rest);
        assert_ne!(a.pixels, b.pixels, "sub-pixel pen must be honoured");
        assert!(
            (font.measure_cef("Speed", 23.8, 0.0) - font.measure_cef("Speed", 23.8, 1.0)).abs()
                > 3.9
        );
    }

    /// Closed axis-aligned rectangle as four segments.
    fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<[f32; 4]> {
        vec![
            [x0, y0, x1, y0],
            [x1, y0, x1, y1],
            [x1, y1, x0, y1],
            [x0, y1, x0, y0],
        ]
    }

    #[test]
    fn exact_area_coverage_of_a_subpixel_square() {
        // A 2x2 square at (0.5, 0.5): corners get 1/4, edges 1/2, centre 1.
        let c = raster_lines(&rect(0.5, 0.5, 2.5, 2.5), 4, 4);
        let at = |x: usize, y: usize| c[y * 4 + x];
        assert!((at(0, 0) - 0.25).abs() < 1e-4, "corner {}", at(0, 0));
        assert!((at(1, 0) - 0.5).abs() < 1e-4, "edge {}", at(1, 0));
        assert!((at(1, 1) - 1.0).abs() < 1e-4, "centre {}", at(1, 1));
        assert!((at(2, 2) - 0.25).abs() < 1e-4);
        assert_eq!(at(3, 3), 0.0);
        let total: f32 = c.iter().sum();
        assert!((total - 4.0).abs() < 1e-3, "area {total}");
    }

    #[test]
    fn coverage_is_nonzero_with_counters_left_open() {
        // An outer square with an inner counter wound the other way (an
        // "o"): the counter must stay empty, the ring full.
        let mut lines = rect(0.0, 0.0, 6.0, 6.0);
        lines.extend(rect(2.0, 4.0, 4.0, 2.0)); // reversed winding
        let c = raster_lines(&lines, 6, 6);
        assert!((c[6 + 1] - 1.0).abs() < 1e-4, "ring");
        assert!(
            c[2 * 6 + 2].abs() < 1e-4,
            "counter must be open: {}",
            c[2 * 6 + 2]
        );
    }

    #[test]
    fn diagonal_edge_splits_area_exactly() {
        // Triangle (0,0)-(2,0)-(0,2): area 2 distributed over 4 cells.
        let lines = vec![
            [0.0, 0.0, 2.0, 0.0],
            [2.0, 0.0, 0.0, 2.0],
            [0.0, 2.0, 0.0, 0.0],
        ];
        let c = raster_lines(&lines, 3, 3);
        let total: f32 = c.iter().sum();
        assert!((total - 2.0).abs() < 1e-3, "area {total}");
        assert!((c[0] - 1.0).abs() < 1e-4);
        assert!((c[1] - 0.5).abs() < 1e-4 && (c[3] - 0.5).abs() < 1e-4);
    }

    #[test]
    fn missing_face_is_not_an_error() {
        assert!(UiFont::load(std::path::Path::new("/definitely/not/a/font.ttf")).is_none());
        assert!(UiFont::load_retail(std::path::Path::new("/definitely/not/a/cache")).is_none());
    }
}

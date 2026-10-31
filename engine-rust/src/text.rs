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
}

struct CachedGlyph {
    metrics: fontdue::Metrics,
    bitmap: Vec<u8>,
}

impl UiFont {
    /// Load a TrueType/OpenType face from an on-disk file.
    pub fn load(path: &std::path::Path) -> Option<Self> {
        let bytes = std::fs::read(path).ok()?;
        let font = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()?;
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("font")
            .to_string();
        Some(Self {
            font,
            cache: HashMap::new(),
            name,
        })
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
    fn missing_face_is_not_an_error() {
        assert!(UiFont::load(std::path::Path::new("/definitely/not/a/font.ttf")).is_none());
        assert!(UiFont::load_retail(std::path::Path::new("/definitely/not/a/cache")).is_none());
    }
}

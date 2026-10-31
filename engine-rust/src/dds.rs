//! DirectDraw Surface decoding for the Remastered glue/console art.
//!
//! StarCraft Remastered ships its menus, briefing backdrops, score panels and
//! console plates as `.dds` files in CASC.  The layouts were recovered from
//! the shipped `locales/enus/assets/rez/*.ui.json` dialogs and the art itself
//! is DXT1/DXT3/DXT5 (plus the occasional uncompressed ARGB surface).  This
//! module decodes those files into the same ARGB pixels the software
//! framebuffer uses, so the client can draw the real retail screens instead
//! of approximating them.
//!
//! Nothing here depends on the installation: the decoder is a pure function
//! of the file bytes and is covered by synthetic DXT1/DXT5/uncompressed
//! cases in the unit tests.

/// Decoded surface, row-major ARGB (`0xAARRGGBB`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DdsImage {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>,
}

impl DdsImage {
    #[inline]
    pub fn pixel(&self, x: usize, y: usize) -> u32 {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x]
        } else {
            0
        }
    }

    /// Bilinear resample to `width`x`height`.  Used once when a briefing or
    /// score backdrop is loaded; the result is cached for the whole screen.
    pub fn resize(&self, width: usize, height: usize) -> DdsImage {
        if width == 0 || height == 0 || self.width == 0 || self.height == 0 {
            return DdsImage {
                width,
                height,
                pixels: vec![0; width * height],
            };
        }
        let mut out = vec![0u32; width * height];
        for y in 0..height {
            // Map pixel centres, clamped at the edges.
            let sy = ((y as f32 + 0.5) * self.height as f32 / height as f32 - 0.5)
                .clamp(0.0, self.height as f32 - 1.0);
            let y0 = sy.floor() as usize;
            let y1 = (y0 + 1).min(self.height - 1);
            let fy = sy - y0 as f32;
            for x in 0..width {
                let sx = ((x as f32 + 0.5) * self.width as f32 / width as f32 - 0.5)
                    .clamp(0.0, self.width as f32 - 1.0);
                let x0 = sx.floor() as usize;
                let x1 = (x0 + 1).min(self.width - 1);
                let fx = sx - x0 as f32;
                let p00 = self.pixels[y0 * self.width + x0];
                let p10 = self.pixels[y0 * self.width + x1];
                let p01 = self.pixels[y1 * self.width + x0];
                let p11 = self.pixels[y1 * self.width + x1];
                let channel = |shift: u32| -> u32 {
                    let c = |p: u32| ((p >> shift) & 0xff) as f32;
                    let top = c(p00) * (1.0 - fx) + c(p10) * fx;
                    let bottom = c(p01) * (1.0 - fx) + c(p11) * fx;
                    (top * (1.0 - fy) + bottom * fy).round().clamp(0.0, 255.0) as u32
                };
                let a = channel(24);
                let r = channel(16);
                let g = channel(8);
                let b = channel(0);
                out[y * width + x] = (a << 24) | (r << 16) | (g << 8) | b;
            }
        }
        DdsImage {
            width,
            height,
            pixels: out,
        }
    }

    /// Crop-style cover fit into `width`x`height`, preserving aspect ratio.
    pub fn cover(&self, width: usize, height: usize) -> DdsImage {
        if self.width == 0 || self.height == 0 {
            return DdsImage {
                width,
                height,
                pixels: vec![0; width * height],
            };
        }
        let scale = (width as f32 / self.width as f32).max(height as f32 / self.height as f32);
        let scaled_w = (self.width as f32 * scale).ceil() as usize;
        let scaled_h = (self.height as f32 * scale).ceil() as usize;
        let scaled = self.resize(scaled_w.max(1), scaled_h.max(1));
        let ox = (scaled_w.saturating_sub(width)) / 2;
        let oy = (scaled_h.saturating_sub(height)) / 2;
        let mut out = vec![0u32; width * height];
        for y in 0..height {
            for x in 0..width {
                out[y * width + x] = scaled.pixels[(y + oy) * scaled_w + x + ox];
            }
        }
        DdsImage {
            width,
            height,
            pixels: out,
        }
    }

    /// Draw the surface with its alpha channel blended into `fb`.
    pub fn draw(&self, fb: &mut crate::render::Framebuffer, x: i32, y: i32) {
        self.draw_scaled(fb, x, y, self.width as i32, self.height as i32);
    }

    /// Fast 1:1 alpha blit (no resampling).
    pub fn blit(&self, fb: &mut crate::render::Framebuffer, x: i32, y: i32) {
        for sy in 0..self.height {
            for sx in 0..self.width {
                let p = self.pixels[sy * self.width + sx];
                let a = p >> 24;
                if a == 0 {
                    continue;
                }
                let src = crate::render::Rgb::new(
                    ((p >> 16) & 0xff) as u8,
                    ((p >> 8) & 0xff) as u8,
                    (p & 0xff) as u8,
                );
                let dst = fb.get(x + sx as i32, y + sy as i32);
                fb.put(x + sx as i32, y + sy as i32, dst.blend(src, a as u8));
            }
        }
    }

    /// Bilinear scale from a source sub-rectangle into `dest`.
    ///
    /// The console plates are wider than the band the retail client shows; the
    /// right plate is drawn cropped to the region that carries its button
    /// wells so they line up with the command grid.
    pub fn draw_sub(
        &self,
        fb: &mut crate::render::Framebuffer,
        src: (i32, i32, i32, i32),
        dest: (i32, i32, i32, i32),
    ) {
        let (sx0, sy0, sw, sh) = src;
        let (dx0, dy0, dw, dh) = dest;
        if sw <= 0 || sh <= 0 || dw <= 0 || dh <= 0 {
            return;
        }
        for dy in 0..dh {
            let sy = sy0 + (dy as f32 * sh as f32 / dh as f32) as i32;
            if sy < 0 || sy as usize >= self.height {
                continue;
            }
            for dx in 0..dw {
                let sx = sx0 + (dx as f32 * sw as f32 / dw as f32) as i32;
                if sx < 0 || sx as usize >= self.width {
                    continue;
                }
                let p = self.pixels[sy as usize * self.width + sx as usize];
                let a = p >> 24;
                if a == 0 {
                    continue;
                }
                let c = crate::render::Rgb::new(
                    ((p >> 16) & 0xff) as u8,
                    ((p >> 8) & 0xff) as u8,
                    (p & 0xff) as u8,
                );
                let dst = fb.get(dx0 + dx, dy0 + dy);
                fb.put(dx0 + dx, dy0 + dy, dst.blend(c, a as u8));
            }
        }
    }

    /// Bilinear scale + alpha blend.  `w`/`h` are the destination extent;
    /// transparent source texels leave the framebuffer untouched.
    ///
    /// The sampler uses 8-bit fixed-point weights and writes straight into
    /// the destination row: this is the per-frame title/chrome path, so the
    /// float-per-pixel version was the menu's main frame-time cost.
    pub fn draw_scaled(&self, fb: &mut crate::render::Framebuffer, x: i32, y: i32, w: i32, h: i32) {
        if self.width == 0 || self.height == 0 || w <= 0 || h <= 0 {
            return;
        }
        let mut xs = Vec::with_capacity(w as usize);
        for dx in 0..w {
            let sx = ((dx as f32 + 0.5) * self.width as f32 / w as f32 - 0.5)
                .clamp(0.0, self.width as f32 - 1.0);
            let x0 = sx.floor() as usize;
            let x1 = (x0 + 1).min(self.width - 1);
            let fx = ((sx - x0 as f32) * 256.0).round() as u32;
            xs.push((x0, x1, fx.min(256)));
        }
        let stride = fb.width as usize;
        let fw = fb.width;
        let fh = fb.height;
        #[inline]
        fn channel(p00: u32, p10: u32, p01: u32, p11: u32, fx: u32, fy: u32, shift: u32) -> u32 {
            let c = |p: u32| (p >> shift) & 0xff;
            let top = c(p00) * (256 - fx) + c(p10) * fx;
            let bottom = c(p01) * (256 - fx) + c(p11) * fx;
            (top * (256 - fy) + bottom * fy + 32_768) >> 16
        }
        for dy in 0..h {
            let oy = y + dy;
            if oy < 0 || oy >= fh {
                continue;
            }
            let sy = ((dy as f32 + 0.5) * self.height as f32 / h as f32 - 0.5)
                .clamp(0.0, self.height as f32 - 1.0);
            let y0 = sy.floor() as usize;
            let y1 = (y0 + 1).min(self.height - 1);
            let fy = ((sy - y0 as f32) * 256.0).round() as u32;
            let fy = fy.min(256);
            let src_row0 = y0 * self.width;
            let src_row1 = y1 * self.width;
            let dst_row = &mut fb.pixels[oy as usize * stride..];
            for (dx, &(x0, x1, fx)) in xs.iter().enumerate() {
                let ox = x + dx as i32;
                if ox < 0 || ox >= fw {
                    continue;
                }
                let p00 = self.pixels[src_row0 + x0];
                let p10 = self.pixels[src_row0 + x1];
                let p01 = self.pixels[src_row1 + x0];
                let p11 = self.pixels[src_row1 + x1];
                let a = channel(p00, p10, p01, p11, fx, fy, 24);
                if a == 0 {
                    continue;
                }
                let r = channel(p00, p10, p01, p11, fx, fy, 16) as u8;
                let g = channel(p00, p10, p01, p11, fx, fy, 8) as u8;
                let b = channel(p00, p10, p01, p11, fx, fy, 0) as u8;
                let dst = crate::render::Rgb(dst_row[ox as usize]);
                dst_row[ox as usize] = dst.blend(crate::render::Rgb::new(r, g, b), a as u8).0;
            }
        }
    }
}

const DDS_MAGIC: &[u8; 4] = b"DDS ";
const DDPF_FOURCC: u32 = 0x4;
const DDPF_RGB: u32 = 0x40;
const FOURCC_DXT1: u32 = u32::from_le_bytes(*b"DXT1");
const FOURCC_DXT3: u32 = u32::from_le_bytes(*b"DXT3");
const FOURCC_DXT5: u32 = u32::from_le_bytes(*b"DXT5");

fn u32_at(bytes: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *bytes.get(o)?,
        *bytes.get(o + 1)?,
        *bytes.get(o + 2)?,
        *bytes.get(o + 3)?,
    ]))
}

#[inline]
fn rgb565(c: u16) -> (u8, u8, u8) {
    let r = ((c >> 11) & 0x1f) as u32;
    let g = ((c >> 5) & 0x3f) as u32;
    let b = (c & 0x1f) as u32;
    (
        ((r * 255 + 15) / 31) as u8,
        ((g * 255 + 31) / 63) as u8,
        ((b * 255 + 15) / 31) as u8,
    )
}

#[inline]
fn lerp(a: u8, b: u8, num: u32, den: u32) -> u8 {
    ((a as u32 * (den - num) + b as u32 * num) / den) as u8
}

/// Decode the 8-byte DXT1 colour block for pixel (`px`, `py`) inside a 4x4
/// block, returning `None` for DXT1's transparent selector.
#[inline]
fn dxt1_color(block: &[u8], px: usize, py: usize) -> Option<(u8, u8, u8, u8)> {
    if block.len() < 8 {
        return None;
    }
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    let idx = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
    let bit = (py * 4 + px) * 2;
    let sel = ((idx >> bit) & 3) as usize;
    let (r0, g0, b0) = rgb565(c0);
    let (r1, g1, b1) = rgb565(c1);
    let (r, g, b, a) = if c0 > c1 {
        match sel {
            0 => (r0, g0, b0, 255),
            1 => (r1, g1, b1, 255),
            2 => (
                lerp(r0, r1, 1, 3),
                lerp(g0, g1, 1, 3),
                lerp(b0, b1, 1, 3),
                255,
            ),
            _ => (
                lerp(r0, r1, 2, 3),
                lerp(g0, g1, 2, 3),
                lerp(b0, b1, 2, 3),
                255,
            ),
        }
    } else {
        match sel {
            0 => (r0, g0, b0, 255),
            1 => (r1, g1, b1, 255),
            2 => (
                ((r0 as u32 + r1 as u32) / 2) as u8,
                ((g0 as u32 + g1 as u32) / 2) as u8,
                ((b0 as u32 + b1 as u32) / 2) as u8,
                255,
            ),
            _ => (0, 0, 0, 0),
        }
    };
    Some((r, g, b, a))
}

#[inline]
fn dxt5_alpha(block: &[u8], px: usize, py: usize) -> u8 {
    let a0 = block[0];
    let a1 = block[1];
    let mut bits = 0u64;
    for i in 0..6 {
        bits |= (block[2 + i] as u64) << (8 * i);
    }
    let sel = ((bits >> ((py * 4 + px) * 3)) & 7) as u32;
    if a0 > a1 {
        match sel {
            0 => a0,
            1 => a1,
            n => {
                // a2..a7 = (6..1)*a0 + (1..6)*a1, divided by 7.
                (((8 - n) * a0 as u32 + (n - 1) * a1 as u32) / 7) as u8
            }
        }
    } else {
        match sel {
            0 => a0,
            1 => a1,
            2..=5 => {
                // a2..a5 = (4..1)*a0 + (1..4)*a1, divided by 5.
                (((6 - sel) * a0 as u32 + (sel - 1) * a1 as u32) / 5) as u8
            }
            6 => 0,
            _ => 255,
        }
    }
}

#[inline]
fn dxt3_alpha(block: &[u8], px: usize, py: usize) -> u8 {
    let byte = block[(py * 4 + px) / 2];
    let nibble = if (py * 4 + px).is_multiple_of(2) {
        byte & 0x0f
    } else {
        byte >> 4
    };
    (nibble as u32 * 17) as u8
}

/// Parse a `.dds` surface.  Returns `None` for malformed or unsupported
/// formats rather than guessing.
pub fn parse(bytes: &[u8]) -> Option<DdsImage> {
    if bytes.len() < 128 || &bytes[0..4] != DDS_MAGIC {
        return None;
    }
    let height = u32_at(bytes, 12)? as usize;
    let width = u32_at(bytes, 16)? as usize;
    if width == 0 || height == 0 || width > 16_384 || height > 16_384 {
        return None;
    }
    let pf = 76;
    let pf_flags = u32_at(bytes, pf + 4)?;
    let fourcc = u32_at(bytes, pf + 8)?;
    let rgb_bits = u32_at(bytes, pf + 12)?;
    let r_mask = u32_at(bytes, pf + 16)?;
    let g_mask = u32_at(bytes, pf + 20)?;
    let b_mask = u32_at(bytes, pf + 24)?;
    let a_mask = u32_at(bytes, pf + 28)?;
    let data = &bytes[128..];
    let mut out = vec![0u32; width * height];

    if pf_flags & DDPF_FOURCC != 0 {
        let block_size = match fourcc {
            FOURCC_DXT1 => 8,
            FOURCC_DXT3 | FOURCC_DXT5 => 16,
            _ => return None,
        };
        let blocks_x = width.div_ceil(4);
        let blocks_y = height.div_ceil(4);
        if data.len() < blocks_x * blocks_y * block_size {
            return None;
        }
        for by in 0..blocks_y {
            for bx in 0..blocks_x {
                let o = (by * blocks_x + bx) * block_size;
                let block = &data[o..o + block_size];
                for py in 0..4 {
                    let y = by * 4 + py;
                    if y >= height {
                        continue;
                    }
                    for px in 0..4 {
                        let x = bx * 4 + px;
                        if x >= width {
                            continue;
                        }
                        let color = match fourcc {
                            FOURCC_DXT1 => dxt1_color(block, px, py).unwrap_or((0, 0, 0, 0)),
                            FOURCC_DXT3 => {
                                let (r, g, b, _) =
                                    dxt1_color(&block[8..], px, py).unwrap_or((0, 0, 0, 255));
                                (r, g, b, dxt3_alpha(block, px, py))
                            }
                            _ => {
                                let (r, g, b, _) =
                                    dxt1_color(&block[8..], px, py).unwrap_or((0, 0, 0, 255));
                                (r, g, b, dxt5_alpha(block, px, py))
                            }
                        };
                        out[y * width + x] = ((color.3 as u32) << 24)
                            | ((color.0 as u32) << 16)
                            | ((color.1 as u32) << 8)
                            | color.2 as u32;
                    }
                }
            }
        }
        return Some(DdsImage {
            width,
            height,
            pixels: out,
        });
    }

    if pf_flags & DDPF_RGB == 0 || rgb_bits == 0 {
        return None;
    }
    let bytes_per_pixel = (rgb_bits / 8) as usize;
    let need = width * height * bytes_per_pixel;
    if data.len() < need || !(1..=4).contains(&bytes_per_pixel) {
        return None;
    }
    let extract = |value: u32, mask: u32| -> u8 {
        if mask == 0 {
            return 255;
        }
        let shift = mask.trailing_zeros();
        let bits = mask.count_ones();
        let raw = (value & mask) >> shift;
        if bits >= 8 {
            (raw >> (bits - 8)) as u8
        } else {
            ((raw * 255) / ((1u32 << bits) - 1)) as u8
        }
    };
    for y in 0..height {
        for x in 0..width {
            let o = (y * width + x) * bytes_per_pixel;
            let mut value = 0u32;
            for i in 0..bytes_per_pixel {
                value |= (data[o + i] as u32) << (8 * i);
            }
            let r = extract(value, r_mask);
            let g = extract(value, g_mask);
            let b = extract(value, b_mask);
            let a = if a_mask == 0 {
                255
            } else {
                extract(value, a_mask)
            };
            out[y * width + x] =
                ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | b as u32;
        }
    }
    Some(DdsImage {
        width,
        height,
        pixels: out,
    })
}

/// A Remastered `.dds.grp` archive: one embedded DDS surface per frame.
#[derive(Clone, Debug, Default)]
pub struct DdsGrp {
    pub frames: Vec<DdsImage>,
}

impl DdsGrp {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let total = u32::from_le_bytes(bytes.get(0..4)?.try_into().ok()?) as usize;
        let count = u16::from_le_bytes(bytes.get(4..6)?.try_into().ok()?) as usize;
        if count == 0 || count > 4096 || total > bytes.len() {
            return None;
        }
        let mut frames = Vec::with_capacity(count);
        let mut off = 8usize;
        for _ in 0..count {
            let w = u16::from_le_bytes(bytes.get(off + 4..off + 6)?.try_into().ok()?) as usize;
            let h = u16::from_le_bytes(bytes.get(off + 6..off + 8)?.try_into().ok()?) as usize;
            let size = u32::from_le_bytes(bytes.get(off + 8..off + 12)?.try_into().ok()?) as usize;
            let start = off + 12;
            let end = start.checked_add(size)?;
            let slice = bytes.get(start..end)?;
            let mut image = parse(slice)?;
            // The record dimensions are authoritative for the button/portrait
            // slot; some HD frames include padding in the DDS header.
            if image.width != w || image.height != h {
                image = image.resize(w.max(1), h.max(1));
            }
            frames.push(image);
            off = end;
        }
        Some(Self { frames })
    }

    pub fn frame(&self, index: usize) -> Option<&DdsImage> {
        if self.frames.is_empty() {
            None
        } else {
            Some(&self.frames[index.min(self.frames.len() - 1)])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dds_header(width: u32, height: u32, fourcc: u32, flags: u32, bits: u32) -> Vec<u8> {
        let mut h = vec![0u8; 128];
        h[0..4].copy_from_slice(DDS_MAGIC);
        h[4..8].copy_from_slice(&124u32.to_le_bytes());
        h[12..16].copy_from_slice(&height.to_le_bytes());
        h[16..20].copy_from_slice(&width.to_le_bytes());
        h[76..80].copy_from_slice(&32u32.to_le_bytes());
        h[80..84].copy_from_slice(&flags.to_le_bytes());
        h[84..88].copy_from_slice(&fourcc.to_le_bytes());
        h[88..92].copy_from_slice(&bits.to_le_bytes());
        h
    }

    #[test]
    fn dxt1_decodes_four_colour_and_transparent_modes() {
        let mut bytes = dds_header(4, 4, FOURCC_DXT1, DDPF_FOURCC, 0);
        // c0 white, c1 black, c0 > c1: 4-colour mode.  Pixel selectors 0,1,2,3.
        bytes.extend_from_slice(&0xffffu16.to_le_bytes());
        bytes.extend_from_slice(&0x0000u16.to_le_bytes());
        // indices 0,1,2,3 repeated across the 16 pixels (2 bits each).
        bytes.extend_from_slice(&0b11100100_11100100u32.to_le_bytes());
        let img = parse(&bytes).unwrap();
        assert_eq!((img.width, img.height), (4, 4));
        assert_eq!(img.pixel(0, 0), 0xffff_ffff);
        assert_eq!(img.pixel(1, 0), 0xff00_0000);
        assert_eq!(img.pixel(2, 0), 0xffaa_aaaa);
        assert_eq!(img.pixel(3, 0), 0xff55_5555);

        // c0 <= c1: selector 3 is transparent black.
        let mut bytes = dds_header(4, 4, FOURCC_DXT1, DDPF_FOURCC, 0);
        bytes.extend_from_slice(&0x0000u16.to_le_bytes());
        bytes.extend_from_slice(&0xffffu16.to_le_bytes());
        bytes.extend_from_slice(&0xffff_ffffu32.to_le_bytes());
        let img = parse(&bytes).unwrap();
        assert_eq!(img.pixel(0, 0), 0x0000_0000);
    }

    #[test]
    fn dxt5_decodes_eight_level_alpha() {
        let mut bytes = dds_header(4, 4, FOURCC_DXT5, DDPF_FOURCC, 0);
        // alpha endpoints 255 and 0; selector 0 selects a0 (=255).
        bytes.extend_from_slice(&[255, 0]);
        bytes.extend_from_slice(&[0x00; 6]);
        bytes.extend_from_slice(&0x0000u16.to_le_bytes());
        bytes.extend_from_slice(&0x0000u16.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        let img = parse(&bytes).unwrap();
        assert_eq!(img.pixel(0, 0) >> 24, 0xff);
        assert_eq!(img.pixel(3, 3) >> 24, 0xff);
    }

    #[test]
    fn uncompressed_argb_is_read_with_its_masks() {
        let mut bytes = dds_header(2, 1, 0, DDPF_RGB | 0x1, 32);
        // Masks: A8R8G8B8.
        bytes[92..96].copy_from_slice(&0x00ff_0000u32.to_le_bytes());
        bytes[96..100].copy_from_slice(&0x0000_ff00u32.to_le_bytes());
        bytes[100..104].copy_from_slice(&0x0000_00ffu32.to_le_bytes());
        bytes[104..108].copy_from_slice(&0xff00_0000u32.to_le_bytes());
        bytes.extend_from_slice(&0x8012_3456u32.to_le_bytes());
        bytes.extend_from_slice(&0xff65_4321u32.to_le_bytes());
        let img = parse(&bytes).unwrap();
        assert_eq!(img.pixel(0, 0), 0x8012_3456);
        assert_eq!(img.pixel(1, 0), 0xff65_4321);
    }

    #[test]
    fn cover_preserves_aspect_and_crops_the_long_axis() {
        let img = DdsImage {
            width: 4,
            height: 2,
            pixels: (0..8).map(|i| 0xff00_0000 | i).collect(),
        };
        let out = img.cover(2, 2);
        assert_eq!((out.width, out.height), (2, 2));
        // 4x2 -> 4x4 then centre crop: rows 1..3.
        assert_eq!(out.pixel(0, 0), 0xff00_0001);
        assert_eq!(out.pixel(1, 1), 0xff00_0006);
    }

    #[test]
    fn dds_grp_reads_the_embedded_frame_surfaces() {
        let mut dds = dds_header(4, 4, FOURCC_DXT1, DDPF_FOURCC, 0);
        dds.extend_from_slice(&0xffffu16.to_le_bytes());
        dds.extend_from_slice(&0x0000u16.to_le_bytes());
        dds.extend_from_slice(&0u32.to_le_bytes());
        let mut grp = Vec::new();
        grp.extend_from_slice(&(8u32 + 12 + dds.len() as u32).to_le_bytes());
        grp.extend_from_slice(&1u16.to_le_bytes());
        grp.extend_from_slice(&0x1004u16.to_le_bytes());
        grp.extend_from_slice(&0u32.to_le_bytes());
        grp.extend_from_slice(&4u16.to_le_bytes());
        grp.extend_from_slice(&4u16.to_le_bytes());
        grp.extend_from_slice(&(dds.len() as u32).to_le_bytes());
        grp.extend_from_slice(&dds);
        let parsed = DdsGrp::parse(&grp).expect("dds.grp");
        assert_eq!(parsed.frames.len(), 1);
        assert_eq!((parsed.frames[0].width, parsed.frames[0].height), (4, 4));
        assert_eq!(parsed.frames[0].pixel(0, 0), 0xffff_ffff);
    }

    #[test]
    fn draw_scaled_one_to_one_preserves_rgba_pixels() {
        let img = DdsImage {
            width: 2,
            height: 1,
            pixels: vec![0xffff_0000, 0xff00_ff00],
        };
        let mut fb = crate::render::Framebuffer::new(2, 1);
        img.draw_scaled(&mut fb, 0, 0, 2, 1);
        assert_eq!(fb.pixels, vec![0x00ff_0000, 0x0000_ff00]);
    }
}

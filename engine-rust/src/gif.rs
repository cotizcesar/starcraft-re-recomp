//! Minimal GIF89a decoder for the retail CEF images.
//!
//! The Remastered front end draws a few animated GIFs from
//! `webui/dist/lib/images/` (the main-menu panel's `loadingcircle.gif`
//! spinner: 650x651, 30 frames of 30 ms, one global palette, sub-rectangle
//! frames with disposal 1).  This decodes the subset those files use:
//! global/local palettes, the Graphic Control Extension (delay,
//! transparency, disposal 0/1/2), non-interlaced images and LZW data.
//! Frames are composed onto a canvas, so every returned frame is the full
//! picture the browser would show.
//!
//! Pure function of the bytes; malformed or unsupported input returns
//! `None`.

use crate::dds::DdsImage;

/// Largest canvas accepted, checked before allocating.
const MAX_PIXELS: usize = 4096 * 4096;
/// Largest number of frames accepted.
const MAX_FRAMES: usize = 1024;

/// One composed animation frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GifFrame {
    pub image: DdsImage,
    /// Display time in milliseconds (the GCE delay, in 1/100 s units).
    pub delay_ms: u32,
}

/// A decoded animation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gif {
    pub width: usize,
    pub height: usize,
    pub frames: Vec<GifFrame>,
}

impl Gif {
    /// Total loop length in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        self.frames.iter().map(|f| f.delay_ms as u64).sum()
    }

    /// Frame shown `elapsed_ms` after the animation started (it loops).
    pub fn frame_at(&self, elapsed_ms: u64) -> Option<&GifFrame> {
        let total = self.duration_ms();
        if total == 0 {
            return self.frames.first();
        }
        let mut t = elapsed_ms % total;
        for f in &self.frames {
            if t < f.delay_ms as u64 {
                return Some(f);
            }
            t -= f.delay_ms as u64;
        }
        self.frames.last()
    }
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn u8(&mut self) -> Option<u8> {
        let b = *self.data.get(self.at)?;
        self.at += 1;
        Some(b)
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from(self.u8()?) | (u16::from(self.u8()?) << 8))
    }
    fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.data.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        Some(s)
    }
    /// Concatenated data sub-blocks up to the zero terminator.
    fn sub_blocks(&mut self) -> Option<Vec<u8>> {
        let mut out = Vec::new();
        loop {
            let n = self.u8()? as usize;
            if n == 0 {
                return Some(out);
            }
            out.extend_from_slice(self.bytes(n)?);
        }
    }
}

fn palette(bytes: &[u8]) -> Vec<u32> {
    bytes
        .as_chunks::<3>()
        .0
        .iter()
        .map(|c| 0xff00_0000 | (u32::from(c[0]) << 16) | (u32::from(c[1]) << 8) | u32::from(c[2]))
        .collect()
}

/// Variable-width LZW as used by GIF.  Returns at most `limit` indices.
fn lzw_decode(min_code: u8, data: &[u8], limit: usize) -> Option<Vec<u8>> {
    if !(1..=11).contains(&min_code) {
        return None;
    }
    let clear = 1usize << min_code;
    let end = clear + 1;
    // Each entry: (prefix code, last byte, length).
    let mut table: Vec<(u16, u8, u16)> = Vec::with_capacity(4096);
    let reset = |table: &mut Vec<(u16, u8, u16)>| {
        table.clear();
        for i in 0..clear {
            table.push((u16::MAX, i as u8, 1));
        }
        table.push((u16::MAX, 0, 0));
        table.push((u16::MAX, 0, 0));
    };
    reset(&mut table);
    let mut width = min_code as u32 + 1;
    let mut out = Vec::with_capacity(limit);
    let mut prev: Option<usize> = None;
    let (mut acc, mut bits, mut pos) = (0u32, 0u32, 0usize);
    let mut scratch = Vec::new();
    loop {
        while bits < width {
            let Some(&b) = data.get(pos) else {
                return Some(out);
            };
            acc |= u32::from(b) << bits;
            bits += 8;
            pos += 1;
        }
        let code = (acc & ((1 << width) - 1)) as usize;
        acc >>= width;
        bits -= width;
        if code == clear {
            reset(&mut table);
            width = min_code as u32 + 1;
            prev = None;
            continue;
        }
        if code == end {
            return Some(out);
        }
        let emit_from = if code < table.len() {
            code
        } else if code == table.len() {
            prev?
        } else {
            return None;
        };
        // Expand `emit_from` into scratch (reversed walk of prefixes).
        scratch.clear();
        let mut c = emit_from;
        loop {
            let (p, b, _) = table[c];
            scratch.push(b);
            if p == u16::MAX {
                break;
            }
            c = p as usize;
        }
        scratch.reverse();
        let first = scratch[0];
        if code == table.len() {
            scratch.push(first);
        }
        out.extend_from_slice(&scratch);
        if out.len() >= limit {
            out.truncate(limit);
            return Some(out);
        }
        if let Some(p) = prev {
            if table.len() < 4096 {
                let len = table[p].2.saturating_add(1);
                table.push((p as u16, first, len));
                if table.len() == (1 << width) && width < 12 {
                    width += 1;
                }
            }
        }
        prev = Some(code);
    }
}

/// Decode a GIF87a/89a file into composed RGBA frames.
pub fn decode(bytes: &[u8]) -> Option<Gif> {
    let mut r = Reader { data: bytes, at: 0 };
    let sig = r.bytes(6)?;
    if sig != b"GIF89a" && sig != b"GIF87a" {
        return None;
    }
    let width = r.u16()? as usize;
    let height = r.u16()? as usize;
    if width == 0 || height == 0 || width * height > MAX_PIXELS {
        return None;
    }
    let flags = r.u8()?;
    let _background = r.u8()?;
    let _aspect = r.u8()?;
    let global = if flags & 0x80 != 0 {
        palette(r.bytes(3 * (2usize << (flags & 7)))?)
    } else {
        Vec::new()
    };
    let mut canvas = vec![0u32; width * height];
    let mut frames = Vec::new();
    let (mut delay_ms, mut transparent, mut disposal) = (0u32, None::<u8>, 0u8);
    loop {
        match r.u8()? {
            0x21 => {
                let label = r.u8()?;
                let block = r.sub_blocks()?;
                if label == 0xf9 && block.len() >= 4 {
                    disposal = (block[0] >> 2) & 7;
                    delay_ms = (u32::from(block[1]) | (u32::from(block[2]) << 8)) * 10;
                    transparent = (block[0] & 1 != 0).then_some(block[3]);
                }
            }
            0x2c => {
                let fx = r.u16()? as usize;
                let fy = r.u16()? as usize;
                let fw = r.u16()? as usize;
                let fh = r.u16()? as usize;
                let iflags = r.u8()?;
                if iflags & 0x40 != 0 || fx + fw > width || fy + fh > height {
                    return None; // interlaced or out-of-canvas: unsupported
                }
                let local = if iflags & 0x80 != 0 {
                    Some(palette(r.bytes(3 * (2usize << (iflags & 7)))?))
                } else {
                    None
                };
                let pal = local.as_ref().unwrap_or(&global);
                let min_code = r.u8()?;
                let data = r.sub_blocks()?;
                let indices = lzw_decode(min_code, &data, fw * fh)?;
                let before = (disposal == 2).then(|| canvas.clone());
                for (i, &ix) in indices.iter().enumerate() {
                    if Some(ix) == transparent {
                        continue;
                    }
                    let Some(&c) = pal.get(ix as usize) else {
                        continue;
                    };
                    canvas[(fy + i / fw) * width + fx + i % fw] = c;
                }
                frames.push(GifFrame {
                    image: DdsImage {
                        width,
                        height,
                        pixels: canvas.clone(),
                    },
                    delay_ms,
                });
                if frames.len() > MAX_FRAMES {
                    return None;
                }
                // Disposal 2: the frame's rectangle goes back to transparent
                // before the next frame (`before` keeps everything else).
                if let Some(mut restored) = before {
                    for y in fy..fy + fh {
                        for x in fx..fx + fw {
                            restored[y * width + x] = 0;
                        }
                    }
                    canvas = restored;
                }
                delay_ms = 0;
                transparent = None;
                disposal = 0;
            }
            0x3b => break,
            _ => return None,
        }
    }
    if frames.is_empty() {
        return None;
    }
    Some(Gif {
        width,
        height,
        frames,
    })
}

/// Area-average (box) resample with premultiplied alpha, for large
/// reductions where a bilinear tap would alias (650 -> 60 px).
pub fn downsample(src: &DdsImage, width: usize, height: usize) -> DdsImage {
    let mut pixels = vec![0u32; width * height];
    if src.width == 0 || src.height == 0 {
        return DdsImage {
            width,
            height,
            pixels,
        };
    }
    for y in 0..height {
        let y0 = y * src.height / height;
        let y1 = ((y + 1) * src.height / height).max(y0 + 1);
        for x in 0..width {
            let x0 = x * src.width / width;
            let x1 = ((x + 1) * src.width / width).max(x0 + 1);
            let (mut a, mut r, mut g, mut b, mut n) = (0u64, 0u64, 0u64, 0u64, 0u64);
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let p = src.pixels[sy * src.width + sx];
                    let pa = u64::from(p >> 24);
                    a += pa;
                    r += pa * u64::from((p >> 16) & 0xff);
                    g += pa * u64::from((p >> 8) & 0xff);
                    b += pa * u64::from(p & 0xff);
                    n += 1;
                }
            }
            if a > 0 {
                let out_a = (a / n) as u32;
                let c = |v: u64| ((v + a / 2) / a) as u32;
                pixels[y * width + x] = (out_a << 24) | (c(r) << 16) | (c(g) << 8) | c(b);
            }
        }
    }
    DdsImage {
        width,
        height,
        pixels,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2x2, two frames, palette [black, red, green, blue], index 0
    /// transparent.  Frame 0 fills the canvas red; frame 1 (disposal 1)
    /// paints a 1x1 green pixel at (1,1) over it.  LZW streams were made
    /// with min code size 2 (clear 4, end 5).
    fn sample() -> Vec<u8> {
        let mut g = b"GIF89a".to_vec();
        g.extend_from_slice(&[2, 0, 2, 0, 0x81, 0, 0]);
        g.extend_from_slice(&[0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255]);
        // GCE: disposal 1, delay 3 cs, transparent index 0.
        g.extend_from_slice(&[0x21, 0xf9, 4, 0x05, 3, 0, 0, 0]);
        g.extend_from_slice(&[0x2c, 0, 0, 0, 0, 2, 0, 2, 0, 0]);
        g.push(2);
        let data = lzw_encode_literals(2, &[1, 1, 1, 1]);
        g.push(data.len() as u8);
        g.extend_from_slice(&data);
        g.push(0);
        g.extend_from_slice(&[0x21, 0xf9, 4, 0x05, 5, 0, 0, 0]);
        g.extend_from_slice(&[0x2c, 1, 0, 1, 0, 1, 0, 1, 0, 0]);
        g.push(2);
        let data = lzw_encode_literals(2, &[2]);
        g.push(data.len() as u8);
        g.extend_from_slice(&data);
        g.push(0);
        g.push(0x3b);
        g
    }

    /// Reference encoder that only emits literal codes (with a clear code
    /// first, the end code last and the decoder's code-width growth).
    fn lzw_encode_literals(min_code: u8, indices: &[u8]) -> Vec<u8> {
        let clear = 1u32 << min_code;
        let mut width = u32::from(min_code) + 1;
        let mut next = clear + 2;
        let (mut acc, mut bits, mut out) = (0u64, 0u32, Vec::new());
        let put = |code: u32, width: u32, acc: &mut u64, bits: &mut u32, out: &mut Vec<u8>| {
            *acc |= u64::from(code) << *bits;
            *bits += width;
            while *bits >= 8 {
                out.push(*acc as u8);
                *acc >>= 8;
                *bits -= 8;
            }
        };
        put(clear, width, &mut acc, &mut bits, &mut out);
        for (i, &ix) in indices.iter().enumerate() {
            put(u32::from(ix), width, &mut acc, &mut bits, &mut out);
            // The decoder adds an entry after every code but the first.
            if i > 0 {
                next += 1;
                if next == (1 << width) && width < 12 {
                    width += 1;
                }
            }
        }
        put(clear + 1, width, &mut acc, &mut bits, &mut out);
        if bits > 0 {
            out.push(acc as u8);
        }
        out
    }

    #[test]
    fn decodes_composed_frames_delays_and_transparency() {
        let gif = decode(&sample()).expect("valid gif");
        assert_eq!((gif.width, gif.height, gif.frames.len()), (2, 2, 2));
        let red = 0xffff_0000;
        let green = 0xff00_ff00;
        assert_eq!(gif.frames[0].image.pixels, vec![red; 4]);
        assert_eq!(gif.frames[0].delay_ms, 30);
        // Disposal 1 keeps frame 0 under frame 1's sub-rectangle.
        assert_eq!(gif.frames[1].image.pixels, vec![red, red, red, green]);
        assert_eq!(gif.duration_ms(), 80);
        assert_eq!(gif.frame_at(29).map(|f| f.delay_ms), Some(30));
        assert_eq!(gif.frame_at(30).map(|f| f.delay_ms), Some(50));
        assert_eq!(gif.frame_at(80).map(|f| f.delay_ms), Some(30), "loops");
    }

    #[test]
    fn rejects_corrupt_and_oversized_files() {
        let good = sample();
        assert!(decode(&good[..good.len() - 12]).is_none(), "truncated");
        let mut bad = good.clone();
        bad[0] = b'X';
        assert!(decode(&bad).is_none(), "signature");
        let mut huge = good.clone();
        huge[6..10].copy_from_slice(&[0xff, 0xff, 0xff, 0xff]);
        assert!(decode(&huge).is_none(), "canvas over the size limit");
        let mut interlaced = good;
        let at = interlaced.iter().position(|&b| b == 0x2c).unwrap();
        interlaced[at + 9] |= 0x40;
        assert!(decode(&interlaced).is_none(), "interlace is unsupported");
    }

    #[test]
    fn downsample_averages_premultiplied() {
        let src = DdsImage {
            width: 2,
            height: 1,
            pixels: vec![0xffff_0000, 0x0000_0000],
        };
        let out = downsample(&src, 1, 1);
        // Half coverage, colour stays pure red (no dark fringe).
        assert_eq!(out.pixels[0], 0x7fff_0000);
    }
}

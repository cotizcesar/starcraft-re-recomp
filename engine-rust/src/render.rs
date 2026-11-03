//! Software renderer for the reconstructed rules engine.
//!
//! This module is deliberately dependency-free apart from the text glyphs:
//! it decodes the retail GRP sprites and the CV5/VX4EX/VR4/WPE tileset chain
//! and draws them into a 32-bit RGB framebuffer.  `sc_view` presents that
//! framebuffer with `minifb`; `sc_shot` writes it to an image for headless
//! verification.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use font8x8::UnicodeFonts;

use crate::data::GameData;
use crate::ids::unit as uid;
use crate::map::Scenario;
use crate::sim::SimUnit;

pub const TILE_PX: i32 = 32;
pub const MINITILE_PX: i32 = 8;

/// ARGB pixel used by `minifb` (`0x00RRGGBB`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rgb(pub u32);

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self(((r as u32) << 16) | ((g as u32) << 8) | b as u32)
    }
    pub fn blend(self, other: Rgb, alpha: u8) -> Rgb {
        let a = alpha as u32;
        let inv = 255 - a;
        let r = (((self.0 >> 16) & 0xff) * inv + ((other.0 >> 16) & 0xff) * a) / 255;
        let g = (((self.0 >> 8) & 0xff) * inv + ((other.0 >> 8) & 0xff) * a) / 255;
        let b = ((self.0 & 0xff) * inv + (other.0 & 0xff) * a) / 255;
        Rgb((r << 16) | (g << 8) | b)
    }
}

pub const PLAYER_COLORS: [Rgb; 12] = [
    Rgb::new(255, 35, 35),
    Rgb::new(0, 80, 255),
    Rgb::new(0, 200, 200),
    Rgb::new(160, 60, 220),
    Rgb::new(255, 140, 0),
    Rgb::new(110, 70, 40),
    Rgb::new(230, 230, 230),
    Rgb::new(230, 220, 40),
    Rgb::new(40, 180, 70),
    Rgb::new(220, 110, 160),
    Rgb::new(120, 120, 255),
    Rgb::new(180, 180, 120),
];

pub struct Framebuffer {
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<u32>,
}

impl Framebuffer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width: width as i32,
            height: height as i32,
            pixels: vec![0; width * height],
        }
    }

    pub fn clear(&mut self, color: Rgb) {
        self.pixels.fill(color.0);
    }

    #[inline]
    pub fn put(&mut self, x: i32, y: i32, color: Rgb) {
        if x >= 0 && y >= 0 && x < self.width && y < self.height {
            self.pixels[y as usize * self.width as usize + x as usize] = color.0;
        }
    }

    #[inline]
    pub fn get(&self, x: i32, y: i32) -> Rgb {
        if x >= 0 && y >= 0 && x < self.width && y < self.height {
            Rgb(self.pixels[y as usize * self.width as usize + x as usize])
        } else {
            Rgb(0)
        }
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: Rgb) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.put(xx, yy, color);
            }
        }
    }

    pub fn rect_outline(&mut self, x: i32, y: i32, w: i32, h: i32, color: Rgb) {
        for xx in x..x + w {
            self.put(xx, y, color);
            self.put(xx, y + h - 1, color);
        }
        for yy in y..y + h {
            self.put(x, yy, color);
            self.put(x + w - 1, yy, color);
        }
    }

    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgb) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let (mut x, mut y) = (x0, y0);
        loop {
            self.put(x, y, color);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Filled diamond, used for the mineral HUD icon.
    pub fn diamond_icon(&mut self, x: i32, y: i32, size: i32, color: Rgb) {
        let s = size.max(2);
        let half = s / 2;
        for dy in 0..s {
            let dist = (dy - half).abs();
            let w = half - dist / 2;
            for dx in -w..=w {
                self.put(x + half + dx, y + dy, color);
            }
        }
    }

    /// Filled up-chevron, used for the supply HUD icon.
    pub fn triangle_icon(&mut self, x: i32, y: i32, size: i32, color: Rgb) {
        let s = size.max(2);
        for dy in 0..s {
            let w = (s - dy) / 2;
            for dx in -w..=w {
                self.put(x + s / 2 + dx, y + dy, color);
            }
        }
    }

    pub fn circle_outline(&mut self, cx: i32, cy: i32, radius: i32, color: Rgb) {
        let mut x = radius;
        let mut y = 0;
        let mut err = 1 - radius;
        while x >= y {
            for (px, py) in [
                (cx + x, cy + y),
                (cx + y, cy + x),
                (cx - y, cy + x),
                (cx - x, cy + y),
                (cx - x, cy - y),
                (cx - y, cy - x),
                (cx + y, cy - x),
                (cx + x, cy - y),
            ] {
                self.put(px, py, color);
            }
            y += 1;
            if err < 0 {
                err += 2 * y + 1;
            } else {
                x -= 1;
                err += 2 * (y - x) + 1;
            }
        }
    }

    pub fn text(&mut self, x: i32, y: i32, text: &str, color: Rgb, scale: i32) {
        let mut cx = x;
        for ch in text.chars() {
            if ch == '\n' {
                cx = x;
                continue;
            }
            if let Some(glyph) = font8x8::BASIC_FONTS.get(ch) {
                for (row, bits) in glyph.iter().enumerate() {
                    for col in 0..8 {
                        if bits & (1 << col) != 0 {
                            self.rect(
                                cx + col * scale,
                                y + row as i32 * scale,
                                scale,
                                scale,
                                color,
                            );
                        }
                    }
                }
            }
            cx += 8 * scale;
        }
    }

    pub fn text_centered(&mut self, cx: i32, y: i32, text: &str, color: Rgb, scale: i32) {
        let w = text.chars().count() as i32 * 8 * scale;
        self.text(cx - w / 2, y, text, color, scale);
    }

    pub fn to_rgb_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.pixels.len() * 3);
        for p in &self.pixels {
            out.push(((p >> 16) & 0xff) as u8);
            out.push(((p >> 8) & 0xff) as u8);
            out.push((p & 0xff) as u8);
        }
        out
    }

    pub fn write_ppm(&self, path: &Path) -> std::io::Result<()> {
        let mut out = format!("P6\n{} {}\n255\n", self.width, self.height).into_bytes();
        out.extend_from_slice(&self.to_rgb_bytes());
        fs::write(path, out)
    }

    pub fn write_bmp(&self, path: &Path) -> std::io::Result<()> {
        let row = ((self.width * 3 + 3) / 4 * 4) as usize;
        let size = 54 + row * self.height as usize;
        let mut out = Vec::with_capacity(size);
        out.extend_from_slice(b"BM");
        out.extend_from_slice(&(size as u32).to_le_bytes());
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&54u32.to_le_bytes());
        out.extend_from_slice(&40u32.to_le_bytes());
        out.extend_from_slice(&self.width.to_le_bytes());
        out.extend_from_slice(&self.height.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&24u16.to_le_bytes());
        out.extend_from_slice(&[0; 24]);
        for y in (0..self.height).rev() {
            let mut line = vec![0u8; row];
            for x in 0..self.width {
                let p = self.get(x, y);
                let b = x as usize * 3;
                line[b] = (p.0 & 0xff) as u8;
                line[b + 1] = ((p.0 >> 8) & 0xff) as u8;
                line[b + 2] = ((p.0 >> 16) & 0xff) as u8;
            }
            out.extend_from_slice(&line);
        }
        fs::write(path, out)
    }
}

/// 256-colour palette.  WPE files are stored as RGBX; `Units.pal` is RGB.
pub struct Palette {
    pub colors: [Rgb; 256],
}

impl Palette {
    pub fn from_wpe(bytes: &[u8]) -> Self {
        let mut colors = [Rgb(0); 256];
        for (i, color) in colors.iter_mut().enumerate() {
            let o = i * 4;
            if o + 3 <= bytes.len() {
                *color = Rgb::new(bytes[o], bytes[o + 1], bytes[o + 2]);
            }
        }
        Self { colors }
    }

    pub fn from_pal(bytes: &[u8]) -> Self {
        let mut colors = [Rgb(0); 256];
        for (i, color) in colors.iter_mut().enumerate() {
            let o = i * 3;
            if o + 3 <= bytes.len() {
                *color = Rgb::new(bytes[o], bytes[o + 1], bytes[o + 2]);
            }
        }
        Self { colors }
    }

    /// Retail GRPs reserve palette entries 8..=15 for the player colour.
    pub fn color(&self, index: u8, team: Option<Rgb>) -> Rgb {
        if let Some(team) = team {
            if (8..=15).contains(&index) {
                return team;
            }
        }
        self.colors[index as usize]
    }
}

#[derive(Clone, Debug)]
pub struct GrpFrame {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// Palette indices, row-major, 0 = transparent.
    pub indices: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct Grp {
    pub width: i32,
    pub height: i32,
    pub frames: Vec<GrpFrame>,
}

fn u16_at(data: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*data.get(o)?, *data.get(o + 1)?]))
}

fn u32_at(data: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *data.get(o)?,
        *data.get(o + 1)?,
        *data.get(o + 2)?,
        *data.get(o + 3)?,
    ]))
}

impl Grp {
    pub fn parse(data: &[u8]) -> Option<Self> {
        let frames = u16_at(data, 0)? as usize;
        let width = u16_at(data, 2)? as i32;
        let height = u16_at(data, 4)? as i32;
        if frames == 0 || !(1..=2400).contains(&frames) || width <= 0 || height <= 0 {
            return None;
        }
        let mut out = Vec::with_capacity(frames);
        for i in 0..frames {
            let hdr = 6 + i * 8;
            let x = *data.get(hdr)? as i32;
            let y = *data.get(hdr + 1)? as i32;
            let w = *data.get(hdr + 2)? as i32;
            let h = *data.get(hdr + 3)? as i32;
            let fd = u32_at(data, hdr + 4)? as usize;
            if w <= 0 || h <= 0 || fd >= data.len() {
                out.push(GrpFrame {
                    x,
                    y,
                    w: 0,
                    h: 0,
                    indices: Vec::new(),
                });
                continue;
            }
            let mut indices = vec![0u8; (w * h) as usize];
            for row in 0..h as usize {
                let off = u16_at(data, fd + row * 2).map(|v| fd + v as usize)?;
                if off >= data.len() {
                    return None;
                }
                let mut pos = off;
                let mut col = 0usize;
                while col < w as usize {
                    let op = *data.get(pos)?;
                    pos += 1;
                    if op & 0x80 != 0 {
                        col += (op - 0x80) as usize;
                    } else if op & 0x40 != 0 {
                        let n = (op - 0x40) as usize;
                        let v = *data.get(pos)?;
                        pos += 1;
                        for _ in 0..n {
                            if col < w as usize {
                                indices[row * w as usize + col] = v;
                            }
                            col += 1;
                        }
                    } else {
                        let n = op as usize;
                        for _ in 0..n {
                            if col < w as usize {
                                indices[row * w as usize + col] = *data.get(pos)?;
                            }
                            pos += 1;
                            col += 1;
                        }
                    }
                }
            }
            out.push(GrpFrame {
                x,
                y,
                w,
                h,
                indices,
            });
        }
        Some(Self {
            width,
            height,
            frames: out,
        })
    }

    pub fn frame(&self, index: usize) -> Option<&GrpFrame> {
        if self.frames.is_empty() {
            None
        } else {
            Some(&self.frames[index.min(self.frames.len() - 1)])
        }
    }
}

/// Full tileset chain from the retail CASC storage.
pub struct Tileset {
    pub name: String,
    pub palette: Palette,
    cv5: Vec<u8>,
    vx4: Vec<u8>,
    vr4: Vec<u8>,
    /// Remastered HD megatile art (`hd2/tileset/<era>.dds.vr4`): one 32x32
    /// RGBA image per megatile, downsampled from the shipped 64x64 DXT1 DDS.
    /// When present the terrain is drawn from this instead of the SD palette
    /// composition, which is what makes the captures look like the retail HD
    /// client.
    hd: Option<Vec<Option<Box<[u32]>>>>,
    pub minimap: Vec<Rgb>,
    pub minimap_size: (usize, usize),
}

pub fn era_name(era: u16) -> &'static str {
    match era {
        0 => "badlands",
        1 => "platform",
        2 => "install",
        3 => "ashworld",
        4 => "jungle",
        5 => "desert",
        6 => "ice",
        _ => "twilight",
    }
}

/// Load the Remastered HD megatile art.
///
/// `hd2/tileset/<era>.dds.vr4` is a flat archive: `[u32 total]` followed by
/// records of `[u32 flags][u16 w][u16 h][u32 len][DDS payload]`. Each payload
/// is a 64x64 DXT1 surface holding one megatile (4x4 tiles of 16 px, the HD
/// counterpart of the SD 32x32 megatile). The art is downsampled to the world
/// scale used by the renderer so the whole coordinate system stays in SD units.
pub fn load_hd_megatiles(path: &Path) -> Option<Vec<Option<Box<[u32]>>>> {
    let bytes = fs::read(path).ok()?;
    let mut out: Vec<Option<Box<[u32]>>> = Vec::new();
    let mut off = 4usize;
    while off + 16 <= bytes.len() {
        // The payload length sits right before the "DDS " magic.
        let mut found = None;
        let mut i = off;
        while i + 4 <= bytes.len() {
            if &bytes[i..i + 4] == b"DDS " {
                found = Some(i);
                break;
            }
            i += 1;
        }
        let Some(dds_at) = found else { break };
        let len = u32_at(&bytes, dds_at - 4)? as usize;
        if len < 128 || dds_at + len > bytes.len() {
            break;
        }
        let img = crate::dds::parse(&bytes[dds_at..dds_at + len]);
        out.push(img.map(|im| downsample_rgba(&im, 32, 32)));
        off = dds_at + len;
    }
    (!out.is_empty()).then_some(out)
}

/// One resampled megatile (ARGB, `size * size`).
type TilePixels = std::rc::Rc<Box<[u32]>>;

/// 4x HD terrain (`tileset/<era>.dds.vr4`, 128x128 DXT1 per megatile),
/// drawn at screen resolution.  Payloads stay compressed; a megatile is
/// decoded and resampled to the current tile size the first time it is
/// drawn (a map uses a few hundred of the ~7000).
pub struct HdTerrain {
    bytes: Vec<u8>,
    /// `(offset, len)` of each megatile's DDS payload.
    records: Vec<(usize, usize)>,
    cache: std::cell::RefCell<HashMap<(usize, usize), TilePixels>>,
}

impl HdTerrain {
    /// Index the archive: `[u32 file size][u16 count][u16 ?]` then
    /// `[u32 flags][u16 w][u16 h][u32 len][DDS]` records (jungle: 7038).
    pub fn parse(bytes: Vec<u8>) -> Option<Self> {
        let mut records = Vec::new();
        let mut off = 8usize;
        while off + 12 <= bytes.len() {
            let len = u32_at(&bytes, off + 8)? as usize;
            let at = off + 12;
            if len < 128 || at + len > bytes.len() || &bytes[at..at + 4] != b"DDS " {
                break;
            }
            records.push((at, len));
            off = at + len;
        }
        (!records.is_empty()).then(|| Self {
            bytes,
            records,
            cache: Default::default(),
        })
    }

    pub fn load(root: &Path, era: u16) -> Option<Self> {
        let path = root.join(format!("hd/tileset/{}.dds.vr4", era_name(era)));
        Self::parse(fs::read(path).ok()?)
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Megatile `mega` resampled to `size`x`size` (premultiplied-alpha box
    /// filter over the 128 px source).
    pub fn tile(&self, mega: usize, size: usize) -> Option<TilePixels> {
        if let Some(t) = self.cache.borrow().get(&(mega, size)) {
            return Some(t.clone());
        }
        let &(at, len) = self.records.get(mega)?;
        let img = crate::dds::parse(&self.bytes[at..at + len])?;
        let mut px = downsample_rgba(&img, size, size);
        // Retail draws the HD terrain darker than the raw texels: lit grass
        // and dirt measure 0.91 of the DDS colour on 135051 / 135020.
        for p in px.iter_mut() {
            let scale = |c: u32| ((c as f32) * HD_TERRAIN_GAIN).round() as u32;
            *p = (*p & 0xff00_0000)
                | (scale((*p >> 16) & 0xff) << 16)
                | (scale((*p >> 8) & 0xff) << 8)
                | scale(*p & 0xff);
        }
        let t = std::rc::Rc::new(px);
        self.cache.borrow_mut().insert((mega, size), t.clone());
        Some(t)
    }

    /// Draw the terrain of `scenario` into the screen rectangle `dest`
    /// (`x, y, w, h`), with the camera at game px `cam` and `s` screen px per
    /// game px.  `tileset` resolves MTXM entries to megatiles; tiles for
    /// which `creep(tx, ty)` holds draw a creep megatile (cv5 group 1).
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        fb: &mut Framebuffer,
        tileset: &Tileset,
        scenario: &Scenario,
        cam: (i32, i32),
        dest: (i32, i32, i32, i32),
        s: f32,
        creep: &dyn Fn(i32, i32) -> bool,
    ) {
        let size = (TILE_PX as f32 * s).round().max(1.0) as usize;
        let (dx, dy, dw, dh) = dest;
        let view_w = (dw as f32 / s).ceil() as i32;
        let view_h = (dh as f32 / s).ceil() as i32;
        let first_x = (cam.0 / TILE_PX).max(0);
        let first_y = (cam.1 / TILE_PX).max(0);
        let last_x = ((cam.0 + view_w) / TILE_PX + 1).min(scenario.width as i32);
        let last_y = ((cam.1 + view_h) / TILE_PX + 1).min(scenario.height as i32);
        for ty in first_y..last_y {
            for tx in first_x..last_x {
                let Some(&mtxm) = scenario
                    .tiles
                    .get(ty as usize * scenario.width + tx as usize)
                else {
                    continue;
                };
                let sx = dx + ((tx * TILE_PX - cam.0) as f32 * s).round() as i32;
                let sy = dy + ((ty * TILE_PX - cam.1) as f32 * s).round() as i32;
                let mtxm = if creep(tx, ty) {
                    creep_mtxm(tx, ty)
                } else {
                    mtxm
                };
                let group = ((mtxm >> 4) & 0x7ff) as usize;
                let bg = tileset.background_megatile(group) as usize;
                let mega = tileset.megatile(mtxm) as usize;
                // Group background first, then the tile's own megatile
                // (its cut-out alpha reveals the background), as in SD.
                let layers: &[usize] = if mega == bg { &[bg] } else { &[bg, mega] };
                for &m in layers {
                    if let Some(px) = self.tile(m, size) {
                        blit_tile(fb, &px, size, sx, sy, (dx, dy, dx + dw, dy + dh));
                    }
                }
            }
        }
    }
}

/// Measured brightness of the retail HD terrain relative to its texels.
pub const HD_TERRAIN_GAIN: f32 = 0.91;

/// Fog darkness of an explored tile that is not in sight (0..=255).
pub const FOG_EXPLORED_ALPHA: f32 = 150.0;
/// Fog grid samples per tile and Gaussian width in tiles.
const FOG_SUB: i32 = 4;
const FOG_SIGMA: f32 = 0.8;

/// In-place separable Gaussian blur of a `w x h` grid (clamped edges).
fn gaussian_blur(v: &mut [f32], w: usize, h: usize, sigma: f32) {
    let r = (sigma * 3.0).ceil() as i32;
    let k: Vec<f32> = (-r..=r)
        .map(|i| (-(i * i) as f32 / (2.0 * sigma * sigma)).exp())
        .collect();
    let norm: f32 = k.iter().sum();
    let mut tmp = vec![0f32; v.len()];
    for y in 0..h {
        for x in 0..w {
            let mut acc = 0.0;
            for (n, kv) in k.iter().enumerate() {
                let xx = (x as i32 + n as i32 - r).clamp(0, w as i32 - 1) as usize;
                acc += v[y * w + xx] * kv;
            }
            tmp[y * w + x] = acc / norm;
        }
    }
    for y in 0..h {
        for x in 0..w {
            let mut acc = 0.0;
            for (n, kv) in k.iter().enumerate() {
                let yy = (y as i32 + n as i32 - r).clamp(0, h as i32 - 1) as usize;
                acc += tmp[yy * w + x] * kv;
            }
            v[y * w + x] = acc / norm;
        }
    }
}

/// Retail HD fog at screen resolution: one darkness value per tile (0 in
/// sight, [`FOG_EXPLORED_ALPHA`] explored, 255 unexplored), interpolated
/// bilinearly between tile centres so the edge of vision is a soft ramp
/// instead of 32 px blocks.
#[allow(clippy::too_many_arguments)]
pub fn draw_fog_smooth(
    fb: &mut Framebuffer,
    scenario: &Scenario,
    world: &crate::sim::World,
    owner: u8,
    cam: (i32, i32),
    dest: (i32, i32, i32, i32),
    s: f32,
) {
    let (dx, dy, dw, dh) = dest;
    // Margin of 4 tiles so the blur sees the tiles beyond the screen edge.
    let tx0 = (cam.0 / TILE_PX) - 4;
    let ty0 = (cam.1 / TILE_PX) - 4;
    let tw = (dw as f32 / s / TILE_PX as f32).ceil() as i32 + 9;
    let th = (dh as f32 / s / TILE_PX as f32).ceil() as i32 + 9;
    let (mw, mh) = (scenario.width as i32, scenario.height as i32);
    let mut grid = vec![0f32; (tw * th) as usize];
    for j in 0..th {
        for i in 0..tw {
            let (tx, ty) = ((tx0 + i).clamp(0, mw - 1), (ty0 + j).clamp(0, mh - 1));
            let (cx, cy) = (tx * TILE_PX + TILE_PX / 2, ty * TILE_PX + TILE_PX / 2);
            grid[(j * tw + i) as usize] = if !world.is_explored_by(owner, cx, cy) {
                255.0
            } else if !world.is_visible_to(owner, cx, cy) {
                FOG_EXPLORED_ALPHA
            } else {
                0.0
            };
        }
    }
    // Upsample to FOG_SUB samples per tile (piecewise constant) and blur
    // with a Gaussian of FOG_SIGMA tiles: the retail edge of vision falls
    // from 0.9 to 0.1 over ~3 tiles (fitted on 135051 / 135020).
    let sub = FOG_SUB;
    let (gw, gh) = (tw * sub, th * sub);
    let mut fine = vec![0f32; (gw * gh) as usize];
    for y in 0..gh {
        for x in 0..gw {
            fine[(y * gw + x) as usize] = grid[((y / sub) * tw + x / sub) as usize];
        }
    }
    gaussian_blur(&mut fine, gw as usize, gh as usize, FOG_SIGMA * sub as f32);
    let at = |i: i32, j: i32| fine[(j.clamp(0, gh - 1) * gw + i.clamp(0, gw - 1)) as usize];
    let step = TILE_PX as f32 / sub as f32;
    for y in dy.max(0)..(dy + dh).min(fb.height) {
        let gy = cam.1 as f32 + (y - dy) as f32 / s;
        let fy = (gy - (ty0 * TILE_PX) as f32) / step - 0.5;
        let j = fy.floor() as i32;
        let wy = fy - j as f32;
        for x in dx.max(0)..(dx + dw).min(fb.width) {
            let gx = cam.0 as f32 + (x - dx) as f32 / s;
            let fx = (gx - (tx0 * TILE_PX) as f32) / step - 0.5;
            let i = fx.floor() as i32;
            let wx = fx - i as f32;
            let top = at(i, j) + (at(i + 1, j) - at(i, j)) * wx;
            let bottom = at(i, j + 1) + (at(i + 1, j + 1) - at(i, j + 1)) * wx;
            let a = top + (bottom - top) * wy;
            if a > 0.5 {
                let c = fb.get(x, y);
                fb.put(x, y, c.blend(Rgb::new(0, 0, 0), a.round().min(255.0) as u8));
            }
        }
    }
}

/// MTXM entry of a creep tile: cv5 group 1 holds the 13 creep megatiles.
/// INFERENCE: retail picks the variant with its own generator; a fixed
/// per-tile hash stands in for it (the variants share one texture).
pub fn creep_mtxm(tx: i32, ty: i32) -> u16 {
    let h = (tx as u32).wrapping_mul(0x9e37_79b9) ^ (ty as u32).wrapping_mul(0x85eb_ca6b);
    let h = h ^ (h >> 15);
    (1 << 4) | (h % 13) as u16
}

/// Alpha-blend a square ARGB tile clipped to `clip` (`x0, y0, x1, y1`).
fn blit_tile(
    fb: &mut Framebuffer,
    px: &[u32],
    size: usize,
    sx: i32,
    sy: i32,
    clip: (i32, i32, i32, i32),
) {
    for y in 0..size as i32 {
        let py = sy + y;
        if py < clip.1 || py >= clip.3 {
            continue;
        }
        for x in 0..size as i32 {
            let qx = sx + x;
            if qx < clip.0 || qx >= clip.2 {
                continue;
            }
            let p = px[y as usize * size + x as usize];
            let a = (p >> 24) as u8;
            if a == 0 {
                continue;
            }
            let fg = Rgb(p & 0x00ff_ffff);
            if a == 255 {
                fb.put(qx, py, fg);
            } else {
                let bgc = fb.get(qx, py);
                fb.put(qx, py, bgc.blend(fg, a));
            }
        }
    }
}

/// Box-filter an image down to `w`x`h`, keeping the alpha channel.
fn downsample_rgba(img: &crate::dds::DdsImage, w: usize, h: usize) -> Box<[u32]> {
    let mut out = vec![0u32; w * h];
    if img.width == 0 || img.height == 0 {
        return out.into_boxed_slice();
    }
    for ty in 0..h {
        let y0 = ty * img.height / h;
        let y1 = (((ty + 1) * img.height) / h).max(y0 + 1).min(img.height);
        for tx in 0..w {
            let x0 = tx * img.width / w;
            let x1 = (((tx + 1) * img.width) / w).max(x0 + 1).min(img.width);
            let (mut r, mut g, mut b, mut a, mut n) = (0u32, 0u32, 0u32, 0u32, 0u32);
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = img.pixel(x, y);
                    let pa = p >> 24;
                    r += ((p >> 16) & 0xff) * pa / 255;
                    g += ((p >> 8) & 0xff) * pa / 255;
                    b += (p & 0xff) * pa / 255;
                    a += pa;
                    n += 1;
                }
            }
            if n == 0 || a == 0 {
                continue;
            }
            let a_out = a / n;
            let scale = |v: u32| -> u32 { (v / n).min(255) };
            out[ty * w + tx] = (a_out << 24) | (scale(r) << 16) | (scale(g) << 8) | scale(b);
        }
    }
    out.into_boxed_slice()
}

impl Tileset {
    pub fn load(dir: &Path, era: u16, scenario: &Scenario) -> Option<Self> {
        let name = era_name(era);
        let load = |ext: &str| fs::read(dir.join(format!("{name}.{ext}"))).ok();
        let cv5 = load("cv5")?;
        let vx4 = load("vx4ex")?;
        let vr4 = load("vr4")?;
        let palette = Palette::from_wpe(&load("wpe")?);
        let mut ts = Self {
            name: name.to_string(),
            palette,
            cv5,
            vx4,
            vr4,
            minimap: Vec::new(),
            minimap_size: (0, 0),
            hd: dir
                .parent()
                .and_then(|root| {
                    root.join("hd2/tileset")
                        .join(format!("{name}.dds.vr4"))
                        .to_str()
                        .map(|p| load_hd_megatiles(Path::new(p)))
                })
                .flatten(),
        };
        ts.build_minimap(scenario);
        Some(ts)
    }

    /// The tileset's `cv5` group table (MTXM entry -> megatile).
    pub fn cv5(&self) -> &[u8] {
        &self.cv5
    }

    #[inline]
    pub fn megatile(&self, mtxm: u16) -> u16 {
        let group = ((mtxm >> 4) & 0x7ff) as usize;
        let index = (mtxm & 0x0f) as usize;
        u16_at(&self.cv5, group * 52 + 20 + index * 2).unwrap_or(0)
    }

    /// Megatile painted underneath a group's 16 tiles.
    ///
    /// CV5 stores a group background at `+10`; the per-tile megatiles are
    /// mostly transparent (cliffs, edges, space), so without this layer the
    /// holes show whatever was in the framebuffer (black).
    #[inline]
    pub fn background_megatile(&self, group: usize) -> u16 {
        u16_at(&self.cv5, group * 52 + 10).unwrap_or(0)
    }

    /// Paint one megatile (4x4 minitiles, 32x32 px) at `(sx, sy)`.
    fn draw_megatile(&self, fb: &mut Framebuffer, mega: usize, sx: i32, sy: i32) {
        let base = mega * 64;
        for my in 0..4 {
            for mx in 0..4 {
                let Some(vid) = u32_at(&self.vx4, base + (my * 4 + mx) * 4) else {
                    continue;
                };
                let flip = vid & 1 != 0;
                let vr = (vid >> 1) as usize;
                let vbase = vr * 64;
                for py in 0..8 {
                    for px in 0..8 {
                        let src = vbase + py * 8 + if flip { 7 - px } else { px };
                        let Some(&c) = self.vr4.get(src) else {
                            continue;
                        };
                        if c == 0 {
                            continue;
                        }
                        fb.put(
                            sx + mx as i32 * 8 + px as i32,
                            sy + my as i32 * 8 + py as i32,
                            self.palette.colors[c as usize],
                        );
                    }
                }
            }
        }
    }

    /// Fill one 32x32 map tile at `(sx, sy)`.
    ///
    /// Two layers, like the retail engine: the group background first, then
    /// the tile's own megatile (whose transparent pixels reveal the
    /// background).
    pub fn draw_tile(&self, fb: &mut Framebuffer, mtxm: u16, sx: i32, sy: i32) {
        let group = ((mtxm >> 4) & 0x7ff) as usize;
        let bg = self.background_megatile(group) as usize;
        if self.hd.is_some() {
            self.draw_hd_megatile(fb, bg, sx, sy);
            let mega = self.megatile(mtxm) as usize;
            if mega != bg {
                self.draw_hd_megatile(fb, mega, sx, sy);
            }
            return;
        }
        self.draw_megatile(fb, bg, sx, sy);
        let mega = self.megatile(mtxm) as usize;
        if mega != bg {
            self.draw_megatile(fb, mega, sx, sy);
        }
    }

    /// Paint one megatile from the HD art (32x32 px at world scale), honouring
    /// the DXT1 cut-out alpha the retail terrain uses for cliff edges.
    fn draw_hd_megatile(&self, fb: &mut Framebuffer, mega: usize, sx: i32, sy: i32) {
        let Some(hd) = &self.hd else {
            return;
        };
        let Some(Some(px)) = hd.get(mega) else {
            // Missing in the HD set: fall back to the SD composition.
            self.draw_megatile(fb, mega, sx, sy);
            return;
        };
        for y in 0..32 {
            for x in 0..32 {
                let p = px[y * 32 + x];
                let a = (p >> 24) as u8;
                if a == 0 {
                    continue;
                }
                let fg = Rgb(p & 0x00ff_ffff);
                let (px_x, px_y) = (sx + x as i32, sy + y as i32);
                if a == 255 {
                    fb.put(px_x, px_y, fg);
                } else {
                    let bg = fb.get(px_x, px_y);
                    fb.put(px_x, px_y, bg.blend(fg, a));
                }
            }
        }
    }

    pub fn draw_terrain(&self, fb: &mut Framebuffer, scenario: &Scenario, cam: (i32, i32)) {
        if scenario.width == 0 || scenario.height == 0 {
            return;
        }
        let first_x = (cam.0 / TILE_PX).max(0);
        let first_y = (cam.1 / TILE_PX).max(0);
        let last_x = ((cam.0 + fb.width) / TILE_PX + 1).min(scenario.width as i32);
        let last_y = ((cam.1 + fb.height) / TILE_PX + 1).min(scenario.height as i32);
        for ty in first_y..last_y {
            for tx in first_x..last_x {
                let idx = ty as usize * scenario.width + tx as usize;
                let Some(&mtxm) = scenario.tiles.get(idx) else {
                    continue;
                };
                self.draw_tile(fb, mtxm, tx * TILE_PX - cam.0, ty * TILE_PX - cam.1);
            }
        }
    }

    /// Draw the viewport with rules-level fog: unexplored tiles are black and
    /// explored-but-not-visible tiles are dimmed.
    pub fn draw_terrain_fog(
        &self,
        fb: &mut Framebuffer,
        scenario: &Scenario,
        cam: (i32, i32),
        world: &crate::sim::World,
        owner: u8,
    ) {
        self.draw_terrain(fb, scenario, cam);
        self.draw_fog(fb, scenario, cam, world, owner);
    }

    /// Apply the rules-level fog mask to whatever is already in the
    /// framebuffer.  Split out from `draw_terrain_fog` so the client can draw
    /// the creep overlay between the terrain and the fog.
    pub fn draw_fog(
        &self,
        fb: &mut Framebuffer,
        scenario: &Scenario,
        cam: (i32, i32),
        world: &crate::sim::World,
        owner: u8,
    ) {
        let first_x = (cam.0 / TILE_PX).max(0);
        let first_y = (cam.1 / TILE_PX).max(0);
        let last_x = ((cam.0 + fb.width) / TILE_PX + 1).min(scenario.width as i32);
        let last_y = ((cam.1 + fb.height) / TILE_PX + 1).min(scenario.height as i32);
        for ty in first_y..last_y {
            for tx in first_x..last_x {
                let px = tx * TILE_PX - cam.0;
                let py = ty * TILE_PX - cam.1;
                let cx = tx * TILE_PX + TILE_PX / 2;
                let cy = ty * TILE_PX + TILE_PX / 2;
                if !world.is_explored_by(owner, cx, cy) {
                    fb.rect(px, py, TILE_PX, TILE_PX, Rgb::new(0, 0, 0));
                } else if !world.is_visible_to(owner, cx, cy) {
                    for yy in 0..TILE_PX {
                        for xx in 0..TILE_PX {
                            let c = fb.get(px + xx, py + yy);
                            fb.put(px + xx, py + yy, c.blend(Rgb::new(0, 0, 0), 150));
                        }
                    }
                }
            }
        }
    }

    fn build_minimap(&mut self, scenario: &Scenario) {
        let (w, h) = (scenario.width, scenario.height);
        self.minimap_size = (w, h);
        self.minimap = vec![Rgb::new(10, 10, 14); w * h];
        if w == 0 || h == 0 {
            return;
        }
        for ty in 0..h {
            for tx in 0..w {
                let mtxm = scenario.tiles[ty * w + tx];
                let mega = self.megatile(mtxm) as usize;
                let base = mega * 64;
                let mut sum = (0u32, 0u32, 0u32);
                let mut n = 0u32;
                for k in 0..16 {
                    let vid = u32_at(&self.vx4, base + k * 4).unwrap_or(0);
                    let vr = (vid >> 1) as usize;
                    let vbase = vr * 64;
                    // Sample four pixels from the minitile.
                    for p in [10usize, 26, 37, 53] {
                        let c = *self.vr4.get(vbase + p).unwrap_or(&0);
                        if c != 0 {
                            let rgb = self.palette.colors[c as usize].0;
                            sum.0 += (rgb >> 16) & 0xff;
                            sum.1 += (rgb >> 8) & 0xff;
                            sum.2 += rgb & 0xff;
                            n += 1;
                        }
                    }
                }
                let n = n.max(1);
                self.minimap[ty * w + tx] =
                    Rgb::new((sum.0 / n) as u8, (sum.1 / n) as u8, (sum.2 / n) as u8);
            }
        }
    }

    pub fn draw_minimap(
        &self,
        fb: &mut Framebuffer,
        rect: (i32, i32, i32, i32),
        units: &[(i32, i32, u8, bool)],
        cam: (i32, i32, i32, i32),
    ) {
        self.draw_minimap_with_terrain(fb, rect, units, cam, None, 0, true)
    }

    /// Radar with retail fog of war: unexplored tiles are black and explored
    /// but not currently visible ones are dimmed. `terrain == false` is the
    /// "Show Terrain in Minimap (Tab)" state the reference screenshot shows
    /// (black radar with unit dots and the camera box only).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_minimap_with_terrain(
        &self,
        fb: &mut Framebuffer,
        rect: (i32, i32, i32, i32),
        units: &[(i32, i32, u8, bool)],
        cam: (i32, i32, i32, i32),
        fog: Option<&crate::vision::Vision>,
        owner: u8,
        terrain: bool,
    ) {
        let (x, y, w, h) = rect;
        let (mw, mh) = self.minimap_size;
        if mw == 0 || mh == 0 {
            return;
        }
        let sx = w as f32 / mw as f32;
        let sy = h as f32 / mh as f32;
        if !terrain {
            fb.rect(x, y, w, h, Rgb::new(0, 0, 0));
        }
        for ty in 0..mh {
            if !terrain {
                break;
            }
            for tx in 0..mw {
                let mut c = self.minimap[ty * mw + tx];
                if let Some(v) = fog {
                    let (wx, wy) = (tx as i32 * TILE_PX, ty as i32 * TILE_PX);
                    if !v.is_explored(owner, wx, wy) {
                        c = Rgb::new(0, 0, 0);
                    } else if !v.is_visible(owner, wx, wy) {
                        c = c.blend(Rgb::new(0, 0, 0), 140);
                    }
                }
                let x0 = x + (tx as f32 * sx) as i32;
                let y0 = y + (ty as f32 * sy) as i32;
                let x1 = x + ((tx + 1) as f32 * sx) as i32;
                let y1 = y + ((ty + 1) as f32 * sy) as i32;
                fb.rect(x0, y0, (x1 - x0).max(1), (y1 - y0).max(1), c);
            }
        }
        for &(ux, uy, owner, is_building) in units {
            let px = x + (ux as f32 / TILE_PX as f32 * sx) as i32;
            let py = y + (uy as f32 / TILE_PX as f32 * sy) as i32;
            let c = PLAYER_COLORS[owner as usize % PLAYER_COLORS.len()];
            fb.rect(
                px,
                py,
                if is_building { 2 } else { 1 },
                if is_building { 2 } else { 1 },
                c,
            );
        }
        // Camera viewport.
        let vx = x + (cam.0 as f32 / TILE_PX as f32 * sx) as i32;
        let vy = y + (cam.1 as f32 / TILE_PX as f32 * sy) as i32;
        let vw = ((cam.2 - cam.0) as f32 / TILE_PX as f32 * sx) as i32;
        let vh = ((cam.3 - cam.1) as f32 / TILE_PX as f32 * sy) as i32;
        fb.rect_outline(vx, vy, vw.max(2), vh.max(2), Rgb::new(255, 255, 255));
    }
}

/// Stable hash for one 8-pixel creep cell.  The retail client draws creep
/// procedurally from the gameplay maturity grid, so the reconstructed client
/// uses a deterministic texture instead of sampling an asset that does not
/// exist in CASC.
fn creep_hash(x: i32, y: i32, phase: u32) -> u32 {
    let mut h = (x as u32)
        .wrapping_mul(0x9e37_79b1)
        .wrapping_add((y as u32).wrapping_mul(0x85eb_ca6b))
        .wrapping_add(phase.wrapping_mul(0xc2b2_ae35));
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h
}

/// Sample the creep colour at one 4-pixel grain cell, or `None` when the
/// noise leaves a gap.  `strength` is the tile maturity (1..=3).
///
/// The mat is drawn from overlapping grain rather than a pre-rendered sprite:
/// retail ships no creep GRP, so the engine composites the texture from the
/// gameplay maturity grid.  The hash is stable across frames; only the slow
/// pulse and the edge noise move.
pub fn creep_cell(tx: i32, ty: i32, mx: i32, my: i32, strength: u8, tick: u64) -> Option<Rgb> {
    if strength == 0 {
        return None;
    }
    let phase = ((tick / 6) & 3) as u32;
    let wx = tx * 8 + mx;
    let wy = ty * 8 + my;
    let h = creep_hash(wx, wy, phase);
    let pulse = [0i32, 5, 1, -4][((tick / 8) & 3) as usize];
    // Dark organic mat with raised blisters and hairline veins.
    let vein = if (h >> 12) & 0x1f == 0 { -14 } else { 0 };
    let blister = if (h >> 18) & 0x1f == 0 { 22 } else { 0 };
    let ring = if (h >> 23) & 0x1f == 0 { -10 } else { 0 };
    let r = (48 + (h & 13) as i32 + pulse + vein + blister + ring).clamp(0, 255) as u8;
    let g = (30 + ((h >> 4) & 9) as i32 + pulse + vein + blister + ring).clamp(0, 255) as u8;
    let b = (42 + ((h >> 8) & 11) as i32 + vein + blister + ring).clamp(0, 255) as u8;
    Some(Rgb::new(r, g, b))
}

/// Blend the creep mat into the framebuffer over already-drawn terrain.
///
/// `grid` is `World::creep_grid()`: one maturity byte per 32-pixel tile.
/// Creep is drawn between the terrain and the fog mask, so unexplored creep
/// stays hidden and explored creep dims with the rest of the map.
pub fn draw_creep(
    fb: &mut Framebuffer,
    scenario: &Scenario,
    cam: (i32, i32),
    grid: &[u8],
    tick: u64,
) {
    let w = scenario.width as i32;
    let h = scenario.height as i32;
    if w == 0 || h == 0 || grid.len() < (w * h) as usize {
        return;
    }
    let first_x = (cam.0 / TILE_PX).max(0);
    let first_y = (cam.1 / TILE_PX).max(0);
    let last_x = ((cam.0 + fb.width) / TILE_PX + 1).min(w);
    let last_y = ((cam.1 + fb.height) / TILE_PX + 1).min(h);
    let at = |tx: i32, ty: i32| -> u8 {
        if tx < 0 || ty < 0 || tx >= w || ty >= h {
            0
        } else {
            grid[(ty * w + tx) as usize]
        }
    };
    let phase = ((tick / 6) & 3) as u32;
    for ty in first_y..last_y {
        for tx in first_x..last_x {
            let strength = at(tx, ty);
            if strength == 0 {
                continue;
            }
            let px = tx * TILE_PX - cam.0;
            let py = ty * TILE_PX - cam.1;
            let left = at(tx - 1, ty) == 0;
            let right = at(tx + 1, ty) == 0;
            let up = at(tx, ty - 1) == 0;
            let down = at(tx, ty + 1) == 0;
            let base_alpha = 190 + strength as i32 * 18;
            for yy in 0..TILE_PX {
                for xx in 0..TILE_PX {
                    let wx = tx * TILE_PX + xx;
                    let wy = ty * TILE_PX + yy;
                    let n = creep_hash(wx >> 2, wy >> 2, phase);
                    // Ragged, noise-driven border instead of a 32-pixel
                    // square: each missing neighbour eats a 3..=12 px strip.
                    let inset_l = 3 + (n & 15) as i32;
                    let inset_r = 3 + ((n >> 4) & 15) as i32;
                    let inset_u = 3 + ((n >> 8) & 15) as i32;
                    let inset_d = 3 + ((n >> 12) & 15) as i32;
                    if (left && xx < inset_l)
                        || (right && xx >= TILE_PX - inset_r)
                        || (up && yy < inset_u)
                        || (down && yy >= TILE_PX - inset_d)
                    {
                        continue;
                    }
                    let Some(col) = creep_cell(tx, ty, xx >> 2, yy >> 2, strength, tick) else {
                        continue;
                    };
                    // Raised blisters and their shadow rings, one 8x8 cell
                    // at a time.  This is the visible texture of the mat at
                    // normal zoom rather than a flat tint.
                    let cell = creep_hash(wx >> 3, wy >> 3, phase ^ 0x5bd1_e995);
                    let center_x = 3 + ((cell >> 2) & 3) as i32;
                    let center_y = 3 + ((cell >> 6) & 3) as i32;
                    let cx = (wx & 7) - center_x;
                    let cy = (wy & 7) - center_y;
                    let d2 = cx * cx + cy * cy;
                    let shade = if cell.is_multiple_of(23) && d2 <= 3 {
                        20
                    } else if cell.is_multiple_of(23) && (7..=12).contains(&d2) {
                        -8
                    } else {
                        0
                    };
                    let col = if shade != 0 {
                        let r = (((col.0 >> 16) & 0xff) as i32 + shade).clamp(0, 255) as u8;
                        let g = (((col.0 >> 8) & 0xff) as i32 + shade / 2).clamp(0, 255) as u8;
                        let b = ((col.0 & 0xff) as i32 + shade / 2).clamp(0, 255) as u8;
                        Rgb::new(r, g, b)
                    } else {
                        col
                    };
                    // Low-frequency mottling keeps the mat from reading as a
                    // flat tint at normal zoom.
                    let blob = creep_hash(wx >> 4, wy >> 4, phase);
                    let alpha =
                        (base_alpha + ((n >> 16) & 0x0f) as i32 + ((blob >> 8) & 0x1f) as i32 - 22)
                            .clamp(0, 245) as u8;
                    let c = fb.get(px + xx, py + yy);
                    fb.put(px + xx, py + yy, c.blend(col, alpha));
                }
            }
        }
    }
}

fn sprite_path(type_id: u16) -> Option<&'static str> {
    Some(match type_id {
        uid::MARINE => "terran/marine.grp",
        uid::GHOST => "terran/ghost.grp",
        uid::VULTURE => "terran/vulture.grp",
        uid::GOLIATH => "terran/goliath.grp",
        uid::SIEGE_TANK | uid::SIEGE_MODE => "terran/tank.grp",
        uid::SCV => "terran/scv.grp",
        uid::SCIENCE_VESSEL => "terran/wessel.grp",
        uid::DROPSHIP => "terran/dropship.grp",
        uid::BATTLECRUISER => "terran/battlecr.grp",
        uid::SPIDER_MINE => "terran/spider.grp",
        uid::FIREBAT => "terran/firebat.grp",
        uid::MEDIC => "terran/medic.grp",
        uid::ZERGLING => "zerg/zergling.grp",
        uid::HYDRALISK => "zerg/hydra.grp",
        uid::ULTRALISK => "zerg/ultra.grp",
        uid::BROODLING => "zerg/brood.grp",
        uid::DRONE => "zerg/drone.grp",
        uid::OVERLORD => "zerg/overlord.grp",
        uid::MUTALISK => "zerg/mutalid.grp",
        uid::GUARDIAN => "zerg/guardian.grp",
        uid::QUEEN => "zerg/queen.grp",
        uid::DEFILER => "zerg/defiler.grp",
        uid::COCOON => "zerg/cocoon.grp",
        uid::EGG => "zerg/egg.grp",
        uid::LARVA => "zerg/larva.grp",
        uid::LURKER => "zerg/lurker.grp",
        uid::CORSAIR => "protoss/corsair.grp",
        uid::DARK_TEMPLAR => "protoss/dtemplar.grp",
        uid::DARK_ARCHON => "protoss/darchn.grp",
        uid::PROBE => "protoss/probe.grp",
        uid::ZEALOT => "protoss/zealot.grp",
        uid::DRAGOON => "protoss/dragoon.grp",
        uid::HIGH_TEMPLAR => "protoss/templar.grp",
        uid::ARCHON => "protoss/archon.grp",
        uid::SHUTTLE => "protoss/shuttle.grp",
        uid::SCOUT => "protoss/scout.grp",
        uid::ARBITER => "protoss/arbiter.grp",
        uid::CARRIER => "protoss/carrier.grp",
        uid::INTERCEPTOR => "protoss/intercep.grp",
        uid::REAVER => "protoss/trilob.grp",
        uid::OBSERVER => "protoss/witness.grp",
        uid::SUPPLY_DEPOT => "terran/depot.grp",
        uid::REFINERY => "terran/refinery.grp",
        uid::BARRACKS => "terran/tbarrack.grp",
        uid::ACADEMY => "terran/academy.grp",
        uid::FACTORY => "terran/factory.grp",
        uid::STARPORT => "terran/starport.grp",
        uid::MISSILE_TURRET => "terran/missile.grp",
        uid::BUNKER => "terran/pillbox.grp",
        uid::COMSAT_STATION => "terran/comsat.grp",
        uid::NUCLEAR_SILO => "terran/nukesilo.grp",
        uid::CONTROL_TOWER => "terran/control.grp",
        uid::PHYSICS_LAB => "terran/physics.grp",
        uid::MACHINE_SHOP => "terran/machines.grp",
        uid::COMMAND_CENTER => "neutral/starbase.grp",
        uid::NEXUS => "protoss/nexus.grp",
        uid::GATEWAY => "protoss/gateway.grp",
        uid::ROBOTICS_FACILITY => "protoss/robotic.grp",
        uid::STARGATE => "protoss/stargate.grp",
        uid::PYLON => "protoss/pylon.grp",
        uid::ASSIMILATOR => "protoss/assim.grp",
        uid::FORGE => "protoss/forge.grp",
        uid::CYBERNETICS_CORE => "protoss/gencore.grp",
        uid::CITADEL_OF_ADUN => "protoss/citadel.grp",
        uid::TEMPLAR_ARCHIVES => "protoss/archives.grp",
        uid::PHOTON_CANNON => "protoss/photon.grp",
        uid::SHIELD_BATTERY => "protoss/sbattery.grp",
        uid::HATCHERY => "zerg/hatchery.grp",
        uid::LAIR => "zerg/lair.grp",
        uid::HIVE => "zerg/hive.grp",
        uid::EXTRACTOR => "zerg/extract.grp",
        uid::SPIRE | uid::GREATER_SPIRE => "zerg/spire.grp",
        uid::QUEENS_NEST => "zerg/nest.grp",
        uid::NYDUS_CANAL => "zerg/nyduspit.grp",
        uid::SUNKEN_COLONY => "zerg/scolony.grp",
        uid::CREEP_COLONY => "zerg/fcolony.grp",
        uid::MINERAL_FIELD => "neutral/min01.grp",
        uid::MINERAL_FIELD_2 => "neutral/min02.grp",
        uid::MINERAL_FIELD_3 => "neutral/min03.grp",
        uid::VESPENE_GEYSER => "neutral/geyser.grp",
        _ => return None,
    })
}

/// Draw one classic GRP frame scaled into a destination rectangle.  Used by
/// the retail console (command buttons, wireframes) and score screen.
pub fn draw_grp_frame(
    fb: &mut Framebuffer,
    grp: &Grp,
    index: usize,
    palette: &Palette,
    dest: (i32, i32, i32, i32),
    team: Option<Rgb>,
) {
    if std::env::var("GC_DEBUG_GRP").is_ok() {
        eprintln!("draw_grp_frame {index} dest {dest:?} team {team:?}");
    }
    let (dx, dy, dw, dh) = dest;
    if dw <= 0 || dh <= 0 {
        return;
    }
    let Some(frame) = grp.frame(index) else {
        return;
    };
    if frame.w <= 0 || frame.h <= 0 {
        return;
    }
    for y in 0..dh {
        let sy = (y as i64 * frame.h as i64 / dh as i64) as i32;
        for x in 0..dw {
            let sx = (x as i64 * frame.w as i64 / dw as i64) as i32;
            let c = frame.indices[(sy * frame.w + sx) as usize];
            if c != 0 {
                fb.put(dx + x, dy + y, palette.color(c, team));
            }
        }
    }
}

/// Draw a GRP frame as a flat single-colour silhouette.
///
/// Retail command icons are one-colour glyphs; tinting every non-transparent
/// pixel reproduces that regardless of which palette indices the GRP uses.
pub fn draw_grp_silhouette(
    fb: &mut Framebuffer,
    grp: &Grp,
    index: usize,
    dest: (i32, i32, i32, i32),
    color: Rgb,
) {
    let (dx, dy, dw, dh) = dest;
    if dw <= 0 || dh <= 0 {
        return;
    }
    let Some(frame) = grp.frame(index) else {
        return;
    };
    if frame.w <= 0 || frame.h <= 0 {
        return;
    }
    for y in 0..dh {
        let sy = (y as i64 * frame.h as i64 / dh as i64) as i32;
        for x in 0..dw {
            let sx = (x as i64 * frame.w as i64 / dw as i64) as i32;
            if frame.indices[(sy * frame.w + sx) as usize] != 0 {
                fb.put(dx + x, dy + y, color);
            }
        }
    }
}

pub struct SpriteBank {
    grps: HashMap<u16, Grp>,
    palette: Palette,
}
impl SpriteBank {
    pub fn load(asset_root: &Path) -> Self {
        let pal = fs::read(asset_root.join("unit.pal"))
            .or_else(|_| fs::read(asset_root.join("units.pal")))
            .ok()
            .map(|b| Palette::from_pal(&b))
            .unwrap_or_else(|| Palette {
                colors: [Rgb::new(180, 180, 180); 256],
            });
        let mut grps = HashMap::new();
        for &id in &[
            uid::MARINE,
            uid::GHOST,
            uid::VULTURE,
            uid::GOLIATH,
            uid::SIEGE_TANK,
            uid::SIEGE_MODE,
            uid::SCV,
            uid::SCIENCE_VESSEL,
            uid::DROPSHIP,
            uid::BATTLECRUISER,
            uid::SPIDER_MINE,
            uid::FIREBAT,
            uid::MEDIC,
            uid::ZERGLING,
            uid::HYDRALISK,
            uid::ULTRALISK,
            uid::BROODLING,
            uid::DRONE,
            uid::OVERLORD,
            uid::MUTALISK,
            uid::GUARDIAN,
            uid::QUEEN,
            uid::DEFILER,
            uid::COCOON,
            uid::EGG,
            uid::LARVA,
            uid::LURKER,
            uid::CORSAIR,
            uid::DARK_TEMPLAR,
            uid::DARK_ARCHON,
            uid::PROBE,
            uid::ZEALOT,
            uid::DRAGOON,
            uid::HIGH_TEMPLAR,
            uid::ARCHON,
            uid::SHUTTLE,
            uid::SCOUT,
            uid::ARBITER,
            uid::CARRIER,
            uid::INTERCEPTOR,
            uid::REAVER,
            uid::OBSERVER,
            uid::MINERAL_FIELD,
            uid::MINERAL_FIELD_2,
            uid::MINERAL_FIELD_3,
            uid::VESPENE_GEYSER,
            uid::SUPPLY_DEPOT,
            uid::REFINERY,
            uid::BARRACKS,
            uid::ACADEMY,
            uid::FACTORY,
            uid::STARPORT,
            uid::MISSILE_TURRET,
            uid::BUNKER,
            uid::COMSAT_STATION,
            uid::NUCLEAR_SILO,
            uid::CONTROL_TOWER,
            uid::PHYSICS_LAB,
            uid::MACHINE_SHOP,
            uid::COMMAND_CENTER,
            uid::NEXUS,
            uid::GATEWAY,
            uid::ROBOTICS_FACILITY,
            uid::STARGATE,
            uid::PYLON,
            uid::ASSIMILATOR,
            uid::FORGE,
            uid::CYBERNETICS_CORE,
            uid::CITADEL_OF_ADUN,
            uid::TEMPLAR_ARCHIVES,
            uid::PHOTON_CANNON,
            uid::SHIELD_BATTERY,
            uid::HATCHERY,
            uid::LAIR,
            uid::HIVE,
            uid::EXTRACTOR,
            uid::SPIRE,
            uid::GREATER_SPIRE,
            uid::QUEENS_NEST,
            uid::NYDUS_CANAL,
            uid::SUNKEN_COLONY,
            uid::CREEP_COLONY,
        ] {
            let Some(path) = sprite_path(id) else {
                continue;
            };
            let Ok(bytes) = fs::read(asset_root.join("unit").join(path)) else {
                continue;
            };
            if let Some(grp) = Grp::parse(&bytes) {
                grps.insert(id, grp);
            }
        }
        Self { grps, palette: pal }
    }

    pub fn has(&self, type_id: u16) -> bool {
        self.grps.contains_key(&type_id)
    }

    pub fn gpr(&self, type_id: u16) -> Option<&Grp> {
        self.grps.get(&type_id)
    }

    /// Draw a unit sprite.  Returns false when the unit has no bundled GRP.
    pub fn draw_unit(
        &self,
        fb: &mut Framebuffer,
        unit: &SimUnit,
        screen: (i32, i32),
        tick: u64,
    ) -> bool {
        let Some(grp) = self.grps.get(&unit.type_id) else {
            return false;
        };
        let moving = !unit.path.is_empty();
        // Mobile GRPs are laid out as animation frame * 17 + direction.
        // Buildings, resources and other static GRPs use their first frame.
        let index = if grp.frames.len() < 17 {
            0
        } else {
            let dir = ((unit.facing as i32 + 8) * 17 / 256).rem_euclid(17) as usize;
            let per_dir = (grp.frames.len() / 17).max(1);
            let anim = if moving {
                (1 + (tick / 5) as usize) % per_dir
            } else {
                0
            };
            (anim * 17 + dir).min(grp.frames.len() - 1)
        };
        let Some(frame) = grp.frame(index) else {
            return false;
        };
        if frame.w == 0 || frame.h == 0 {
            return false;
        }
        let team = Some(PLAYER_COLORS[unit.owner as usize % PLAYER_COLORS.len()]);
        let ox = screen.0 - grp.width / 2 + frame.x;
        let oy = screen.1 - grp.height / 2 + frame.y;
        for y in 0..frame.h {
            for x in 0..frame.w {
                let c = frame.indices[(y * frame.w + x) as usize];
                if c != 0 {
                    fb.put(ox + x, oy + y, self.palette.color(c, team));
                }
            }
        }
        true
    }

    pub fn draw_health(
        &self,
        fb: &mut Framebuffer,
        unit: &SimUnit,
        data: &GameData,
        screen: (i32, i32),
    ) {
        let ty = data.unit(unit.type_id);
        let max = ty.hit_points_raw as i32;
        if max <= 0 {
            return;
        }
        let w = ((ty.dimensions.right - ty.dimensions.left) as i32 / 2).clamp(16, 64);
        let hp = (unit.hp as i64 * w as i64 / max as i64).clamp(0, w as i64) as i32;
        let y = screen.1 - ((ty.dimensions.bottom - ty.dimensions.top) as i32 / 2) - 8;
        fb.rect(screen.0 - w / 2, y, w, 3, Rgb::new(20, 20, 20));
        let color = if unit.owner == 0 {
            Rgb::new(60, 220, 60)
        } else {
            Rgb::new(220, 60, 60)
        };
        fb.rect(screen.0 - w / 2, y, hp, 3, color);
        if unit.shields > 0 {
            let sw = (unit.shields as i64 * w as i64 / (ty.shield_points as i64 * 256).max(1))
                .clamp(0, w as i64) as i32;
            fb.rect(screen.0 - w / 2, y - 3, sw, 2, Rgb::new(80, 160, 255));
        }
    }
}

/// Draw a fallback box for units without a bundled sprite (buildings, effects).
pub fn draw_unit_box(
    fb: &mut Framebuffer,
    unit: &SimUnit,
    data: &GameData,
    screen: (i32, i32),
    selected: bool,
) {
    let ty = data.unit(unit.type_id);
    let w = ((ty.dimensions.right - ty.dimensions.left) as i32).clamp(14, 96);
    let h = ((ty.dimensions.bottom - ty.dimensions.top) as i32).clamp(14, 96);
    let color = PLAYER_COLORS[unit.owner as usize % PLAYER_COLORS.len()];
    let fill = color.blend(Rgb::new(10, 10, 10), 150);
    fb.rect(screen.0 - w / 2, screen.1 - h / 2, w, h, fill);
    fb.rect_outline(screen.0 - w / 2, screen.1 - h / 2, w, h, color);
    let label = data.unit_name(unit.type_id);
    let short: String = label
        .split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(3)
        .collect();
    fb.text_centered(screen.0, screen.1 - 4, &short, Rgb::new(240, 240, 240), 1);
    if selected {
        fb.circle_outline(
            screen.0,
            screen.1,
            (w.max(h) / 2 + 6).min(64),
            Rgb::new(80, 255, 80),
        );
    }
}

pub fn asset_root() -> PathBuf {
    if let Ok(p) = std::env::var("GALAXYCRAFT_ASSETS") {
        return PathBuf::from(p);
    }
    // Prefer the crate-relative assets directory, then the working directory.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    if manifest.exists() {
        manifest
    } else {
        PathBuf::from("assets")
    }
}

#[cfg(test)]
mod hd_terrain_tests {
    use super::HdTerrain;

    /// One 4x4 DXT1 record whose single block is solid red.
    fn record() -> Vec<u8> {
        let mut dds = b"DDS ".to_vec();
        let mut hdr = [0u8; 124];
        hdr[0] = 124;
        hdr[8..12].copy_from_slice(&4u32.to_le_bytes()); // height
        hdr[12..16].copy_from_slice(&4u32.to_le_bytes()); // width
        hdr[72] = 32; // pixel format size
        hdr[76] = 4; // DDPF_FOURCC
        hdr[80..84].copy_from_slice(b"DXT1");
        dds.extend_from_slice(&hdr);
        dds.extend_from_slice(&[0x00, 0xf8, 0x00, 0xf8, 0, 0, 0, 0]);
        let mut r = Vec::new();
        r.extend_from_slice(&0u32.to_le_bytes());
        r.extend_from_slice(&4u16.to_le_bytes());
        r.extend_from_slice(&4u16.to_le_bytes());
        r.extend_from_slice(&(dds.len() as u32).to_le_bytes());
        r.extend_from_slice(&dds);
        r
    }

    #[test]
    fn archive_index_skips_the_eight_byte_header() {
        let mut bytes = vec![0u8; 8];
        bytes[4] = 2;
        bytes.extend(record());
        bytes.extend(record());
        let total = bytes.len() as u32;
        bytes[0..4].copy_from_slice(&total.to_le_bytes());
        let t = HdTerrain::parse(bytes).expect("index");
        assert_eq!(t.len(), 2);
        let px = t.tile(1, 2).expect("decode");
        assert_eq!(px.len(), 4);
        // Solid red, scaled by the measured terrain gain (255 * 0.91).
        assert_eq!(px[0] & 0x00ff_ffff, 0x00e8_0000, "red after resample");
        assert!(HdTerrain::parse(vec![0u8; 8]).is_none());
    }
}

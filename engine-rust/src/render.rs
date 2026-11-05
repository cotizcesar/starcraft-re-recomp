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
        if let Some(team) = team
            && (8..=15).contains(&index)
        {
            return team;
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
    /// SD creep-edge frames (`tileset/<era>.grp`, 37 palette-indexed frames
    /// with their offsets inside the 32 px tile), when the install has them.
    creep_grp: Option<Grp>,
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
    /// 4x creep-edge frames (`tileset/<era>.dds.grp`) with their SD
    /// placement (`tileset/<era>.grp`: x/y offset inside the 32 px tile).
    creep_edges: Vec<(crate::dds::DdsImage, u8, u8)>,
}

/// Retail creep-edge frame of a non-creep tile from its 9-bit neighbour
/// mask (bit order (1,1) (0,1) (-1,1) (1,0) (-1,0) (1,-1) (0,-1) (-1,-1)
/// self): 0 = no edge, n = grp frame n - 1.  Construction as in OpenBW's
/// `load_image_data` (ui.h).
pub fn creep_edge_frame_table() -> [u8; 256] {
    let canon = |i: usize| {
        let mut v = 0usize;
        if i & 2 != 0 {
            v |= 0x10;
        }
        if i & 8 != 0 {
            v |= 0x24;
        }
        if i & 0x10 != 0 {
            v |= 9;
        }
        if i & 0x40 != 0 {
            v |= 2;
        }
        if i & 0xc0 == 0xc0 {
            v |= 1;
        }
        if i & 0x60 == 0x60 {
            v |= 4;
        }
        if i & 3 == 3 {
            v |= 0x20;
        }
        if i & 6 == 6 {
            v |= 8;
        }
        if v & 0x21 == 0x21 {
            v |= 0x40;
        }
        if v & 0xc == 0xc {
            v |= 0x40;
        }
        v
    };
    let canon_of: Vec<usize> = (0..256).map(canon).collect();
    let mut rank = [0u8; 128];
    let mut n = 0u8;
    for (v, r) in rank.iter_mut().enumerate() {
        if canon_of.contains(&v) {
            *r = n;
            n += 1;
        }
    }
    let mut out = [0u8; 256];
    for (i, o) in out.iter_mut().enumerate() {
        *o = rank[canon_of[i]];
    }
    out
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
            creep_edges: Vec::new(),
        })
    }

    pub fn load(root: &Path, era: u16) -> Option<Self> {
        let dir = root.join("hd/tileset");
        let name = era_name(era);
        let mut t = Self::parse(fs::read(dir.join(format!("{name}.dds.vr4"))).ok()?)?;
        let hd = fs::read(dir.join(format!("{name}.dds.grp")))
            .ok()
            .and_then(|b| crate::dds::DdsGrp::parse(&b));
        let sd = fs::read(dir.join(format!("{name}.grp"))).ok();
        if let (Some(hd), Some(sd)) = (hd, sd) {
            t.creep_edges = hd
                .frames
                .into_iter()
                .enumerate()
                .map(|(i, mut img)| {
                    // SD frame header: {u8 x, u8 y, u8 w, u8 h, u32 off}.
                    let at = 6 + i * 8;
                    let (x, y) = (
                        sd.get(at).copied().unwrap_or(0),
                        sd.get(at + 1).copied().unwrap_or(0),
                    );
                    for p in img.pixels.iter_mut() {
                        *p = apply_gain(*p);
                    }
                    (img, x, y)
                })
                .collect();
        }
        Some(t)
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
        for p in px.iter_mut() {
            *p = apply_gain(*p);
        }
        let t = std::rc::Rc::new(px);
        self.cache.borrow_mut().insert((mega, size), t.clone());
        Some(t)
    }

    /// Draw the terrain of `scenario` into the screen rectangle `dest`
    /// (`x, y, w, h`), with the camera at game px `cam` and `s` screen px per
    /// game px.  `tileset` resolves MTXM entries to megatiles; tiles for
    /// which `creep(tx, ty)` returns an MTXM entry draw that creep tile
    /// (cv5 group 1, see [`CreepTiles`]); the others get creep edges.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        fb: &mut Framebuffer,
        tileset: &Tileset,
        scenario: &Scenario,
        cam: (i32, i32),
        dest: (i32, i32, i32, i32),
        s: f32,
        creep: &dyn Fn(i32, i32) -> Option<u16>,
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
                let mtxm = creep(tx, ty).unwrap_or(mtxm);
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
                if let Some(frame) = creep_edge_frame(creep, tx, ty)
                    && let Some((img, ox, oy)) = self.creep_edges.get(frame)
                {
                    let k = s / 4.0;
                    let (w, h) = (
                        (img.width as f32 * k).round() as i32,
                        (img.height as f32 * k).round() as i32,
                    );
                    // A positive SD offset means the frame hugs the
                    // tile's right / bottom edge (the HD frames are
                    // padded, so the offset itself does not scale).
                    let ex = if *ox > 0 { sx + size as i32 - w } else { sx };
                    let ey = if *oy > 0 { sy + size as i32 - h } else { sy };
                    img.draw_scaled(fb, ex, ey, w, h);
                }
            }
        }
    }
}

/// Retail draws the HD terrain darker than the raw texels: lit grass and
/// dirt measure 0.91 of the DDS colour on 135051 / 135020.
fn apply_gain(p: u32) -> u32 {
    let scale = |c: u32| ((c as f32) * HD_TERRAIN_GAIN).round() as u32;
    (p & 0xff00_0000)
        | (scale((p >> 16) & 0xff) << 16)
        | (scale((p >> 8) & 0xff) << 8)
        | scale(p & 0xff)
}

/// Measured brightness of the retail HD terrain relative to its texels.
pub const HD_TERRAIN_GAIN: f32 = 0.91;

/// Minimap camera box `(x, y, w, h, stroke)` in screen px for a minimap
/// whose left edge is `x0` and width `w`, top `y0`, over a `tiles` map:
/// the view `cam` (x0, y0, x1, y1 game px) on the classic 128 px grid,
/// position floored and size ceiled, stroke one classic px.
pub fn minimap_view_box(
    (x0, w): (i32, i32),
    y0: i32,
    (mw, mh): (usize, usize),
    cam: (i32, i32, i32, i32),
) -> (i32, i32, i32, i32, i32) {
    let classic_per_tile = 128.0 / mw.max(mh).max(1) as f32;
    let q = w as f32 / (mw.max(1) as f32 * classic_per_tile);
    let to_classic = |v: i32| v as f32 / TILE_PX as f32 * classic_per_tile;
    let sx = (to_classic(cam.0).floor() * q).round() as i32;
    let sy = (to_classic(cam.1).floor() * q).round() as i32;
    let sw = (to_classic(cam.2 - cam.0).ceil() * q).round() as i32;
    let sh = (to_classic(cam.3 - cam.1).ceil() * q).round() as i32;
    (x0 + sx, y0 + sy, sw, sh, q.round().max(1.0) as i32)
}

/// Minimap background mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MinimapTerrain {
    /// Black radar: blips and camera box only.
    Hidden,
    /// One averaged colour per tile.
    Tiles,
    /// Keep what the caller already painted in the rect.
    Prepainted,
}

/// One unit on the minimap: position and placement box in game px.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MinimapDot {
    pub x: i32,
    pub y: i32,
    pub owner: u8,
    /// `units.dat` placement box (game px).
    pub placement: (i32, i32),
    pub building: bool,
    pub resource: bool,
}

/// Minimap blip of a unit in whole tiles `(tx, ty, w, h)`: OpenBW's
/// `draw_minimap` rule (placement box / 32, buildings clamped to 4 tiles,
/// at least 2 x 2, top-left at `(position - placement / 2) / 32`).  FACT
/// (2026-11-03): the retail 135051 minimap matches it row for row (geyser
/// 12 x 6 px, Nexus plus probes 12 x 12, the overlapping mineral fields);
/// the earlier square as wide as the placement box did not.
pub fn minimap_blip_tiles(d: &MinimapDot) -> (i32, i32, i32, i32) {
    let min = 2;
    let (pw, ph) = d.placement;
    let (mut w, mut h) = (pw / TILE_PX, ph / TILE_PX);
    if d.building {
        w = w.min(4);
        h = h.min(4);
    }
    (
        (d.x - pw / 2).div_euclid(TILE_PX),
        (d.y - ph / 2).div_euclid(TILE_PX),
        w.max(min),
        h.max(min),
    )
}

/// Minimap colours measured on 135051: own units and resources.
pub const MINIMAP_SELF: Rgb = Rgb::new(16, 252, 24);
pub const MINIMAP_RESOURCE: Rgb = Rgb::new(0, 228, 252);

/// Fog darkness of an explored tile that is not in sight (0..=255).
pub const FOG_EXPLORED_ALPHA: f32 = 150.0;
/// Horizontal offset (game px) at which the fog grid is sampled: retail's
/// fog edges sit half a tile left of a tile-centred grid on every reference
/// (134905 / 135020 / 135051, 17-80 px at 3x).  Measured by sweep (0, 16,
/// 24, 32; best 16); vertical offsets did not help.  Unexplained, so kept as
/// a measured constant.
const FOG_SAMPLE_DX: f32 = 16.0;
/// Fog grid samples per tile, and the box-filter radius in samples.
///
/// The edge of vision is a box blur of the per-tile darkness (a linear
/// ramp 11 samples = 2.75 tiles wide).  INFERENCE (2026-11-03): retail's
/// measured ramp is close to linear (0.15 -> 0.9 over ~170 screen px on
/// 135020 / 135051), and against the previous Gaussian (sigma 0.8 tiles) the
/// box lowers the frame mean on all three references: 3.438 -> 3.428
/// (135051), 2.510 -> 2.499 (135020), 3.222 -> 3.193 (134905); widths 2.0 /
/// 3.0 tiles and two passes score worse.  Falsifier: a capture whose fog
/// edge has a visible Gaussian tail.
const FOG_SUB: i32 = 4;
const FOG_BOX_RADIUS: i32 = 5;

/// In-place separable box blur of radius `r` over a `w x h` grid (clamped
/// edges).
fn box_blur(v: &mut [f32], w: usize, h: usize, r: i32) {
    let n = (2 * r + 1) as f32;
    let mut tmp = vec![0f32; v.len()];
    for y in 0..h {
        for x in 0..w {
            let acc: f32 = (-r..=r)
                .map(|d| v[y * w + (x as i32 + d).clamp(0, w as i32 - 1) as usize])
                .sum();
            tmp[y * w + x] = acc / n;
        }
    }
    for y in 0..h {
        for x in 0..w {
            let acc: f32 = (-r..=r)
                .map(|d| tmp[(y as i32 + d).clamp(0, h as i32 - 1) as usize * w + x])
                .sum();
            v[y * w + x] = acc / n;
        }
    }
}

/// Stretch the first `src_w` columns of `rect` (`x, y, w, h`) across its
/// full width with linear filtering, sampling at pixel centres.
///
/// Retail's playfield has an integer logical width (853 game px in a
/// 2560 px 16:9 box) and is scaled to fill the box, so at 1440p one game px
/// is 2560 / 853 = 3.0012 screen px horizontally and exactly 3 vertically.
/// The HD world is drawn at 3.0 here (2559 px) and then stretched.  FACT,
/// measured on 134905 / 135051 terrain: the best horizontal shift grows
/// linearly with x (0.00 at x 120-280, 0.50 at 1240, 1.00 at 2360;
/// slope 0.00046 against 0.00039 predicted, no vertical component).
pub fn stretch_columns(fb: &mut Framebuffer, rect: (i32, i32, i32, i32), src_w: i32) {
    let (x0, y0, w, h) = rect;
    if src_w <= 0 || src_w >= w {
        return;
    }
    let k = src_w as f32 / w as f32;
    let taps: Vec<(i32, i32, f32)> = (0..w)
        .map(|x| {
            let f = ((x as f32 + 0.5) * k - 0.5).max(0.0);
            let i = (f.floor() as i32).min(src_w - 1);
            (i, (i + 1).min(src_w - 1), f - i as f32)
        })
        .collect();
    let mut row = vec![Rgb(0); src_w as usize];
    for y in y0..y0 + h {
        for (i, p) in row.iter_mut().enumerate() {
            *p = fb.get(x0 + i as i32, y);
        }
        for (x, &(a, b, t)) in taps.iter().enumerate() {
            let (pa, pb) = (row[a as usize].0, row[b as usize].0);
            let mix = |sh: u32| {
                let ca = ((pa >> sh) & 0xff) as f32;
                let cb = ((pb >> sh) & 0xff) as f32;
                (ca + (cb - ca) * t).round() as u8
            };
            fb.put(x0 + x as i32, y, Rgb::new(mix(16), mix(8), mix(0)));
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
    // Upsample to FOG_SUB samples per tile (piecewise constant) and box-blur
    // (see FOG_BOX_RADIUS).
    let sub = FOG_SUB;
    let (gw, gh) = (tw * sub, th * sub);
    let mut fine = vec![0f32; (gw * gh) as usize];
    for y in 0..gh {
        for x in 0..gw {
            fine[(y * gw + x) as usize] = grid[((y / sub) * tw + x / sub) as usize];
        }
    }
    box_blur(&mut fine, gw as usize, gh as usize, FOG_BOX_RADIUS);
    let at = |i: i32, j: i32| fine[(j.clamp(0, gh - 1) * gw + i.clamp(0, gw - 1)) as usize];
    let step = TILE_PX as f32 / sub as f32;
    for y in dy.max(0)..(dy + dh).min(fb.height) {
        let gy = cam.1 as f32 + (y - dy) as f32 / s;
        let fy = (gy - (ty0 * TILE_PX) as f32) / step - 0.5;
        let j = fy.floor() as i32;
        let wy = fy - j as f32;
        for x in dx.max(0)..(dx + dw).min(fb.width) {
            let gx = cam.0 as f32 + (x - dx) as f32 / s;
            let fx = (gx + FOG_SAMPLE_DX - (tx0 * TILE_PX) as f32) / step - 0.5;
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

/// Client-side creep tile picker: which of the 13 creep megatiles (cv5
/// group 1) each creep tile draws with.
///
/// FACT (2026-11-03, 64-bit 1.23.10 in-game dump
/// `sc_mem_ingame64_20261028_0814`, file offsets): the client picks a tile's
/// creep megatile when the tile gains creep (`0x64fb30`, reached through
/// `0x64fbe0` from the creep set/spread loops at `0x64f967` and `0x650010`):
/// `r = rand() % 100; v = rand(); tile = 0x10 + (r < 4 ? 6 + v % 7 : v % 6)`.
/// `rand` is the CRT generator at `0xa12380`
/// (`s = s * 214013 + 2531011; (s >> 16) & 0x7fff`), seeded with
/// `srand(time(NULL))` at `0x2b17e2`, so the variants are per-session state
/// and not part of the synchronised game.  Falsified on the way: OpenBW's
/// prefilled 256 x 256 table (Borland or MSVC LCG, stride 128 or 256) does
/// not reproduce the 160 tiles recorded from the 134905 capture for any of
/// the 2^32 generator states, even allowing 60 mismatches.
#[derive(Clone, Debug, Default)]
pub struct CreepTiles {
    width: usize,
    /// MTXM entry per tile (`0` = no creep).
    tiles: Vec<u16>,
}

/// The client's CRT `rand()` (64-bit 1.23.10 `0xa12380`): one stream per
/// process, shared by every non-synchronised random choice (creep tiles,
/// portrait clips, ...).  Retail seeds it with `srand(time(NULL))`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrtRand(u32);

impl CrtRand {
    pub fn new(seed: u32) -> Self {
        Self(seed)
    }

    /// `s = s * 214013 + 2531011; (s >> 16) & 0x7fff`.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(214_013).wrapping_add(2_531_011);
        (self.0 >> 16) & 0x7fff
    }
}

impl CreepTiles {
    /// Pick the MTXM entry of a tile that just gained creep (`0x64fb30`).
    pub fn pick(rand: &mut CrtRand) -> u16 {
        let roll = rand.next() % 100;
        let v = rand.next();
        0x10 | if roll < 4 { 6 + v % 7 } else { v % 6 } as u16
    }

    /// Follow the sim's creep grid (`World::creep_grid`, maturity per tile):
    /// a tile that gained creep picks its megatile, one that lost it forgets
    /// it.  A new map (different size) starts empty; the CRT stream carries
    /// on, as it does in the retail process.
    pub fn update(&mut self, rand: &mut CrtRand, grid: &[u8], width: usize) {
        if self.width != width || self.tiles.len() != grid.len() {
            self.width = width;
            self.tiles = vec![0; grid.len()];
        }
        for (tile, &maturity) in self.tiles.iter_mut().zip(grid) {
            match (maturity > 0, *tile != 0) {
                (true, false) => *tile = Self::pick(rand),
                (false, true) => *tile = 0,
                _ => {}
            }
        }
    }

    /// MTXM entry the creep tile `(tx, ty)` draws with, `None` off creep.
    pub fn get(&self, tx: i32, ty: i32) -> Option<u16> {
        if tx < 0 || ty < 0 || tx as usize >= self.width {
            return None;
        }
        let v = *self.tiles.get(ty as usize * self.width + tx as usize)?;
        (v != 0).then_some(v)
    }
}

/// Creep-edge frame (0-based index into `tileset/<era>.grp`) drawn on the
/// non-creep tile `(tx, ty)`, from its creep neighbours (OpenBW
/// `draw_tiles`); `None` on creep tiles and where no edge applies.
pub fn creep_edge_frame(
    creep: &dyn Fn(i32, i32) -> Option<u16>,
    tx: i32,
    ty: i32,
) -> Option<usize> {
    if creep(tx, ty).is_some() {
        return None;
    }
    let mut mask = 0usize;
    for (bit, (ax, ay)) in [
        (1, 1),
        (0, 1),
        (-1, 1),
        (1, 0),
        (-1, 0),
        (1, -1),
        (0, -1),
        (-1, -1),
    ]
    .into_iter()
    .enumerate()
    {
        if creep(tx + ax, ty + ay).is_some() {
            mask |= 1 << bit;
        }
    }
    (creep_edge_frame_table()[mask] as usize).checked_sub(1)
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

/// Area-filter an image down to `w`x`h` (each output pixel averages the
/// exact source footprint, fractional texels weighted by coverage), in
/// premultiplied alpha; the result is straight alpha, as [`blit_tile`]
/// blends it.
///
/// At 1440p a 128 px HD megatile becomes 96 px (1.333 texels per pixel).
/// The earlier integer-boundary box averaged alternating 1- and 2-texel
/// spans and stored premultiplied colour; on the (8)The Hunters terrain the
/// area filter lowers the mean error from 1.81 / 2.55 to 1.61 / 2.50
/// (135020 / 135051).
fn downsample_rgba(img: &crate::dds::DdsImage, w: usize, h: usize) -> Box<[u32]> {
    let mut out = vec![0u32; w * h];
    if img.width == 0 || img.height == 0 || w == 0 || h == 0 {
        return out.into_boxed_slice();
    }
    let (sw, sh) = (img.width as f32 / w as f32, img.height as f32 / h as f32);
    let texel = |x: i32, y: i32| {
        let p = img.pixel(x as usize, y as usize);
        let a = (p >> 24) as f32 / 255.0;
        [
            ((p >> 16) & 0xff) as f32 * a,
            ((p >> 8) & 0xff) as f32 * a,
            (p & 0xff) as f32 * a,
            a,
        ]
    };
    // Coverage of source cells [i, i + 1) by the span [lo, hi).
    let spans = |n: usize, k: f32, max: usize| -> Vec<Vec<(i32, f32)>> {
        (0..n)
            .map(|t| {
                let (lo, hi) = (t as f32 * k, ((t + 1) as f32 * k).min(max as f32));
                (lo.floor() as i32..hi.ceil() as i32)
                    .map(|i| (i, (hi.min(i as f32 + 1.0) - lo.max(i as f32)).max(0.0)))
                    .filter(|&(_, c)| c > 0.0)
                    .collect()
            })
            .collect()
    };
    let xs = spans(w, sw, img.width);
    let ys = spans(h, sh, img.height);
    for (ty, ycov) in ys.iter().enumerate() {
        for (tx, xcov) in xs.iter().enumerate() {
            let mut acc = [0f32; 4];
            let mut area = 0.0;
            for &(sy, wy) in ycov {
                for &(sx, wx) in xcov {
                    let t = texel(sx, sy);
                    for (a, v) in acc.iter_mut().zip(t) {
                        *a += v * wx * wy;
                    }
                    area += wx * wy;
                }
            }
            if area <= 0.0 || acc[3] <= 0.0 {
                continue;
            }
            let a = acc[3] / area;
            let c = |v: f32| (v / acc[3]).round().clamp(0.0, 255.0) as u32;
            out[ty * w + tx] = (((a * 255.0).round() as u32) << 24)
                | (c(acc[0]) << 16)
                | (c(acc[1]) << 8)
                | c(acc[2]);
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
            creep_grp: dir
                .parent()
                .and_then(|root| fs::read(root.join("hd/tileset").join(format!("{name}.grp"))).ok())
                .and_then(|b| Grp::parse(&b)),
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

    /// Draw creep over the terrain as the retail client does (OpenBW
    /// `draw_tiles`): every tile for which `creep` returns an MTXM entry
    /// (cv5 group 1, see [`CreepTiles`]) draws that tile, and the other
    /// tiles draw their creep-edge frame from `tileset/<era>.grp` at the
    /// frame's offset.  The art is static: retail does not animate creep.
    pub fn draw_creep(
        &self,
        fb: &mut Framebuffer,
        scenario: &Scenario,
        cam: (i32, i32),
        creep: &dyn Fn(i32, i32) -> Option<u16>,
    ) {
        let first_x = (cam.0 / TILE_PX).max(0);
        let first_y = (cam.1 / TILE_PX).max(0);
        let last_x = ((cam.0 + fb.width) / TILE_PX + 1).min(scenario.width as i32);
        let last_y = ((cam.1 + fb.height) / TILE_PX + 1).min(scenario.height as i32);
        for ty in first_y..last_y {
            for tx in first_x..last_x {
                let (sx, sy) = (tx * TILE_PX - cam.0, ty * TILE_PX - cam.1);
                if let Some(mtxm) = creep(tx, ty) {
                    self.draw_tile(fb, mtxm, sx, sy);
                    continue;
                }
                let Some(frame) = creep_edge_frame(creep, tx, ty)
                    .and_then(|f| self.creep_grp.as_ref()?.frames.get(f))
                else {
                    continue;
                };
                for y in 0..frame.h {
                    for x in 0..frame.w {
                        let c = frame.indices[(y * frame.w + x) as usize];
                        if c != 0 {
                            fb.put(
                                sx + frame.x + x,
                                sy + frame.y + y,
                                self.palette.colors[c as usize],
                            );
                        }
                    }
                }
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
        units: &[MinimapDot],
        cam: (i32, i32, i32, i32),
    ) {
        self.draw_minimap_with_terrain(fb, rect, units, cam, None, 0, MinimapTerrain::Tiles)
    }

    /// Radar with retail fog of war: unexplored tiles are black and explored
    /// but not currently visible ones are dimmed.  `MinimapTerrain::Hidden`
    /// is the black radar most references show (blips and camera box only);
    /// `Prepainted` keeps a background the caller already drew (the HD
    /// terrain minimap, `sc_view::draw_hud_minimap`).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_minimap_with_terrain(
        &self,
        fb: &mut Framebuffer,
        rect: (i32, i32, i32, i32),
        units: &[MinimapDot],
        cam: (i32, i32, i32, i32),
        fog: Option<&crate::vision::Vision>,
        owner: u8,
        terrain: MinimapTerrain,
    ) {
        let (x, y, w, h) = rect;
        let (mw, mh) = self.minimap_size;
        if mw == 0 || mh == 0 {
            return;
        }
        let sx = w as f32 / mw as f32;
        let sy = h as f32 / mh as f32;
        if terrain == MinimapTerrain::Hidden {
            fb.rect(x, y, w, h, Rgb::new(0, 0, 0));
        }
        for ty in 0..mh {
            if terrain != MinimapTerrain::Tiles {
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
        // Retail blips (`minimap_blip_tiles`): local units green, resources
        // cyan, others in player colour, each with a 1 px black outline
        // drawn under every fill, so overlapping blips share one outline
        // around their union (134905, visible over the terrain minimap).
        let rects: Vec<(i32, i32, i32, i32, Rgb)> = units
            .iter()
            .map(|d| {
                let (tx, ty, tw, th) = minimap_blip_tiles(d);
                let px = x + (tx as f32 * sx) as i32;
                let py = y + (ty as f32 * sy) as i32;
                let bw = x + ((tx + tw) as f32 * sx) as i32 - px;
                let bh = y + ((ty + th) as f32 * sy) as i32 - py;
                let c = if d.resource {
                    MINIMAP_RESOURCE
                } else if d.owner == owner {
                    MINIMAP_SELF
                } else {
                    PLAYER_COLORS[d.owner as usize % PLAYER_COLORS.len()]
                };
                (px, py, bw, bh, c)
            })
            .collect();
        for &(px, py, bw, bh, _) in &rects {
            fb.rect(px - 1, py - 1, bw + 2, bh + 2, Rgb::new(0, 0, 0));
        }
        for &(px, py, bw, bh, c) in &rects {
            fb.rect(px, py, bw, bh, c);
        }
        // Camera viewport, on the classic 128 px minimap grid (134905 /
        // 135051: every edge reproduced): position floored and size ceiled
        // in classic px, scaled to the screen, with a 1-classic-px white
        // stroke outside the view.
        let (vx, vy, vw, vh, t) = minimap_view_box((x, w), y, (mw, mh), cam);
        let white = Rgb::new(255, 255, 255);
        fb.rect(vx - t, vy - t, vw + 2 * t, t, white);
        fb.rect(vx - t, vy + vh, vw + 2 * t, t, white);
        fb.rect(vx - t, vy, t, vh, white);
        fb.rect(vx + vw, vy, t, vh, white);
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
mod minimap_blip_tests {
    use super::{MinimapDot, minimap_blip_tiles};

    fn dot(x: i32, y: i32, placement: (i32, i32), building: bool) -> MinimapDot {
        MinimapDot {
            x,
            y,
            owner: 0,
            placement,
            building,
            resource: false,
        }
    }

    #[test]
    fn blips_follow_the_placement_box_in_whole_tiles() {
        // Geyser 128x64 centred at (1000, 3000): 4 x 2 tiles at (29, 92).
        assert_eq!(
            minimap_blip_tiles(&dot(1000, 3000, (128, 64), true)),
            (29, 92, 4, 2)
        );
        // Mineral field 64x32: at least 2 x 2 (OpenBW, and 135051's rows).
        let mineral = minimap_blip_tiles(&dot(1000, 3000, (64, 32), false));
        assert_eq!((mineral.2, mineral.3), (2, 2));
        // Nexus 128x96: 4 x 3; buildings are clamped to 4 tiles.
        assert_eq!(minimap_blip_tiles(&dot(1000, 3000, (128, 96), true)).2, 4);
        assert_eq!(minimap_blip_tiles(&dot(1000, 3000, (128, 96), true)).3, 3);
        assert_eq!(minimap_blip_tiles(&dot(1000, 3000, (192, 160), true)).2, 4);
        // Units are not clamped.
        assert_eq!(minimap_blip_tiles(&dot(1000, 3000, (192, 160), false)).2, 6);
    }
}

#[cfg(test)]
mod view_scale_tests {
    use super::{Framebuffer, Rgb, box_blur, downsample_rgba, stretch_columns};
    use crate::dds::DdsImage;

    #[test]
    fn the_853_px_view_is_stretched_across_the_2560_px_box() {
        // 2559 drawn columns (853 game px at 3x): black, then white from
        // source x 2000 on; column 2559 was never drawn.
        let mut fb = Framebuffer::new(2560, 1);
        for x in 0..2559 {
            let v = if x < 2000 { 0 } else { 255 };
            fb.put(x, 0, Rgb::new(v, v, v));
        }
        fb.put(2559, 0, Rgb::new(255, 0, 0));
        stretch_columns(&mut fb, (0, 0, 2560, 1), 2559);
        let at = |x: i32| fb.get(x, 0);
        // The edge moves ~0.78 px right (retail drift measured on the
        // terrain: 0.95 px at x 2200, 1.00 at 2360): output 2000 samples
        // source 2000.5 * 2559 / 2560 - 0.5 = 1999.22 (a 22 % blend, 56)
        // instead of the white texel; the 50 % point moves to 2000.28.
        assert_eq!(at(1999), Rgb::new(0, 0, 0));
        assert_eq!(at(2000), Rgb::new(56, 56, 56));
        assert_eq!(at(2001), Rgb::new(255, 255, 255));
        // The undrawn column is replaced by the last drawn one.
        assert_eq!(at(2559), Rgb::new(255, 255, 255));
        // Equal widths are a no-op.
        let mut same = Framebuffer::new(4, 1);
        same.put(1, 0, Rgb::new(9, 9, 9));
        stretch_columns(&mut same, (0, 0, 4, 1), 4);
        assert_eq!(same.get(1, 0), Rgb::new(9, 9, 9));
    }

    fn image(w: usize, h: usize, px: &[u32]) -> DdsImage {
        DdsImage {
            width: w,
            height: h,
            pixels: px.to_vec(),
        }
    }

    #[test]
    fn hd_tiles_are_area_filtered_with_straight_alpha_out() {
        // 4 texels -> 3 px (the 128 -> 96 ratio): coverage 1 + 1/3,
        // 2/3 + 2/3, 1/3 + 1.  The old integer box gave 0, 255, 127.
        let g = |v: u32| 0xff00_0000 | (v << 16) | (v << 8) | v;
        let img = image(4, 1, &[g(0), g(255), g(0), g(255)]);
        let out = downsample_rgba(&img, 3, 1);
        let red = |p: u32| (p >> 16) & 0xff;
        // The middle pixel is exactly 127.5, so allow either rounding.
        let got: Vec<i32> = out.iter().map(|&p| red(p) as i32).collect();
        for (g, want) in got.iter().zip([64, 128, 191]) {
            assert!((g - want).abs() <= 1, "{got:?}");
        }
        assert!(out.iter().all(|&p| p >> 24 == 255));
        // A half-transparent texel keeps its colour (blit_tile blends with
        // straight alpha); the old path stored the premultiplied 100.
        let img = image(1, 1, &[(128 << 24) | (200 << 16) | (200 << 8) | 200]);
        let out = downsample_rgba(&img, 1, 1);
        assert_eq!((out[0] >> 24, red(out[0])), (128, 200));
    }

    #[test]
    fn the_fog_edge_is_a_linear_ramp() {
        // Step from 0 to 255 between samples 19 and 20 of a 40-wide row.
        let (w, h) = (40usize, 1usize);
        let mut v: Vec<f32> = (0..w).map(|x| if x < 20 { 0.0 } else { 255.0 }).collect();
        box_blur(&mut v, w, h, 5);
        // 11-sample ramp: 0 up to x 14, then equal steps of 255 / 11.
        assert_eq!(v[14], 0.0);
        let step = 255.0 / 11.0;
        for x in 15..25 {
            assert!((v[x] - v[x - 1] - step).abs() < 1e-3, "x {x}");
        }
        assert!((v[25] - 255.0).abs() < 1e-3);
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
    fn minimap_box_follows_the_classic_grid() {
        // 134905: 384 px minimap at x 18, y 1044, 128-tile map, camera
        // (1936, 112) + 853 x 368 view -> inner box 198..279 x 1053..1089,
        // 3 px stroke (measured edges 195/281 and 1050/1091).
        let b = super::minimap_view_box(
            (18, 384),
            1044,
            (128, 128),
            (1936, 112, 1936 + 853, 112 + 368),
        );
        assert_eq!(b, (198, 1053, 81, 36, 3));
    }

    #[test]
    fn creep_picker_is_the_crt_rand_rule_of_the_retail_client() {
        // MSVC `srand(1)` yields 41, 18467, 6334, 26500: the first pick
        // rolls 41 (common tile) with 18467 % 6 = 5, the second 34 with
        // 26500 % 6 = 4.
        use super::{CreepTiles, CrtRand};
        let mut r = CrtRand::new(1);
        assert_eq!(CreepTiles::pick(&mut r), 0x15);
        assert_eq!(CreepTiles::pick(&mut r), 0x14);
        // 4 % of picks are the speckled tiles 6..=12, the rest 0..=5.
        let mut r = CrtRand::new(12345);
        let n = 100_000;
        let rare = (0..n)
            .map(|_| CreepTiles::pick(&mut r))
            .inspect(|&t| assert!((0x10..=0x1c).contains(&t)))
            .filter(|&t| t >= 0x16)
            .count();
        assert!((3_500..4_500).contains(&rare), "{rare} rare tiles");
    }

    #[test]
    fn creep_tiles_pick_once_per_gain_and_forget_on_loss() {
        let mut c = super::CreepTiles::default();
        let r = &mut super::CrtRand::new(1);
        let mut grid = vec![0u8; 6];
        grid[4] = 3;
        c.update(r, &grid, 3);
        assert_eq!(c.get(1, 1), Some(0x15));
        assert_eq!(c.get(0, 0), None);
        // Maturity changes do not re-pick; a new tile draws the next pair.
        grid[4] = 1;
        grid[0] = 2;
        c.update(r, &grid, 3);
        assert_eq!((c.get(1, 1), c.get(0, 0)), (Some(0x15), Some(0x14)));
        // Losing creep forgets the tile; regaining it picks afresh.
        grid[4] = 0;
        c.update(r, &grid, 3);
        assert_eq!(c.get(1, 1), None);
        grid[4] = 3;
        c.update(r, &grid, 3);
        assert!(c.get(1, 1).is_some());
        assert_eq!(c.get(3, 0), None, "outside the map width");
    }

    #[test]
    fn creep_edges_only_surround_creep() {
        let creep = |x: i32, y: i32| (x == 5 && y == 5).then_some(0x10);
        assert_eq!(super::creep_edge_frame(&creep, 5, 5), None, "creep tile");
        assert_eq!(super::creep_edge_frame(&creep, 9, 9), None, "far away");
        let left = super::creep_edge_frame(&creep, 4, 5).expect("edge left of creep");
        let right = super::creep_edge_frame(&creep, 6, 5).expect("edge right of creep");
        assert_ne!(left, right);
    }

    #[test]
    fn creep_edge_table_matches_the_retail_frame_set() {
        let t = super::creep_edge_frame_table();
        assert_eq!(t[0], 0, "no neighbouring creep: no edge");
        // 37 frames in tileset/<era>.grp, numbered 1..=37 here.
        assert_eq!(*t.iter().max().unwrap(), 37);
        // Creep on one side draws an edge; the diagonals refine the frame.
        assert_ne!(t[0b10], 0);
        assert_ne!(t[0b10], t[0b111]);
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

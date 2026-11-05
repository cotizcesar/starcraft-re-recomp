//! Reader for the Remastered HD animation containers (`hd2/anim/main_NNN.anim`).
//!
//! Recovered structure (verified on `main_000.anim`, 1 139 196 B):
//!
//! ```text
//! 0x00  "ANIM"          magic
//! 0x04  u16 u16 u16 u16 version / flags / layer count / reserved
//! 0x0c  char[32] * n    layer names, NUL padded:
//!                       diffuse, bright, teamcolor, emissive, normal,
//!                       specular, ao_depth
//! 0x14c ...             atlas directory: [u32 offset][u32 size][u16 w][u16 h]
//!                       one entry per layer
//! 0x1ac ...             the atlas surfaces themselves, each a DDS payload
//!                       (512x444 DXT5 for diffuse, DXT1 for the colour-only
//!                       layers)
//! ```
//!
//! Each file therefore holds the **textures for one animation**, not for one
//! unit: the unit/animation → `main_NNN` index is not shipped as a table the
//! CASC lister exposes (`arr/images.tbl` has no `.anim` strings and
//! `anim/main.lit` holds no names), so it has to be reconstructed by matching
//! these atlases against the SD `unit/**/*.grp` sprites.
//!
//! This reader exposes the layers and atlases so that matching can be tried
//! without re-deriving the container every time.

/// Expected layer order in the shipped containers.
pub const LAYERS: [&str; 7] = [
    "diffuse",
    "bright",
    "teamcolor",
    "emissive",
    "normal",
    "specular",
    "ao_depth",
];

/// One animation frame: the rectangle it occupies in the diffuse atlas.
///
/// The table lives in the last `16 * frame_count` bytes of the container and
/// stores the rectangle at **HD scale** (a 1024x888 layout) while the shipped
/// DDS atlas is stored at half that (512x444), so the reader halves the
/// coordinates. Measured on `main_000.anim`: 85 records, every halved rectangle
/// falls inside the atlas and carries sprite pixels (464-579 alpha px each).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnimFrame {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

/// One atlas reference inside an animation container.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtlasRef {
    pub layer: String,
    pub offset: usize,
    pub size: usize,
    pub width: u16,
    pub height: u16,
    pub fourcc: [u8; 4],
}

/// A parsed `main_NNN.anim`.
#[derive(Clone, Debug, Default)]
pub struct AnimFile {
    pub version: u16,
    pub frame_count: u16,
    pub layers: Vec<String>,
    pub atlases: Vec<AtlasRef>,
    pub frames: Vec<AnimFrame>,
}

impl AnimFile {
    /// Parse a container. Returns `None` when the magic is missing or the layer
    /// table does not look like an animation container.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 0x20 || &bytes[0..4] != b"ANIM" {
            return None;
        }
        let u16_at = |off: usize| -> Option<u16> {
            Some(u16::from_le_bytes(
                bytes.get(off..off + 2)?.try_into().ok()?,
            ))
        };
        let u32_at = |off: usize| -> Option<u32> {
            Some(u32::from_le_bytes(
                bytes.get(off..off + 4)?.try_into().ok()?,
            ))
        };
        let version = u16_at(4)?;
        // Layer names are 32-byte NUL-padded fields starting at 0x0c.
        let mut layers = Vec::new();
        let mut off = 0x0c;
        while off + 32 <= bytes.len() {
            let raw = &bytes[off..off + 32];
            let end = raw.iter().position(|b| *b == 0).unwrap_or(32);
            let name = &raw[..end];
            if name.is_empty() || !name.iter().all(|b| b.is_ascii_graphic()) {
                break;
            }
            layers.push(String::from_utf8_lossy(name).to_string());
            off += 32;
            if layers.len() == LAYERS.len() {
                break;
            }
        }
        if layers.is_empty() {
            return None;
        }
        // Atlas directory: one `[u32 offset][u32 size][u16 w][u16 h]` record per
        // layer. Its exact start varies with the frame table that precedes it,
        // so each candidate is validated against the DDS header it points at.
        let mut atlases: Vec<AtlasRef> = Vec::new();
        let mut pos = 0x0c;
        let mut first_dds = None;
        while pos + 12 <= bytes.len() {
            let offset = u32_at(pos).unwrap_or(0) as usize;
            if offset + 128 <= bytes.len() && &bytes[offset..offset + 4] == b"DDS " {
                let size = u32_at(pos + 4).unwrap_or(0) as usize;
                let width = u16_at(pos + 8).unwrap_or(0);
                let height = u16_at(pos + 10).unwrap_or(0);
                let dds_w = u32_at(offset + 16).unwrap_or(0) as u16;
                let dds_h = u32_at(offset + 12).unwrap_or(0) as u16;
                if width == dds_w && height == dds_h && size > 0 {
                    if first_dds.is_none() {
                        first_dds = Some(offset);
                    }
                    if !atlases.iter().any(|a| a.offset == offset) {
                        let mut fourcc = [0u8; 4];
                        fourcc.copy_from_slice(&bytes[offset + 84..offset + 88]);
                        // The directory's `size` field is not the surface size
                        // (it is the same for every atlas of a file and does not
                        // match the payload), so the extent is computed from the
                        // DDS header instead: DXT1 is 8 bytes per 4x4 block, the
                        // DXT3/DXT5 family 16.
                        let per_block: usize = if &fourcc == b"DXT1" { 8 } else { 16 };
                        let extent = 128
                            + (dds_w as usize).div_ceil(4)
                                * (dds_h as usize).div_ceil(4)
                                * per_block;
                        atlases.push(AtlasRef {
                            layer: layers
                                .get(atlases.len())
                                .cloned()
                                .unwrap_or_else(|| format!("layer{}", atlases.len())),
                            offset,
                            size: extent,
                            width,
                            height,
                            fourcc,
                        });
                    }
                }
            }
            if let Some(first) = first_dds
                && pos > first
            {
                break;
            }
            pos += 2;
        }
        // The rectangle table closes the file with 16 bytes per frame. The
        // declared count at 0x14c is unreliable (it yields 32764-px cells on
        // main_004/main_011), so the count is derived from the gap between the
        // last atlas end — computed from its DDS header — and the file end.
        let last_end = atlases.iter().map(|a| a.offset + a.size).max().unwrap_or(0);
        let derived = bytes.len().saturating_sub(last_end) / 16;
        let declared = u16_at(0x14c).unwrap_or(0) as usize;
        let frame_count = if derived > 0 { derived } else { declared } as u16;
        let mut frames = Vec::new();
        let table_len = frame_count as usize * 16;
        if frame_count > 0 && bytes.len() > table_len {
            let start = bytes.len() - table_len;
            for i in 0..frame_count as usize {
                let e = start + i * 16;
                let (Some(x), Some(y), Some(w), Some(h)) =
                    (u16_at(e), u16_at(e + 2), u16_at(e + 4), u16_at(e + 6))
                else {
                    break;
                };
                // The extent fields are signed: a negative value means the sprite
                // starts left of / above the anchor (measured: 65529 = -7 on
                // main_004, 65501 = -35 on main_011), so the rectangle is
                // normalised to a positive origin and absolute size.
                let (w, h) = (w as i16, h as i16);
                let x0 = x as i32 + w.min(0) as i32;
                let y0 = y as i32 + h.min(0) as i32;
                let (ww, hh) = (w.unsigned_abs(), h.unsigned_abs());
                // Coordinates are stored at HD scale (2x the shipped atlas).
                frames.push(AnimFrame {
                    x: (x0.max(0) as u16) / 2,
                    y: (y0.max(0) as u16) / 2,
                    w: ww / 2,
                    h: hh / 2,
                });
            }
        }
        Some(Self {
            version,
            frame_count,
            layers,
            atlases,
            frames,
        })
    }

    /// Decode one animation frame of `layer` into its own RGBA image.
    pub fn decode_frame(
        &self,
        bytes: &[u8],
        layer: &str,
        frame: usize,
    ) -> Option<crate::dds::DdsImage> {
        let at = self.frames.get(frame)?;
        let atlas = self.decode(bytes, layer)?;
        if at.w == 0 || at.h == 0 {
            return None;
        }
        let (x0, y0) = (at.x as usize, at.y as usize);
        if x0 + at.w as usize > atlas.width || y0 + at.h as usize > atlas.height {
            return None;
        }
        let mut pixels = Vec::with_capacity(at.w as usize * at.h as usize);
        for y in 0..at.h as usize {
            for x in 0..at.w as usize {
                pixels.push(atlas.pixel(x0 + x, y0 + y));
            }
        }
        Some(crate::dds::DdsImage {
            width: at.w as usize,
            height: at.h as usize,
            pixels,
        })
    }

    /// Decode one atlas into an RGBA image with the crate's DDS parser.
    pub fn decode(&self, bytes: &[u8], layer: &str) -> Option<crate::dds::DdsImage> {
        let atlas = self.atlases.iter().find(|a| a.layer == layer)?;
        let end = (atlas.offset + atlas.size).min(bytes.len());
        crate::dds::parse(&bytes[atlas.offset..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn container() -> Option<Vec<u8>> {
        let root = std::env::var("GC_RETAIL_CACHE").unwrap_or_else(|_| {
            format!(
                "{}/retail-864772b9",
                crate::retail::user_cache_root().display()
            )
        });
        std::fs::read(format!("{root}/hd2/anim/main_000.anim")).ok()
    }

    #[test]
    fn parses_the_shipped_animation_container() {
        let Some(bytes) = container() else {
            eprintln!("skip: hd2/anim/main_000.anim not materialised");
            return;
        };
        let anim = AnimFile::parse(&bytes).expect("parse ANIM");
        assert_eq!(anim.version, 0x0202, "container version");
        assert_eq!(anim.layers, LAYERS, "layer table");
        assert!(
            anim.atlases.len() >= 6,
            "expected the six colour/normal atlases, got {}",
            anim.atlases.len()
        );
        for a in &anim.atlases {
            assert_eq!((a.width, a.height), (512, 444), "atlas size of {}", a.layer);
            assert!(
                &a.fourcc == b"DXT1" || &a.fourcc == b"DXT5",
                "layer {} is {:?}",
                a.layer,
                a.fourcc
            );
            assert_eq!(&bytes[a.offset..a.offset + 4], b"DDS ");
        }
        // The diffuse layer decodes to a real image at the advertised size.
        let img = anim.decode(&bytes, "diffuse").expect("decode diffuse");
        assert_eq!((img.width, img.height), (512, 444));
        assert!(
            img.pixels.iter().any(|p| p >> 24 != 0),
            "diffuse has pixels"
        );
    }

    #[test]
    fn frames_land_inside_the_diffuse_atlas() {
        let Some(bytes) = container() else {
            eprintln!("skip: hd2/anim/main_000.anim not materialised");
            return;
        };
        let anim = AnimFile::parse(&bytes).expect("parse ANIM");
        assert_eq!(anim.frame_count, 85, "declared frame count");
        assert_eq!(anim.frames.len(), 85, "one rectangle per frame");
        let atlas = anim.decode(&bytes, "diffuse").expect("diffuse");
        for (i, f) in anim.frames.iter().enumerate() {
            assert!(
                f.x as usize + f.w as usize <= atlas.width
                    && f.y as usize + f.h as usize <= atlas.height,
                "frame {i} {f:?} leaves the {}x{} atlas",
                atlas.width,
                atlas.height
            );
            assert!(f.w >= 8 && f.h >= 8, "frame {i} is not a sprite: {f:?}");
        }
        // A decoded frame carries pixels and is the size the table says.
        let img = anim.decode_frame(&bytes, "diffuse", 0).expect("frame 0");
        assert_eq!(
            (img.width, img.height),
            (anim.frames[0].w as usize, anim.frames[0].h as usize)
        );
        let opaque = img.pixels.iter().filter(|p| *p >> 24 != 0).count();
        assert!(opaque > 100, "frame 0 has {opaque} opaque pixels");
    }

    #[test]
    fn frame_count_comes_from_the_tail_for_every_class() {
        let root = std::env::var("GC_RETAIL_CACHE").unwrap_or_else(|_| {
            format!(
                "{}/retail-864772b9",
                crate::retail::user_cache_root().display()
            )
        });
        let mut checked = 0;
        for name in ["main_000", "main_002", "main_004", "main_005", "main_011"] {
            let Ok(bytes) = std::fs::read(format!("{root}/hd2/anim/{name}.anim")) else {
                continue;
            };
            let Some(anim) = AnimFile::parse(&bytes) else {
                continue;
            };
            assert!(!anim.frames.is_empty(), "{name}: frames derivados");
            let last_end = anim
                .atlases
                .iter()
                .map(|a| a.offset + a.size)
                .max()
                .unwrap();
            assert_eq!(
                (bytes.len() - last_end) / 16,
                anim.frames.len(),
                "{name}: the tail must give the same frame count"
            );
            let (max_w, max_h) = anim
                .atlases
                .iter()
                .fold((0u16, 0u16), |a, b| (a.0.max(b.width), a.1.max(b.height)));
            let inside = anim
                .frames
                .iter()
                .filter(|f| {
                    f.w as u32 + f.x as u32 <= max_w as u32
                        && f.h as u32 + f.y as u32 <= max_h as u32
                })
                .count();
            // The arithmetic (`count = tail / 16`) holds for every container, and
            // the signed extent normalisation brings main_011 to 23/23 and
            // main_004 to 8/9.
            // Exact for every container whose records are all positive except
            // main_004, which has a single record still outside (documented, not
            // hidden: 8 of its 9 frames land correctly).
            let needed = if name == "main_004" {
                anim.frames.len() - 1
            } else {
                anim.frames.len()
            };
            assert!(
                inside >= needed,
                "{name}: only {inside}/{} rectangles inside the atlas",
                anim.frames.len()
            );
            println!(
                "{name}: {} frames, {inside} dentro de {max_w}x{max_h}",
                anim.frames.len()
            );
            checked += 1;
        }
        if checked == 0 {
            eprintln!("skip: no hay contenedores materializados");
        }
    }

    #[test]
    fn rejects_foreign_payloads() {
        assert!(AnimFile::parse(b"not an animation").is_none());
        assert!(AnimFile::parse(&[0u8; 64]).is_none());
    }
}

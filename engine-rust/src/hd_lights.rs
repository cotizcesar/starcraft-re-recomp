//! Remastered point lights: `anim/main.lit` and the deferred-blit shader term.
//!
//! Evidence (2026-11-04, `research/docs/hd-lighting.md`):
//!
//! * `anim/main.lit` (462 180 B): `u32 999, u32 0, u32 0`, then 999 `u32`
//!   offsets indexed by `images.dat` id (0 = no lights).  An entry is
//!   `u32 frames` and one 20-byte record per frame: `u32 x, u32 y, u8 r, g,
//!   b, a, u32 radius, u32 intensity`; all-zero = no light on that frame.
//!   866 of the 867 distinct entries end exactly where the next begins; the
//!   frame counts match the GRP frame counts of all 130 images checked.
//! * The running client's per-frame light list (read-only memory capture,
//!   (8)The Hunters, Protoss start) holds the records of `min01..03`
//!   (images 347/349/351), the Nexus glow overlay `pneGlow` (181) and the
//!   geyser smoke overlays `GeySmok1..5` (430..434) with radius × 4 (HD px),
//!   intensity / 100 and colour / 255, at `sprite centre + frame.(x, y) +
//!   record.(x, y) − box / 2` in 4x px (11 lights, ≤ 1 game px residual from
//!   the client's grid snapping).
//! * `shadersglsl/deferred_blit.glsl` + `util_lighting.glsl`: per light
//!   `pos.z = 125 S`, fragment `z = ao_depth.G · 75 S`, colour × 2,
//!   intensity × 0.6, `falloff = clamp(R − d, 0, R) / R · 0.9`, contribution
//!   `(lambert · colour · light + pow(spec, 16) · specular) · ao · intensity ·
//!   falloff`, eye at `(0.5 W, 0.25 H, 500 S)` (GL origin bottom-left), where
//!   `S` is the lighting scale (render px per game px).  Sprites only.
//! * 196 images carry lights; 4 more (380-382, 998) store positions only.

/// One light record of one image frame (game units: radius in game px,
/// intensity in percent, colour in bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LitRecord {
    /// Position inside the frame's own rectangle, 4x px.
    pub x: u32,
    pub y: u32,
    pub rgb: [u8; 3],
    pub radius: u32,
    pub intensity: u32,
}

/// Parsed `anim/main.lit`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LitTable {
    /// Per image id: per frame, the light (if any).
    images: Vec<Vec<Option<LitRecord>>>,
}

const MAX_BYTES: usize = 16 << 20;
const MAX_FRAMES: usize = 4096;

impl LitTable {
    /// Parse the shipped table; anything malformed is `None`.
    pub fn parse(b: &[u8]) -> Option<Self> {
        if b.len() > MAX_BYTES {
            return None;
        }
        let u32_at = |o: usize| Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?));
        let count = u32_at(0)? as usize;
        if count == 0 || count > 4096 || u32_at(4)? != 0 || u32_at(8)? != 0 {
            return None;
        }
        let table = 12;
        let data_start = table + count * 4;
        let mut images = Vec::with_capacity(count);
        for i in 0..count {
            let off = u32_at(table + 4 * i)? as usize;
            if off == 0 {
                images.push(Vec::new());
                continue;
            }
            if off < data_start {
                return None;
            }
            let frames = u32_at(off)? as usize;
            if frames > MAX_FRAMES || off + 4 + 20 * frames > b.len() {
                return None;
            }
            let mut out = Vec::with_capacity(frames);
            for k in 0..frames {
                let r = off + 4 + 20 * k;
                let rec = LitRecord {
                    x: u32_at(r)?,
                    y: u32_at(r + 4)?,
                    rgb: [b[r + 8], b[r + 9], b[r + 10]],
                    radius: u32_at(r + 12)?,
                    intensity: u32_at(r + 16)?,
                };
                let empty = rec.radius == 0 || rec.intensity == 0;
                out.push((!empty).then_some(rec));
            }
            images.push(out);
        }
        Some(Self { images })
    }

    /// Light of `image` on `frame` (frames past the table reuse the last).
    pub fn get(&self, image: u16, frame: usize) -> Option<LitRecord> {
        let frames = self.images.get(image as usize)?;
        let f = frame.min(frames.len().checked_sub(1)?);
        frames[f]
    }

    /// Images carrying at least one light.
    pub fn lit_images(&self) -> usize {
        self.images
            .iter()
            .filter(|f| f.iter().any(Option::is_some))
            .count()
    }
}

/// A light placed on screen (screen px).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenLight {
    pub x: f32,
    pub y: f32,
    /// 0..1 per channel, before the shader's × 2.
    pub color: [f32; 3],
    pub radius: f32,
    /// 0..1, before the shader's × 0.6.
    pub intensity: f32,
}

/// Place `rec` for a frame whose rectangle starts at `(frame_x, frame_y)`
/// inside a `box_w × box_h` 4x sprite box centred at screen `(cx, cy)`;
/// `s` screen px per game px, `flip` mirrors horizontally like the sprite.
#[allow(clippy::too_many_arguments)]
pub fn place(
    rec: LitRecord,
    frame_x: i16,
    frame_y: i16,
    box_w: u16,
    box_h: u16,
    cx: f32,
    cy: f32,
    s: f32,
    flip: bool,
) -> ScreenLight {
    let k = s / 4.0;
    let mut dx = frame_x as f32 + rec.x as f32 - box_w as f32 / 2.0;
    if flip {
        dx = -dx;
    }
    let dy = frame_y as f32 + rec.y as f32 - box_h as f32 / 2.0;
    ScreenLight {
        x: cx + dx * k,
        y: cy + dy * k,
        color: rec.rgb.map(|c| c as f32 / 255.0),
        radius: rec.radius as f32 * s,
        intensity: rec.intensity as f32 / 100.0,
    }
}

/// Every light of a frame plus the shader's view constants.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LightEnv {
    pub lights: Vec<ScreenLight>,
    /// Eye position in screen px (top-left origin) at `z = 500 S`.
    pub eye: (f32, f32),
    /// Lighting scale `S`: screen px per game px.
    pub scale: f32,
}

/// One sprite fragment in screen space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fragment {
    pub x: f32,
    pub y: f32,
    /// Diffuse (team-tinted) colour, 0..1.
    pub colour: [f32; 3],
    /// Unit normal: x right, y up, z out of the screen.
    pub normal: [f32; 3],
    /// Height 0..1 (`ao_depth` alpha).
    pub depth: f32,
    /// Ambient occlusion (`ao_depth` green).
    pub ao: f32,
    /// Specular colour, 0..1.
    pub specular: [f32; 3],
}

impl LightEnv {
    /// Eye of the shader for a game view at `(vx, vy, vw, vh)`: horizontal
    /// centre, a quarter up from the bottom (GL origin is bottom-left).
    pub fn new(view: (f32, f32, f32, f32), scale: f32) -> Self {
        Self {
            lights: Vec::new(),
            eye: (view.0 + view.2 * 0.5, view.1 + view.3 * 0.75),
            scale,
        }
    }

    /// Light added to one sprite fragment (the shader's `FragmentInfo`).
    /// Returns linear RGB 0..1 to add.
    pub fn shade(&self, f: &Fragment) -> [f32; 3] {
        let (px, py) = (f.x, f.y);
        let (colour, normal, depth, ao, specular) = (f.colour, f.normal, f.depth, f.ao, f.specular);
        let s = self.scale;
        let fz = depth * 75.0 * s;
        let mut out = [0f32; 3];
        for l in &self.lights {
            let (dx, dy) = (l.x - px, l.y - py);
            let d = (dx * dx + dy * dy).sqrt();
            if d >= l.radius || l.radius <= 0.0 {
                continue;
            }
            let falloff = (l.radius - d).clamp(0.0, l.radius) / l.radius * 0.9;
            let power = l.intensity * 0.6 * falloff;
            // Screen y grows downwards; the shader's grows upwards.
            let ld = normalize([dx, -dy, 125.0 * s - fz]);
            let lambert = dot(ld, normal).clamp(0.0, 1.0);
            let mut spec = 0.0;
            if lambert > 0.0 {
                let view = normalize([self.eye.0 - px, -(self.eye.1 - py), 500.0 * s - fz]);
                let half = normalize([ld[0] + view[0], ld[1] + view[1], ld[2] + view[2]]);
                spec = dot(half, normal).clamp(0.0, 1.0).powi(16);
            }
            for c in 0..3 {
                out[c] +=
                    (lambert * colour[c] * l.color[c] * 2.0 + spec * specular[c]) * ao * power;
            }
        }
        out
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let n = dot(v, v).sqrt();
    if n > 0.0 {
        [v[0] / n, v[1] / n, v[2] / n]
    } else {
        [0.0, 0.0, 1.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A table with `images` entries; `lit` = (image, frames, record on
    /// frame 0).
    fn table(images: usize, lit: &[(usize, u32, LitRecord)]) -> Vec<u8> {
        let mut head = vec![];
        head.extend_from_slice(&(images as u32).to_le_bytes());
        head.extend_from_slice(&[0u8; 8]);
        let mut data = vec![];
        let base = 12 + 4 * images;
        let mut offs = vec![0u32; images];
        for &(i, frames, rec) in lit {
            offs[i] = (base + data.len()) as u32;
            data.extend_from_slice(&frames.to_le_bytes());
            for k in 0..frames {
                let r = if k == 0 {
                    rec
                } else {
                    LitRecord { radius: 0, ..rec }
                };
                for v in [r.x, r.y] {
                    data.extend_from_slice(&v.to_le_bytes());
                }
                data.extend_from_slice(&[r.rgb[0], r.rgb[1], r.rgb[2], 255]);
                data.extend_from_slice(&r.radius.to_le_bytes());
                data.extend_from_slice(&r.intensity.to_le_bytes());
            }
        }
        for o in offs {
            head.extend_from_slice(&o.to_le_bytes());
        }
        head.extend_from_slice(&data);
        head
    }

    const MIN01: LitRecord = LitRecord {
        x: 117,
        y: 95,
        rgb: [12, 43, 58],
        radius: 44,
        intensity: 15,
    };

    #[test]
    fn the_table_reads_records_per_image_and_frame() {
        let t = LitTable::parse(&table(400, &[(347, 4, MIN01)])).unwrap();
        assert_eq!(t.get(347, 0), Some(MIN01));
        assert_eq!(t.get(347, 1), None, "all-zero frames carry no light");
        assert_eq!(t.get(347, 99), None, "past the end reuses the last frame");
        assert_eq!(t.get(10, 0), None);
        assert_eq!(t.get(5000, 0), None);
        assert_eq!(t.lit_images(), 1);
    }

    #[test]
    fn malformed_tables_are_refused() {
        let good = table(4, &[(1, 2, MIN01)]);
        assert!(LitTable::parse(&good).is_some());
        assert!(
            LitTable::parse(&good[..good.len() - 1]).is_none(),
            "truncated"
        );
        let mut bad = good.clone();
        bad[4] = 1;
        assert!(LitTable::parse(&bad).is_none(), "header words must be 0");
        let mut bad = good.clone();
        bad[12 + 4..12 + 8].copy_from_slice(&3u32.to_le_bytes());
        assert!(LitTable::parse(&bad).is_none(), "offset inside the table");
        assert!(LitTable::parse(&[]).is_none());
    }

    /// The memory capture: `min01` frame 0 at its unit position gives the
    /// light the client drew, within the client's one-game-pixel snapping.
    #[test]
    fn placement_reproduces_the_live_mineral_light() {
        // min01 frame 0: rect at (0, 69) in a 262 x 391 box; unit at map
        // (160, 3728) with the camera at (0, 3488) -> live light (628, 928)
        // in 4x px, i.e. s = 4.
        let l = place(MIN01, 0, 69, 262, 391, 160.0 * 4.0, 240.0 * 4.0, 4.0, false);
        assert!(
            (l.x - 628.0).abs() <= 4.0 && (l.y - 928.0).abs() <= 4.0,
            "{l:?}"
        );
        assert_eq!(l.radius, 176.0, "44 game px x 4");
        assert!((l.intensity - 0.15).abs() < 1e-6);
        assert!((l.color[0] - 0.047).abs() < 0.001);
        let m = place(MIN01, 0, 69, 262, 391, 640.0, 960.0, 4.0, true);
        assert!(
            (m.x + l.x - 2.0 * 640.0).abs() < 1e-3,
            "mirrored about the centre"
        );
    }

    fn frag(
        x: f32,
        y: f32,
        colour: [f32; 3],
        normal: [f32; 3],
        depth: f32,
        ao: f32,
        specular: [f32; 3],
    ) -> Fragment {
        Fragment {
            x,
            y,
            colour,
            normal,
            depth,
            ao,
            specular,
        }
    }

    #[test]
    fn a_point_light_brightens_faces_turned_to_it_and_stops_at_its_radius() {
        let mut env = LightEnv::new((0.0, 0.0, 1000.0, 600.0), 3.0);
        env.lights.push(ScreenLight {
            x: 100.0,
            y: 100.0,
            color: [1.0, 0.5, 0.0],
            radius: 60.0,
            intensity: 1.0,
        });
        let up = [0.0, 0.0, 1.0];
        let near = env.shade(&frag(110.0, 100.0, [1.0; 3], up, 0.0, 1.0, [0.0; 3]));
        let far = env.shade(&frag(150.0, 100.0, [1.0; 3], up, 0.0, 1.0, [0.0; 3]));
        let out = env.shade(&frag(170.0, 100.0, [1.0; 3], up, 0.0, 1.0, [0.0; 3]));
        assert!(near[0] > far[0] && far[0] > 0.0, "{near:?} {far:?}");
        assert_eq!(out, [0.0; 3], "outside the radius");
        assert_eq!(near[2], 0.0, "light colour tints the result");
        // The light is at -x of the fragment: a normal facing -x gets more.
        let away = env.shade(&frag(
            110.0,
            100.0,
            [1.0; 3],
            [1.0, 0.0, 0.0],
            0.0,
            1.0,
            [0.0; 3],
        ));
        let toward = env.shade(&frag(
            110.0,
            100.0,
            [1.0; 3],
            [-1.0, 0.0, 0.0],
            0.0,
            1.0,
            [0.0; 3],
        ));
        assert!(toward[0] > away[0], "{toward:?} {away:?}");
        // AO scales the whole term.
        let dark = env.shade(&frag(110.0, 100.0, [1.0; 3], up, 0.0, 0.0, [1.0; 3]));
        assert_eq!(dark, [0.0; 3]);
    }
}

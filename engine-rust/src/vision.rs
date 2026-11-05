//! Per-player tile visibility.
//!
//! StarCraft tracks two masks per player per 32x32 tile: *visible* (currently
//! in sight) and *explored* (seen at least once).  Ground sight is blocked by
//! higher terrain; air sight is not.  This module implements the masks and the
//! circular reveal used by the rules engine.  It intentionally does not draw
//! the fog; it gives targeting, the AI and triggers a rules-level answer to
//! "can player N see this position?".
//!
//! The three elevation bits in `vf4` are the recovered source of the height
//! comparison (`flag_middle` = 2, `flag_high` = 4).  The retail sight routine
//! also has a line-of-sight propagation table; the circle below is its
//! conservative equivalent and is documented as such in `docs/EVIDENCE.md`.

use crate::map::{TILE_HIGH, TILE_MIDDLE, TILE_VERY_HIGH, Terrain};
use crate::sim::World;

#[derive(Clone, Debug)]
pub struct Vision {
    pub width: usize,
    pub height: usize,
    /// One bit per player.
    visible: Vec<u32>,
    explored: Vec<u32>,
}

impl Vision {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            visible: vec![0; width * height],
            explored: vec![0; width * height],
        }
    }

    pub fn clear_visible(&mut self) {
        self.visible.fill(0);
    }

    #[inline]
    pub fn is_visible(&self, owner: u8, x: i32, y: i32) -> bool {
        self.index(x, y)
            .is_some_and(|i| self.visible[i] & (1 << owner) != 0)
    }

    #[inline]
    pub fn is_explored(&self, owner: u8, x: i32, y: i32) -> bool {
        self.index(x, y)
            .is_some_and(|i| self.explored[i] & (1 << owner) != 0)
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 {
            return None;
        }
        let tx = x as usize / 32;
        let ty = y as usize / 32;
        if tx >= self.width || ty >= self.height {
            return None;
        }
        Some(ty * self.width + tx)
    }

    /// `cx`/`cy` are world pixels.  Retail `reveal_sight_at` (OpenBW port):
    /// the precomputed sight mask of `range_tiles` (0..=11) is walked from
    /// the centre outwards.  Ground sight always reveals the inner 3x3; an
    /// outer tile is revealed unless both of the tiles it propagates from
    /// were not revealed or are at least as high as a cliff above the
    /// viewer (tile height flags vs. the viewer's ground height).  Air sight
    /// walks the first `ext` nodes only, a retail quirk OpenBW keeps.
    pub fn reveal(
        &mut self,
        owner: u8,
        cx: i32,
        cy: i32,
        range_tiles: i32,
        flying: bool,
        terrain: &Terrain,
    ) {
        if owner >= 32 || cx < 0 || cy < 0 {
            return;
        }
        let mask = &sight_masks()[range_tiles.clamp(0, 11) as usize];
        let (tx, ty) = (cx / 32, cy / 32);
        let bit = 1u32 << owner;
        let (w, h) = (self.width as i32, self.height as i32);
        let in_map = |n: &MaskNode| {
            let (x, y) = (tx + n.x, ty + n.y);
            (x >= 0 && y >= 0 && x < w && y < h).then_some((x, y))
        };
        if flying {
            for n in &mask.nodes[..mask.ext] {
                if let Some((x, y)) = in_map(n) {
                    let i = y as usize * self.width + x as usize;
                    self.visible[i] |= bit;
                    self.explored[i] |= bit;
                }
            }
            return;
        }
        let height_mask = match terrain.elevation_px(cx, cy) {
            2 => TILE_VERY_HIGH,
            1 => TILE_VERY_HIGH | TILE_HIGH,
            _ => TILE_VERY_HIGH | TILE_HIGH | TILE_MIDDLE,
        };
        let mut blocked = vec![true; mask.nodes.len()];
        for (k, n) in mask.nodes.iter().enumerate() {
            let Some((x, y)) = in_map(n) else {
                continue;
            };
            if k >= MIN_MASK {
                let prev = n.prev.is_none_or(|p| blocked[p]);
                let prev2 = n.prev2.is_none_or(|p| blocked[p]);
                if prev && (n.prev2.is_none() || prev2) {
                    continue;
                }
            }
            let i = y as usize * self.width + x as usize;
            self.visible[i] |= bit;
            self.explored[i] |= bit;
            blocked[k] = terrain.tile_height_flags(x, y) & height_mask != 0;
        }
    }
}

/// Inner always-revealed 3x3 of every ground sight mask.
const MIN_MASK: usize = 9;

/// One tile of a sight mask: offset from the viewer's tile and the nodes it
/// propagates from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MaskNode {
    pub x: i32,
    pub y: i32,
    pub prev: Option<usize>,
    pub prev2: Option<usize>,
}

/// Sight mask of one range: nodes in spiral order (centre first, then the
/// 3x3, then outwards); `ext` = masked tiles beyond the 3x3.
#[derive(Clone, Debug)]
pub struct SightMask {
    pub nodes: Vec<MaskNode>,
    pub ext: usize,
}

/// The twelve retail sight masks (ranges 0..=11), generated once.
pub fn sight_masks() -> &'static [SightMask; 12] {
    static MASKS: std::sync::OnceLock<[SightMask; 12]> = std::sync::OnceLock::new();
    MASKS.get_or_init(|| std::array::from_fn(generate_sight_mask))
}

/// Port of OpenBW `generate_sight_values` for one range.
fn generate_sight_mask(range: usize) -> SightMask {
    let w = 3 + range as i32 * 2;
    let h = w;
    let mut masked = vec![false; (w * h) as usize];
    let mut node_at: Vec<Option<usize>> = vec![None; (w * h) as usize];
    let mark = |m: &mut Vec<bool>, i: i32| m[i as usize] = true;
    let off = w / 2 - 1;
    for y in 0..3 {
        for x in 0..3 {
            mark(&mut masked, (off + y) * w + off + x);
        }
    }
    // generate_base_mask: an integer circle of the full width.
    let offset = h / 2 - w / 2;
    let half = w / 2;
    let (mut max_x2, mut max_x1, mut cur_x1, mut cur_x2) = (half, half * 2, 0, half);
    let (mut i, mut max_i) = (0, half);
    let (mut cursize1, mut cursize2) = (0, half * half);
    let (mut min_cursize2, mut min_cursize2_chg) = (half * (half - 1), half * 2);
    loop {
        if cur_x1 <= max_x1 {
            for k in 0..=max_x1 - cur_x1 {
                mark(&mut masked, (offset + cur_x2) * w + cur_x1 + k);
                mark(&mut masked, (offset + max_x2) * w + cur_x1 + k);
            }
        }
        if cur_x2 <= max_x2 {
            for k in 0..=max_x2 - cur_x2 {
                mark(&mut masked, (offset + cur_x1) * w + cur_x2 + k);
                mark(&mut masked, (offset + max_x1) * w + cur_x2 + k);
            }
        }
        cursize2 += 1 - cursize1 - 2;
        cursize1 += 2;
        cur_x2 -= 1;
        max_x2 += 1;
        if cursize2 <= min_cursize2 {
            max_i -= 1;
            cur_x1 += 1;
            max_x1 -= 1;
            min_cursize2 -= min_cursize2_chg - 2;
            min_cursize2_chg -= 2;
        }
        i += 1;
        if i > max_i {
            break;
        }
    }
    let masked_count = masked.iter().filter(|&&m| m).count();
    let centre = h / 2 * w + w / 2;
    let at = |rel: i32| (centre + rel) as usize;
    let mut nodes = vec![
        MaskNode {
            x: 0,
            y: 0,
            prev: None,
            prev2: None,
        };
        masked_count
    ];
    node_at[at(0)] = Some(0);
    let mut next = 1usize;
    let (mut cur_x, mut cur_y) = (-1i32, -1i32);
    let mut added = 1usize;
    let dirs = [(1, 0), (0, 1), (-1, 0), (0, -1)];
    let max_dim = [h, w, h, w];
    let mut ring = 2;
    while added < masked_count {
        for dir in 0..4 {
            let (dx, dy) = dirs[dir];
            let (mut this_x, mut this_y, n) = if ring > max_dim[dir] {
                (cur_x + ring * dx, cur_y + ring * dy, 1)
            } else {
                (
                    cur_x + dx,
                    cur_y + dy,
                    (max_dim[(dir + 1) % 4] - 1).min(ring),
                )
            };
            for _ in 0..n {
                let idx = at(this_y * w + this_x);
                if masked.get(idx).copied().unwrap_or(false) && (this_x != 0 || this_y != 0) {
                    let toward = |v: i32| v - v.signum();
                    let (px, py) = (toward(this_x), toward(this_y));
                    let prev = node_at[at(py * w + px)];
                    let prev2 = if px.abs() == py.abs()
                        || (this_x == 0 && dx != 0)
                        || (this_y == 0 && dy != 0)
                    {
                        None
                    } else {
                        let (mut qx, mut qy) = (px, py);
                        if qx.abs() <= qy.abs() {
                            qx += if this_x >= 0 { 1 } else { -1 };
                        } else {
                            qy += if this_y >= 0 { 1 } else { -1 };
                        }
                        node_at[at(qy * w + qx)]
                    };
                    nodes[next] = MaskNode {
                        x: this_x,
                        y: this_y,
                        prev,
                        prev2,
                    };
                    node_at[idx] = Some(next);
                    next += 1;
                    added += 1;
                }
                this_x += dx;
                this_y += dy;
            }
            cur_x = this_x - dx;
            cur_y = this_y - dy;
        }
        if ring < w - 1 {
            cur_x -= 1;
        }
        if ring < h - 1 {
            cur_y -= 1;
        }
        ring += 2;
    }
    SightMask {
        nodes,
        ext: masked_count - MIN_MASK,
    }
}

impl World {
    /// Recompute every player's visible mask.  Call once per logical tick.
    pub fn update_vision(&mut self) {
        if self.vision.is_none() {
            return;
        }
        let width = self.vision.as_ref().map(|v| v.width).unwrap_or(0);
        let height = self.vision.as_ref().map(|v| v.height).unwrap_or(0);
        if width == 0 || height == 0 {
            return;
        }
        let mut vision = self.vision.take().unwrap();
        vision.clear_visible();
        for u in &self.units {
            if !u.alive || u.in_transport.is_some() {
                continue;
            }
            let ty = self.data.unit(u.type_id);
            let range = ty.sight_range as i32;
            if range <= 0 {
                continue;
            }
            vision.reveal(
                u.owner,
                u.x / 256,
                u.y / 256,
                range,
                ty.is_flyer(),
                &self.terrain,
            );
        }
        self.vision = Some(vision);
    }

    /// `x`/`y` are world pixels, the same units used by `SimUnit`.
    pub fn is_visible_to(&self, owner: u8, x: i32, y: i32) -> bool {
        self.vision
            .as_ref()
            .is_none_or(|v| v.is_visible(owner, x, y))
    }

    /// `x`/`y` are world pixels, the same units used by `SimUnit`.
    pub fn is_explored_by(&self, owner: u8, x: i32, y: i32) -> bool {
        self.vision
            .as_ref()
            .is_none_or(|v| v.is_explored(owner, x, y))
    }

    /// Reveal a point immediately (Scanner Sweep, map revealers, triggers).
    /// `x`/`y` are world pixels.
    pub fn reveal_point(&mut self, owner: u8, x: i32, y: i32, tiles: i32) {
        let Some(mut vision) = self.vision.take() else {
            return;
        };
        vision.reveal(owner, x, y, tiles, true, &self.terrain);
        self.vision = Some(vision);
    }

    /// Whether `observer_owner` can see the unit through cloak/burrow.
    pub fn can_see_unit(&self, observer_owner: u8, target_idx: usize) -> bool {
        let Some(target) = self.units.get(target_idx) else {
            return false;
        };
        if !target.alive {
            return false;
        }
        if target.status.revealed_ticks > 0 {
            return true;
        }
        if !target.status.cloaked && !target.status.burrowed {
            return !self.fog_of_war
                || self.is_visible_to(observer_owner, target.x / 256, target.y / 256);
        }
        // A detector sees cloaked and burrowed units inside its detection
        // radius.  Buildings use the hardened 7-tile radius recovered from
        // the retail detector path; mobile detectors use their sight range.
        for detector in &self.units {
            if !detector.alive
                || detector.owner != observer_owner
                || detector.in_transport.is_some()
            {
                continue;
            }
            let dty = self.data.unit(detector.type_id);
            if !dty.is_detector() || detector.status.hallucination {
                continue;
            }
            // `detector.x`/`target.x` are 1/256-pixel values, so scale the
            // pixel radius into the same fixed-point space before squaring.
            let range = if dty.is_building() {
                7 * 32 * 256
            } else {
                dty.sight_range as i32 * 32 * 256
            };
            let dx = (detector.x - target.x) as i64;
            let dy = (detector.y - target.y) as i64;
            if dx * dx + dy * dy <= (range as i64) * (range as i64)
                && (!self.fog_of_war
                    || self.is_visible_to(observer_owner, target.x / 256, target.y / 256))
            {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::Vision;
    use crate::map::Terrain;

    #[test]
    fn sight_masks_propagate_from_earlier_nodes() {
        let masks = super::sight_masks();
        let mut last = 0;
        for (r, m) in masks.iter().enumerate() {
            assert_eq!(m.nodes.len(), m.ext + 9, "range {r}");
            assert!(m.nodes.len() > last, "range {r} grows");
            last = m.nodes.len();
            let half = 1 + r as i32;
            for (k, n) in m.nodes.iter().enumerate().skip(1) {
                assert!(n.x.abs() <= half && n.y.abs() <= half, "range {r} node {k}");
                if k >= 9 {
                    let p = n.prev.expect("outer nodes have a predecessor");
                    assert!(p < k && n.prev2.is_none_or(|q| q < k));
                }
            }
        }
    }

    #[test]
    fn a_cliff_hides_what_lies_behind_it_but_not_itself() {
        // Viewer on low ground at tile (10, 10); a high wall on column 13.
        let mut terrain = Terrain::open(32, 32);
        for wy in 0..32 * 4 {
            for wx in 13 * 4..14 * 4 {
                terrain.set_elevation_cell(wx, wy, 2);
            }
        }
        let mut v = Vision::new(32, 32);
        v.reveal(0, 10 * 32 + 16, 10 * 32 + 16, 7, false, &terrain);
        assert!(
            v.is_visible(0, 13 * 32 + 16, 10 * 32 + 16),
            "the cliff face"
        );
        assert!(
            !v.is_visible(0, 15 * 32 + 16, 10 * 32 + 16),
            "behind the cliff"
        );
        // An air unit sees past it.
        let mut air = Vision::new(32, 32);
        air.reveal(0, 10 * 32 + 16, 10 * 32 + 16, 7, true, &terrain);
        assert!(air.is_visible(0, 15 * 32 + 16, 10 * 32 + 16));
    }

    #[test]
    fn sight_reaches_one_tile_beyond_the_range() {
        // A seer at the centre of tile (20, 20) with sight 11 reveals the tile
        // 12 tiles east (the retail 3 + 2r mask) but not the one 13 east.
        let terrain = Terrain::open(64, 64);
        let mut v = Vision::new(64, 64);
        v.reveal(0, 20 * 32 + 16, 20 * 32 + 16, 11, false, &terrain);
        assert!(v.is_visible(0, 32 * 32 + 16, 20 * 32 + 16));
        assert!(!v.is_visible(0, 33 * 32 + 16, 20 * 32 + 16));
        assert!(v.is_explored(0, 32 * 32 + 16, 20 * 32 + 16));
        assert!(
            !v.is_visible(1, 21 * 32, 20 * 32),
            "other players see nothing"
        );
    }
}

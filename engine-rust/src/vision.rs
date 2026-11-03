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

use crate::map::Terrain;
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

    /// `cx`/`cy` are world pixels.
    pub fn reveal(
        &mut self,
        owner: u8,
        cx: i32,
        cy: i32,
        range_tiles: i32,
        flying: bool,
        terrain: &Terrain,
    ) {
        if owner >= 32 {
            return;
        }
        let tx = cx.div_euclid(32);
        let ty = cy.div_euclid(32);
        let center_height = terrain.elevation_px(cx, cy);
        // INFERENCE (2026-11-03): the retail sight mask is `3 + 2r` tiles wide
        // (one tile beyond `r`).  Measured on 135051 / 135020: the fog's 50 %
        // point sits at 12.0 tiles from the Nexus (sight 11) and 11.0 from
        // the Command Center (sight 10).  Falsifier: a unit exactly r+1 tiles
        // from a lone seer staying hidden in the retail client.
        let r = range_tiles.max(0) + 1;
        let r2 = r * r;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy > r2 {
                    continue;
                }
                let x = tx + dx;
                let y = ty + dy;
                if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
                    continue;
                }
                let px = x * 32 + 16;
                let py = y * 32 + 16;
                // Ground units cannot see over a higher ledge.  Adjacent
                // tiles are always revealed so a unit at a cliff edge still
                // sees the tile it is standing next to.
                if !flying && (dx.abs() > 1 || dy.abs() > 1) {
                    let h = terrain.elevation_px(px, py);
                    if h > center_height {
                        continue;
                    }
                }
                let i = y as usize * self.width + x as usize;
                let bit = 1u32 << owner;
                self.visible[i] |= bit;
                self.explored[i] |= bit;
            }
        }
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

//! Terrain contours for the client's short-range path search.
//!
//! Port of the 32-bit client's map-load step (session log 2026-11-06 (140)):
//! `FUN_00760c20` builds a walk grid of one byte per 8-pixel cell (`0x80`
//! unwalkable; on a walkable cell bits 1 / 2 / 4 / 8 mark the edge above /
//! right / below / left as blocked), and `FUN_00760350` traces every blocked
//! boundary into straight segments, filed by the side they block (0 up,
//! 1 right, 2 down, 3 left) and sorted by (position, span start).  The
//! lookups (`FUN_007620f0` / `FUN_00762000` / `FUN_00761e30` /
//! `FUN_00761f20`) binary-search these lists.
//!
//! The trace is ported literally (same scan order, same XOR bookkeeping,
//! same corner tables from the dump at `0xf2b6a0`-`0xf2b733`), so the
//! segments, including where a closed contour is split at its start cell,
//! match the client's.  Not yet used by movement.

use crate::map::Terrain;

/// One contour segment (the client's 8-byte record).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment {
    /// Fixed coordinate in pixels (y for up / down, x for right / left).
    pub pos: i16,
    /// Span along the segment, in pixels (inclusive client coordinates).
    pub start: i16,
    pub end: i16,
    /// Blocked side: 0 up, 1 right, 2 down, 3 left (byte 6).
    pub facing: u8,
    /// Byte 7: corner kinds; the lookups add the mover's box extents
    /// indexed by bits 0-1 and 2-3.
    pub flags: u8,
}

/// The four sorted contour lists.
#[derive(Clone, Debug, Default)]
pub struct Contours {
    pub lists: [Vec<Segment>; 4],
}

/// The search context's mover extents (`FUN_007601c0`, context `+0x20`):
/// from the `units.dat` box (left, up, right, down) as
/// `[up, -right, -down, left]`.  A segment's span ends are widened by the
/// extents its flags select, which moves the walls into mover-centre space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mover {
    pub ext: [i16; 4],
}

impl Mover {
    pub fn from_box(left: i16, up: i16, right: i16, down: i16) -> Self {
        Self {
            ext: [up, -right, -down, left],
        }
    }

    fn overlaps(&self, s: &Segment, lo: i32, hi: i32) -> bool {
        let a = (self.ext[(s.flags & 3) as usize] as i32 + s.start as i32).max(lo);
        let b = (self.ext[(s.flags >> 2 & 3) as usize] as i32 + s.end as i32).min(hi);
        a <= b
    }
}

const UNWALKABLE: u8 = 0x80;
const STRIDE: usize = 0x400;

// Tables from the dump (`0xf2b6a0`, `0xf2b6b0`, `0xf2b6c0`, `0xf2b6c8`,
// `0xf2b6d0`, `0xf2b6f4`, `0xf2b714`).
const LOWEST_EDGE: [u8; 16] = [0xff, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0];
const START_KIND: [u8; 16] = [0xff, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 0, 1, 0, 1, 0];
const STEP: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
const CONCAVE_STEP: [(i32, i32); 4] = [(1, -1), (1, 1), (-1, 1), (-1, -1)];
const START_OFF: [(i16, i16); 8] = [
    (-1, -1),
    (8, -1),
    (8, 8),
    (-1, 8),
    (0, -1),
    (8, 0),
    (7, 8),
    (-1, 7),
];
const END_OFF: [(i16, i16); 8] = [
    (8, -1),
    (8, 8),
    (-1, 8),
    (-1, -1),
    (7, -1),
    (8, 7),
    (0, 8),
    (-1, 0),
];

/// `FUN_00760c20`: the client's walk grid (cells beyond the map are
/// unwalkable).  The bottom eight cell rows lose 20 cells at each end and the
/// bottom four rows are fully unwalkable, as the client does.
pub fn walk_grid(terrain: &Terrain) -> (Vec<u8>, usize, usize) {
    let (w, h) = (terrain.width * 4, terrain.height * 4);
    let mut g = vec![UNWALKABLE; STRIDE * (h + 2).max(STRIDE / 4)];
    for y in 0..h {
        for x in 0..w {
            if terrain.walkable_cell(x as i32, y as i32) {
                g[y * STRIDE + x] = 0;
            }
        }
    }
    for y in h.saturating_sub(8)..h {
        let full = y + 4 >= h;
        for x in 0..w {
            if full || x < 20 || x + 20 >= w {
                g[y * STRIDE + x] = UNWALKABLE;
            }
        }
    }
    let blocked = |g: &Vec<u8>, x: i32, y: i32| {
        x < 0
            || y < 0
            || x >= w as i32
            || y >= h as i32
            || g[y as usize * STRIDE + x as usize] & 0x80 != 0
    };
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            if blocked(&g, x, y) {
                continue;
            }
            let mut b = 0u8;
            for (bit, (dx, dy)) in [(1u8, (0, -1)), (2, (1, 0)), (4, (0, 1)), (8, (-1, 0))] {
                if blocked(&g, x + dx, y + dy) {
                    b |= bit;
                }
            }
            g[y as usize * STRIDE + x as usize] = b;
        }
    }
    (g, w, h)
}

impl Contours {
    /// `FUN_00760350` over the walk grid of `terrain`.
    pub fn from_terrain(terrain: &Terrain) -> Self {
        let (mut g, w, h) = walk_grid(terrain);
        let mut lists: [Vec<Segment>; 4] = Default::default();
        let at = |x: i32, y: i32| -> Option<usize> {
            (x >= 0 && y >= 0 && (x as usize) < STRIDE && (y as usize) < h + 2)
                .then(|| y as usize * STRIDE + x as usize)
        };
        // Row-major scan; a finished contour has cleared its bits, so the
        // next start is the first cell with an edge bit left.
        let mut scan = 0usize;
        loop {
            while scan < STRIDE * h
                && (g[scan] & 0x80 != 0 || g[scan] & 0x0f == 0 || scan % STRIDE >= w)
            {
                scan += 1;
            }
            if scan >= STRIDE * h {
                break;
            }
            let (mut col, mut row) = ((scan % STRIDE) as i32, (scan / STRIDE) as i32);
            let mut cell = scan;
            let mut v = g[cell];
            let mut dir = LOWEST_EDGE[(v & 0xf) as usize];
            let mut kind = START_KIND[(v & 0xf) as usize];
            if dir == 2 {
                // The client restarts a bottom edge from the right-facing
                // edge of the cell below-left.
                if let Some(c) = at(col - 1, row + 1)
                    && g[c] & 0x80 == 0
                    && g[c] & 0x0f != 0
                {
                    col -= 1;
                    row += 1;
                    cell = c;
                    v = g[cell];
                    dir = LOWEST_EDGE[(v & 0xf) as usize];
                    kind = START_KIND[(v & 0xf) as usize];
                }
            }
            let si = dir as usize + (kind as usize & 1) * 4;
            let first = (
                col as i16 * 8 + START_OFF[si].0,
                row as i16 * 8 + START_OFF[si].1,
            );
            g[cell] ^= 1 << dir;
            let (mut sx, mut sy) = first;
            let mut guard = 0usize;
            loop {
                guard += 1;
                if guard > 1_000_000 {
                    break;
                }
                let walk = (dir + 1) & 3;
                let bit = 1u8 << dir;
                g[cell] ^= bit;
                let mut concave = 0u8;
                while g[cell] & (1 << walk) == 0 {
                    let (dx, dy) = STEP[walk as usize];
                    let Some(next) = at(col + dx, row + dy) else {
                        break;
                    };
                    if g[next] & 0x80 != 0 || g[next] & bit == 0 {
                        concave = 1;
                        break;
                    }
                    col += dx;
                    row += dy;
                    cell = next;
                    g[cell] ^= bit;
                }
                let ei = dir as usize + concave as usize * 4;
                let ex = col as i16 * 8 + END_OFF[ei].0;
                let ey = row as i16 * 8 + END_OFF[ei].1;
                let b22 = (!(dir.wrapping_mul(2)) ^ kind.wrapping_mul(2) ^ dir) & 3;
                let b8 = (((concave ^ dir).wrapping_mul(2)) ^ dir ^ 1) & 3;
                let (pos, start, end, flags) = match dir {
                    0 => (
                        sy,
                        sx,
                        ex,
                        (((kind & 1) << 2 | b8) << 2) | concave << 5 | b22,
                    ),
                    1 => (
                        sx,
                        sy,
                        ey,
                        (((kind & 1) << 2 | b8) << 2) | concave << 5 | b22,
                    ),
                    2 => (
                        sy,
                        ex,
                        sx,
                        (((kind & 1) << 3 | b22) << 2) | concave << 4 | b8,
                    ),
                    _ => (
                        sx,
                        ey,
                        sy,
                        (((kind & 1) << 3 | b22) << 2) | concave << 4 | b8,
                    ),
                };
                lists[dir as usize].push(Segment {
                    pos,
                    start,
                    end,
                    facing: dir,
                    flags,
                });
                if concave == 1 {
                    let (dx, dy) = CONCAVE_STEP[dir as usize];
                    col += dx;
                    row += dy;
                    match at(col, row) {
                        Some(c) => cell = c,
                        None => break,
                    }
                    dir = (dir + 3) & 3;
                } else {
                    dir = walk;
                }
                kind = concave;
                sx = ex;
                sy = ey;
                if (ex, ey) == first {
                    break;
                }
            }
            g[cell] ^= 1 << dir;
        }
        for l in &mut lists {
            // `FUN_00c9cb40` (qsort) with `FUN_007609f0`: by position, then
            // span start.
            l.sort_by_key(|s| (s.pos, s.start));
        }
        Self { lists }
    }

    /// `FUN_007620f0`: the nearest up-facing wall at or above `y` (in
    /// mover-centre space), not above `limit`, whose widened span meets
    /// `x0..=x1`.
    pub fn up(&self, m: &Mover, y: i32, x0: i32, x1: i32, limit: i32) -> Option<&Segment> {
        let l = &self.lists[0];
        let a = y - m.ext[0] as i32;
        let n = l.partition_point(|s| s.pos as i32 <= a);
        l[..n]
            .iter()
            .rev()
            .take_while(|s| s.pos as i32 >= limit - m.ext[0] as i32)
            .find(|s| m.overlaps(s, x0, x1))
    }

    /// `FUN_00762000`: the nearest right-facing wall at or right of `x`, not
    /// beyond `limit`, meeting `y0..=y1`.
    pub fn right(&self, m: &Mover, x: i32, y0: i32, y1: i32, limit: i32) -> Option<&Segment> {
        let l = &self.lists[1];
        let a = x - m.ext[1] as i32;
        let n = l.partition_point(|s| (s.pos as i32) < a);
        l[n..]
            .iter()
            .take_while(|s| s.pos as i32 <= limit - m.ext[1] as i32)
            .find(|s| m.overlaps(s, y0, y1))
    }

    /// `FUN_00761e30`: the nearest down-facing wall at or below `y`, not
    /// beyond `limit`, meeting `x0..=x1`.
    pub fn down(&self, m: &Mover, y: i32, x0: i32, x1: i32, limit: i32) -> Option<&Segment> {
        let l = &self.lists[2];
        let a = y - m.ext[2] as i32;
        let n = l.partition_point(|s| (s.pos as i32) < a);
        l[n..]
            .iter()
            .take_while(|s| s.pos as i32 <= limit - m.ext[2] as i32)
            .find(|s| m.overlaps(s, x0, x1))
    }

    /// `FUN_00761f20`: the nearest left-facing wall at or left of `x`, not
    /// beyond `limit`, meeting `y0..=y1`.
    pub fn left(&self, m: &Mover, x: i32, y0: i32, y1: i32, limit: i32) -> Option<&Segment> {
        let l = &self.lists[3];
        let a = x - m.ext[3] as i32;
        let n = l.partition_point(|s| s.pos as i32 <= a);
        l[..n]
            .iter()
            .rev()
            .take_while(|s| s.pos as i32 >= limit - m.ext[3] as i32)
            .find(|s| m.overlaps(s, y0, y1))
    }

    pub fn len(&self) -> usize {
        self.lists.iter().map(Vec::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(t: &mut Terrain, x0: i32, y0: i32, x1: i32, y1: i32) {
        for y in y0..y1 {
            for x in x0..x1 {
                t.set_walkable_cell(x, y, false);
            }
        }
    }

    #[test]
    fn a_square_obstacle_gives_one_segment_per_side() {
        // 16 x 16 tiles = 64 x 64 cells; a 4 x 4-cell block at cells 20..24.
        let mut t = Terrain::open(16, 16);
        block(&mut t, 20, 20, 24, 24);
        let c = Contours::from_terrain(&t);
        // The obstacle: the walkable cells facing it (row 24 faces up at the
        // block's bottom edge, etc.).
        let up: Vec<_> = c.lists[0].iter().filter(|s| s.pos == 24 * 8 - 1).collect();
        assert_eq!(up.len(), 1, "{:?}", c.lists[0]);
        assert_eq!((up[0].start, up[0].end), (20 * 8, 24 * 8 - 1));
        let left: Vec<_> = c.lists[3]
            .iter()
            .filter(|s| s.pos == 24 * 8 - 1 + 1)
            .collect();
        assert!(left.len() <= 1);
        for l in &c.lists {
            assert!(
                l.windows(2)
                    .all(|w| (w[0].pos, w[0].start) <= (w[1].pos, w[1].start))
            );
        }
    }

    #[test]
    fn lookups_find_the_nearest_wall_in_mover_centre_space() {
        // Obstacle at pixels 160..=191; an SCV-sized mover (11 px each way).
        let mut t = Terrain::open(16, 16);
        block(&mut t, 20, 20, 24, 24);
        let c = Contours::from_terrain(&t);
        let m = Mover::from_box(11, 11, 11, 11);
        // Straight below the obstacle, looking up: its bottom (y 191).
        assert_eq!(c.up(&m, 300, 176, 176, 0).map(|s| s.pos), Some(191));
        // Beyond the widened span (149..=202) only the map border is found.
        assert_eq!(c.up(&m, 300, 140, 140, 0).map(|s| s.pos), Some(-1));
        assert_eq!(c.up(&m, 300, 150, 150, 0).map(|s| s.pos), Some(191));
        // The limit stops the scan before the border.
        assert!(c.up(&m, 300, 140, 140, 100).is_none());
        // From the left, looking right: the obstacle's left side (x 160).
        assert_eq!(c.right(&m, 100, 176, 176, 400).map(|s| s.pos), Some(160));
        // From above looking down, and from the right looking left.
        assert_eq!(c.down(&m, 100, 176, 176, 400).map(|s| s.pos), Some(160));
        assert_eq!(c.left(&m, 300, 176, 176, 0).map(|s| s.pos), Some(191));
    }

    #[test]
    fn every_blocked_edge_is_covered_once() {
        let mut t = Terrain::open(16, 16);
        block(&mut t, 10, 10, 14, 30);
        block(&mut t, 30, 5, 33, 8);
        block(&mut t, 40, 20, 41, 21);
        let c = Contours::from_terrain(&t);
        let (g, w, h) = walk_grid(&t);
        let mut want = 0usize;
        for y in 0..h {
            for x in 0..w {
                let v = g[y * STRIDE + x];
                if v & 0x80 == 0 {
                    want += (v & 0xf).count_ones() as usize;
                }
            }
        }
        // Sum of segment lengths in cells (inclusive client spans: up /
        // right lists run forwards, down / left store end <= start).
        let mut got = 0usize;
        for (d, l) in c.lists.iter().enumerate() {
            for s in l {
                let span = (s.end as i32 - s.start as i32).abs() + 1;
                assert!(span >= 8, "dir {d} {s:?}");
                got += (span as usize).div_ceil(8);
            }
        }
        assert!(
            got >= want && got <= want + 2 * c.len(),
            "edges {want}, segment cells {got}"
        );
    }
}

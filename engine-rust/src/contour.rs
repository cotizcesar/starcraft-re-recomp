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

/// One `FUN_0075f290` step (see `Contours::node_step`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeStep {
    /// (left, top, right, bottom) the rays reached.
    pub reach: (i32, i32, i32, i32),
    /// The walls that stopped the up, right, down, left rays.
    pub hits: [Option<Segment>; 4],
    pub next: Vec<((i32, i32), u8)>,
    pub quadrants: u8,
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

    /// Terrain segments plus the edges of `units` (inclusive pixel boxes of
    /// stationary units, `FUN_0075f8e0`), re-sorted: the walls one search
    /// node sees.  Unit edges carry the client's flag bytes (`0x3d` for the
    /// up / down lists, `0x32` for right / left).  INFERENCE: the client
    /// also merges overlapping same-position segments (`FUN_0075ff00`) and
    /// filters to the node's window; neither changes a ray's hit distance.
    pub fn with_units(&self, units: &[(i32, i32, i32, i32)]) -> Self {
        let mut c = self.clone();
        for &(l, t, r, b) in units {
            let (l, t, r, b) = (l as i16, t as i16, r as i16, b as i16);
            // A unit's bottom edge stops upward movement, etc.
            c.lists[0].push(Segment {
                pos: b,
                start: l,
                end: r,
                facing: 0,
                flags: 0x3d,
            });
            c.lists[1].push(Segment {
                pos: l,
                start: t,
                end: b,
                facing: 1,
                flags: 0x32,
            });
            c.lists[2].push(Segment {
                pos: t,
                start: l,
                end: r,
                facing: 2,
                flags: 0x3d,
            });
            c.lists[3].push(Segment {
                pos: r,
                start: t,
                end: b,
                facing: 3,
                flags: 0x32,
            });
        }
        for l in &mut c.lists {
            l.sort_by_key(|s| (s.pos, s.start));
        }
        c
    }

    /// `FUN_0075f290`, the per-node step: four rays from `node` along its
    /// row and column, each at most 64 px (`limits` = the map's last pixel),
    /// a side held at the node when `mask` lacks its bits (0x19 up, 0x23
    /// right, 0x46 down, 0x8c left).  Returns the reach box, which rays hit
    /// a wall, the successors (point, client mask) for the open directions
    /// 0x10 / 0x20 / 0x40 / 0x80, and the quadrant bits (1 / 2 / 4 / 8) that
    /// select the corner sweeps.
    pub fn node_step(&self, m: &Mover, node: (i32, i32), mask: u8, limits: (i32, i32)) -> NodeStep {
        let (x, y) = node;
        let mut top = (y - 64).max(0);
        let mut right = (x + 64).min(limits.0);
        let mut bottom = (y + 64).min(limits.1);
        let mut left = (x - 64).max(0);
        if mask & 0x19 == 0 {
            top = y;
        }
        if mask & 0x23 == 0 {
            right = x;
        }
        if mask & 0x46 == 0 {
            bottom = y;
        }
        if mask & 0x8c == 0 {
            left = x;
        }
        let e = m.ext.map(i32::from);
        let hit_up = self.up(m, y, x, x, top - 1).copied();
        if let Some(p) = hit_up.map(|s| s.pos as i32) {
            top = e[0] + 1 + p;
        }
        let hit_right = self.right(m, x, y, y, right + 1).copied();
        if let Some(p) = hit_right.map(|s| s.pos as i32) {
            right = e[1] - 1 + p;
        }
        let hit_down = self.down(m, y, x, x, bottom + 1).copied();
        if let Some(p) = hit_down.map(|s| s.pos as i32) {
            bottom = e[2] - 1 + p;
        }
        let hit_left = self.left(m, x, y, y, left - 1).copied();
        if let Some(p) = hit_left.map(|s| s.pos as i32) {
            left = e[3] + 1 + p;
        }
        // Bits of directions that went nowhere are dropped (the client's
        // sequence of and-masks).
        let mut b = mask;
        if top == y && right == x {
            b &= 0xfe;
        }
        // Quadrants: 1 up-right, 2 down-right, 4 down-left, 8 up-left.
        if bottom == y && right == x {
            b &= 0xfd;
        }
        if bottom == y && left == x {
            b &= 0xfb;
        }
        if top == y && left == x {
            b &= 0xf7;
        }
        if top == y {
            b &= 0xef;
        }
        if right == x {
            b &= 0xdf;
        }
        if bottom == y {
            b &= 0xbf;
        }
        if left == x {
            b &= 0x7f;
        }
        let mut next = Vec::new();
        if b & 0x10 != 0 {
            b |= 9;
            next.push(((x, top), if hit_up.is_some() { 0xa6 } else { 0xbf }));
        }
        if b & 0x20 != 0 {
            b |= 3;
            next.push(((right, y), if hit_right.is_some() { 0x5c } else { 0x7f }));
        }
        if b & 0x40 != 0 {
            b |= 6;
            next.push(((x, bottom), if hit_down.is_some() { 0xa9 } else { 0xef }));
        }
        if b & 0x80 != 0 {
            b |= 0xc;
            next.push(((left, y), if hit_left.is_some() { 0x53 } else { 0xdf }));
        }
        NodeStep {
            reach: (left, top, right, bottom),
            hits: [hit_up, hit_right, hit_down, hit_left],
            next,
            quadrants: b & 0x0f,
        }
    }

    /// `0x75e0e0`: the up-right quadrant's corner sweep after a node step.
    /// Band A (the node's row up to the up limit, the node's column to the
    /// right reach) finds the nearest up-facing wall and walks its segments
    /// leftwards, emitting a candidate one pixel outside each end
    /// (`FUN_0075db50`, extents `+0x30`); band B (above A) does the same
    /// with right-facing walls downwards.  Returns (point, mask) candidates
    /// in the client's emission order.  INFERENCE where the decompile loses
    /// control flow: band B's segment walk stops like band A's.
    pub fn sweep_up_right(
        &self,
        m: &Mover,
        node: (i32, i32),
        step: &NodeStep,
    ) -> Vec<((i32, i32), u8)> {
        let (x, y) = node;
        let (_, top, right, _) = step.reach;
        let e = m.ext.map(i32::from);
        let e30 = [e[0] + 1, e[1] - 1, e[2] - 1, e[3] + 1];
        let norm = |s: &Segment| {
            (
                s.pos as i32 + e30[s.facing as usize & 3],
                s.start as i32 + e30[(s.flags & 3) as usize],
                s.end as i32 + e30[(s.flags >> 2 & 3) as usize],
            )
        };
        let index_of =
            |list: &Vec<Segment>, seg: &Segment| list.iter().position(|q| std::ptr::eq(q, seg));
        let mut out = Vec::new();
        // Band A.
        let mut band_bottom = y;
        let mut band_right = right;
        if x < right {
            let mut upper = top;
            let mut right_wall_top = top;
            if let Some(rh) = step.hits[1] {
                right_wall_top = e[(rh.flags & 3) as usize] - 1 + rh.start as i32;
                if top < right_wall_top {
                    upper = right_wall_top;
                }
            }
            let found = self.up(m, y - 1, x, right, upper - 1);
            let mut mask: u8 = 0x3b;
            let mut wall = None;
            if let Some(seg) = found {
                let n = norm(seg);
                upper = n.0;
                if right < n.2 {
                    mask = 0x22;
                } else if right == n.2 {
                    mask = if seg.flags & 0x20 != 0 { 0x33 } else { 0 };
                }
                wall = Some((index_of(&self.lists[0], seg), n));
            }
            if step.hits[1].is_some() {
                mask &= 0xfd;
                if right_wall_top < upper {
                    mask &= 0xdc;
                }
            }
            if upper < y {
                out.push(((right, y), 0xe7));
            } else {
                mask |= 0x44;
            }
            out.push(((right, upper), mask));
            if upper == top {
                out.push(((x, upper), 0xdd));
            }
            let mut next_right = right;
            if let Some((Some(mut idx), mut cur)) = wall {
                if cur.2 < right {
                    out.push(((cur.2, cur.0), 0x11));
                }
                next_right = x;
                while x <= cur.1 {
                    out.push(((cur.1, cur.0), 0x18));
                    if idx == 0 {
                        next_right = cur.1;
                        break;
                    }
                    idx -= 1;
                    let p = norm(&self.lists[0][idx]);
                    if p.0 != upper || p.2 < x {
                        next_right = cur.1;
                        break;
                    }
                    if p.2 <= right {
                        out.push(((p.2, p.0), 0x11));
                    }
                    cur = p;
                }
            }
            band_bottom = upper;
            band_right = next_right;
        }
        // Band B.
        if top < band_bottom {
            let mut limit = band_right;
            let mut up_wall_end = band_right;
            if let Some(uh) = step.hits[0] {
                up_wall_end = e[(uh.flags >> 2 & 3) as usize] + 1 + uh.end as i32;
                if up_wall_end < band_right {
                    limit = up_wall_end;
                }
            }
            let found = self.right(m, x + 1, top, band_bottom, limit + 1);
            let mut mask: u8 = 0x3b;
            let mut wall = None;
            if let Some(seg) = found {
                let n = norm(seg);
                limit = n.0;
                if n.1 < top {
                    mask = 0x18;
                } else if n.1 == top {
                    mask = if seg.flags & 0x10 != 0 { 0x39 } else { 0 };
                }
                wall = Some((index_of(&self.lists[1], seg), n));
            }
            if step.hits[0].is_some() {
                mask &= 0xf7;
                if limit < up_wall_end {
                    mask &= 0xd6;
                }
            }
            if x < limit {
                out.push(((x, top), 0xdd));
            } else {
                mask |= 0x84;
            }
            out.push(((limit, top), mask));
            if limit < band_right {
                out.push(((limit, band_bottom), 1));
            }
            if let Some((Some(mut idx), mut cur)) = wall {
                if top < cur.1 {
                    out.push(((cur.0, cur.1), 0x21));
                }
                while cur.2 <= band_bottom {
                    out.push(((cur.0, cur.2), 0x22));
                    idx += 1;
                    if idx >= self.lists[1].len() {
                        break;
                    }
                    let p = norm(&self.lists[1][idx]);
                    if p.0 != limit || band_bottom < p.1 {
                        break;
                    }
                    if top <= p.1 {
                        out.push(((p.0, p.1), 0x21));
                    }
                    cur = p;
                }
            }
        }
        out
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
    fn node_step_casts_four_rays_stopped_by_walls_and_standing_units() {
        // Open 16 x 16-tile map with the 4 x 4-cell block at 160..=191 and a
        // Marine-sized unit box at (300..=316, 290..=310).
        let mut t = Terrain::open(16, 16);
        block(&mut t, 20, 20, 24, 24);
        let c = Contours::from_terrain(&t).with_units(&[(300, 290, 316, 310)]);
        let m = Mover::from_box(11, 11, 11, 11);
        // From (176, 260): up hits the block's bottom (191) widened by 11.
        let s = c.node_step(&m, (176, 260), 0xff, (511, 511));
        assert_eq!(s.reach.1, 191 + 11 + 1);
        assert!(s.hits[0].is_some() && s.hits[1].is_none() && s.hits[3].is_none());
        assert_eq!(s.reach.2, 176 + 64);
        assert_eq!(s.quadrants, 0x0f);
        assert_eq!(s.next.len(), 4);
        assert_eq!(s.next[0], ((176, 203), 0xa6));
        assert_eq!(s.next[1], ((240, 260), 0x7f));
        // From (250, 300) looking right: the unit's left edge (300) less 11.
        let s = c.node_step(&m, (250, 300), 0xff, (511, 511));
        assert!(s.hits[1].is_some());
        assert_eq!(s.reach.2, 300 - 11 - 1);
        // Mask 0x19 (up, quadrants 1 and 8) shares bits with the right
        // (0x23) and left (0x8c) sides; only the down side stays at the
        // node, and with nothing below, the down quadrants close.
        let s = c.node_step(&m, (250, 300), 0x19, (511, 511));
        assert_eq!((s.reach.0, s.reach.2, s.reach.3), (250 - 64, 300 - 12, 300));
        assert_eq!(s.quadrants & 0b0110, 0);
        assert!(s.next.iter().all(|(p, _)| p.1 <= 300));
    }

    #[test]
    fn up_right_sweep_emits_the_corners_of_the_marine_gap() {
        // Boot Camp, session log 131: the field-18 SCV turned up at about
        // (1670, 237), between the Marine groups (units.dat Marine box
        // 8 / 9 / 8 / 10, SCV 11 each way).
        let t = Terrain::open(64, 64);
        let marines = [
            (1651, 172),
            (1615, 187),
            (1670, 199),
            (1640, 211),
            (1722, 304),
            (1688, 288),
            (1715, 340),
            (1685, 321),
        ];
        let boxes: Vec<_> = marines
            .iter()
            .map(|&(x, y)| (x - 8, y - 9, x + 8, y + 10))
            .collect();
        let c = Contours::from_terrain(&t).with_units(&boxes);
        let m = Mover::from_box(11, 11, 11, 11);
        let s = c.node_step(&m, (1600, 291), 0xff, (2047, 2047));
        let pts = c.sweep_up_right(&m, (1600, 291), &s);
        // One pixel below the upper group's bottom (211 + 10 + 11 + 1 = 233),
        // at both ends of the bottom Marine's widened edge.
        assert!(pts.contains(&((1660, 233), 0x11)), "{pts:?}");
        assert!(pts.contains(&((1620, 233), 0x18)), "{pts:?}");
        // Out of the Marines' reach the sweep only gives the box corners.
        let s = c.node_step(&m, (1484, 295), 0xff, (2047, 2047));
        assert_eq!(
            c.sweep_up_right(&m, (1484, 295), &s),
            vec![
                ((1548, 295), 0xe7),
                ((1548, 231), 0x3b),
                ((1484, 231), 0xdd)
            ]
        );
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

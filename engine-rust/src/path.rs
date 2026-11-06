//! Deterministic grid pathfinding.
//!
//! StarCraft uses a two-level pathfinder: coarse pathfinding regions on the
//! 32-pixel tile grid and a short-range grid search over 8x8-pixel walk
//! cells.  The region half follows the recovered builder at `0x7a302c10`
//! (tile resolution, bbox centres, adjacency and the size-6 merge pass); the
//! short-range half is an octile A* over the recovered tileset walk grid with
//! unit footprints and building obstacles.  The module is deliberately
//! dependency-free and deterministic so simulations are reproducible.
//!
//! Tie-breaking follows the recovered retail open-list heap.  `FUN_7a305590`
//! is the region-graph A*: its open list is a binary heap ordered **only** by
//! the `f` field at `+0x14` of a heap entry.  `FUN_7a305e20` (sift-up) keeps
//! bubbling while the new entry's `f` is less than *or equal to* its parent's,
//! and `FUN_7a305da0` (sift-down) stops as soon as the parent is less than or
//! equal to the chosen child and chooses the left child on a tie.  Equal-`f`
//! entries therefore pop newest-first instead of the old `(f,g,x,y)` order.
//! Neighbours are visited in the recovered `local_18` order from
//! `FUN_7a304c10`: left, up, right, down, up-left, up-right, down-left,
//! down-right.  The decompile is stored in
//! `docs/evidence/pathfinding_astar.txt`.

use crate::map::{Terrain, WALK_TILE_PX};

pub const STRAIGHT_COST: i32 = 10;
pub const DIAGONAL_COST: i32 = 14;

/// The retail distance helper `FUN_7a1a4060` (runtime `0x7a1a4060`).
///
/// The decompile is:
///
/// ```c
/// dx = abs(x2-x1); dy = abs(y2-y1);
/// if (dx < dy) { a = dx; b = dy; } else { a = dy; b = dx; }
/// if (b >> 2 < a) {
///     a = (a >> 1) + a;
///     return (a >> 2) + ((a >> 7) - (b >> 6)) - (b >> 4) + b;
/// }
/// return b;
/// ```
///
/// It is the exact octile approximation used by both the region A* and the
/// movement code.  The integer expression deliberately reproduces the
/// decompiled shifts so a Rust unit test can pin the recovered values.
pub fn retail_distance(a: (i32, i32), b: (i32, i32)) -> i32 {
    let dx = (a.0 - b.0).abs();
    let dy = (a.1 - b.1).abs();
    let (mut lo, hi) = if dx < dy { (dx, dy) } else { (dy, dx) };
    if hi >> 2 < lo {
        lo = (lo >> 1) + lo;
        return (lo >> 2) + ((lo >> 7) - (hi >> 6)) - (hi >> 4) + hi;
    }
    hi
}

/// The recovered retail open list.
///
/// `FUN_7a305590` keeps one heap entry per region and compares only the
/// 32-bit `f` at `+0x14`.  This is a faithful 0-based port of the two heap
/// helpers:
///
/// * `FUN_7a305e20` sift-up: stop when `f(parent) < f(child)`, so equal-cost
///   children bubble above their parents;
/// * `FUN_7a305da0` sift-down: choose the right child only when
///   `f(right) < f(left)` (ties keep the left child) and stop when
///   `f(parent) <= f(chosen)`.
///
/// This differs from `BinaryHeap<Reverse<(f, g, x, y)>>`, which ordered ties by
/// `g` and coordinate.  The recovered heap makes the order depend only on
/// insertion/discovery order, which in turn is the recovered neighbour order.
#[derive(Clone, Debug, Default)]
struct RetailOpen {
    /// Heap of node indices.
    heap: Vec<u32>,
    /// Position of each node in `heap`, or `u32::MAX`.
    pos: Vec<u32>,
    /// Cached `f` value for each node.
    f: Vec<i32>,
}

impl RetailOpen {
    fn new(nodes: usize) -> Self {
        Self {
            heap: Vec::new(),
            pos: vec![u32::MAX; nodes],
            f: vec![0; nodes],
        }
    }

    fn sift_up(&mut self, mut i: usize) {
        while i > 0 {
            let p = (i - 1) / 2;
            // Recovered: stop only when the parent is strictly cheaper.
            if self.f[self.heap[p] as usize] < self.f[self.heap[i] as usize] {
                break;
            }
            self.heap.swap(p, i);
            self.pos[self.heap[p] as usize] = p as u32;
            self.pos[self.heap[i] as usize] = i as u32;
            i = p;
        }
    }

    fn sift_down(&mut self, mut i: usize) {
        let n = self.heap.len();
        loop {
            let left = i * 2 + 1;
            if left >= n {
                break;
            }
            let right = left + 1;
            let mut child = left;
            if right < n && self.f[self.heap[right] as usize] < self.f[self.heap[left] as usize] {
                child = right;
            }
            if self.f[self.heap[i] as usize] <= self.f[self.heap[child] as usize] {
                break;
            }
            self.heap.swap(i, child);
            self.pos[self.heap[i] as usize] = i as u32;
            self.pos[self.heap[child] as usize] = child as u32;
            i = child;
        }
    }

    /// Insert a node, or lower its `f` and sift it up.  A node that is already
    /// closed is re-opened by the caller before this call.
    fn push_or_decrease(&mut self, node: u32, f: i32) {
        let ni = node as usize;
        if self.pos[ni] == u32::MAX {
            self.f[ni] = f;
            self.heap.push(node);
            let i = self.heap.len() - 1;
            self.pos[ni] = i as u32;
            self.sift_up(i);
        } else {
            self.f[ni] = f;
            let i = self.pos[ni] as usize;
            self.sift_up(i);
        }
    }

    fn pop_min(&mut self) -> Option<u32> {
        let last = self.heap.pop()?;
        if self.heap.is_empty() {
            self.pos[last as usize] = u32::MAX;
            return Some(last);
        }
        let root = self.heap[0];
        self.heap[0] = last;
        self.pos[last as usize] = 0;
        self.pos[root as usize] = u32::MAX;
        self.sift_down(0);
        Some(root)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn from_center(cx: i32, cy: i32, half_w: i32, half_h: i32) -> Self {
        Self::new(
            cx.saturating_sub(half_w),
            cy.saturating_sub(half_h),
            cx.saturating_add(half_w),
            cy.saturating_add(half_h),
        )
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.left < other.right
            && other.left < self.right
            && self.top < other.bottom
            && other.top < self.bottom
    }

    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
}

#[derive(Clone, Debug)]
pub struct PathRequest {
    pub start: (i32, i32),
    pub goal: (i32, i32),
    /// Half extent of the moving unit's footprint, in pixels.
    pub half_width: i32,
    pub half_height: i32,
    /// Flyers ignore terrain and obstacles.
    pub flying: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Path {
    /// Waypoints in pixels (center of each 8-pixel cell), excluding the
    /// starting position and ending at the requested goal cell.
    pub waypoints: Vec<(i32, i32)>,
    pub cost: i32,
}

#[derive(Clone, Debug, Default)]
pub struct Obstacles {
    rects: Vec<Rect>,
}

/// One pathfinding region from the retail builder.
#[derive(Clone, Debug, Default)]
pub struct Region {
    pub id: u32,
    /// Number of map tiles in the region (retail keeps this in the region
    /// header and the AI `region_size` command reads it).
    pub size: u32,
    pub min_tile: (i32, i32),
    pub max_tile: (i32, i32),
    /// 8-connected neighbours in recovered append order (not sorted).  The
    /// order is observable because the A* open-list tie-break follows
    /// discovery order.
    pub neighbours: Vec<u32>,
}

impl Region {
    /// Region centre in 8-pixel walk cells.  The retail builder stores the
    /// bounding-box midpoint (`0x7a303840`); tiles are 32 pixels, so the
    /// midpoint tile maps to walk cell `tile * 4 + 2`.
    pub fn centre_cell(&self) -> (i32, i32) {
        let cx = (self.min_tile.0 + self.max_tile.0) / 2;
        let cy = (self.min_tile.1 + self.max_tile.1) / 2;
        (cx * 4 + 2, cy * 4 + 2)
    }
}

/// Region graph built the way the retail builder does it.
///
/// `0x7a302c10` operates on a **tile grid** with a 256-cell row stride
/// (`grid[y*0x100 + x]`), not on the 8-pixel walk grid: the 16 minitile
/// flags of each tile are used only to decide whether two tiles touch.  A
/// region stores its bounding box and centre, and the adjacency lists are
/// built by `0x7a3046b0` / `0x7a303500`.  `0x7a303fd0` then merges regions
/// below an increasing size threshold into their smallest neighbour; the
/// same pass is implemented here (thresholds 6, 8, ..., 64).
/// The recovered `DAT_7ad46470` grid holds one byte per 8-pixel minitile:
/// `0x80` when the matching `vf4` walkable bit is clear and `0x00` otherwise.
/// `FUN_7a300c20` builds it from the tileset's 16 minitile flags.
///
/// `FUN_7a3046b0` classifies a neighbouring tile as connected using that
/// grid.  Orthogonal neighbours are accepted unconditionally (cases 0..=3
/// branch straight to the append code).  A diagonal neighbour (cases 4..=7)
/// needs the two 2x2 corner blocks that touch the shared corner — the current
/// tile's corner and the diagonal tile's opposite corner — and then at least
/// one of the two side tiles' corner blocks.  The offsets below are the exact
/// `pcVar1 + 0/1/0x400/...` reads from the decompile.
pub fn retail_tile_adjacent(terrain: &Terrain, x: i32, y: i32, dx: i32, dy: i32) -> bool {
    if dx == 0 && dy == 0 {
        return false;
    }
    if dx == 0 || dy == 0 {
        return true;
    }
    let open2 = |ox: i32, oy: i32| {
        (0..2).all(|oy2| (0..2).all(|ox2| terrain.walkable_cell(ox + ox2, oy + oy2)))
    };
    let bx = x * 4;
    let by = y * 4;
    let corner_x = if dx > 0 { 2 } else { 0 };
    let corner_y = if dy > 0 { 2 } else { 0 };
    let diag_x = if dx > 0 { 4 } else { -2 };
    let diag_y = if dy > 0 { 4 } else { -2 };
    let side_h = (bx + diag_x, by + corner_y);
    let side_v = (bx + corner_x, by + diag_y);
    open2(bx + corner_x, by + corner_y)
        && open2(bx + diag_x, by + diag_y)
        && (open2(side_h.0, side_h.1) || open2(side_v.0, side_v.1))
}

#[derive(Clone, Debug, Default)]
pub struct RegionMap {
    /// Tile dimensions (the retail 256-stride grid).
    pub width: i32,
    pub height: i32,
    labels: Vec<u32>,
    regions: Vec<Region>,
    count: u32,
}

impl RegionMap {
    pub fn from_terrain(terrain: &Terrain) -> Self {
        let width = terrain.width as i32;
        let height = terrain.height as i32;
        let n = (width * height) as usize;
        let mut labels = vec![u32::MAX; n];
        let mut stack: Vec<(i32, i32)> = Vec::new();

        let tile_walkable = |tx: i32, ty: i32| -> bool {
            if tx < 0 || ty < 0 || tx >= width || ty >= height {
                return false;
            }
            for my in 0..4 {
                for mx in 0..4 {
                    if terrain.walkable_cell(tx * 4 + mx, ty * 4 + my) {
                        return true;
                    }
                }
            }
            false
        };
        // Retail region creation (`FUN_00763840`, 32-bit dump, 2026-11-06):
        // unassigned walkable tiles carry a group label (>= 5000; here
        // walkability + ground height, INFERENCE); regions are seeded from
        // the largest rectangle of one group found among up to 25 candidates
        // (`FUN_00764d60`: at most 8 x 7 tiles, aspect <= 3), accepted once
        // its area reaches the previous region's area (56 at first), grown
        // one tile around (`FUN_00764360`, two passes), and every region
        // grows once more when the rectangles drop below 7 tiles.  The old
        // flood fill made each connected area one region (Boot Camp: 1).
        // Group labels sit far above any region id (retail uses >= 5000
        // and aborts above 5000 regions).
        const GROUP: u32 = 0x4000_0000;
        for (i, l) in labels.iter_mut().enumerate() {
            let (tx, ty) = (i as i32 % width, i as i32 / width);
            if tile_walkable(tx, ty) {
                *l = GROUP + terrain.tile_height_flags(tx, ty) as u32;
            }
        }
        let idx = |x: i32, y: i32| (y * width + x) as usize;
        let rect_at = |labels: &Vec<u32>, x: i32, y: i32, g: u32| -> (i32, i32, i32, i32) {
            let max_x = (x + 8).min(width);
            let max_y = (y + 7).min(height);
            let (mut x1, mut y1) = (x + 1, y + 1);
            let (mut right, mut down, mut corner_prev) = (true, true, true);
            while x1 < max_x && y1 < max_y {
                if right && (y..y1).any(|yy| labels[idx(x1, yy)] != g) {
                    right = false;
                }
                if down && (x..x1).any(|xx| labels[idx(xx, y1)] != g) {
                    down = false;
                }
                if !right && !down {
                    break;
                }
                let corner = labels[idx(x1, y1)] == g && corner_prev;
                if corner {
                    if down {
                        y1 += 1;
                    }
                    if right {
                        x1 += 1;
                    }
                } else if down {
                    y1 += 1;
                } else if right {
                    x1 += 1;
                }
                corner_prev = corner;
            }
            let (mut w, mut h) = (x1 - x, y1 - y);
            if h < w {
                w = w.min(h * 3);
            } else if w * 3 < h {
                h = w * 3;
            }
            (x, y, x + w, y + h)
        };
        let grow = |labels: &mut Vec<u32>, b: (i32, i32, i32, i32), id: u32, g: u32| {
            for _ in 0..2 {
                for y in b.1..b.3 {
                    for x in b.0..b.2 {
                        if labels[idx(x, y)] != g {
                            continue;
                        }
                        let near = (y > 0 && labels[idx(x, y - 1)] == id)
                            || (x > 0 && labels[idx(x - 1, y)] == id)
                            || (x + 1 < width && labels[idx(x + 1, y)] == id)
                            || (y + 1 < height && labels[idx(x, y + 1)] == id);
                        if near {
                            labels[idx(x, y)] = id;
                        }
                    }
                }
            }
        };
        let expand = |b: (i32, i32, i32, i32)| {
            (
                (b.0 - 1).max(0),
                (b.1 - 1).max(0),
                (b.2 + 1).min(width),
                (b.3 + 1).min(height),
            )
        };
        let mut boxes: Vec<((i32, i32, i32, i32), u32)> = Vec::new();
        let mut threshold = 0x38i32;
        let mut resume = 0usize;
        let mut grown_once = false;
        loop {
            let mut best: Option<((i32, i32, i32, i32), u32)> = None;
            let mut best_area = 0;
            let mut tries = 0;
            let mut k = 0usize;
            while k < n {
                let i = (resume + k) % n;
                let g = labels[i];
                if g >= GROUP && g != u32::MAX {
                    let (x, y) = (i as i32 % width, i as i32 / width);
                    let r = rect_at(&labels, x, y, g);
                    tries += 1;
                    let area = (r.2 - r.0) * (r.3 - r.1);
                    if area > best_area {
                        best_area = area;
                        best = Some((r, g));
                    }
                    if best_area >= threshold || tries > 25 {
                        break;
                    }
                    k += (r.2 - x).max(1) as usize;
                    continue;
                }
                k += 1;
            }
            let Some((r, g)) = best else {
                break;
            };
            threshold = best_area;
            let id = boxes.len() as u32;
            for y in r.1..r.3 {
                for x in r.0..r.2 {
                    labels[idx(x, y)] = id;
                }
            }
            let b = expand(r);
            grow(&mut labels, b, id, g);
            boxes.push((b, g));
            resume = idx(r.2.min(width - 1), r.1);
            if threshold < 7 && !grown_once {
                grown_once = true;
                for (rid, (bb, gg)) in boxes.iter_mut().enumerate() {
                    *bb = expand(*bb);
                    grow(&mut labels, *bb, rid as u32, *gg);
                }
            }
        }
        for (i, l) in labels.iter_mut().enumerate() {
            let _ = i;
            if *l >= GROUP && (*l as usize) >= boxes.len() {
                *l = u32::MAX;
            }
        }
        let mut regions: Vec<Region> = (0..boxes.len() as u32)
            .map(|id| Region {
                id,
                size: 0,
                min_tile: (i32::MAX, i32::MAX),
                max_tile: (i32::MIN, i32::MIN),
                neighbours: Vec::new(),
            })
            .collect();
        for (i, &l) in labels.iter().enumerate() {
            if let Some(r) = regions.get_mut(l as usize) {
                let (x, y) = (i as i32 % width, i as i32 / width);
                r.size += 1;
                r.min_tile = (r.min_tile.0.min(x), r.min_tile.1.min(y));
                r.max_tile = (r.max_tile.0.max(x), r.max_tile.1.max(y));
            }
        }
        let _ = &mut stack;
        let count = regions.len() as u32;
        let mut map = Self {
            width,
            height,
            labels,
            regions,
            count,
        };
        map.rebuild_adjacency();
        map.merge_small_regions();
        map
    }

    /// Reciprocal adjacency from the current labels.
    fn rebuild_adjacency(&mut self) {
        for r in &mut self.regions {
            r.neighbours.clear();
        }
        let w = self.width;
        for y in 0..self.height {
            for x in 0..self.width {
                let a = self.labels[(y * w + x) as usize];
                if a == u32::MAX {
                    continue;
                }
                for (dx, dy) in [(1, 0), (0, 1), (1, 1), (-1, 1)] {
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx < 0 || ny < 0 || nx >= self.width || ny >= self.height {
                        continue;
                    }
                    let b = self.labels[(ny * w + nx) as usize];
                    if b == u32::MAX || a == b {
                        continue;
                    }
                    if !self.regions[a as usize].neighbours.contains(&b) {
                        self.regions[a as usize].neighbours.push(b);
                    }
                    if !self.regions[b as usize].neighbours.contains(&a) {
                        self.regions[b as usize].neighbours.push(a);
                    }
                }
            }
        }
        for r in &mut self.regions {
            r.neighbours.dedup();
        }
    }

    /// The `0x7a303fd0` merge loop: regions below a growing threshold are
    /// folded into the smallest adjacent region.  A merge rewrites the tile
    /// labels and the region's bounding box, exactly like the retail routine
    /// (`psVar9[2] += psVar7[2]`, bbox union, source marked `0x1fff`).
    ///
    /// The ordinary case is **one pass at size < 6**.  The threshold only
    /// grows (8, 10, ...) while the live region count is still at least
    /// `0x9c4` (2500), the pathological-map guard at the end of the routine.
    fn merge_small_regions(&mut self) {
        let mut threshold = 6u32;
        loop {
            let mut changed = false;
            let mut remap: Vec<u32> = (0..self.regions.len() as u32).collect();
            let mut merged_into: Vec<Option<u32>> = vec![None; self.regions.len()];
            for id in 0..self.regions.len() {
                if merged_into[id].is_some() || self.regions[id].size == 0 {
                    continue;
                }
                if self.regions[id].size >= threshold {
                    continue;
                }
                let mut best: Option<(u32, u32)> = None;
                for &nb in &self.regions[id].neighbours {
                    let target = remap[nb as usize];
                    if target == id as u32 {
                        continue;
                    }
                    let size = self.regions[target as usize].size;
                    if best.is_none_or(|(bs, _)| size < bs) {
                        best = Some((size, target));
                    }
                }
                if let Some((_, target)) = best {
                    let src = id as u32;
                    let dst = target;
                    self.regions[dst as usize].size += self.regions[src as usize].size;
                    self.regions[dst as usize].min_tile.0 = self.regions[dst as usize]
                        .min_tile
                        .0
                        .min(self.regions[src as usize].min_tile.0);
                    self.regions[dst as usize].min_tile.1 = self.regions[dst as usize]
                        .min_tile
                        .1
                        .min(self.regions[src as usize].min_tile.1);
                    self.regions[dst as usize].max_tile.0 = self.regions[dst as usize]
                        .max_tile
                        .0
                        .max(self.regions[src as usize].max_tile.0);
                    self.regions[dst as usize].max_tile.1 = self.regions[dst as usize]
                        .max_tile
                        .1
                        .max(self.regions[src as usize].max_tile.1);
                    self.regions[src as usize].size = 0;
                    for (i, r) in remap.iter_mut().enumerate() {
                        if *r == src {
                            *r = dst;
                        }
                        if merged_into[i] == Some(src) {
                            merged_into[i] = Some(dst);
                        }
                    }
                    merged_into[src as usize] = Some(dst);
                    changed = true;
                }
            }
            if changed {
                for label in &mut self.labels {
                    if *label != u32::MAX {
                        *label = remap[*label as usize];
                    }
                }
                self.rebuild_adjacency();
            }
            let live = self.regions.iter().filter(|r| r.size > 0).count();
            if live < 2500 || threshold >= 64 {
                break;
            }
            if !changed {
                threshold += 2;
            }
        }
        // Compact ids so `count()` and the AI `region_size` lookups stay
        // dense.  Keep dead entries out of the public region list.
        let mut compact = vec![u32::MAX; self.regions.len()];
        let mut next = 0u32;
        for (i, r) in self.regions.iter().enumerate() {
            if r.size > 0 {
                compact[i] = next;
                next += 1;
            }
        }
        let mut out = Vec::with_capacity(next as usize);
        for (i, mut r) in self.regions.iter().cloned().enumerate() {
            if r.size == 0 {
                continue;
            }
            r.id = compact[i];
            r.neighbours = r
                .neighbours
                .iter()
                .filter_map(|&n| {
                    let c = compact[n as usize];
                    (c != u32::MAX).then_some(c)
                })
                .collect();
            r.neighbours.dedup();
            out.push(r);
        }
        for label in &mut self.labels {
            if *label != u32::MAX {
                *label = compact[*label as usize];
            }
        }
        self.regions = out;
        self.count = next;
    }

    pub fn count(&self) -> u32 {
        self.count
    }

    pub fn region(&self, id: u32) -> Option<&Region> {
        self.regions.get(id as usize)
    }

    pub fn regions(&self) -> &[Region] {
        &self.regions
    }

    /// Region id for an 8-pixel walk cell.  The tile grid is four times
    /// coarser, so the cell maps to `cell / 4`.
    pub fn region_at_cell(&self, cell: (i32, i32)) -> Option<u32> {
        self.region_at_tile((cell.0.div_euclid(4), cell.1.div_euclid(4)))
    }

    pub fn region_at_tile(&self, tile: (i32, i32)) -> Option<u32> {
        if tile.0 < 0 || tile.1 < 0 || tile.0 >= self.width || tile.1 >= self.height {
            return None;
        }
        let v = self.labels[(tile.1 * self.width + tile.0) as usize];
        if v == u32::MAX { None } else { Some(v) }
    }

    pub fn connected(&self, a: (i32, i32), b: (i32, i32)) -> bool {
        self.region_path(a, b).is_some()
    }

    /// Exact retail region path: A* over the recovered region graph.
    ///
    /// `FUN_7a305590` searches the region adjacency lists directly (one node
    /// per region) and compares the region-centre distance from
    /// `FUN_7a1a4060`.  The open list is `RetailOpen`, so ties are resolved by
    /// discovery order and the heap rules recovered from `FUN_7a305e20` /
    /// `FUN_7a305da0`, not by a secondary `g`/coordinate key.  `FUN_7a305590`
    /// also stops after 125 open-list entries (`0x7d`); the same bound is
    /// applied here.
    pub fn region_path(&self, a: (i32, i32), b: (i32, i32)) -> Option<Vec<u32>> {
        let start = self.region_at_cell(a)?;
        let goal = self.region_at_cell(b)?;
        if start == goal {
            return Some(vec![start]);
        }
        let n = self.regions.len();
        let mut g = vec![i32::MAX; n];
        let mut parent = vec![u32::MAX; n];
        let mut closed = vec![false; n];
        let mut open = RetailOpen::new(n);
        let goal_centre = self.regions[goal as usize].centre_cell();
        g[start as usize] = 0;
        let h = retail_distance(self.regions[start as usize].centre_cell(), goal_centre);
        open.push_or_decrease(start, h);

        let mut found = false;
        let mut pops = 0usize;
        while let Some(cur) = open.pop_min() {
            if cur == goal {
                found = true;
                break;
            }
            closed[cur as usize] = true;
            pops += 1;
            if pops >= 125 {
                break;
            }
            let cur_centre = self.regions[cur as usize].centre_cell();
            // `neighbours` preserves the recovered append order.  It is the
            // order in which `FUN_7a3046b0` walks `local_18` (left, up, right,
            // down, up-left, up-right, down-left, down-right) and therefore
            // the order the retail heap sees new nodes.
            for &nb in &self.regions[cur as usize].neighbours {
                if closed[nb as usize] {
                    continue;
                }
                let step = retail_distance(cur_centre, self.regions[nb as usize].centre_cell());
                let tentative = g[cur as usize].saturating_add(step);
                if tentative < g[nb as usize] {
                    g[nb as usize] = tentative;
                    parent[nb as usize] = cur;
                    let h = retail_distance(self.regions[nb as usize].centre_cell(), goal_centre);
                    open.push_or_decrease(nb, tentative.saturating_add(h));
                }
            }
        }
        if !found {
            return None;
        }
        let mut path = vec![goal];
        let mut cur = goal;
        while cur != start {
            cur = parent[cur as usize];
            if cur == u32::MAX {
                return None;
            }
            path.push(cur);
        }
        path.reverse();
        Some(path)
    }
}

impl Obstacles {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.rects.clear();
    }

    pub fn push(&mut self, rect: Rect) {
        self.rects.push(rect);
    }

    pub fn blocks_point(&self, x: i32, y: i32) -> bool {
        self.rects.iter().any(|r| r.contains_point(x, y))
    }

    pub fn blocks_rect(&self, rect: &Rect) -> bool {
        self.rects.iter().any(|r| r.intersects(rect))
    }

    pub fn len(&self) -> usize {
        self.rects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rects.is_empty()
    }
}

fn cell_of(px: i32) -> i32 {
    px.div_euclid(WALK_TILE_PX)
}

fn cell_center(c: i32) -> i32 {
    c.saturating_mul(WALK_TILE_PX)
        .saturating_add(WALK_TILE_PX / 2)
}

fn octile(a: (i32, i32), b: (i32, i32)) -> i32 {
    let dx = (a.0 - b.0).abs();
    let dy = (a.1 - b.1).abs();
    let (hi, lo) = if dx > dy { (dx, dy) } else { (dy, dx) };
    STRAIGHT_COST * (hi - lo) + DIAGONAL_COST * lo
}

fn cell_free(terrain: &Terrain, obstacles: &Obstacles, wx: i32, wy: i32, hw: i32, hh: i32) -> bool {
    let cx = cell_center(wx);
    let cy = cell_center(wy);
    let rect = Rect::from_center(cx, cy, hw, hh);
    if obstacles.blocks_rect(&rect) {
        return false;
    }
    let x0 = cell_of(rect.left);
    let y0 = cell_of(rect.top);
    let x1 = cell_of(rect.right - 1);
    let y1 = cell_of(rect.bottom - 1);
    for y in y0..=y1 {
        for x in x0..=x1 {
            if !terrain.walkable_cell(x, y) {
                return false;
            }
        }
    }
    true
}

/// Find the nearest free cell to `start`, spiralling outwards.  Mirrors the
/// original's behaviour when a unit's requested position is blocked.
fn nearest_free(
    terrain: &Terrain,
    obstacles: &Obstacles,
    start: (i32, i32),
    hw: i32,
    hh: i32,
    limit: i32,
) -> Option<(i32, i32)> {
    if cell_free(terrain, obstacles, start.0, start.1, hw, hh) {
        return Some(start);
    }
    for radius in 1..=limit {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs() != radius && dy.abs() != radius {
                    continue;
                }
                let p = (start.0 + dx, start.1 + dy);
                if cell_free(terrain, obstacles, p.0, p.1, hw, hh) {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// Line-of-sight smoothing using a dense walk-cell trace.  Returns true when
/// every cell centre along the segment is free for the footprint.
fn line_clear(
    terrain: &Terrain,
    obstacles: &Obstacles,
    a: (i32, i32),
    b: (i32, i32),
    hw: i32,
    hh: i32,
) -> bool {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let steps = dx.abs().max(dy.abs());
    if steps == 0 {
        return true;
    }
    for i in 0..=steps {
        let x = a.0 + dx * i / steps;
        let y = a.1 + dy * i / steps;
        if !cell_free(terrain, obstacles, x, y, hw, hh) {
            return false;
        }
    }
    true
}

fn simplify(
    terrain: &Terrain,
    obstacles: &Obstacles,
    cells: &[(i32, i32)],
    hw: i32,
    hh: i32,
) -> Vec<(i32, i32)> {
    if cells.len() <= 2 {
        return cells
            .iter()
            .map(|&(x, y)| (cell_center(x), cell_center(y)))
            .collect();
    }
    let mut out = Vec::new();
    let mut anchor = 0usize;
    while anchor + 1 < cells.len() {
        let mut far = cells.len() - 1;
        while far > anchor + 1 && !line_clear(terrain, obstacles, cells[anchor], cells[far], hw, hh)
        {
            far -= 1;
        }
        out.push((cell_center(cells[far].0), cell_center(cells[far].1)));
        anchor = far;
    }
    out
}

/// Low-level octile A* between two free walk cells.  Returns the full cell
/// chain (including both endpoints) and its cost.
///
/// The frontier uses the recovered `RetailOpen` heap (f only, newest-first on
/// ties) and the recovered neighbour visit order.  The heuristic remains the
/// exact `10*straight + 4*diagonal` octile bound; only the tie-break policy is
/// the one recovered from `0x7a305590`.
fn astar_cells(
    terrain: &Terrain,
    obstacles: &Obstacles,
    start: (i32, i32),
    goal: (i32, i32),
    hw: i32,
    hh: i32,
) -> Option<(Vec<(i32, i32)>, i32)> {
    if start == goal {
        return Some((vec![start], 0));
    }
    let w = terrain.walk_width as i32;
    let h = terrain.walk_height as i32;
    let n = (w * h) as usize;
    let mut g = vec![i32::MAX; n];
    let mut parent = vec![u32::MAX; n];
    let mut closed = vec![false; n];
    let index = |c: (i32, i32)| (c.1 * w + c.0) as usize;

    let mut open = RetailOpen::new(n);
    let si = index(start);
    g[si] = 0;
    open.push_or_decrease(si as u32, octile(start, goal));
    let mut expansions = 0usize;
    let mut found = false;
    // Recovered `local_18` order from `FUN_7a304c10`.
    let dirs = [
        (-1, 0, STRAIGHT_COST),
        (0, -1, STRAIGHT_COST),
        (1, 0, STRAIGHT_COST),
        (0, 1, STRAIGHT_COST),
        (-1, -1, DIAGONAL_COST),
        (1, -1, DIAGONAL_COST),
        (-1, 1, DIAGONAL_COST),
        (1, 1, DIAGONAL_COST),
    ];

    while let Some(ci_u32) = open.pop_min() {
        let ci = ci_u32 as usize;
        if closed[ci] {
            continue;
        }
        closed[ci] = true;
        let cur = ((ci as i32) % w, (ci as i32) / w);
        if cur == goal {
            found = true;
            break;
        }
        expansions += 1;
        if expansions > 120_000 {
            break;
        }
        for &(dx, dy, step) in &dirs {
            let nx = cur.0 + dx;
            let ny = cur.1 + dy;
            if nx < 0 || ny < 0 || nx >= w || ny >= h {
                continue;
            }
            if !cell_free(terrain, obstacles, nx, ny, hw, hh) {
                continue;
            }
            // The recovered A* forbids cutting a diagonal corner.
            if dx != 0
                && dy != 0
                && (!cell_free(terrain, obstacles, cur.0 + dx, cur.1, hw, hh)
                    || !cell_free(terrain, obstacles, cur.0, cur.1 + dy, hw, hh))
            {
                continue;
            }
            let ni = index((nx, ny));
            let ng = g[ci] + step;
            if ng < g[ni] {
                g[ni] = ng;
                parent[ni] = ci as u32;
                if closed[ni] {
                    // Recovered A* re-opens an improved closed node.
                    closed[ni] = false;
                }
                let f = ng + octile((nx, ny), goal);
                open.push_or_decrease(ni as u32, f);
            }
        }
    }

    if !found {
        return None;
    }
    let mut cells = Vec::new();
    let mut cur = goal;
    loop {
        cells.push(cur);
        let ci = index(cur);
        if cur == start {
            break;
        }
        let p = parent[ci];
        if p == u32::MAX {
            return None;
        }
        cur = ((p as i32) % w, (p as i32) / w);
    }
    cells.reverse();
    let cost = g[index(goal)];
    Some((cells, cost))
}

/// Compute a path.  Returns `None` when no route exists within the expansion
/// budget.  Flyers always receive a direct two-point path.
pub fn find_path(terrain: &Terrain, obstacles: &Obstacles, req: &PathRequest) -> Option<Path> {
    find_path_with_regions(terrain, obstacles, req, None)
}

pub fn find_path_with_regions(
    terrain: &Terrain,
    obstacles: &Obstacles,
    req: &PathRequest,
    regions: Option<&RegionMap>,
) -> Option<Path> {
    let goal = (cell_of(req.goal.0), cell_of(req.goal.1));
    if req.flying {
        let waypoints = if req.goal == req.start {
            Vec::new()
        } else {
            vec![req.goal]
        };
        let dx = req.goal.0 - req.start.0;
        let dy = req.goal.1 - req.start.1;
        let cost = ((dx * dx + dy * dy) as f64).sqrt() as i32;
        return Some(Path { waypoints, cost });
    }

    let start = (cell_of(req.start.0), cell_of(req.start.1));
    let start = nearest_free(
        terrain,
        obstacles,
        start,
        req.half_width,
        req.half_height,
        8,
    )?;
    let goal = nearest_free(
        terrain,
        obstacles,
        goal,
        req.half_width,
        req.half_height,
        12,
    )?;
    if let Some(regions) = regions
        && !regions.connected(start, goal)
    {
        return None;
    }
    if start == goal {
        return Some(Path {
            waypoints: if req.goal == req.start {
                Vec::new()
            } else {
                vec![(cell_center(goal.0), cell_center(goal.1))]
            },
            cost: 0,
        });
    }

    // Two-level pathfinder: first the recovered region graph, then the local
    // octile grid search.  The retail builder stores a bounding-box centre
    // for every region and links adjacent regions; long routes are therefore
    // routed through the intermediate region centres.  Short routes use the
    // grid search directly (the region route would only add a detour).
    let region_route = regions.and_then(|r| r.region_path(start, goal));
    if regions.is_some() && region_route.is_none() {
        return None;
    }
    let (cells, mut cost) = if let Some(route) = region_route {
        let mut combined: Option<(Vec<(i32, i32)>, i32)> = None;
        // Keep the coarse route bounded: running a full A* per region centre
        // is only worthwhile for a handful of regions.  Very long region
        // chains fall back to one grid search.
        if route.len() > 2 && route.len() <= 8 && octile(start, goal) > 200 {
            let mut points = vec![start];
            for rid in &route[1..route.len() - 1] {
                if let Some(r) = regions.and_then(|m| m.region(*rid)) {
                    let c = r.centre_cell();
                    let c = nearest_free(terrain, obstacles, c, req.half_width, req.half_height, 6)
                        .unwrap_or(c);
                    if points.last().copied() != Some(c) {
                        points.push(c);
                    }
                }
            }
            if *points.last().unwrap() != goal {
                points.push(goal);
            }
            let mut all: Vec<(i32, i32)> = Vec::new();
            let mut total = 0;
            let mut ok = true;
            for pair in points.windows(2) {
                if pair[0] == pair[1] {
                    continue;
                }
                match astar_cells(
                    terrain,
                    obstacles,
                    pair[0],
                    pair[1],
                    req.half_width,
                    req.half_height,
                ) {
                    Some((mut cs, c)) => {
                        if all.is_empty() {
                            all.append(&mut cs);
                        } else {
                            all.extend_from_slice(&cs[1..]);
                        }
                        total += c;
                    }
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                combined = Some((all, total));
            }
        }
        match combined {
            Some(v) => v,
            None => astar_cells(
                terrain,
                obstacles,
                start,
                goal,
                req.half_width,
                req.half_height,
            )?,
        }
    } else {
        astar_cells(
            terrain,
            obstacles,
            start,
            goal,
            req.half_width,
            req.half_height,
        )?
    };
    if cells.len() < 2 {
        return Some(Path {
            waypoints: Vec::new(),
            cost,
        });
    }
    let mut waypoints = simplify(
        terrain,
        obstacles,
        &cells[1..],
        req.half_width,
        req.half_height,
    );
    if goal == (cell_of(req.goal.0), cell_of(req.goal.1))
        && let Some(last) = waypoints.last_mut()
    {
        *last = req.goal;
    }
    if waypoints.is_empty() {
        waypoints.push(req.goal);
    }
    cost = cost.max(0);
    Some(Path { waypoints, cost })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_ground_takes_a_straight_line() {
        let terrain = Terrain::open(32, 32);
        let path = find_path(
            &terrain,
            &Obstacles::new(),
            &PathRequest {
                start: (16, 16),
                goal: (240, 16),
                half_width: 6,
                half_height: 6,
                flying: false,
            },
        )
        .unwrap();
        assert!(!path.waypoints.is_empty());
        assert_eq!(*path.waypoints.last().unwrap(), (240, 16));
        assert!(path.cost > 0);
    }

    #[test]
    fn wall_forces_a_detour() {
        let terrain = Terrain::open(32, 32);
        let mut obstacles = Obstacles::new();
        // A wall from y=0..20 tiles at x=16..17 tiles.
        obstacles.push(Rect::new(16 * 32, 0, 17 * 32, 20 * 32));
        let path = find_path(
            &terrain,
            &obstacles,
            &PathRequest {
                start: (8 * 32, 8 * 32),
                goal: (24 * 32, 8 * 32),
                half_width: 8,
                half_height: 8,
                flying: false,
            },
        )
        .unwrap();
        // The route must pass below the wall.
        assert!(path.waypoints.iter().any(|&(_, y)| y >= 20 * 32));
    }

    #[test]
    fn no_route_when_fully_walled() {
        let terrain = Terrain::open(16, 16);
        let mut obstacles = Obstacles::new();
        obstacles.push(Rect::new(8 * 32, 0, 9 * 32, 16 * 32));
        let path = find_path(
            &terrain,
            &obstacles,
            &PathRequest {
                start: (4 * 32, 8 * 32),
                goal: (12 * 32, 8 * 32),
                half_width: 8,
                half_height: 8,
                flying: false,
            },
        );
        assert!(path.is_none());
    }

    #[test]
    fn region_graph_uses_the_tile_grid_and_centres() {
        let mut terrain = Terrain::open(16, 16);
        let regions = RegionMap::from_terrain(&terrain);
        assert_eq!(regions.width, 16);
        assert_eq!(regions.height, 16);
        // `FUN_00763840`: regions are seeded from rectangles of at most 8 x 7
        // tiles and grown by one tile, so open ground is split (an open
        // 16 x 16 map is not one region).
        assert!(regions.count() >= 4, "{} regions", regions.count());
        let total: u32 = (0..regions.count())
            .filter_map(|i| regions.region(i))
            .map(|r| r.size)
            .sum();
        assert_eq!(total, 256, "every tile in exactly one region");
        let r = regions.region(0).unwrap();
        assert_eq!(r.min_tile, (0, 0));
        assert!(r.max_tile.0 - r.min_tile.0 < 10 && r.max_tile.1 - r.min_tile.1 < 9);
        assert!(regions.connected((0, 0), (60, 60)));

        // One 32-pixel (four walk-cell) column splits the map in two.
        for y in 0..64 {
            for x in 32..36 {
                terrain.set_walkable_cell(x, y, false);
            }
        }
        let regions = RegionMap::from_terrain(&terrain);
        assert!(!regions.connected((8, 8), (56, 56)));
        assert!(regions.region_path((8, 8), (56, 56)).is_none());

        // Open two minitile rows and the two regions join again.
        for x in 32..36 {
            terrain.set_walkable_cell(x, 30, true);
            terrain.set_walkable_cell(x, 31, true);
        }
        let regions = RegionMap::from_terrain(&terrain);
        assert!(regions.connected((8, 8), (56, 56)));
        assert!(regions.region_path((8, 8), (56, 56)).is_some());
    }

    #[test]
    fn recovered_edge_masks_accept_orthogonal_neighbours_unconditionally() {
        // `FUN_7a3046b0` accepts cases 0..=3 (the four orthogonal
        // neighbours) before consulting DAT_7ad46470.  Even two tiles whose
        // only walkable minitiles are on opposite corners connect.
        let mut terrain = Terrain::open(2, 1);
        for y in 0..4 {
            for x in 0..8 {
                terrain.set_walkable_cell(x, y, false);
            }
        }
        terrain.set_walkable_cell(0, 0, true);
        terrain.set_walkable_cell(7, 3, true);
        assert!(retail_tile_adjacent(&terrain, 0, 0, 1, 0));
        assert!(retail_tile_adjacent(&terrain, 1, 0, -1, 0));
        assert!(retail_tile_adjacent(&terrain, 0, 0, 0, 1));
    }

    #[test]
    fn recovered_edge_masks_require_corner_and_side_blocks_for_diagonals() {
        // Four tiles around a shared corner, all otherwise blocked.  Open
        // the 2x2 blocks that touch the corner in all four tiles.
        let mut terrain = Terrain::open(2, 2);
        for y in 0..8 {
            for x in 0..8 {
                terrain.set_walkable_cell(x, y, false);
            }
        }
        let open = |t: &mut Terrain, x0: i32, y0: i32| {
            for y in y0..y0 + 2 {
                for x in x0..x0 + 2 {
                    t.set_walkable_cell(x, y, true);
                }
            }
        };
        open(&mut terrain, 2, 2); // tile (0,0) bottom-right
        open(&mut terrain, 4, 4); // tile (1,1) top-left
        open(&mut terrain, 2, 4); // tile (0,1) top-right
        open(&mut terrain, 4, 2); // tile (1,0) bottom-left
        assert!(retail_tile_adjacent(&terrain, 1, 1, -1, -1));
        assert!(retail_tile_adjacent(&terrain, 0, 0, 1, 1));

        // Removing the whole horizontal side tile's corner still leaves the
        // vertical side route, which the recovered predicate accepts.
        for y in 4..6 {
            for x in 2..4 {
                terrain.set_walkable_cell(x, y, false);
            }
        }
        assert!(retail_tile_adjacent(&terrain, 1, 1, -1, -1));

        // With both side corners blocked the diagonal is rejected.
        for y in 2..4 {
            for x in 4..6 {
                terrain.set_walkable_cell(x, y, false);
            }
        }
        assert!(!retail_tile_adjacent(&terrain, 1, 1, -1, -1));

        // A missing cell in the diagonal tile's own corner also rejects it.
        open(&mut terrain, 2, 4);
        open(&mut terrain, 4, 2);
        terrain.set_walkable_cell(2, 2, false);
        assert!(!retail_tile_adjacent(&terrain, 1, 1, -1, -1));
    }

    #[test]
    fn flyers_ignore_obstacles() {
        let terrain = Terrain::open(16, 16);
        let mut obstacles = Obstacles::new();
        obstacles.push(Rect::new(0, 0, 16 * 32, 16 * 32));
        let path = find_path(
            &terrain,
            &obstacles,
            &PathRequest {
                start: (0, 0),
                goal: (400, 400),
                half_width: 8,
                half_height: 8,
                flying: true,
            },
        )
        .unwrap();
        assert_eq!(path.waypoints, vec![(400, 400)]);
    }

    #[test]
    fn retail_distance_matches_the_recovered_integer_formula() {
        // Values from the decompiled `FUN_7a1a4060` expression.
        for (delta, expected) in [
            ((0, 0), 0),
            ((1, 0), 1),
            ((10, 0), 10),
            ((10, 10), 13),
            ((5, 10), 11),
            ((10, 5), 11),
            ((3, 7), 8),
            ((100, 100), 131),
            ((100, 50), 111),
            ((255, 255), 334),
        ] {
            assert_eq!(retail_distance((0, 0), delta), expected, "delta {delta:?}");
            assert_eq!(
                retail_distance(delta, (0, 0)),
                expected,
                "reverse {delta:?}"
            );
        }
    }

    #[test]
    fn retail_open_list_orders_ties_by_the_recovered_heap_rules() {
        // `FUN_7a305e20` bubbles an equal-`f` entry above its parent, and
        // `FUN_7a305da0` keeps a parent that is <= its chosen child.  This
        // sequence is the observable consequence, including the fact that the
        // last-inserted equal-`f` entry (5) pops first.
        let mut open = RetailOpen::new(8);
        for (node, f) in [(0, 5), (1, 3), (2, 3), (3, 4), (4, 3), (5, 3)] {
            open.push_or_decrease(node, f);
        }
        let mut order = Vec::new();
        while let Some(node) = open.pop_min() {
            order.push(node);
        }
        assert_eq!(order, vec![5, 1, 2, 4, 3, 0]);
    }

    #[test]
    fn equal_cost_routes_use_the_recovered_neighbour_order() {
        // On an empty grid every monotone route has the same cost.  The
        // recovered visit order and heap tie-break must still produce a
        // stable, valid chain (the old implementation ordered ties by
        // `(f, g, x, y)`).
        let terrain = Terrain::open(32, 32);
        let obstacles = Obstacles::new();
        let (cells, _) = astar_cells(&terrain, &obstacles, (10, 10), (18, 18), 4, 4).unwrap();
        assert_eq!(cells.first(), Some(&(10, 10)));
        assert_eq!(cells.last(), Some(&(18, 18)));
        let step = (cells[1].0 - cells[0].0, cells[1].1 - cells[0].1);
        assert!(
            step.0.abs() <= 1 && step.1.abs() <= 1 && step != (0, 0),
            "first step must be an adjacent recovered cell, got {step:?}"
        );
        let (again, _) = astar_cells(&terrain, &obstacles, (10, 10), (18, 18), 4, 4).unwrap();
        assert_eq!(cells, again, "tie-breaking must be deterministic");
    }
}

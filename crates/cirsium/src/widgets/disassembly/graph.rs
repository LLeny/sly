use rust_sugiyama::configure::Config;

use super::{DisasmLine, FlowKind};

pub(super) const BLOCK_HEADER_H: f32 = 20.;
pub(super) const BLOCK_LINE_H: f32 = 16.;
const BLOCK_PAD_X: f32 = 8.;
pub(super) const BLOCK_GUTTER_W: f32 = 20.;
pub(super) const BLOCK_PAD_Y: f32 = 5.;
const CHAR_W: f32 = 7.;
const MIN_BLOCK_W: f32 = 96.;
const VERTEX_SPACING: f64 = 38.;
const COMPONENT_GAP: f32 = 56.;
const BACK_LANE_GAP: f32 = 22.;
const PORT_STUB: f32 = 10.;
const CHANNEL_STEP: f32 = 8.;
const LANE_MARGIN: f32 = 6.;
const MARGIN: f32 = 24.;
const NOTE_RESERVE: f32 = 150.;
const MIN_BLOCK_CHARS: usize = 8;
const BLOCK_BORDER_H: f32 = 2.;
const RANK_EPS: f32 = 0.5;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum EdgeKind {
    Taken,
    FallThrough,
    Normal,
}

pub(super) struct GraphBlock {
    pub start: usize,
    pub end: usize,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

pub(super) struct GraphEdge {
    pub kind: EdgeKind,
    pub to: usize,
    pub points: Vec<(f32, f32)>,
}

pub(super) struct GraphLayout {
    pub blocks: Vec<GraphBlock>,
    pub edges: Vec<GraphEdge>,
}

const EDGE_HIT_DIST: f32 = 6.;

pub(super) fn edge_at(edges: &[GraphEdge], x: f32, y: f32) -> Option<usize> {
    let hit_sq = EDGE_HIT_DIST * EDGE_HIT_DIST;
    let mut best: Option<(usize, f32)> = None;
    for (ei, edge) in edges.iter().enumerate() {
        for seg in edge.points.windows(2) {
            let d = self::point_segment_dist_sq(x, y, seg[0], seg[1]);
            if d <= hit_sq && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((ei, d));
            }
        }
    }
    best.map(|(ei, _)| ei)
}

fn point_segment_dist_sq(px: f32, py: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let (apx, apy) = (px - a.0, py - a.1);
    let len_sq = abx * abx + aby * aby;
    let t = if len_sq <= f32::EPSILON {
        0.
    } else {
        ((apx * abx + apy * aby) / len_sq).clamp(0., 1.)
    };
    let (cx, cy) = (a.0 + abx * t, a.1 + aby * t);
    (px - cx).powi(2) + (py - cy).powi(2)
}

pub(super) fn build(lines: &[DisasmLine]) -> GraphLayout {
    let n = lines.len();
    if n == 0 {
        return GraphLayout {
            blocks: Vec::new(),
            edges: Vec::new(),
        };
    }

    let addr_index = super::addr_index(lines);

    let mut is_leader = vec![false; n];
    is_leader[0] = true;

    for (i, line) in lines.iter().enumerate() {
        if matches!(
            line.flow,
            FlowKind::CondBranch | FlowKind::Branch | FlowKind::Jump | FlowKind::Return
        ) && i + 1 < n
        {
            is_leader[i + 1] = true;
        }
        if matches!(
            line.flow,
            FlowKind::CondBranch | FlowKind::Branch | FlowKind::Jump
        ) && let Some(target) = line.target
            && let Some(&j) = addr_index.get(&target)
        {
            is_leader[j] = true;
        }
    }

    let leaders: Vec<usize> = (0..n).filter(|&i| is_leader[i]).collect();

    let mut block_at_line = vec![0usize; n];
    let mut blocks: Vec<GraphBlock> = Vec::new();

    for (idx, &start) in leaders.iter().enumerate() {
        let end = leaders.get(idx + 1).copied().unwrap_or(n);
        blocks.push(self::block_geometry(lines, start, end));
    }

    for (bi, block) in blocks.iter().enumerate() {
        for slot in block_at_line.iter_mut().take(block.end).skip(block.start) {
            *slot = bi;
        }
    }

    let target_block = |line: &DisasmLine| -> Option<usize> {
        line.target
            .and_then(|t| addr_index.get(&t))
            .map(|&j| block_at_line[j])
    };

    let mut edges: Vec<(usize, usize, EdgeKind)> = Vec::new();
    for (bi, block) in blocks.iter().enumerate() {
        let last = &lines[block.end - 1];

        match last.flow {
            FlowKind::Return => {}
            FlowKind::CondBranch => {
                if let Some(target) = target_block(last) {
                    edges.push((bi, target, EdgeKind::Taken));
                }
                if block.end < n {
                    edges.push((bi, block_at_line[block.end], EdgeKind::FallThrough));
                }
            }
            FlowKind::Branch | FlowKind::Jump => {
                if let Some(target) = target_block(last) {
                    edges.push((bi, target, EdgeKind::Normal));
                }
            }
            FlowKind::None | FlowKind::Call => {
                if block.end < n {
                    edges.push((bi, block_at_line[block.end], EdgeKind::Normal));
                }
            }
        }
    }

    let (layer, max_width) = self::layout_blocks(&mut blocks, &edges);

    let edges = self::route_edges(&edges, &blocks, &layer, max_width);

    for block in &mut blocks {
        block.x += MARGIN;
        block.y += MARGIN;
    }

    let edges = edges
        .into_iter()
        .map(|mut e| {
            for p in &mut e.points {
                p.0 += MARGIN;
                p.1 += MARGIN;
            }
            e
        })
        .collect();

    GraphLayout { blocks, edges }
}

fn block_geometry(lines: &[DisasmLine], start: usize, end: usize) -> GraphBlock {
    let rows = end - start;

    let mut max_chars = MIN_BLOCK_CHARS;
    for line in &lines[start..end] {
        max_chars = max_chars.max(line.text.chars().count());
    }

    let w = (BLOCK_GUTTER_W + max_chars as f32 * CHAR_W + NOTE_RESERVE + BLOCK_PAD_X * 2.)
        .max(MIN_BLOCK_W);
    let h = BLOCK_HEADER_H + rows as f32 * BLOCK_LINE_H + BLOCK_PAD_Y * 2. + BLOCK_BORDER_H;
    GraphBlock {
        start,
        end,
        x: 0.,
        y: 0.,
        w,
        h,
    }
}

fn layout_blocks(blocks: &mut [GraphBlock], edges: &[(usize, usize, EdgeKind)]) -> (Vec<i32>, f32) {
    let nb = blocks.len();
    let mut layer = vec![0i32; nb];

    let vertices: Vec<(u32, (f64, f64))> = blocks
        .iter()
        .enumerate()
        .map(|(i, b)| (i as u32, (b.w as f64, b.h as f64)))
        .collect();
    let su_edges: Vec<(u32, u32)> = edges
        .iter()
        .filter(|(f, t, _)| f != t)
        .map(|&(f, t, _)| (f as u32, t as u32))
        .collect();

    let config = Config {
        vertex_spacing: VERTEX_SPACING,
        ..Default::default()
    };
    let mut layouts = rust_sugiyama::from_vertices_and_edges(&vertices, &su_edges, &config);
    layouts.sort_by_key(|(nodes, _, _)| nodes.iter().map(|&(id, _)| id).min().unwrap_or(0));

    let mut running_y = 0.;
    let mut max_width: f32 = 0.;
    let mut rank_base = 0i32;

    for (nodes, _, _) in &layouts {
        let mut ranks: Vec<f32> = nodes.iter().map(|&(_, (_, cy))| cy as f32).collect();
        ranks.sort_by(|a, b| a.total_cmp(b));
        ranks.dedup_by(|a, b| (*a - *b).abs() < RANK_EPS);

        let base = rank_base;
        rank_base += ranks.len() as i32;

        let mut min_left = f32::MAX;
        let mut min_top = f32::MAX;
        let mut max_right = f32::MIN;
        let mut max_bot = f32::MIN;
        for &(id, (cx, cy)) in nodes {
            let (cx, cy) = (cx as f32, cy as f32);
            let (w, h) = (blocks[id].w, blocks[id].h);
            min_left = min_left.min(cx - w / 2.);
            min_top = min_top.min(cy - h / 2.);
            max_right = max_right.max(cx + w / 2.);
            max_bot = max_bot.max(cy + h / 2.);
            layer[id] = base
                + ranks
                    .iter()
                    .position(|&r| (r - cy).abs() < RANK_EPS)
                    .unwrap_or(0) as i32;
        }

        if !min_left.is_finite() {
            continue;
        }

        for &(id, (cx, cy)) in nodes {
            blocks[id].x = (cx as f32 - blocks[id].w / 2.) - min_left;
            blocks[id].y = (cy as f32 - blocks[id].h / 2.) - min_top + running_y;
        }

        running_y += (max_bot - min_top) + COMPONENT_GAP;
        max_width = max_width.max(max_right - min_left);
    }

    (layer, max_width)
}

fn route_edges(
    edges: &[(usize, usize, EdgeKind)],
    blocks: &[GraphBlock],
    layer: &[i32],
    max_width: f32,
) -> Vec<GraphEdge> {
    let mut out_total = vec![0usize; blocks.len()];
    let mut in_total = vec![0usize; blocks.len()];
    let mut out_rank = vec![0usize; edges.len()];
    let mut in_rank = vec![0usize; edges.len()];

    for (ei, &(f, t, _)) in edges.iter().enumerate() {
        out_rank[ei] = out_total[f];
        out_total[f] += 1;
        in_rank[ei] = in_total[t];
        in_total[t] += 1;
    }

    let port = |block: &GraphBlock, idx: usize, total: usize| {
        block.x + block.w * (idx as f32 + 1.) / (total as f32 + 1.)
    };

    let max_layer = layer.iter().copied().max().unwrap_or(0).max(0) as usize;
    let mut rank_top = vec![f32::MAX; max_layer + 1];
    let mut rank_bot = vec![f32::MIN; max_layer + 1];
    for (i, b) in blocks.iter().enumerate() {
        let l = layer[i].max(0) as usize;
        rank_top[l] = rank_top[l].min(b.y);
        rank_bot[l] = rank_bot[l].max(b.y + b.h);
    }

    let ends: Vec<EdgeEnds> = edges
        .iter()
        .enumerate()
        .map(|(ei, &(f, t, _))| EdgeEnds {
            sx: port(&blocks[f], out_rank[ei], out_total[f]),
            tx: port(&blocks[t], in_rank[ei], in_total[t]),
            sy: blocks[f].y + blocks[f].h,
            ty: blocks[t].y,
        })
        .collect();

    let is_forward = |f: usize, t: usize| f != t && layer[t] == layer[f] + 1;

    let mut back_lane = 0usize;
    let mut routed: Vec<GraphEdge> = edges
        .iter()
        .enumerate()
        .map(|(ei, &(f, t, kind))| {
            let e = &ends[ei];
            let lf = layer[f].max(0) as usize;
            let lt = layer[t].max(0) as usize;
            let points = if f == t {
                self::self_loop(&blocks[f], e.sx, e.tx)
            } else if is_forward(f, t) {
                let channel = (rank_bot[lf] + rank_top[lt]) / 2.;
                self::forward_route(e.sx, e.sy, e.tx, e.ty, channel)
            } else {
                let lane = max_width + BACK_LANE_GAP * (1 + back_lane) as f32;
                back_lane += 1;
                let out_y = rank_bot[lf] + PORT_STUB;
                let in_y = rank_top[lt] - PORT_STUB;
                self::back_route(e.sx, e.sy, e.tx, e.ty, lane, out_y, in_y)
            };
            GraphEdge {
                kind,
                to: t,
                points,
            }
        })
        .collect();

    self::separate_channels(&mut routed);
    routed
}

struct EdgeEnds {
    sx: f32,
    tx: f32,
    sy: f32,
    ty: f32,
}

fn separate_channels(edges: &mut [GraphEdge]) {
    let mut segs: Vec<(usize, usize, f32, f32, f32)> = Vec::new();
    for (ei, edge) in edges.iter().enumerate() {
        for i in 0..edge.points.len().saturating_sub(1) {
            let (x0, y0) = edge.points[i];
            let (x1, y1) = edge.points[i + 1];
            if (y0 - y1).abs() < 0.5 && (x0 - x1).abs() > 0.5 {
                segs.push((ei, i, y0, x0.min(x1), x0.max(x1)));
            }
        }
    }
    segs.sort_by(|a, b| a.2.total_cmp(&b.2));

    let mut start = 0;
    while start < segs.len() {
        let mut end = start + 1;
        while end < segs.len() && segs[end].2 - segs[end - 1].2 < CHANNEL_STEP {
            end += 1;
        }

        let cluster = &mut segs[start..end];
        let mean = cluster.iter().map(|s| s.2).sum::<f32>() / cluster.len() as f32;
        cluster.sort_by(|a, b| a.3.total_cmp(&b.3));

        let mut lane_right: Vec<f32> = Vec::new();
        let lanes_of: Vec<usize> = cluster
            .iter()
            .map(|s| {
                let slot = lane_right.iter().position(|&r| r <= s.3 - LANE_MARGIN);
                match slot {
                    Some(l) => {
                        lane_right[l] = s.4;
                        l
                    }
                    None => {
                        lane_right.push(s.4);
                        lane_right.len() - 1
                    }
                }
            })
            .collect();

        let lanes = lane_right.len() as f32;
        for (s, &l) in cluster.iter().zip(&lanes_of) {
            let y = mean + (l as f32 - (lanes - 1.) / 2.) * CHANNEL_STEP;
            edges[s.0].points[s.1].1 = y;
            edges[s.0].points[s.1 + 1].1 = y;
        }

        start = end;
    }
}

fn forward_route(sx: f32, sy: f32, tx: f32, ty: f32, channel: f32) -> Vec<(f32, f32)> {
    vec![(sx, sy), (sx, channel), (tx, channel), (tx, ty)]
}

fn back_route(
    sx: f32,
    sy: f32,
    tx: f32,
    ty: f32,
    lane: f32,
    out_y: f32,
    in_y: f32,
) -> Vec<(f32, f32)> {
    vec![
        (sx, sy),
        (sx, out_y),
        (lane, out_y),
        (lane, in_y),
        (tx, in_y),
        (tx, ty),
    ]
}

fn self_loop(b: &GraphBlock, sx: f32, tx: f32) -> Vec<(f32, f32)> {
    let lane = b.x + b.w + BACK_LANE_GAP;
    vec![
        (sx, b.y + b.h),
        (sx, b.y + b.h + PORT_STUB),
        (lane, b.y + b.h + PORT_STUB),
        (lane, b.y - PORT_STUB),
        (tx, b.y - PORT_STUB),
        (tx, b.y),
    ]
}

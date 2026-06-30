use std::collections::HashMap;

use super::{DisasmLine, FlowKind};

pub(super) const BLOCK_HEADER_H: f32 = 20.;
pub(super) const BLOCK_LINE_H: f32 = 16.;
const BLOCK_PAD_X: f32 = 8.;
pub(super) const BLOCK_PAD_Y: f32 = 5.;
const CHAR_W: f32 = 7.;
const MIN_BLOCK_W: f32 = 96.;
const H_GAP: f32 = 36.;
const V_GAP: f32 = 38.;
const COMPONENT_GAP: f32 = 56.;
const BACK_LANE_GAP: f32 = 22.;
const PORT_STUB: f32 = 10.;
const CHANNEL_STEP: f32 = 8.;
const MARGIN: f32 = 24.;
const NOTE_RESERVE: f32 = 150.;
const MIN_BLOCK_CHARS: usize = 8;
const BLOCK_BORDER_H: f32 = 2.;
const ORDER_PASSES: usize = 4;
const COORD_PASSES: usize = 8;

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
    pub points: Vec<(f32, f32)>,
}

pub(super) struct GraphLayout {
    pub blocks: Vec<GraphBlock>,
    pub edges: Vec<GraphEdge>,
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
        blocks.push(block_geometry(lines, start, end));
    }

    for (bi, block) in blocks.iter().enumerate() {
        for slot in block_at_line.iter_mut().take(block.end).skip(block.start) {
            *slot = bi;
        }
    }

    let nb = blocks.len();
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

    let mut succ: Vec<Vec<usize>> = vec![Vec::new(); nb];
    let mut pred: Vec<Vec<usize>> = vec![Vec::new(); nb];
    for &(f, t, _) in &edges {
        if f != t {
            succ[f].push(t);
            pred[t].push(f);
        }
    }

    let (layer, order) = layer_assignment(nb, &succ);
    let components = components(nb, &edges);

    let mut running_y = 0.;
    let mut max_width: f32 = 0.;
    for comp in components {
        let (height, width) =
            layout_component(&comp, &mut blocks, &layer, &order, &succ, &pred, running_y);
        running_y += height + COMPONENT_GAP;
        max_width = max_width.max(width);
    }

    let edges = route_edges(&edges, &blocks, &layer, max_width);

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

    let w = (max_chars as f32 * CHAR_W + NOTE_RESERVE + BLOCK_PAD_X * 2.).max(MIN_BLOCK_W);
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

fn layer_assignment(nb: usize, succ: &[Vec<usize>]) -> (Vec<i32>, Vec<usize>) {
    let mut state = vec![0u8; nb];
    let mut disc = vec![usize::MAX; nb];
    let mut tick = 0usize;
    let mut back: Vec<(usize, usize)> = Vec::new();

    for s in 0..nb {
        if state[s] != 0 {
            continue;
        }
        let mut stack: Vec<(usize, usize)> = vec![(s, 0)];
        state[s] = 1;
        disc[s] = tick;
        tick += 1;

        while let Some(&(u, ci)) = stack.last() {
            if ci < succ[u].len() {
                stack.last_mut().unwrap().1 += 1;
                let v = succ[u][ci];
                match state[v] {
                    0 => {
                        state[v] = 1;
                        disc[v] = tick;
                        tick += 1;
                        stack.push((v, 0));
                    }
                    1 => back.push((u, v)),
                    _ => {}
                }
            } else {
                state[u] = 2;
                stack.pop();
            }
        }
    }

    let back_set: std::collections::HashSet<(usize, usize)> = back.iter().copied().collect();

    let forward: Vec<(usize, usize)> = (0..nb)
        .flat_map(|u| succ[u].iter().map(move |&v| (u, v)))
        .filter(|e| !back_set.contains(e))
        .collect();

    let mut layer = vec![0i32; nb];
    loop {
        let mut changed = false;
        for &(u, v) in &forward {
            if layer[v] < layer[u] + 1 {
                layer[v] = layer[u] + 1;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let mut order: Vec<usize> = (0..nb).collect();
    order.sort_by_key(|&u| disc[u]);

    (layer, order)
}

fn components(nb: usize, edges: &[(usize, usize, EdgeKind)]) -> Vec<Vec<usize>> {
    let mut parent: Vec<usize> = (0..nb).collect();
    for &(f, t, _) in edges {
        let (a, b) = (find(&mut parent, f), find(&mut parent, t));
        if a != b {
            parent[a] = b;
        }
    }

    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for u in 0..nb {
        let root = find(&mut parent, u);
        groups.entry(root).or_default().push(u);
    }

    let mut comps: Vec<Vec<usize>> = groups.into_values().collect();
    comps.sort_by_key(|c| c.iter().copied().min().unwrap_or(0));
    comps
}

fn find(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]];
        x = parent[x];
    }
    x
}

#[allow(clippy::needless_range_loop)]
fn layout_component(
    comp: &[usize],
    blocks: &mut [GraphBlock],
    layer: &[i32],
    order: &[usize],
    succ: &[Vec<usize>],
    pred: &[Vec<usize>],
    offset_y: f32,
) -> (f32, f32) {
    let min_layer = comp.iter().map(|&u| layer[u]).min().unwrap_or(0);
    let max_layer = comp.iter().map(|&u| layer[u]).max().unwrap_or(0);
    let depth = (max_layer - min_layer + 1) as usize;

    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); depth];
    let mut disc_rank: HashMap<usize, usize> = HashMap::new();
    for (rank, &u) in order.iter().enumerate() {
        disc_rank.insert(u, rank);
    }

    let mut comp_nodes = comp.to_vec();
    comp_nodes.sort_by_key(|u| disc_rank.get(u).copied().unwrap_or(0));
    for &u in &comp_nodes {
        layers[(layer[u] - min_layer) as usize].push(u);
    }

    for _ in 0..ORDER_PASSES {
        for l in 1..depth {
            sort_layer_by_neighbor(&mut layers, l, pred, true);
        }
        for l in (0..depth.saturating_sub(1)).rev() {
            sort_layer_by_neighbor(&mut layers, l, succ, false);
        }
    }

    let layer_y: Vec<f32> = {
        let mut ys = vec![0.; depth];
        let mut cursor = offset_y;
        for l in 0..depth {
            ys[l] = cursor;
            let row_h = layers[l].iter().map(|&u| blocks[u].h).fold(0., f32::max);
            cursor += row_h + V_GAP;
        }
        ys
    };

    for l in 0..depth {
        for &u in &layers[l] {
            blocks[u].y = layer_y[l];
        }
    }

    for l in 0..depth {
        let mut cursor = 0.;
        for &u in &layers[l] {
            blocks[u].x = cursor;
            cursor += blocks[u].w + H_GAP;
        }
    }

    for _ in 0..COORD_PASSES {
        for l in 0..depth {
            let nodes = &layers[l];
            let desired: Vec<f32> = nodes
                .iter()
                .map(|&u| {
                    let mut sum = 0.;
                    let mut cnt = 0.;
                    for &v in succ[u].iter().chain(pred[u].iter()) {
                        sum += blocks[v].x + blocks[v].w / 2.;
                        cnt += 1.;
                    }
                    if cnt > 0. {
                        sum / cnt - blocks[u].w / 2.
                    } else {
                        blocks[u].x
                    }
                })
                .collect();

            for k in 0..nodes.len() {
                let u = nodes[k];
                let want = desired[k];
                blocks[u].x = if k == 0 {
                    want
                } else {
                    let prev = nodes[k - 1];
                    want.max(blocks[prev].x + blocks[prev].w + H_GAP)
                };
            }
        }
    }

    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = offset_y;
    for &u in comp {
        min_x = min_x.min(blocks[u].x);
        max_x = max_x.max(blocks[u].x + blocks[u].w);
        max_y = max_y.max(blocks[u].y + blocks[u].h);
    }

    if min_x.is_finite() {
        for &u in comp {
            blocks[u].x -= min_x;
        }
    }

    let width = (max_x - min_x).max(0.);
    let height = max_y - offset_y;
    (height, width)
}

fn sort_layer_by_neighbor(
    layers: &mut [Vec<usize>],
    l: usize,
    neighbors: &[Vec<usize>],
    look_up: bool,
) {
    let adj = if look_up {
        if l == 0 {
            return;
        }
        l - 1
    } else {
        if l + 1 >= layers.len() {
            return;
        }
        l + 1
    };
    let mut pos: HashMap<usize, usize> = HashMap::new();
    for (i, &u) in layers[adj].iter().enumerate() {
        pos.insert(u, i);
    }

    let mut keyed: Vec<(f32, usize)> = layers[l]
        .iter()
        .enumerate()
        .map(|(idx, &u)| {
            let mut vals: Vec<usize> = neighbors[u]
                .iter()
                .filter_map(|v| pos.get(v).copied())
                .collect();
            vals.sort_unstable();

            let key = if vals.is_empty() {
                idx as f32
            } else {
                vals[vals.len() / 2] as f32
            };

            (key, u)
        })
        .collect();

    keyed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    layers[l] = keyed.into_iter().map(|(_, u)| u).collect();
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

    let mut back_lane = 0usize;
    edges
        .iter()
        .enumerate()
        .map(|(ei, &(f, t, kind))| {
            let sx = port(&blocks[f], out_rank[ei], out_total[f]);
            let tx = port(&blocks[t], in_rank[ei], in_total[t]);
            let sy = blocks[f].y + blocks[f].h;
            let ty = blocks[t].y;

            let points = if f == t {
                self_loop(&blocks[f], sx, tx)
            } else if layer[t] == layer[f] + 1 {
                let in_n = in_total[t] as f32;
                let bias = (in_rank[ei] as f32 - (in_n - 1.) / 2.) * CHANNEL_STEP;
                let channel = (sy + ty) / 2. + bias;
                forward_route(sx, sy, tx, ty, channel)
            } else {
                let lane = max_width + BACK_LANE_GAP * (1 + back_lane) as f32;
                let stub = (back_lane % 4) as f32 * 4.;
                back_lane += 1;
                back_route(sx, sy, tx, ty, lane, stub)
            };
            GraphEdge { kind, points }
        })
        .collect()
}

fn forward_route(sx: f32, sy: f32, tx: f32, ty: f32, channel: f32) -> Vec<(f32, f32)> {
    vec![(sx, sy), (sx, channel), (tx, channel), (tx, ty)]
}

fn back_route(sx: f32, sy: f32, tx: f32, ty: f32, lane: f32, stub: f32) -> Vec<(f32, f32)> {
    let out_y = sy + PORT_STUB + stub;
    let in_y = ty - PORT_STUB - stub;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn line(addr: u32, flow: FlowKind, target: Option<u32>) -> DisasmLine {
        DisasmLine {
            addr,
            flow,
            target,
            ..Default::default()
        }
    }

    fn count(layout: &GraphLayout, kind: EdgeKind) -> usize {
        layout.edges.iter().filter(|e| e.kind == kind).count()
    }

    #[test]
    fn conditional_branch_splits_into_taken_and_fallthrough() {
        let lines = vec![
            line(0x10, FlowKind::None, None),
            line(0x12, FlowKind::CondBranch, Some(0x10)),
            line(0x14, FlowKind::Return, None),
        ];
        let layout = build(&lines);

        assert_eq!(layout.blocks.len(), 2);
        assert_eq!((layout.blocks[0].start, layout.blocks[0].end), (0, 2));
        assert_eq!((layout.blocks[1].start, layout.blocks[1].end), (2, 3));
        assert_eq!(layout.edges.len(), 2);
        assert_eq!(count(&layout, EdgeKind::Taken), 1);
        assert_eq!(count(&layout, EdgeKind::FallThrough), 1);
    }

    #[test]
    fn calls_stay_mid_block_and_branch_is_normal() {
        let lines = vec![
            line(0x00, FlowKind::Call, Some(0x100)),
            line(0x03, FlowKind::None, None),
            line(0x04, FlowKind::Branch, Some(0x00)),
        ];
        let layout = build(&lines);

        assert_eq!(layout.blocks.len(), 1);
        assert_eq!((layout.blocks[0].start, layout.blocks[0].end), (0, 3));
        assert_eq!(layout.edges.len(), 1);
        assert_eq!(count(&layout, EdgeKind::Normal), 1);
    }

    #[test]
    fn return_has_no_successor_edges() {
        let lines = vec![line(0x00, FlowKind::Return, None)];
        let layout = build(&lines);

        assert_eq!(layout.blocks.len(), 1);
        assert!(layout.edges.is_empty());
    }
}

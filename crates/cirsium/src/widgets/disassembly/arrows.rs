use std::collections::HashMap;

use gpui::{Bounds, Hsla, PathBuilder, Pixels, Window, hsla, point, px};

use super::{DisasmLine, ROW_HEIGHT};
use crate::theme::with_alpha;

const MAX_ARROW_LANES: usize = 8;
const ARROW_COL_WIDTH: f32 = 8.;
const ARROW_THICKNESS: f32 = 1.5;
const ARROW_EMPHASIS: f32 = 1.;
const ARROW_HEAD: f32 = 8.;
const ARROW_HEAD_HALF: f32 = 4.;
const ARROW_RADIUS: f32 = 4.;
const ARROW_HEAD_GAP: f32 = 7.;

#[derive(Clone, Copy)]
pub(super) enum GutterDraw {
    Through {
        x: f32,
        color: Hsla,
        thickness: f32,
    },
    Elbow {
        x: f32,
        edge_y: f32,
        x_end: f32,
        color: Hsla,
        thickness: f32,
    },
    Head {
        x_base: f32,
        color: Hsla,
    },
}

pub(super) type GutterCell = Vec<GutterDraw>;

fn arrow_color(index: usize) -> Hsla {
    const GOLDEN: f32 = 0.618_034;
    let hue = (index as f32 * GOLDEN).fract();
    hsla(hue, 0.62, 0.62, 1.0)
}

pub(super) fn arrow_gutters(lines: &[DisasmLine]) -> (Vec<GutterCell>, f32) {
    let n = lines.len();
    let mut by_addr: HashMap<u32, usize> = HashMap::with_capacity(n);
    for (i, line) in lines.iter().enumerate() {
        by_addr.entry(line.addr).or_insert(i);
    }

    let mut arrows: Vec<(usize, usize)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if let Some(target) = line.target
            && let Some(&j) = by_addr.get(&target)
            && j != i
        {
            arrows.push((i, j));
        }
    }
    if arrows.is_empty() {
        return (Vec::new(), 0.);
    }

    arrows.sort_by_key(|&(a, b)| a.max(b) - a.min(b));

    let mut lanes: Vec<Vec<(usize, usize)>> = Vec::new();
    let mut placed: Vec<(usize, usize, usize, usize)> = Vec::new();
    'arrow: for (from, to) in arrows {
        let (lo, hi) = (from.min(to), from.max(to));
        for lane in 0..MAX_ARROW_LANES {
            if lane == lanes.len() {
                lanes.push(Vec::new());
            }

            if !lanes[lane].iter().any(|&(a, b)| lo <= b && a <= hi) {
                lanes[lane].push((lo, hi));
                placed.push((from, to, lane, placed.len()));
                continue 'arrow;
            }
        }
    }

    let lane_count = lanes.len();
    if lane_count == 0 {
        return (Vec::new(), 0.);
    }

    let lanes_width = lane_count as f32 * ARROW_COL_WIDTH;
    let head_left = lanes_width + ARROW_HEAD_GAP;
    let width = head_left + ARROW_HEAD;

    let selected = lines.iter().position(|l| l.selected);

    placed.sort_by_key(|&(from, to, _, _)| {
        usize::from(selected == Some(from) || selected == Some(to))
    });

    let mut cells = vec![GutterCell::default(); n];
    for (from, to, lane, color_index) in placed {
        let related = selected == Some(from) || selected == Some(to);
        let mut color = arrow_color(color_index);
        let thickness = if related {
            ARROW_THICKNESS + ARROW_EMPHASIS
        } else {
            ARROW_THICKNESS
        };
        if selected.is_some() && !related {
            color = with_alpha(color, 0.25);
        }

        let col = lane_count - 1 - lane;
        let xc = col as f32 * ARROW_COL_WIDTH + ARROW_COL_WIDTH / 2.;
        let (lo, hi) = (from.min(to), from.max(to));
        let down = to > from;

        for cell in cells.iter_mut().take(hi).skip(lo + 1) {
            cell.push(GutterDraw::Through {
                x: xc,
                color,
                thickness,
            });
        }

        cells[from].push(GutterDraw::Elbow {
            x: xc,
            edge_y: if down { ROW_HEIGHT } else { 0. },
            x_end: width,
            color,
            thickness,
        });

        cells[to].push(GutterDraw::Elbow {
            x: xc,
            edge_y: if down { 0. } else { ROW_HEIGHT },
            x_end: head_left + 1.5,
            color,
            thickness,
        });
        cells[to].push(GutterDraw::Head {
            x_base: head_left,
            color,
        });
    }

    (cells, width)
}

pub(super) fn paint_arrows(window: &mut Window, bounds: Bounds<Pixels>, draws: &[GutterDraw]) {
    let (ox, oy) = (bounds.origin.x, bounds.origin.y);
    let mid = ROW_HEIGHT / 2.;
    let r = ARROW_RADIUS;
    let pt = |x: f32, y: f32| point(ox + px(x), oy + px(y));

    for draw in draws {
        match *draw {
            GutterDraw::Through {
                x,
                color,
                thickness,
            } => {
                let mut b = PathBuilder::stroke(px(thickness));
                b.move_to(pt(x, 0.));
                b.line_to(pt(x, ROW_HEIGHT));
                if let Ok(path) = b.build() {
                    window.paint_path(path, color);
                }
            }
            GutterDraw::Elbow {
                x,
                edge_y,
                x_end,
                color,
                thickness,
            } => {
                let mut b = PathBuilder::stroke(px(thickness));
                b.move_to(pt(x, edge_y));

                let approach = if edge_y < mid { mid - r } else { mid + r };
                b.line_to(pt(x, approach));
                b.curve_to(pt(x + r, mid), pt(x, mid));
                b.line_to(pt(x_end, mid));
                if let Ok(path) = b.build() {
                    window.paint_path(path, color);
                }
            }
            GutterDraw::Head { x_base, color } => {
                let mut b = PathBuilder::fill();
                b.move_to(pt(x_base, mid - ARROW_HEAD_HALF));
                b.line_to(pt(x_base + ARROW_HEAD, mid));
                b.line_to(pt(x_base, mid + ARROW_HEAD_HALF));
                b.line_to(pt(x_base, mid - ARROW_HEAD_HALF));
                if let Ok(path) = b.build() {
                    window.paint_path(path, color);
                }
            }
        }
    }
}

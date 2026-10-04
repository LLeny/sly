use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use web_time::{Duration, Instant};

use cirsium::widgets::{BreakpointKind, DisassemblyLayoutCache, DisassemblyView, ViewMode};
use gpui::{
    Pixels, Point, ScrollStrategy, SharedString, Size, UniformListScrollHandle, ease_out_quint,
    size,
};

const JUMP_DURATION: Duration = Duration::from_millis(260);

struct GraphAnim {
    from: Point<Pixels>,
    to: Point<Pixels>,
    start: Instant,
}

use debugger_model::{
    BreakpointMark, CpuDescriptor, CpuId, DebugCore, DecodeMode, DisassemblyRequest, mark_at,
};

const LEAD_BYTES: u32 = 48;
const COUNT: usize = 200;
// Rows kept visible above the PC when following it.
const CONTEXT_ROWS: usize = 2;

pub struct Menu {
    pub position: Point<Pixels>,
    pub addr: u32,
    pub len: u32,
    pub address: SharedString,
    pub bytes: SharedString,
    pub text: SharedString,
}

fn aligned_base(pc: u32, lead: u32, mut sweep: impl FnMut(u32, usize) -> Vec<u32>) -> u32 {
    for start in pc.saturating_sub(lead)..pc {
        let addrs = sweep(start, (pc - start) as usize + 1);
        if addrs.contains(&pc) {
            return start;
        }
    }
    pc
}

pub struct Disasm {
    pub view: Arc<DisassemblyView>,
    pub scroll: UniformListScrollHandle,
    pub menu: Option<Menu>,
    pub layout_cache: Rc<RefCell<DisassemblyLayoutCache>>,
    cpu: CpuId,
    space: Arc<str>,
    decode_modes: Vec<DecodeMode>,
    decode_mode: Option<usize>,
    follow_pc: bool,
    show_bytes: bool,
    view_mode: ViewMode,
    graph_offset: Point<Pixels>,
    graph_drag: Option<Point<Pixels>>,
    graph_viewport: Size<f32>,
    graph_anim: Option<GraphAnim>,
    hovered_edge: Option<usize>,
    base: u32,
    selected: Option<u32>,
    breakpoints: Vec<BreakpointMark>,
    fingerprint: Option<u64>,
    current_ix: Option<usize>,
    pipeline_ix: Option<usize>,
    current_note: Option<SharedString>,
    bp_rows: Rc<HashMap<usize, BreakpointKind>>,
}

impl Disasm {
    pub fn new(desc: &CpuDescriptor) -> Self {
        Self {
            view: Arc::default(),
            scroll: UniformListScrollHandle::new(),
            menu: None,
            layout_cache: Rc::default(),
            cpu: desc.id.clone(),
            space: desc.space.clone(),
            decode_modes: desc.decode_modes.clone(),
            decode_mode: None,
            follow_pc: true,
            show_bytes: false,
            view_mode: ViewMode::Linear,
            graph_offset: Point::default(),
            graph_drag: None,
            graph_viewport: size(0., 0.),
            graph_anim: None,
            hovered_edge: None,
            base: 0,
            selected: None,
            breakpoints: Vec::new(),
            fingerprint: None,
            current_ix: None,
            pipeline_ix: None,
            current_note: None,
            bp_rows: Rc::default(),
        }
    }

    fn request(&self, base: u32, count: usize) -> DisassemblyRequest {
        DisassemblyRequest {
            base,
            count,
            mode: self.decode_mode.map(|ix| self.decode_modes[ix].id.clone()),
        }
    }

    pub fn decode_modes(&self) -> &[DecodeMode] {
        &self.decode_modes
    }

    pub fn decode_mode(&self) -> Option<&str> {
        self.decode_mode.map(|ix| self.decode_modes[ix].id.as_ref())
    }

    pub fn set_decode_mode(&mut self, mode: Option<&str>, target: &dyn DebugCore) {
        let Ok(selected) = mode
            .map(|id| {
                self.decode_modes
                    .iter()
                    .position(|mode| mode.id.as_ref() == id)
                    .ok_or(())
            })
            .transpose()
        else {
            return;
        };
        if self.decode_mode == selected {
            return;
        }

        self.decode_mode = selected;

        self.fingerprint = None;
        self.rebuild(target, true);
    }

    pub fn follow_pc(&self) -> bool {
        self.follow_pc
    }

    pub fn show_bytes(&self) -> bool {
        self.show_bytes
    }

    pub fn current_ix(&self) -> Option<usize> {
        self.current_ix
    }

    pub fn pipeline_ix(&self) -> Option<usize> {
        self.pipeline_ix
    }

    pub fn current_note(&self) -> Option<SharedString> {
        self.current_note.clone()
    }

    pub fn selected_ix(&self) -> Option<usize> {
        self.selected.and_then(|addr| self.line_index(addr))
    }

    fn line_index(&self, addr: u32) -> Option<usize> {
        self.view.lines.iter().position(|line| line.addr == addr)
    }

    pub fn breakpoint_rows(&self) -> Rc<HashMap<usize, BreakpointKind>> {
        self.bp_rows.clone()
    }

    pub fn recompute(&mut self, target: &dyn DebugCore) {
        self.rebuild(target, false);
    }

    fn rebuild(&mut self, target: &dyn DebugCore, force_rebase: bool) {
        let pc = target.pc(&self.cpu);

        let mut checked_fp = None;
        if !force_rebase && !self.view.lines.is_empty() {
            let holds_pc = self.view.lines.iter().any(|l| l.addr == pc);
            if !self.follow_pc || holds_pc {
                let fp = target.disasm_fingerprint(&self.cpu, &self.request(self.base, COUNT));
                if fp.is_some() && fp == self.fingerprint {
                    self.refresh_row_states(target, pc);
                    self.follow_scroll();
                    return;
                }

                checked_fp = Some(fp);
            }
        }

        let lead = if self.view_mode == ViewMode::Graph {
            0
        } else {
            LEAD_BYTES
        };

        let (lines, base) = if self.follow_pc {
            let held = if force_rebase {
                None
            } else {
                let held_lines = target.disassemble(&self.cpu, &self.request(self.base, COUNT));
                held_lines
                    .iter()
                    .any(|line| line.addr == pc)
                    .then_some(held_lines)
            };

            match held {
                Some(held_lines) => (held_lines, self.base),
                None => {
                    let base = self.aligned_base(target, pc, lead);
                    let lines = target.disassemble(&self.cpu, &self.request(base, COUNT));

                    if lines.iter().any(|line| line.addr == pc) {
                        (lines, base)
                    } else {
                        (target.disassemble(&self.cpu, &self.request(pc, COUNT)), pc)
                    }
                }
            }
        } else {
            (
                target.disassemble(&self.cpu, &self.request(self.base, COUNT)),
                self.base,
            )
        };

        let same_base = base == self.base;
        self.base = base;
        self.view = Arc::new(DisassemblyView {
            lines,
            show_bytes: self.show_bytes,
        });

        self.fingerprint = match checked_fp {
            Some(fp) if same_base => fp,
            _ => target.disasm_fingerprint(&self.cpu, &self.request(base, COUNT)),
        };

        self.refresh_row_states(target, pc);
        self.refresh_breakpoint_rows();

        if self.follow_pc {
            self.graph_offset = Point::default();
            self.graph_anim = None;
        }

        self.follow_scroll();
    }

    fn refresh_row_states(&mut self, target: &dyn DebugCore, pc: u32) {
        self.current_ix = self.line_index(pc);
        self.pipeline_ix = target
            .pipeline_pc(&self.cpu)
            .and_then(|addr| self.line_index(addr));
        self.current_note = self.current_ix.and_then(|_| {
            let note = target
                .disassemble_one(&self.cpu, &self.request(pc, 1))?
                .note;
            (!note.is_empty()).then(|| SharedString::from(note.as_ref()))
        });
    }

    fn refresh_breakpoint_rows(&mut self) {
        self.bp_rows = Rc::new(
            self.view
                .lines
                .iter()
                .enumerate()
                .filter_map(|(ix, line)| {
                    mark_at(
                        &self.breakpoints,
                        &self.cpu,
                        &self.space,
                        line.addr,
                        Some(BreakpointKind::Execute),
                    )
                    .map(|kind| (ix, kind))
                })
                .collect(),
        );
    }

    fn follow_scroll(&mut self) {
        if self.follow_pc
            && self.view_mode == ViewMode::Linear
            && let Some(ix) = self.current_ix
        {
            self.scroll
                .scroll_to_item_strict(ix.saturating_sub(CONTEXT_ROWS), ScrollStrategy::Top);
        }
    }

    fn aligned_base(&self, target: &dyn DebugCore, pc: u32, lead: u32) -> u32 {
        aligned_base(pc, lead, |start, count| {
            target
                .disassemble(&self.cpu, &self.request(start, count))
                .iter()
                .map(|line| line.addr)
                .collect()
        })
    }

    pub fn set_breakpoints(&mut self, breakpoints: &[BreakpointMark]) {
        self.breakpoints = breakpoints
            .iter()
            .filter(|m| m.cpu == self.cpu)
            .cloned()
            .collect();
        self.refresh_breakpoint_rows();
    }

    pub fn select(&mut self, ix: usize) {
        if let Some(line) = self.view.lines.get(ix) {
            if self.selected == Some(line.addr) {
                self.selected = None;
            } else {
                self.selected = Some(line.addr);
            }
        }
    }

    pub fn open_menu(&mut self, ix: usize, position: Point<Pixels>) {
        if let Some(line) = self.view.lines.get(ix) {
            self.selected = Some(line.addr);
            self.menu = Some(Menu {
                position,
                addr: line.addr,
                len: u32::from(line.length),
                address: line.address.clone().into(),
                bytes: line.bytes.clone().into(),
                text: line.text.clone().into(),
            });
        }
    }

    pub fn close_menu(&mut self) -> bool {
        self.menu.take().is_some()
    }

    pub fn toggle_follow(&mut self, target: &dyn DebugCore) {
        self.follow_pc = !self.follow_pc;
        self.recompute(target);
    }

    pub fn toggle_bytes(&mut self) {
        self.show_bytes = !self.show_bytes;
        self.view = Arc::new(DisassemblyView {
            lines: self.view.lines.clone(),
            show_bytes: self.show_bytes,
        });
    }

    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
    }

    pub fn set_view_mode(&mut self, mode: ViewMode, target: &dyn DebugCore) {
        if mode == ViewMode::Graph {
            self.graph_viewport = size(0., 0.);
        }

        self.view_mode = mode;
        self.rebuild(target, true);
    }

    pub fn graph_offset(&self) -> Point<Pixels> {
        match &self.graph_anim {
            Some(anim) => {
                let t = (anim.start.elapsed().as_secs_f32() / JUMP_DURATION.as_secs_f32())
                    .clamp(0., 1.);
                anim.from + (anim.to - anim.from) * ease_out_quint()(t)
            }
            None => self.graph_offset,
        }
    }

    pub fn graph_animating(&mut self) -> bool {
        match &self.graph_anim {
            Some(anim) if anim.start.elapsed() >= JUMP_DURATION => {
                self.graph_offset = anim.to;
                self.graph_anim = None;
                false
            }
            Some(_) => true,
            None => false,
        }
    }

    pub fn jump(&mut self, from: Point<Pixels>, to: Point<Pixels>) {
        self.follow_pc = false;
        self.hovered_edge = None;
        self.graph_anim = Some(GraphAnim {
            from,
            to,
            start: Instant::now(),
        });
    }

    pub fn hovered_edge(&self) -> Option<usize> {
        self.hovered_edge
    }

    pub fn set_hovered_edge(&mut self, edge: Option<usize>) -> bool {
        let changed = self.hovered_edge != edge;
        self.hovered_edge = edge;
        changed
    }

    pub fn pan_begin(&mut self, pos: Point<Pixels>) {
        self.graph_offset = self.graph_offset();
        self.graph_anim = None;
        self.graph_drag = Some(pos);
    }

    pub fn pan_drag(&mut self, pos: Point<Pixels>) {
        if let Some(last) = self.graph_drag {
            self.graph_offset += pos - last;
            self.graph_drag = Some(pos);
        }
    }

    pub fn graph_viewport(&self) -> Size<f32> {
        self.graph_viewport
    }

    pub fn set_graph_viewport(&mut self, viewport: Size<f32>) -> bool {
        let changed = (self.graph_viewport.width - viewport.width).abs() > 0.5
            || (self.graph_viewport.height - viewport.height).abs() > 0.5;

        if changed {
            self.graph_viewport = viewport;
        }

        changed
    }
}

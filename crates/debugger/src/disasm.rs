use cirsium::widgets::{DisassemblyView, ViewMode};
use gpui::{Pixels, Point, ScrollStrategy, SharedString, Size, UniformListScrollHandle, size};

use crate::breakpoints::{BreakpointMark, mark_at};
use crate::processor::CpuId;
use crate::target::DebugTarget;

const LEAD_BYTES: u32 = 48;
const COUNT: usize = 200;

pub struct Menu {
    pub position: Point<Pixels>,
    pub addr: u32,
    pub len: u32,
    pub address: SharedString,
    pub bytes: SharedString,
    pub text: SharedString,
}

pub struct Disasm {
    pub view: DisassemblyView,
    pub scroll: UniformListScrollHandle,
    pub menu: Option<Menu>,
    cpu: CpuId,
    follow_pc: bool,
    show_bytes: bool,
    view_mode: ViewMode,
    graph_offset: Point<Pixels>,
    graph_drag: Option<Point<Pixels>>,
    graph_viewport: Size<f32>,
    base: u32,
    selected: Option<u32>,
    breakpoints: Vec<BreakpointMark>,
}

impl Disasm {
    pub fn new(cpu: CpuId) -> Self {
        Self {
            view: DisassemblyView::default(),
            scroll: UniformListScrollHandle::new(),
            menu: None,
            cpu,
            follow_pc: true,
            show_bytes: false,
            view_mode: ViewMode::Linear,
            graph_offset: Point::default(),
            graph_drag: None,
            graph_viewport: size(600., 400.),
            base: 0,
            selected: None,
            breakpoints: Vec::new(),
        }
    }

    pub fn follow_pc(&self) -> bool {
        self.follow_pc
    }

    pub fn show_bytes(&self) -> bool {
        self.show_bytes
    }

    pub fn recompute(&mut self, target: &dyn DebugTarget) {
        self.rebuild(target, false);
    }

    fn rebuild(&mut self, target: &dyn DebugTarget, force_rebase: bool) {
        let pc = target.pc(&self.cpu);

        let lead = if self.view_mode == ViewMode::Graph {
            0
        } else {
            LEAD_BYTES
        };

        let (lines, base) = if self.follow_pc {
            let held = if force_rebase {
                None
            } else {
                let held_lines = target.disassemble(&self.cpu, self.base, COUNT);
                held_lines
                    .iter()
                    .any(|line| line.current)
                    .then_some(held_lines)
            };

            match held {
                Some(held_lines) => (held_lines, self.base),
                None => {
                    let base = pc.saturating_sub(lead);
                    let lines = target.disassemble(&self.cpu, base, COUNT);

                    if lines.iter().any(|line| line.current) {
                        (lines, base)
                    } else {
                        (target.disassemble(&self.cpu, pc, COUNT), pc)
                    }
                }
            }
        } else {
            (target.disassemble(&self.cpu, self.base, COUNT), self.base)
        };

        self.base = base;
        self.view = DisassemblyView {
            lines: lines.into_iter().map(Into::into).collect(),
            show_bytes: self.show_bytes,
        };
        self.apply_selection();
        self.apply_breakpoints();

        if self.follow_pc {
            self.graph_offset = Point::default();
        }

        if self.follow_pc
            && self.view_mode == ViewMode::Linear
            && let Some(ix) = self.view.lines.iter().position(|l| l.current)
        {
            self.scroll.scroll_to_item_strict(ix, ScrollStrategy::Top);
        }
    }

    fn apply_selection(&mut self) {
        for line in &mut self.view.lines {
            line.selected = self.selected == Some(line.addr);
        }
    }

    fn apply_breakpoints(&mut self) {
        for line in &mut self.view.lines {
            line.breakpoint = mark_at(&self.breakpoints, line.addr, 0xFFFF);
        }
    }

    pub fn set_breakpoints(&mut self, breakpoints: &[BreakpointMark]) {
        self.breakpoints = breakpoints
            .iter()
            .filter(|m| m.cpu == self.cpu)
            .cloned()
            .collect();
        self.apply_breakpoints();
    }

    pub fn select(&mut self, ix: usize) {
        if let Some(line) = self.view.lines.get(ix) {
            self.selected = if self.selected == Some(line.addr) {
                None
            } else {
                Some(line.addr)
            };
            self.apply_selection();
        }
    }

    pub fn open_menu(&mut self, ix: usize, position: Point<Pixels>) {
        if let Some(line) = self.view.lines.get(ix) {
            self.selected = Some(line.addr);
            self.menu = Some(Menu {
                position,
                addr: line.addr,
                len: line.bytes.split_whitespace().count().max(1) as u32,
                address: line.address.clone(),
                bytes: line.bytes.clone(),
                text: line.text.clone(),
            });
            self.apply_selection();
        }
    }

    pub fn close_menu(&mut self) -> bool {
        self.menu.take().is_some()
    }

    pub fn toggle_follow(&mut self, target: &dyn DebugTarget) {
        self.follow_pc = !self.follow_pc;
        self.recompute(target);
    }

    pub fn toggle_bytes(&mut self) {
        self.show_bytes = !self.show_bytes;
        self.view.show_bytes = self.show_bytes;
    }

    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
    }

    pub fn set_view_mode(&mut self, mode: ViewMode, target: &dyn DebugTarget) {
        self.view_mode = mode;
        self.rebuild(target, true);
    }

    pub fn graph_offset(&self) -> Point<Pixels> {
        self.graph_offset
    }

    pub fn pan_begin(&mut self, pos: Point<Pixels>) {
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

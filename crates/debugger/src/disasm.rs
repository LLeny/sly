use cirsium::widgets::DisassemblyView;
use gpui::{Pixels, Point, ScrollStrategy, SharedString, UniformListScrollHandle};

use crate::breakpoints::{BreakpointMark, mark_at};
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DisasmSource {
    Main,
    Spc,
}

pub struct Disasm {
    pub view: DisassemblyView,
    pub scroll: UniformListScrollHandle,
    pub menu: Option<Menu>,
    source: DisasmSource,
    follow_pc: bool,
    show_bytes: bool,
    base: u32,
    selected: Option<u32>,
    breakpoints: Vec<BreakpointMark>,
}

impl Disasm {
    pub fn new() -> Self {
        Self::for_source(DisasmSource::Main)
    }

    pub fn for_source(source: DisasmSource) -> Self {
        Self {
            view: DisassemblyView::default(),
            scroll: UniformListScrollHandle::new(),
            menu: None,
            source,
            follow_pc: true,
            show_bytes: false,
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

    fn pc(&self, target: &dyn DebugTarget) -> u32 {
        match self.source {
            DisasmSource::Main => target.pc(),
            DisasmSource::Spc => target.spc_pc(),
        }
    }

    fn disassemble(&self, target: &dyn DebugTarget, base: u32, count: usize) -> DisassemblyView {
        match self.source {
            DisasmSource::Main => target.disassemble(base, count),
            DisasmSource::Spc => target.spc_disassemble(base, count),
        }
    }

    pub fn recompute(&mut self, target: &dyn DebugTarget) {
        let pc = self.pc(target);
        let (view, base) = if self.follow_pc {
            let base = pc.saturating_sub(LEAD_BYTES);
            let view = self.disassemble(target, base, COUNT);

            if view.lines.iter().any(|l| l.current) {
                (view, base)
            } else {
                (self.disassemble(target, pc, COUNT), pc)
            }
        } else {
            (self.disassemble(target, self.base, COUNT), self.base)
        };

        self.base = base;
        self.view = view;
        self.view.show_bytes = self.show_bytes;
        self.apply_selection();
        self.apply_breakpoints();

        if self.follow_pc
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

    pub fn set_breakpoints(&mut self, breakpoints: Vec<BreakpointMark>) {
        self.breakpoints = breakpoints;
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
}

use cirsium::widgets::DisassemblyView;
use gpui::{Pixels, Point, ScrollStrategy, SharedString, UniformListScrollHandle};

use crate::target::DebugTarget;

const LEAD_BYTES: u32 = 48;
const COUNT: usize = 200;

pub struct Menu {
    pub position: Point<Pixels>,
    pub addr: u32,
    pub address: SharedString,
    pub bytes: SharedString,
    pub text: SharedString,
}

pub struct Disasm {
    pub view: DisassemblyView,
    pub scroll: UniformListScrollHandle,
    pub menu: Option<Menu>,
    follow_pc: bool,
    show_bytes: bool,
    base: u32,
    selected: Option<u32>,
}

impl Disasm {
    pub fn new() -> Self {
        Self {
            view: DisassemblyView::default(),
            scroll: UniformListScrollHandle::new(),
            menu: None,
            follow_pc: true,
            show_bytes: false,
            base: 0,
            selected: None,
        }
    }

    pub fn follow_pc(&self) -> bool {
        self.follow_pc
    }

    pub fn show_bytes(&self) -> bool {
        self.show_bytes
    }

    pub fn recompute(&mut self, target: &dyn DebugTarget) {
        let pc = target.pc();
        let (view, base) = if self.follow_pc {
            let base = pc.saturating_sub(LEAD_BYTES);
            let view = target.disassemble(base, COUNT);

            if view.lines.iter().any(|l| l.current) {
                (view, base)
            } else {
                (target.disassemble(pc, COUNT), pc)
            }
        } else {
            (target.disassemble(self.base, COUNT), self.base)
        };

        self.base = base;
        self.view = view;
        self.view.show_bytes = self.show_bytes;
        self.apply_selection();

        if self.follow_pc
            && let Some(ix) = self.view.lines.iter().position(|l| l.current)
        {
            self.scroll.scroll_to_item_strict(ix, ScrollStrategy::Top);
        }
    }

    fn apply_selection(&mut self) {
        let selected = self.selected;
        for line in &mut self.view.lines {
            line.selected = selected == Some(line.addr);
        }
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

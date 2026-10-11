use super::regs::DisplayControl;
use super::renderer::ObjWindowLine;
use super::{Ppu, WindowView};
use crate::SCREEN_HEIGHT;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowDebugRow {
    pub(crate) windows: WindowView,
    pub(crate) display: DisplayControl,
    pub(crate) hofs: [u16; 4],
    pub(crate) vofs: [u16; 4],
    pub(crate) obj_window: ObjWindowLine,
}

impl Ppu {
    pub(crate) fn window_debug_row(&self, obj_window: ObjWindowLine) -> WindowDebugRow {
        WindowDebugRow {
            windows: self.window_view(),
            display: self.dispctrl,
            hofs: self.bghofs.map(|offset| offset.offset()),
            vofs: self.bgvofs.map(|offset| offset.offset()),
            obj_window,
        }
    }
}

#[derive(Default)]
pub(crate) struct WindowTrace {
    current: Vec<Option<WindowDebugRow>>,
    completed: Vec<Option<WindowDebugRow>>,
    generation: u64,
}

impl WindowTrace {
    pub(crate) fn record(&mut self, line: usize, row: WindowDebugRow) {
        if line == 0 {
            self.current.clear();
        }

        if self.current.len() <= line {
            self.current.resize(line + 1, None);
        }

        self.current[line] = Some(row);

        if self.completed.is_empty() {
            self.generation = self.generation.wrapping_add(1);
        }
    }

    pub(crate) fn finish_frame(&mut self) {
        if !self.current.is_empty() {
            self.current.resize(SCREEN_HEIGHT as usize, None);
            if self.current != self.completed {
                self.generation = self.generation.wrapping_add(1);
            }
            std::mem::swap(&mut self.current, &mut self.completed);
            self.current.clear();
        }
    }

    pub(crate) fn rows(&self) -> &[Option<WindowDebugRow>] {
        if self.completed.is_empty() {
            &self.current
        } else {
            &self.completed
        }
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn clear(&mut self) {
        self.current.clear();
        self.completed.clear();
        self.generation = self.generation.wrapping_add(1);
    }
}

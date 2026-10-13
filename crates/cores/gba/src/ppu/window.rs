use crate::ppu::regs::{WindowControl, WindowXOffset, WindowYOffset};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowView {
    pub(crate) enabled: [bool; 2],
    pub(crate) obj_enabled: bool,
    pub(crate) winh: [WindowXOffset; 2],
    pub(crate) winv: [WindowYOffset; 2],
    pub(crate) winin: WindowControl,
    pub(crate) winout: WindowControl,
}

impl WindowView {
    pub(crate) fn any_enabled(self) -> bool {
        self.enabled.contains(&true) || self.obj_enabled
    }

    pub(crate) fn contains(self, index: usize, x: usize, y: usize) -> bool {
        let inside = |position: usize, start: u16, end: u16| {
            let (start, end) = (usize::from(start), usize::from(end));
            if start > end {
                position >= start || position < end
            } else {
                position >= start && position < end
            }
        };

        let (horizontal, vertical) = (self.winh[index], self.winv[index]);
        self.enabled[index]
            && inside(x, horizontal.x1(), horizontal.x2())
            && inside(y, vertical.y1(), vertical.y2())
    }

    pub(crate) fn region(
        self,
        x: usize,
        y: usize,
        inside_obj_window: bool,
    ) -> Option<WindowRegion> {
        if !self.any_enabled() {
            return None;
        }

        if let Some(index) = (0..2).find(|&index| self.contains(index, x, y)) {
            return Some(WindowRegion::Rectangle(index));
        }

        Some(if self.obj_enabled && inside_obj_window {
            WindowRegion::Obj
        } else {
            WindowRegion::Outside
        })
    }

    pub(crate) fn mask(self, x: usize, y: usize, inside_obj_window: bool) -> WindowMask {
        self.region(x, y, inside_obj_window)
            .map_or(WindowMask(0x3F), |region| region.mask(self))
    }
}

#[derive(Clone, Copy)]
pub(crate) struct WindowMask(u8);

impl WindowMask {
    pub(crate) fn bg_enabled(self, index: usize) -> bool {
        index < 4 && self.0 & (1 << index) != 0
    }

    pub(crate) fn obj_enabled(self) -> bool {
        self.0 & (1 << 4) != 0
    }

    pub(crate) fn effects_enabled(self) -> bool {
        self.0 & (1 << 5) != 0
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowRegion {
    Rectangle(usize),
    Obj,
    Outside,
}

impl WindowRegion {
    pub(crate) fn mask(self, windows: WindowView) -> WindowMask {
        match self {
            Self::Rectangle(index) => WindowMask(windows.winin.raw().to_le_bytes()[index]),
            Self::Obj => WindowMask(windows.winout.raw().to_le_bytes()[1]),
            Self::Outside => WindowMask(windows.winout.raw().to_le_bytes()[0]),
        }
    }
}

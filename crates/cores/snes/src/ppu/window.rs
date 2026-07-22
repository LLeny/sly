use crate::ppu::regs::{W12SEL, WBGLOG, WH0, WH3, WOBJLOG, WindowMaskSettings};

const WINDOW_COUNT: usize = 2;
const LAYER_COUNT: usize = 6;

pub(crate) const COLOR_WINDOW: u8 = 5;

#[derive(Clone, Copy, Default)]
pub(crate) struct Windows {
    selectors: [WindowMaskSettings; 3],
    positions: [u8; 4],
    logic: [u8; 2],
}

#[derive(Clone, Copy)]
pub(crate) struct WindowRowMask {
    enabled: [bool; WINDOW_COUNT],
    inverted: [bool; WINDOW_COUNT],
    bounds: [(u8, u8); WINDOW_COUNT],
    logic: u8,
}

impl WindowRowMask {
    pub(crate) fn contains(&self, x: u8) -> bool {
        let mut areas = [false; WINDOW_COUNT];
        for window in 0..WINDOW_COUNT {
            let (left, right) = self.bounds[window];
            let inside = left <= right && (left..=right).contains(&x);
            areas[window] = inside ^ self.inverted[window];
        }

        match self.enabled {
            [false, false] => false,
            [true, false] => areas[0],
            [false, true] => areas[1],
            [true, true] => match self.logic {
                0 => areas[0] || areas[1],
                1 => areas[0] && areas[1],
                2 => areas[0] ^ areas[1],
                3 => areas[0] == areas[1],
                _ => unreachable!(),
            },
        }
    }
}

impl Windows {
    pub(crate) fn write_selector(&mut self, reg: u16, value: WindowMaskSettings) {
        self.selectors[(reg - W12SEL) as usize] = value;
    }

    pub(crate) fn selector(&self, reg: u16) -> &WindowMaskSettings {
        &self.selectors[(reg - W12SEL) as usize]
    }

    pub(crate) fn write_position(&mut self, reg: u16, value: u8) {
        self.positions[(reg - WH0) as usize] = value;
    }

    pub(crate) fn position(&self, reg: u16) -> u8 {
        self.positions[(reg - WH0) as usize]
    }

    pub(crate) fn write_logic(&mut self, reg: u16, value: u8) {
        self.logic[(reg - WBGLOG) as usize] = value;
    }

    pub(crate) fn logic(&self, reg: u16) -> u8 {
        self.logic[(reg - WBGLOG) as usize]
    }

    pub(crate) fn row_mask(&self, layer: u8) -> WindowRowMask {
        debug_assert!((layer as usize) < LAYER_COUNT);

        let settings = self.settings_for_layer(layer);
        let mut enabled = [false; WINDOW_COUNT];
        let mut inverted = [false; WINDOW_COUNT];
        let mut bounds = [(0u8, 0u8); WINDOW_COUNT];

        for window in 0..WINDOW_COUNT {
            let (window_enabled, invert) = settings[window];
            enabled[window] = window_enabled;
            inverted[window] = invert;
            bounds[window] = (self.positions[window * 2], self.positions[window * 2 + 1]);
        }

        WindowRowMask {
            enabled,
            inverted,
            bounds,
            logic: self.logic_for_layer(layer),
        }
    }

    fn settings_for_layer(&self, layer: u8) -> [(bool, bool); WINDOW_COUNT] {
        match layer {
            0 => {
                let settings = &self.selectors[0];
                [
                    (settings.enable_win1_bg1(), settings.invert_win1_bg1()),
                    (settings.enable_win2_bg1(), settings.invert_win2_bg1()),
                ]
            }
            1 => {
                let settings = &self.selectors[0];
                [
                    (settings.enable_win1_bg2(), settings.invert_win1_bg2()),
                    (settings.enable_win2_bg2(), settings.invert_win2_bg2()),
                ]
            }
            2 => {
                let settings = &self.selectors[1];
                [
                    (settings.enable_win1_bg3(), settings.invert_win1_bg3()),
                    (settings.enable_win2_bg3(), settings.invert_win2_bg3()),
                ]
            }
            3 => {
                let settings = &self.selectors[1];
                [
                    (settings.enable_win1_bg4(), settings.invert_win1_bg4()),
                    (settings.enable_win2_bg4(), settings.invert_win2_bg4()),
                ]
            }
            4 => {
                let settings = &self.selectors[2];
                [
                    (settings.enable_win1_obj(), settings.invert_win1_obj()),
                    (settings.enable_win2_obj(), settings.invert_win2_obj()),
                ]
            }
            5 => {
                let settings = &self.selectors[2];
                [
                    (settings.enable_win1_color(), settings.invert_win1_color()),
                    (settings.enable_win2_color(), settings.invert_win2_color()),
                ]
            }
            _ => unreachable!(),
        }
    }

    fn logic_for_layer(&self, layer: u8) -> u8 {
        let (register, shift) = match layer {
            0 => (0, 0),
            1 => (0, 2),
            2 => (0, 4),
            3 => (0, 6),
            4 => (1, 0),
            5 => (1, 2),
            _ => unreachable!(),
        };
        (self.logic[register] >> shift) & 0x03
    }
}

pub(crate) const POSITION_START: u16 = WH0;
pub(crate) const POSITION_END: u16 = WH3;
pub(crate) const LOGIC_START: u16 = WBGLOG;
pub(crate) const LOGIC_END: u16 = WOBJLOG;

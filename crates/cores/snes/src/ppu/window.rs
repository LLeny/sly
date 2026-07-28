use serde::{Deserialize, Serialize};

#[cfg(feature = "debug")]
use crate::ppu::regs::{BackgroundMode, LayerWindowEnable};
use crate::ppu::regs::{
    MaskLogic, W12SEL, W34SEL, WBGLOG, WH0, WH3, WOBJLOG, WOBJSEL, WindowMaskLogic,
    WindowMaskSettings,
};

const WINDOW_COUNT: usize = 2;
const LAYER_COUNT: usize = 6;

#[cfg(feature = "debug")]
type WindowDebugSettings = ([(bool, bool); WINDOW_COUNT], [(u8, u8); WINDOW_COUNT]);

pub(crate) const COLOR_WINDOW: u8 = 5;

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
pub(crate) struct Windows {
    selectors: [WindowMaskSettings; 3],
    positions: [u8; 4],
    logic: [WindowMaskLogic; 2],
}

#[cfg(feature = "debug")]
#[derive(Clone, Copy)]
pub(crate) struct WindowDebugState {
    pub(crate) windows: Windows,
    pub(crate) main_screen: LayerWindowEnable,
    pub(crate) sub_screen: LayerWindowEnable,
    pub(crate) bg_mode: BackgroundMode,
    pub(crate) extbg: bool,
}

#[cfg(feature = "debug")]
impl Default for WindowDebugState {
    fn default() -> Self {
        Self {
            windows: Windows::default(),
            main_screen: LayerWindowEnable::default(),
            sub_screen: LayerWindowEnable::default(),
            bg_mode: BackgroundMode::Mode0,
            extbg: false,
        }
    }
}

#[cfg(feature = "debug")]
#[derive(Default)]
pub(crate) struct WindowTrace {
    current: Vec<WindowDebugState>,
    completed: Vec<WindowDebugState>,
    generation: u64,
}

#[cfg(feature = "debug")]
impl WindowTrace {
    pub(crate) fn record(&mut self, scanline: usize, state: WindowDebugState) {
        if self.current.len() <= scanline {
            self.current
                .resize(scanline + 1, WindowDebugState::default());
        }
        self.current[scanline] = state;
    }

    pub(crate) fn finish_frame(&mut self) {
        if !self.current.is_empty() {
            self.completed.clone_from(&self.current);
            self.generation = self.generation.wrapping_add(1);
        }
    }

    pub(crate) fn rows(&self) -> &[WindowDebugState] {
        if self.completed.is_empty() {
            &self.current
        } else {
            &self.completed
        }
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn background_mode_mask(&self) -> u16 {
        self.rows().iter().fold(0, |mask, row| {
            let bit = match row.bg_mode {
                BackgroundMode::Invalid => 8,
                mode => mode as u8,
            };
            mask | 1 << bit
        })
    }

    pub(crate) fn uses_mode(&self, mode: BackgroundMode) -> bool {
        self.rows()
            .iter()
            .any(|row| row.bg_mode as u8 == mode as u8)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct WindowRowMask {
    enabled: [bool; WINDOW_COUNT],
    inverted: [bool; WINDOW_COUNT],
    bounds: [(u8, u8); WINDOW_COUNT],
    logic: MaskLogic,
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
                MaskLogic::Or => areas[0] || areas[1],
                MaskLogic::And => areas[0] && areas[1],
                MaskLogic::Xor => areas[0] ^ areas[1],
                MaskLogic::Xnor => areas[0] == areas[1],
            },
        }
    }

    #[cfg(feature = "debug")]
    pub(crate) fn settings(&self) -> WindowDebugSettings {
        (
            std::array::from_fn(|i| (self.enabled[i], self.inverted[i])),
            self.bounds,
        )
    }
}

impl Windows {
    pub(crate) fn write_selector(&mut self, reg: u16, value: WindowMaskSettings) {
        debug_assert!(matches!(reg, W12SEL | W34SEL | WOBJSEL));
        self.selectors[(reg - W12SEL) as usize] = value;
    }

    pub(crate) fn selector(&self, reg: u16) -> &WindowMaskSettings {
        debug_assert!(matches!(reg, W12SEL | W34SEL | WOBJSEL));
        &self.selectors[(reg - W12SEL) as usize]
    }

    pub(crate) fn write_position(&mut self, reg: u16, value: u8) {
        self.positions[(reg - WH0) as usize] = value;
    }

    pub(crate) fn position(&self, reg: u16) -> u8 {
        self.positions[(reg - WH0) as usize]
    }

    pub(crate) fn write_logic(&mut self, reg: u16, value: u8) {
        self.logic[(reg - WBGLOG) as usize] = WindowMaskLogic::from_raw(value);
    }

    pub(crate) fn logic(&self, reg: u16) -> u8 {
        self.logic[(reg - WBGLOG) as usize].raw()
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

    fn logic_for_layer(&self, layer: u8) -> MaskLogic {
        match layer {
            0 => self.logic[0].bg1(),
            1 => self.logic[0].bg2(),
            2 => self.logic[0].bg3(),
            3 => self.logic[0].bg4(),
            4 => self.logic[1].obj(),
            5 => self.logic[1].color(),
            _ => unreachable!(),
        }
    }
}

pub(crate) const POSITION_START: u16 = WH0;
pub(crate) const POSITION_END: u16 = WH3;
pub(crate) const LOGIC_START: u16 = WBGLOG;
pub(crate) const LOGIC_END: u16 = WOBJLOG;

use crate::ppu::cgram::{CHANNEL_MASK, CHANNEL_SHIFTS};
use crate::ppu::regs::{
    ColorMathAddend, ColorMathIntensity, ColorMathOperation, ColorSource, LayerMask, WindowRegion,
};
use serde::{Deserialize, Serialize};

#[chapa::bitfield(u8, order = lsb0)]
pub struct FixedColorData {
    #[bits(0..=4)]
    pub intensity: u8,

    #[bits(5)]
    pub apply_red: bool,

    #[bits(6)]
    pub apply_green: bool,

    #[bits(7)]
    pub apply_blue: bool,
}

#[derive(Default, Serialize, Deserialize)]
pub struct ColorMath {
    pub layers: LayerMask,
    pub intensity: ColorMathIntensity,
    pub operation: ColorMathOperation,
    pub fixed_color: u16,
    pub color_source: ColorSource,
    pub addend: ColorMathAddend,
    pub sub_screen_region: WindowRegion,
    pub main_screen_region: WindowRegion,
}

impl ColorMath {
    pub(crate) fn set_fixed_color(&mut self, value: u8) {
        let data = FixedColorData::from_raw(value);
        let intensity = data.intensity() as u16;
        let selected = [data.apply_red(), data.apply_green(), data.apply_blue()];

        for (apply, shift) in selected.into_iter().zip(CHANNEL_SHIFTS) {
            if apply {
                self.fixed_color = self.fixed_color & !(CHANNEL_MASK << shift) | intensity << shift;
            }
        }
    }

    pub(crate) fn apply(&self, color: u16, addend: u16, halve: bool) -> u16 {
        let mut result = 0;

        for shift in CHANNEL_SHIFTS {
            let main = color >> shift & CHANNEL_MASK;
            let addend = addend >> shift & CHANNEL_MASK;

            let mut channel = match self.operation {
                ColorMathOperation::Add => main + addend,
                ColorMathOperation::Subtract => main.saturating_sub(addend),
            };
            if halve {
                channel /= 2;
            }

            result |= channel.min(CHANNEL_MASK) << shift;
        }

        result
    }
}

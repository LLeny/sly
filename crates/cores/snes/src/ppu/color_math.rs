use crate::ppu::regs::{ColorMathIntensity, ColorMathOperation, LayerMask};

#[derive(Default)]
pub struct ColorMath {
    pub layers: LayerMask,
    pub intensity: ColorMathIntensity,
    pub operation: ColorMathOperation,
    pub fixed_color: u16,
}

impl ColorMath {
    pub(crate) fn set_fixed_color(&mut self, value: u8) {
        let intensity = (value & 0x1F) as u16;
        for (channel, shift) in [(0x20, 0u16), (0x40, 5), (0x80, 10)] {
            if value & channel != 0 {
                self.fixed_color = self.fixed_color & !(0x1F << shift) | intensity << shift;
            }
        }
    }

    pub(crate) fn apply(&self, color: u16) -> u16 {
        let mut result = 0;

        for shift in [0u16, 5, 10] {
            let main = color >> shift & 0x1F;
            let fixed = self.fixed_color >> shift & 0x1F;

            let mut channel = match self.operation {
                ColorMathOperation::Add => main + fixed,
                ColorMathOperation::Subtract => main.saturating_sub(fixed),
            };
            if let ColorMathIntensity::Half = self.intensity {
                channel /= 2;
            }

            result |= channel.min(31) << shift;
        }

        result
    }
}

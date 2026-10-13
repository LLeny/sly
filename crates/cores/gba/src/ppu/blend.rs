use crate::ppu::regs::{BlendControl, SpecialEffect};

#[derive(Clone, Copy)]
pub(crate) struct BlendView {
    pub(crate) control: BlendControl,
    pub(crate) eva: u16,
    pub(crate) evb: u16,
    pub(crate) evy: u16,
}

#[derive(Clone, Copy)]
pub(crate) struct LayerPixel {
    pub(crate) color: u16,
    pub(crate) layer: usize,
    pub(crate) priority: u8,
    pub(crate) semi_transparent: bool,
}

impl BlendView {
    pub(crate) fn apply(self, top: LayerPixel, below: Option<LayerPixel>, window_sfx: bool) -> u16 {
        let below = below.filter(|pixel| self.control.second_target(pixel.layer));
        let effect = match below {
            Some(_) if top.semi_transparent => SpecialEffect::AlphaBlend,
            _ if window_sfx && self.control.first_target(top.layer) => {
                self.control.special_effect()
            }
            _ => SpecialEffect::None,
        };

        // BGR555 <-> [r, g, b] with 5-bit channels.
        let channels = |color: u16| [color & 31, (color >> 5) & 31, (color >> 10) & 31];
        let from_channels = |[r, g, b]: [u16; 3]| r | (g << 5) | (b << 10);

        let top_channels = channels(top.color);
        match (effect, below) {
            (SpecialEffect::AlphaBlend, Some(below)) => {
                let below_channels = channels(below.color);
                from_channels(std::array::from_fn(|i| {
                    (((top_channels[i] * self.eva) + (below_channels[i] * self.evb)) >> 4).min(31)
                }))
            }
            (SpecialEffect::BrightnessIncrease, _) => {
                from_channels(top_channels.map(|c| c + (((31 - c) * self.evy) >> 4)))
            }
            (SpecialEffect::BrightnessDecrease, _) => {
                from_channels(top_channels.map(|c| c - ((c * self.evy) >> 4)))
            }
            _ => top.color,
        }
    }
}

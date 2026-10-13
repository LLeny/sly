use crate::ppu::regs::AffineParameter;
use crate::ppu::sprite::OAM_ATTR_SIZE;

#[derive(Clone, Copy)]
pub(crate) struct AffineMatrix {
    pub(crate) pa: i32,
    pub(crate) pb: i32,
    pub(crate) pc: i32,
    pub(crate) pd: i32,
}

impl AffineMatrix {
    pub(crate) fn from_oam(oam: &[u8], index: usize) -> Self {
        // each matrix spans four OAM entries, one parameter in the last halfword of each
        let base = index * 4 * OAM_ATTR_SIZE;
        let param = |n: usize| {
            let addr = base + (n * OAM_ATTR_SIZE) + 6;
            AffineParameter::from_le_bytes([oam[addr], oam[addr + 1]])
        };

        Self::from_params([param(0), param(1), param(2), param(3)])
    }

    pub(crate) fn from_params([pa, pb, pc, pd]: [AffineParameter; 4]) -> Self {
        Self {
            pa: pa.value(),
            pb: pb.value(),
            pc: pc.value(),
            pd: pd.value(),
        }
    }

    pub(crate) fn apply(self, dx: i32, dy: i32) -> (i32, i32) {
        (
            ((self.pa * dx) + (self.pb * dy)) >> 8,
            ((self.pc * dx) + (self.pd * dy)) >> 8,
        )
    }
}

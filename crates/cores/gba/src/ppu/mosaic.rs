use crate::ppu::regs::Mosaic;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct MosaicSize {
    pub(crate) width: usize,
    pub(crate) height: usize,
}

impl MosaicSize {
    pub(crate) const NONE: Self = Self {
        width: 1,
        height: 1,
    };

    fn new(hsize: u8, vsize: u8) -> Self {
        Self {
            width: hsize as usize + 1,
            height: vsize as usize + 1,
        }
    }

    pub(crate) fn x(self, x: usize) -> usize {
        x - (x % self.width)
    }

    pub(crate) fn y(self, line: usize) -> usize {
        line - (line % self.height)
    }
}

impl Mosaic {
    pub(crate) fn bg_size(self) -> MosaicSize {
        MosaicSize::new(self.bg_mosaic_hsize(), self.bg_mosaic_vsize())
    }

    pub(crate) fn obj_size(self) -> MosaicSize {
        MosaicSize::new(self.obj_mosaic_hsize(), self.obj_mosaic_vsize())
    }
}

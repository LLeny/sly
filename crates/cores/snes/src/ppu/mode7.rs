use crate::Snes;
use crate::macros::register_mmio_raw;
use crate::mmio::MemoryMap;
use crate::ppu::latch::ByteLatch;

const H_OFFSET: u16 = 0x210D;
const V_OFFSET: u16 = 0x210E;
const MATRIX_A: u16 = 0x211B;
const MATRIX_B: u16 = 0x211C;
const MATRIX_C: u16 = 0x211D;
const MATRIX_D: u16 = 0x211E;
const CENTER_X: u16 = 0x211F;
const CENTER_Y: u16 = 0x2120;

pub(crate) const PRODUCT_LOW: u16 = 0x2134;
pub(crate) const PRODUCT_MIDDLE: u16 = 0x2135;
pub(crate) const PRODUCT_HIGH: u16 = 0x2136;

register_mmio_raw! {
    H_OFFSET => "M7HOFS",
    V_OFFSET => "M7VOFS",
    MATRIX_A => "M7A",
    MATRIX_B => "M7B",
    MATRIX_C => "M7C",
    MATRIX_D => "M7D",
    CENTER_X => "M7X",
    CENTER_Y => "M7Y",
    PRODUCT_LOW => "MPYL",
    PRODUCT_MIDDLE => "MPYM",
    PRODUCT_HIGH => "MPYH",
}

#[chapa::bitfield(u16, order = lsb0)]
struct Signed13 {
    #[bits(0..=12)]
    value: i16,
}

#[derive(Default)]
pub(crate) struct Matrix {
    pub(crate) a: i16,
    pub(crate) b: i16,
    pub(crate) c: i16,
    pub(crate) d: i16,
    pub(crate) center_x: i16,
    pub(crate) center_y: i16,
    pub(crate) hofs: i16,
    pub(crate) vofs: i16,
    pub(crate) result: i32,
    latch: ByteLatch,
}

impl Matrix {
    pub(crate) fn write_scroll(&mut self, register: u16, value: u8) {
        match register {
            H_OFFSET => self.hofs = Signed13::from_raw(self.latch(value)).value(),
            V_OFFSET => self.vofs = Signed13::from_raw(self.latch(value)).value(),
            _ => {}
        }
    }

    fn latch(&mut self, value: u8) -> u16 {
        let previous = self.latch.exchange(value);
        (value as u16) << 8 | previous as u16
    }

    fn update_result(&mut self) {
        self.result = self.a as i32 * (self.b >> 8) as i32;
    }
}

pub(crate) struct Mode7Registers;

impl Mode7Registers {
    pub(crate) const START: u16 = MATRIX_A;
    pub(crate) const END: u16 = CENTER_Y;

    pub(crate) fn write<M: MemoryMap>(snes: &mut Snes<M>, register: u16, value: u8) {
        let matrix = &mut snes.ppu.matrix;
        let word = matrix.latch(value);

        match register {
            MATRIX_A => matrix.a = word as i16,
            MATRIX_B => matrix.b = word as i16,
            MATRIX_C => matrix.c = word as i16,
            MATRIX_D => matrix.d = word as i16,
            CENTER_X => matrix.center_x = Signed13::from_raw(word).value(),
            CENTER_Y => matrix.center_y = Signed13::from_raw(word).value(),
            _ => unreachable!(),
        }

        if matches!(register, MATRIX_A | MATRIX_B) {
            matrix.update_result();
        }
    }
}

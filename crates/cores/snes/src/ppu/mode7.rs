use crate::Bus;
use crate::macros::{register_mmio, register_mmio_raw};
use crate::mmio::MemoryMap;
use crate::mmio::device::IoRegister;
use crate::ppu::latch::ByteLatch;
use crate::ppu::vram::Vram;
use serde::{Deserialize, Serialize};

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

register_mmio! {
    /// M7SEL
    pub(crate) struct Mode7Settings @ 0x211A {
        #[bits(0)]
        pub flip_horizontal: bool,

        #[bits(1)]
        pub flip_vertical: bool,

        #[bits(6)]
        pub fill_with_character_zero: bool,

        #[bits(7)]
        pub disable_tilemap_repeat: bool,
    }
}

#[chapa::bitfield(u16, order = lsb0)]
struct Signed13 {
    #[bits(0..=12)]
    value: i16,
}

#[derive(Default, Serialize, Deserialize)]
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
    pub(crate) settings: u8,
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

    pub(crate) fn settings(&self) -> Mode7Settings {
        Mode7Settings::from_raw(self.settings)
    }

    pub(crate) fn scanline_origin(&self, settings: Mode7Settings, screen_y: u32) -> (i32, i32) {
        let y = if settings.flip_vertical() {
            255 - screen_y as i32
        } else {
            screen_y as i32
        };

        let a = self.a as i32;
        let b = self.b as i32;
        let c = self.c as i32;
        let d = self.d as i32;

        let center_x = self.center_x as i32;
        let center_y = self.center_y as i32;

        let h = clip_offset(self.hofs as i32 - center_x);
        let v = clip_offset(self.vofs as i32 - center_y);

        let origin_x = ((a * h) & !63) + ((b * v) & !63) + ((b * y) & !63) + (center_x << 8);
        let origin_y = ((c * h) & !63) + ((d * v) & !63) + ((d * y) & !63) + (center_y << 8);

        (origin_x, origin_y)
    }

    pub(crate) fn map_pixel(
        &self,
        settings: Mode7Settings,
        origin: (i32, i32),
        screen_x: u32,
    ) -> (i32, i32) {
        let x = if settings.flip_horizontal() {
            255 - screen_x as i32
        } else {
            screen_x as i32
        };

        (
            (origin.0 + self.a as i32 * x) >> 8,
            (origin.1 + self.c as i32 * x) >> 8,
        )
    }
}

fn clip_offset(value: i32) -> i32 {
    if value & 0x2000 != 0 {
        value | !0x03FF
    } else {
        value & 0x03FF
    }
}

pub(crate) fn mode7_character_pixel(vram: &Vram, tile: u8, x: u16, y: u16) -> u8 {
    (vram.word((tile as u16) << 6 | (y & 7) << 3 | (x & 7)) >> 8) as u8
}

impl IoRegister for Mode7Settings {
    const START: u16 = Self::ADDRESS;

    fn write<M: MemoryMap>(snes: &mut Bus<M>, _reg: u16, value: u8) {
        snes.ppu.matrix.settings = value;
    }

    fn peek<M: MemoryMap>(snes: &Bus<M>, _reg: u16) -> u8 {
        snes.ppu.matrix.settings
    }
}

pub(crate) struct Mode7Registers;

impl Mode7Registers {
    pub(crate) const START: u16 = MATRIX_A;
    pub(crate) const END: u16 = CENTER_Y;

    pub(crate) fn write<M: MemoryMap>(snes: &mut Bus<M>, register: u16, value: u8) {
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

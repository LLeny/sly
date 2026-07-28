use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

const LOW_TABLE: usize = 512;
const HIGH_TABLE: usize = 32;
const COUNTER_MASK: u16 = 0x3FF;

#[chapa::bitfield(u16, order = lsb0)]
#[derive(Default)]
pub struct OamAddress {
    #[bits(0..=7, overlay = "bytes")]
    pub low: u8,

    #[bits(8..=15, overlay = "bytes")]
    pub high: u8,

    #[bits(0..=8, overlay = "fields")]
    pub reload: u16,

    #[bits(15, overlay = "fields")]
    pub priority_rotation: bool,
}

#[chapa::bitfield(u16, order = lsb0, width = 10)]
#[derive(Default)]
pub struct OamCounter {
    #[bits(0..=9, overlay = "address")]
    pub byte_addr: u16,

    #[bits(2..=8, overlay = "priority")]
    pub obj: u8,
}

#[chapa::bitfield(u8, order = lsb0)]
pub struct ObjectAttributes {
    #[bits(0)]
    pub tile_high: bool,

    #[bits(1..=3)]
    pub palette: u8,

    #[bits(4..=5)]
    pub priority: u8,

    #[bits(6)]
    pub flip_h: bool,

    #[bits(7)]
    pub flip_v: bool,
}

#[chapa::bitfield(u8, order = lsb0)]
pub struct ObjectHighBits {
    #[bits(0)]
    pub x_high: bool,

    #[bits(1)]
    pub large: bool,
}

crate::macros::impl_bitfield_serde!(OamAddress, OamCounter);

#[derive(Serialize, Deserialize)]
pub struct Oam {
    #[serde(with = "BigArray")]
    bytes: [u8; LOW_TABLE + HIGH_TABLE],
    addr: OamAddress,
    internal: OamCounter,
    latch: u8,
}

impl Oam {
    pub fn new() -> Self {
        Self {
            bytes: [0; LOW_TABLE + HIGH_TABLE],
            addr: OamAddress::default(),
            internal: OamCounter::default(),
            latch: 0,
        }
    }

    pub fn byte(&self, index: usize) -> u8 {
        self.bytes[index % self.bytes.len()]
    }

    pub fn word_addr(&self) -> u16 {
        self.addr.raw()
    }

    pub fn priority_rotation(&self) -> bool {
        self.addr.priority_rotation()
    }

    pub fn first_sprite(&self) -> u8 {
        if self.priority_rotation() {
            self.internal.obj()
        } else {
            0
        }
    }

    pub fn attributes(&self, index: u8) -> ObjectAttributes {
        ObjectAttributes::from_raw(self.byte(index as usize * 4 + 3))
    }

    pub fn high_bits(&self, index: u8) -> ObjectHighBits {
        let packed = self.byte(LOW_TABLE + index as usize / 4);
        ObjectHighBits::from_raw(packed >> (index as usize % 4 * 2))
    }

    pub(crate) fn set_addr_low(&mut self, value: u8) {
        self.addr.set_low(value);
        self.reload();
    }

    pub(crate) fn set_addr_high(&mut self, value: u8) {
        self.addr.set_high(value);
        self.reload();
    }

    pub(crate) fn write_data(&mut self, value: u8) {
        let index = self.index();
        let addr = self.internal.byte_addr();
        if addr & 1 == 0 {
            self.latch = value;
        }
        if addr >= LOW_TABLE as u16 {
            self.bytes[index] = value;
        } else if addr & 1 == 1 {
            self.bytes[index - 1] = self.latch;
            self.bytes[index] = value;
        }
        self.increment();
    }

    pub(crate) fn read_data(&mut self) -> u8 {
        let value = self.peek_data();
        self.increment();
        value
    }

    pub(crate) fn peek_data(&self) -> u8 {
        self.bytes[self.index()]
    }

    pub(crate) fn reload(&mut self) {
        self.internal = OamCounter::from_raw(self.addr.reload() << 1);
    }

    fn index(&self) -> usize {
        let addr = self.internal.byte_addr() as usize;
        if addr < LOW_TABLE {
            addr
        } else {
            LOW_TABLE | (addr & (HIGH_TABLE - 1))
        }
    }

    fn increment(&mut self) {
        self.internal = OamCounter::from_raw((self.internal.byte_addr() + 1) & COUNTER_MASK);
    }
}

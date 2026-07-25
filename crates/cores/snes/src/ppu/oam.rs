use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

const LOW_TABLE: usize = 512;
const HIGH_TABLE: usize = 32;

#[derive(Serialize, Deserialize)]
pub struct Oam {
    #[serde(with = "BigArray")]
    bytes: [u8; LOW_TABLE + HIGH_TABLE],
    addr: u16,
    internal: u16,
    latch: u8,
}

impl Oam {
    pub fn new() -> Self {
        Self {
            bytes: [0; LOW_TABLE + HIGH_TABLE],
            addr: 0,
            internal: 0,
            latch: 0,
        }
    }

    pub fn byte(&self, index: usize) -> u8 {
        self.bytes[index % self.bytes.len()]
    }

    pub fn word_addr(&self) -> u16 {
        self.addr
    }

    pub fn priority_rotation(&self) -> bool {
        self.addr & 0x8000 != 0
    }

    pub(crate) fn set_addr_low(&mut self, value: u8) {
        self.addr = (self.addr & 0xFF00) | value as u16;
        self.reload();
    }

    pub(crate) fn set_addr_high(&mut self, value: u8) {
        self.addr = (self.addr & 0x00FF) | ((value as u16) << 8);
        self.reload();
    }

    pub(crate) fn write_data(&mut self, value: u8) {
        let index = self.index();
        if self.internal & 1 == 0 {
            self.latch = value;
        }
        if self.internal >= LOW_TABLE as u16 {
            self.bytes[index] = value;
        } else if self.internal & 1 == 1 {
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

    fn reload(&mut self) {
        self.internal = (self.addr & 0x1FF) << 1;
    }

    fn index(&self) -> usize {
        if self.internal < LOW_TABLE as u16 {
            self.internal as usize
        } else {
            LOW_TABLE | (self.internal as usize & (HIGH_TABLE - 1))
        }
    }

    fn increment(&mut self) {
        self.internal = (self.internal + 1) & 0x3FF;
    }
}

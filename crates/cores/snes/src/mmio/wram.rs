use crate::macros::register_mmio_raw;
use serde::{Deserialize, Serialize};

pub(crate) const WMDATA: u16 = 0x2180;
pub(crate) const WMADDL: u16 = 0x2181;
pub(crate) const WMADDM: u16 = 0x2182;
pub(crate) const WMADDH: u16 = 0x2183;

const WRAM_ADDRESS_MASK: u32 = 0x1_FFFF;

register_mmio_raw! {
    WMDATA => "WMDATA",
    WMADDL => "WMADDL",
    WMADDM => "WMADDM",
    WMADDH => "WMADDH",
}

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct WramPort {
    address: u32,
}

impl WramPort {
    pub(crate) fn reset(&mut self) {
        self.address = 0;
    }

    pub(crate) fn address(&self) -> u32 {
        self.address
    }

    pub(crate) fn set_low(&mut self, value: u8) {
        self.address = (self.address & 0x1_FF00) | value as u32;
    }

    pub(crate) fn set_middle(&mut self, value: u8) {
        self.address = (self.address & 0x1_00FF) | ((value as u32) << 8);
    }

    pub(crate) fn set_high(&mut self, value: u8) {
        self.address = (self.address & 0x0_FFFF) | (((value & 1) as u32) << 16);
    }

    pub(crate) fn peek(&self, wram: &[u8]) -> u8 {
        wram[self.address as usize]
    }

    pub(crate) fn read(&mut self, wram: &[u8]) -> u8 {
        let value = self.peek(wram);
        self.increment();
        value
    }

    pub(crate) fn write(&mut self, wram: &mut [u8], value: u8) {
        wram[self.address as usize] = value;
        self.increment();
    }

    fn increment(&mut self) {
        self.address = self.address.wrapping_add(1) & WRAM_ADDRESS_MASK;
    }
}

use crate::Cartridge;
use crate::mmio::map::rom_index;
use crate::mmio::{Address, AddressSpace, CartRegion};

const RAM_WINDOW_START: u16 = 0x6000;
const RAM_WINDOW_END: u16 = 0x7FFF;

pub fn cart_space(cartridge: &Cartridge, bank: u8, offset: u16) -> AddressSpace {
    match bank {
        0x00..=0x3F | 0x80..=0xBF => match offset {
            RAM_WINDOW_START..=RAM_WINDOW_END => {
                self::ram(cartridge, (offset - RAM_WINDOW_START) as usize)
            }
            _ => self::rom(
                cartridge,
                (((bank & 0x3F) as usize) << 15) | (offset & 0x7FFF) as usize,
            ),
        },
        0x40..=0x5F | 0xC0..=0xDF => self::rom(
            cartridge,
            (((bank & 0x1F) as usize) << 16) | offset as usize,
        ),
        0x60..=0x7D | 0xE0..=0xFF => {
            self::ram(cartridge, ((bank as usize) << 16) | offset as usize)
        }
        _ => AddressSpace::Unmapped,
    }
}

fn rom(cartridge: &Cartridge, index: usize) -> AddressSpace {
    AddressSpace::Cart(CartRegion::Rom, rom_index(cartridge, index))
}

fn ram(cartridge: &Cartridge, index: usize) -> AddressSpace {
    match cartridge.sram_index(index) {
        Some(index) => AddressSpace::Cart(CartRegion::Ram, index),
        None => AddressSpace::Unmapped,
    }
}

pub(crate) fn gsu_space(cartridge: &Cartridge, addr: Address) -> AddressSpace {
    let (bank, offset) = (addr.bank() as usize, addr.address() as usize);
    match addr.bank() {
        0x00..=0x3F => self::rom(cartridge, (bank << 15) | (offset & 0x7FFF)),
        0x40..=0x5F => self::rom(cartridge, ((bank & 0x1F) << 16) | offset),
        0x70 | 0x71 => self::ram(cartridge, ((bank & 1) << 16) | offset),
        _ => AddressSpace::Unmapped,
    }
}

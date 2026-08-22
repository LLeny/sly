use crate::Cartridge;
use crate::mmio::map::rom_index;
use crate::mmio::{AddressSpace, CartRegion};
use crate::sa1::Sa1;

const ROM_BLOCK_SIZE: usize = 0x10_0000;
const BWRAM_MASK: usize = 0x3_FFFF;

pub(crate) fn cart_space(
    cartridge: &Cartridge,
    sa1: Option<&Sa1>,
    bank: u8,
    offset: u16,
) -> AddressSpace {
    if let Some(index) = bwram_offset(sa1, bank, offset) {
        return AddressSpace::Cart(CartRegion::Ram, index);
    }

    let (window, in_block, hi_rom) = match bank {
        0x00..=0x3F | 0x80..=0xBF if offset >= 0x8000 => (
            lo_rom_window(bank),
            (usize::from(bank & 0x1F) << 15) | usize::from(offset & 0x7FFF),
            false,
        ),
        0xC0..=0xFF => (
            (bank - 0xC0) >> 4,
            (usize::from(bank & 0x0F) << 16) | usize::from(offset),
            true,
        ),
        _ => return AddressSpace::Unmapped,
    };

    let block = match sa1 {
        Some(sa1) if hi_rom || sa1.memory.lo_rom_enabled(window) => sa1.memory.rom_block(window),
        _ => window,
    };
    let index = (usize::from(block) * ROM_BLOCK_SIZE) | in_block;

    AddressSpace::Rom(rom_index(cartridge, index))
}

fn lo_rom_window(bank: u8) -> u8 {
    match bank {
        0x00..=0x3F => bank >> 5,
        0x80..=0xBF => (bank - 0x40) >> 5,
        _ => unreachable!("LoROM window called with bank {bank:02X}"),
    }
}

fn bwram_offset(sa1: Option<&Sa1>, bank: u8, offset: u16) -> Option<usize> {
    match bank {
        0x00..=0x3F | 0x80..=0xBF if (0x6000..=0x7FFF).contains(&offset) => {
            let block = sa1.map_or(0, |sa1| sa1.memory.bwram_snes_block());
            Some((block << 13) | usize::from(offset - 0x6000))
        }
        0x40..=0x4F => Some(((usize::from(bank - 0x40) << 16) | usize::from(offset)) & BWRAM_MASK),
        _ => None,
    }
}

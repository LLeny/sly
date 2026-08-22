use crate::Cartridge;
use crate::mmio::map::rom_index;
use crate::mmio::{AddressSpace, CartRegion};
use crate::sa1::{MemoryControl, SNES_IRAM_END, SNES_IRAM_START, Sa1};

const ROM_BLOCK_SIZE: usize = 0x10_0000;
const BWRAM_MASK: usize = 0x3_FFFF;

pub(crate) fn cart_space(
    cartridge: &Cartridge,
    sa1: Option<&Sa1>,
    bank: u8,
    offset: u16,
) -> AddressSpace {
    if let Some(index) = snes_bwram_offset(sa1, bank, offset) {
        return AddressSpace::Cart(CartRegion::Ram, index);
    }

    rom_space(cartridge, sa1.map(|sa1| &sa1.memory), bank, offset)
}

pub(crate) fn cpu_space(
    cartridge: &Cartridge,
    memory: &MemoryControl,
    bank: u8,
    offset: u16,
) -> AddressSpace {
    if matches!(bank, 0x00..=0x3F | 0x80..=0xBF) {
        match offset {
            0x0000..=0x07FF => return AddressSpace::Iram(usize::from(offset)),
            0x2200..=0x23FF => return AddressSpace::Io(offset),
            SNES_IRAM_START..=SNES_IRAM_END => {
                return AddressSpace::Iram(usize::from(offset - SNES_IRAM_START));
            }
            _ => {}
        }
    }

    if let Some(index) = sa1_bwram_offset(memory, bank, offset) {
        return AddressSpace::Cart(CartRegion::Ram, index);
    }

    rom_space(cartridge, Some(memory), bank, offset)
}

fn rom_space(
    cartridge: &Cartridge,
    memory: Option<&MemoryControl>,
    bank: u8,
    offset: u16,
) -> AddressSpace {
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

    let block = match memory {
        Some(memory) if hi_rom || memory.lo_rom_enabled(window) => memory.rom_block(window),
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

fn snes_bwram_offset(sa1: Option<&Sa1>, bank: u8, offset: u16) -> Option<usize> {
    let block = Some(sa1.map_or(0, |sa1| sa1.memory.bwram_snes_block()));
    bwram_offset(block, bank, offset)
}

fn sa1_bwram_offset(memory: &MemoryControl, bank: u8, offset: u16) -> Option<usize> {
    let block = (!memory.bwram_sa1_bitmap()).then(|| memory.bwram_sa1_block());
    bwram_offset(block, bank, offset)
}

fn bwram_offset(block: Option<usize>, bank: u8, offset: u16) -> Option<usize> {
    match bank {
        0x00..=0x3F | 0x80..=0xBF if (0x6000..=0x7FFF).contains(&offset) => {
            block.map(|block| (block << 13) | usize::from(offset - 0x6000))
        }
        0x40..=0x4F => Some(((usize::from(bank - 0x40) << 16) | usize::from(offset)) & BWRAM_MASK),
        _ => None,
    }
}

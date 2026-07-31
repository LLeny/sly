use crate::Cartridge;
use crate::cartridge::MapMode;
use crate::mmio::{Address, AddressSpace, device};

pub(crate) const WAITSTATES_FAST: u64 = 6;
const WAITSTATES_SLOW: u64 = 8;
const WAITSTATES_XSLOW: u64 = 12;

pub trait MemoryMap: Default + 'static {
    const WRAM_SIZE: usize;

    fn map(&self, cartridge: &Cartridge, addr: Address) -> AddressSpace;

    fn speed(&self, addr: Address, fastrom: bool) -> u64 {
        let _ = (addr, fastrom);
        WAITSTATES_FAST
    }
}

#[derive(Default)]
pub struct CartridgeMap;

impl MemoryMap for CartridgeMap {
    const WRAM_SIZE: usize = 0x2_0000;

    fn speed(&self, addr: Address, fastrom: bool) -> u64 {
        match addr.bank() {
            0x00..=0x3F => self::system_speed(addr.address(), false),
            0x80..=0xBF => self::system_speed(addr.address(), fastrom),
            0x40..=0x7F => WAITSTATES_SLOW,
            0xC0..=0xFF => self::rom_speed(fastrom),
        }
    }

    fn map(&self, cartridge: &Cartridge, addr: Address) -> AddressSpace {
        let bank = addr.bank();
        let offset = addr.address();

        if let Some(space) = map_sram(cartridge, bank, offset) {
            return space;
        }

        match bank {
            // "The first 8 KiB of RAM is mirrored into many banks for convenient access,
            // and banks $7E-$7F provide continuous access to the entire 128 KiB in
            // one continuous address range."
            0x7E | 0x7F => {
                let index = (((bank - 0x7E) as usize) << 16) | offset as usize;
                AddressSpace::Wram(index)
            }
            // $00-$3F (and $80-$BF mirror) are Low RAM
            // https://snes.nesdev.org/wiki/Memory_map
            0x00..=0x3F | 0x80..=0xBF => match offset {
                0x0000..=0x1FFF => AddressSpace::Wram(offset as usize), // 8 KiB WRAM
                device::IO_BASE..=device::IO_END => AddressSpace::Io(offset), // PPU/APU/CPU/DMA MMIO
                0x6000..=0x7FFF => AddressSpace::Unmapped,                    // TODO: Expansion?
                0x8000..=0xFFFF => self::map_rom(cartridge, bank, offset),    // cartridge ROM
            },
            // Rest should be ROM
            _ => self::map_rom(cartridge, bank, offset),
        }
    }
}

/// Translate the standard SRAM windows.
fn map_sram(cartridge: &Cartridge, bank: u8, offset: u16) -> Option<AddressSpace> {
    let size = cartridge.sram.len();
    if size == 0 {
        return None;
    }

    let raw_index = match cartridge.map_mode()? {
        MapMode::LoRom if matches!(bank, 0x70..=0x7D | 0xF0..=0xFF) && offset <= 0x7FFF => {
            (((bank & 0x0F) as usize) << 15) | offset as usize
        }
        MapMode::HiRom | MapMode::ExHiRom
            if matches!(bank, 0x20..=0x3F | 0xA0..=0xBF) && (0x6000..=0x7FFF).contains(&offset) =>
        {
            (((bank & 0x1F) as usize) << 13) | (offset as usize - 0x6000)
        }
        _ => return None,
    };

    Some(AddressSpace::Sram(raw_index % size))
}

/// Waitstates within the system banks $00-$3F and $80-$BF.
fn system_speed(offset: u16, fastrom: bool) -> u64 {
    match offset {
        0x0000..=0x1FFF => WAITSTATES_SLOW,  // WRAM mirror
        0x2000..=0x3FFF => WAITSTATES_FAST,  // B bus I/O
        0x4000..=0x41FF => WAITSTATES_XSLOW, // joypad I/O
        0x4200..=0x5FFF => WAITSTATES_FAST,  // internal CPU I/O
        0x6000..=0x7FFF => WAITSTATES_SLOW,  // expansion
        0x8000..=0xFFFF => self::rom_speed(fastrom),
    }
}

fn rom_speed(fastrom: bool) -> u64 {
    if fastrom {
        WAITSTATES_FAST
    } else {
        WAITSTATES_SLOW
    }
}

/// Translate a cartridge address to a ROM byte index per the map mode.
fn map_rom(cartridge: &Cartridge, bank: u8, offset: u16) -> AddressSpace {
    let Some(map_mode) = cartridge.map_mode() else {
        return AddressSpace::Unmapped;
    };

    let rom_addr = match map_mode {
        // HiROM: each bank maps a full 64 KiB window of ROM.
        MapMode::HiRom | MapMode::ExHiRom => (((bank & 0x3F) as usize) << 16) | offset as usize,
        // LoROM (TODO: and others?): a 32 KiB window per bank.
        _ => (((bank & 0x7F) as usize) << 15) | (offset & 0x7FFF) as usize,
    };

    AddressSpace::Rom(match cartridge.rom_mask() {
        Some(mask) => rom_addr & mask,
        None => rom_addr % cartridge.rom.len(),
    })
}

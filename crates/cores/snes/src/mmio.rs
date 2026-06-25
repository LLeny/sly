pub mod dma;
pub mod regs;

use crate::cartridge::MapMode;
use crate::mmio::regs::{DmaTransferStart, HdmaTransferEnable, NmiTimerEnable};
use crate::{Cartridge, Snes};

const WRAM_SIZE: usize = 0x2_0000;
const OPEN_BUS: u8 = 0xFF;

pub struct Mmio {
    pub cartridge: Cartridge,
    pub wram: Box<[u8]>,
}

pub(crate) enum AddressSpace {
    Wram(usize),
    Rom(usize),
    Io(u16),
    Unmapped,
}

impl Mmio {
    pub fn new(cartridge: Cartridge) -> Self {
        Self {
            cartridge,
            wram: vec![0; WRAM_SIZE].into_boxed_slice(),
        }
    }

    /// Translate a CPU address to an address space (WRAM, ROM, I/O or unmapped).
    pub(crate) fn map(&self, addr: u32) -> AddressSpace {
        let bank = (addr >> 16) as u8;
        let offset = addr as u16;

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
                0x2000..=0x5FFF => AddressSpace::Io(offset),            // PPU/APU/CPU/DMA MMIO
                0x6000..=0x7FFF => AddressSpace::Unmapped,              // TODO: Expansion?
                0x8000..=0xFFFF => self.map_rom(bank, offset),          // cartridge ROM
            },
            // Rest should be ROM
            _ => self.map_rom(bank, offset),
        }
    }

    /// Translate a cartridge address to a ROM byte index per the map mode.
    fn map_rom(&self, bank: u8, offset: u16) -> AddressSpace {
        let rom = &self.cartridge.rom;

        let map_mode = match self.cartridge.memory_map() {
            Ok(mode) => mode.map_mode(),
            Err(_) => return AddressSpace::Unmapped,
        };

        let rom_addr = match map_mode {
            // HiROM: each bank maps a full 64 KiB window of ROM.
            MapMode::HiRom | MapMode::ExHiRom => (((bank & 0x3F) as usize) << 16) | offset as usize,
            // LoROM (TODO: and others?): a 32 KiB window per bank.
            _ => (((bank & 0x7F) as usize) << 15) | (offset & 0x7FFF) as usize,
        };

        AddressSpace::Rom(rom_addr % rom.len())
    }
}

impl Snes {
    pub fn read8(&self, bank: u8, addr: u16) -> u8 {
        self.read(((bank as u32) << 16) | addr as u32)
    }

    pub fn read16(&self, bank: u8, addr: u16) -> u16 {
        let base = ((bank as u32) << 16) | addr as u32;
        let lo = self.read(base) as u16;
        let hi = self.read(base.wrapping_add(1)) as u16;
        lo | (hi << 8)
    }

    pub fn read(&self, addr: u32) -> u8 {
        match self.mmu.map(addr) {
            AddressSpace::Wram(index) => self.mmu.wram[index],
            AddressSpace::Rom(index) => self.mmu.cartridge.rom[index],
            AddressSpace::Io(reg) => self.read_io(reg),
            AddressSpace::Unmapped => {
                tracing::error!(addr = format!("{:06X}", addr), "Read from unmapped address");
                OPEN_BUS
            }
        }
    }

    pub fn write(&mut self, addr: u32, value: u8) {
        match self.mmu.map(addr) {
            AddressSpace::Wram(index) => self.mmu.wram[index] = value,
            AddressSpace::Rom(_) => {
                tracing::warn!(
                    addr = format!("{:06X}", addr),
                    value = format!("{:02X}", value),
                    "Write to ROM"
                );
            }
            AddressSpace::Io(reg) => self.write_io(reg, value),
            AddressSpace::Unmapped => {
                tracing::error!(
                    addr = format!("{:06X}", addr),
                    value = format!("{:02X}", value),
                    "Write to unmapped address"
                );
            }
        }
    }

    fn read_io(&self, reg: u16) -> u8 {
        match reg {
            NmiTimerEnable::ADDRESS => self.irq.nmitimen.raw(),
            DmaTransferStart::ADDRESS => self.dma.mdmaen.raw(),
            HdmaTransferEnable::ADDRESS => self.dma.hdmaen.raw(),
            _ => {
                tracing::error!(
                    reg = format!("{:04X}", reg),
                    "Read from unmapped I/O register"
                );
                OPEN_BUS
            }
        }
    }

    fn write_io(&mut self, reg: u16, value: u8) {
        match reg {
            NmiTimerEnable::ADDRESS => self.irq.nmitimen = NmiTimerEnable::from_raw(value),
            DmaTransferStart::ADDRESS => self.dma.mdmaen = DmaTransferStart::from_raw(value),
            HdmaTransferEnable::ADDRESS => self.dma.hdmaen = HdmaTransferEnable::from_raw(value),
            _ => {
                tracing::error!(
                    reg = format!("{:04X}", reg),
                    value = format!("{:02X}", value),
                    "Write to unmapped I/O register"
                );
            }
        }
    }
}

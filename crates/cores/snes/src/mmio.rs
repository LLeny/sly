pub mod addr;
pub mod dma;
pub mod regs;

#[cfg(feature = "debug")]
mod access;
mod symbol;

#[cfg(feature = "debug")]
pub use access::{AccessTracker, MemoryAccess};

use crate::cartridge::MapMode;
use crate::mmio::regs::{
    APU_IO_END, APU_IO_START, DmaTransferStart, HdmaTransferEnable, NmiTimerEnable,
};
use crate::{Cartridge, Snes};

pub use addr::Address;
pub(crate) use symbol::MmioReg;
pub use symbol::{MmioSymbol, address_of, is_symbol, symbol};

const WRAM_SIZE: usize = 0x2_0000;
pub(crate) const OPEN_BUS: u8 = 0xFF;

pub struct Mmio {
    pub cartridge: Cartridge,
    pub wram: Box<[u8]>,

    #[cfg(feature = "debug")]
    pub access: AccessTracker,
}

pub(crate) enum AddressSpace {
    Wram(usize),
    Rom(usize),
    Io(u16),
    Unmapped,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum MemKind {
    Wram,
    Rom,
    Io,
}

fn space_key(space: &AddressSpace) -> Option<(MemKind, u32)> {
    match space {
        AddressSpace::Wram(index) => Some((MemKind::Wram, *index as u32)),
        AddressSpace::Rom(index) => Some((MemKind::Rom, *index as u32)),
        AddressSpace::Io(reg) => Some((MemKind::Io, *reg as u32)),
        AddressSpace::Unmapped => None,
    }
}

impl Mmio {
    pub fn new(cartridge: Cartridge) -> Self {
        Self {
            cartridge,
            wram: vec![0; WRAM_SIZE].into_boxed_slice(),

            #[cfg(feature = "debug")]
            access: AccessTracker::default(),
        }
    }

    /// Translate a CPU address to an address space (WRAM, ROM, I/O or unmapped).
    pub(crate) fn map(&self, addr: Address) -> AddressSpace {
        let bank = addr.bank();
        let offset = addr.address();

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
    pub fn read16(&self, addr: Address) -> u16 {
        let lo = self.read8(addr) as u16;
        let hi = self.read8(addr.wrapping_add(1)) as u16;
        lo | (hi << 8)
    }

    pub fn write16(&mut self, addr: Address, value: u16) {
        self.write8(addr, value as u8);
        self.write8(addr.wrapping_add(1), (value >> 8) as u8);
    }

    /// Record a captured read/write against the access tracker (debug builds only).
    #[cfg(feature = "debug")]
    fn record_access(&self, space: &AddressSpace, write: bool) {
        if self.mmu.access.capturing()
            && let Some(key) = space_key(space)
        {
            if write {
                self.mmu.access.record_write(key);
            } else {
                self.mmu.access.record_read(key);
            }
        }
    }

    pub fn read8(&self, addr: Address) -> u8 {
        let space = self.mmu.map(addr);

        #[cfg(feature = "debug")]
        self.record_access(&space, false);

        match space {
            AddressSpace::Wram(index) => self.mmu.wram[index],
            AddressSpace::Rom(index) => self.mmu.cartridge.rom[index],
            AddressSpace::Io(reg) => self.read_io(reg),
            AddressSpace::Unmapped => {
                tracing::error!(addr = format!("{addr:?}"), "Read from unmapped address");
                OPEN_BUS
            }
        }
    }

    pub fn classify(&self, addr: Address) -> Option<(MemKind, u32)> {
        space_key(&self.mmu.map(addr))
    }

    pub fn peek8(&self, addr: Address) -> u8 {
        match self.mmu.map(addr) {
            AddressSpace::Wram(index) => self.mmu.wram[index],
            AddressSpace::Rom(index) => self.mmu.cartridge.rom[index],
            AddressSpace::Io(reg) => self.peek_io(reg),
            AddressSpace::Unmapped => OPEN_BUS,
        }
    }

    pub fn write8(&mut self, addr: Address, value: u8) {
        let space = self.mmu.map(addr);

        #[cfg(feature = "debug")]
        self.record_access(&space, true);

        match space {
            AddressSpace::Wram(index) => self.mmu.wram[index] = value,
            AddressSpace::Rom(_) => {
                tracing::warn!(
                    addr = format!("{addr:?}"),
                    value = format!("{:02X}", value),
                    "Write to ROM"
                );
            }
            AddressSpace::Io(reg) => self.write_io(reg, value),
            AddressSpace::Unmapped => {
                tracing::error!(
                    addr = format!("{addr:?}"),
                    value = format!("{:02X}", value),
                    "Write to unmapped address"
                );
            }
        }
    }

    fn read_io(&self, reg: u16) -> u8 {
        match reg {
            NmiTimerEnable::ADDRESS => NmiTimerEnable::read(self),
            DmaTransferStart::ADDRESS => DmaTransferStart::read(self),
            HdmaTransferEnable::ADDRESS => HdmaTransferEnable::read(self),
            APU_IO_START..=APU_IO_END => {
                tracing::warn!(reg = format!("{:04X}", reg), "APU I/O read not implemented");
                self.apu.ports[(reg - APU_IO_START) as usize]
            }
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
            NmiTimerEnable::ADDRESS => NmiTimerEnable::write(self, value),
            DmaTransferStart::ADDRESS => DmaTransferStart::write(self, value),
            HdmaTransferEnable::ADDRESS => HdmaTransferEnable::write(self, value),
            APU_IO_START..=APU_IO_END => {
                tracing::warn!(
                    reg = format!("{:04X}", reg),
                    value = format!("{:02X}", value),
                    "APU I/O write not implemented"
                );
                self.apu.ports[(reg - APU_IO_START) as usize] = value;
            }
            _ => {
                tracing::error!(
                    reg = format!("{:04X}", reg),
                    value = format!("{:02X}", value),
                    "Write to unmapped I/O register"
                );
            }
        }
    }

    fn peek_io(&self, reg: u16) -> u8 {
        match reg {
            NmiTimerEnable::ADDRESS => NmiTimerEnable::peek(self),
            DmaTransferStart::ADDRESS => DmaTransferStart::peek(self),
            HdmaTransferEnable::ADDRESS => HdmaTransferEnable::peek(self),
            APU_IO_START..=APU_IO_END => self.apu.ports[(reg - APU_IO_START) as usize],
            _ => OPEN_BUS,
        }
    }
}

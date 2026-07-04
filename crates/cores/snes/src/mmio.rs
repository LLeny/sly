pub mod addr;
pub mod dma;
pub mod map;
pub mod regs;

#[cfg(feature = "debug")]
mod access;
mod symbol;

#[cfg(feature = "debug")]
pub use access::{AccessTracker, MemoryAccess};

use crate::joypad::{JOYPAD_IO_END, JOYPAD_IO_START};
use crate::mmio::dma::{CHANNEL_REGS_END, CHANNEL_REGS_START, Dma};
use crate::mmio::regs::{
    APU_IO_END, APU_IO_START, HdmaTransferEnable, HvStatusJoypadBusy, MdmaTransferStart,
    NmiTimerEnable, VblankFlagCpuVersion,
};
use crate::ppu::regs::{Bg12ChrBase, Bg34ChrBase, CGADD, CGDATA, RDCGRAM};
use crate::{Cartridge, Snes};

pub use addr::Address;
pub use chapa::{EnumInfo, FieldInfo, FieldKind};
pub use map::{CartridgeMap, MemoryMap};
pub(crate) use symbol::MmioReg;
pub use symbol::{MmioSymbol, address_of, is_symbol, symbol, symbols};

pub(crate) const OPEN_BUS: u8 = 0xFF;

pub struct Mmio<M: MemoryMap = CartridgeMap> {
    pub cartridge: Cartridge,
    pub wram: Box<[u8]>,
    map: M,

    #[cfg(feature = "debug")]
    pub access: AccessTracker,
}

pub enum AddressSpace {
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
    Spc,
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
        Self::with_map(cartridge, CartridgeMap)
    }
}

impl<M: MemoryMap> Mmio<M> {
    pub fn with_map(cartridge: Cartridge, map: M) -> Self {
        Self {
            cartridge,
            wram: vec![0; M::WRAM_SIZE].into_boxed_slice(),
            map,

            #[cfg(feature = "debug")]
            access: AccessTracker::default(),
        }
    }

    pub fn reset(&mut self) {
        self.wram.fill(0);

        #[cfg(feature = "debug")]
        {
            self.access = AccessTracker::default();
        }
    }

    /// Translate a CPU address to an address space (WRAM, ROM, I/O or unmapped).
    pub(crate) fn map(&self, addr: Address) -> AddressSpace {
        self.map.map(&self.cartridge, addr)
    }
}

impl<M: MemoryMap> Snes<M> {
    pub fn read16(&mut self, addr: Address) -> u16 {
        let lo = self.read8(addr) as u16;
        let hi = self.read8(addr.wrapping_add(1)) as u16;
        lo | (hi << 8)
    }

    pub fn read24(&mut self, addr: Address) -> u32 {
        let lo = self.read16(addr) as u32;
        let hi = self.read8(addr.wrapping_add(2)) as u32;
        lo | (hi << 16)
    }

    pub fn peek16(&self, addr: Address) -> u16 {
        let lo = self.peek8(addr) as u16;
        let hi = self.peek8(addr.wrapping_add(1)) as u16;
        lo | (hi << 8)
    }

    pub fn peek24(&self, addr: Address) -> u32 {
        let lo = self.peek16(addr) as u32;
        let hi = self.peek8(addr.wrapping_add(2)) as u32;
        lo | (hi << 16)
    }

    #[cfg(feature = "debug")]
    fn record_access(&self, space: &AddressSpace, write: bool) {
        if self.mmu.access.capturing()
            && let Some(key) = self::space_key(space)
        {
            if write {
                self.mmu.access.record_write(key);
            } else {
                self.mmu.access.record_read(key);
            }
        }
    }

    pub fn read8(&mut self, addr: Address) -> u8 {
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
        self::space_key(&self.mmu.map(addr))
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

    fn read_io(&mut self, reg: u16) -> u8 {
        match reg {
            Bg12ChrBase::ADDRESS => Bg12ChrBase::read(self),
            Bg34ChrBase::ADDRESS => Bg34ChrBase::read(self),
            CGADD => self.ppu.cgram.word_addr(),
            CGDATA => self.ppu.cgram.peek_data(),
            RDCGRAM => self.ppu.cgram.read_data(),
            NmiTimerEnable::ADDRESS => NmiTimerEnable::read(self),
            VblankFlagCpuVersion::ADDRESS => VblankFlagCpuVersion::read(self),
            HvStatusJoypadBusy::ADDRESS => HvStatusJoypadBusy::read(self),
            MdmaTransferStart::ADDRESS => MdmaTransferStart::read(self),
            HdmaTransferEnable::ADDRESS => HdmaTransferEnable::read(self),
            CHANNEL_REGS_START..=CHANNEL_REGS_END => {
                self.dma.channels[Dma::channel_index(reg)].read(reg)
            }
            APU_IO_START..=APU_IO_END => self.apu.cpu_read_port((reg - APU_IO_START) as usize),
            JOYPAD_IO_START..=JOYPAD_IO_END => self.joypads.read(reg),
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
            Bg12ChrBase::ADDRESS => Bg12ChrBase::write(self, value),
            Bg34ChrBase::ADDRESS => Bg34ChrBase::write(self, value),
            CGADD => self.ppu.cgram.set_addr(value),
            CGDATA => self.ppu.cgram.write_data(value),
            RDCGRAM => {}
            NmiTimerEnable::ADDRESS => NmiTimerEnable::write(self, value),
            VblankFlagCpuVersion::ADDRESS => VblankFlagCpuVersion::write(self, value),
            HvStatusJoypadBusy::ADDRESS => HvStatusJoypadBusy::write(self, value),
            MdmaTransferStart::ADDRESS => MdmaTransferStart::write(self, value),
            HdmaTransferEnable::ADDRESS => HdmaTransferEnable::write(self, value),
            CHANNEL_REGS_START..=CHANNEL_REGS_END => {
                self.dma.channels[Dma::channel_index(reg)].write(reg, value)
            }
            APU_IO_START..=APU_IO_END => self
                .apu
                .cpu_write_port((reg - APU_IO_START) as usize, value),
            _ => {
                tracing::error!(
                    reg = format!("{:04X}", reg),
                    value = format!("{:02X}", value),
                    "Write to unmapped I/O register"
                );
            }
        }
    }

    /// Guaranteed to be side effect free.
    pub fn peek_mmio(&self, reg: u16) -> u8 {
        self.peek_io(reg)
    }

    fn peek_io(&self, reg: u16) -> u8 {
        match reg {
            Bg12ChrBase::ADDRESS => Bg12ChrBase::peek(self),
            Bg34ChrBase::ADDRESS => Bg34ChrBase::peek(self),
            CGADD => self.ppu.cgram.word_addr(),
            CGDATA | RDCGRAM => self.ppu.cgram.peek_data(),
            NmiTimerEnable::ADDRESS => NmiTimerEnable::peek(self),
            VblankFlagCpuVersion::ADDRESS => VblankFlagCpuVersion::peek(self),
            HvStatusJoypadBusy::ADDRESS => HvStatusJoypadBusy::peek(self),
            MdmaTransferStart::ADDRESS => MdmaTransferStart::peek(self),
            HdmaTransferEnable::ADDRESS => HdmaTransferEnable::peek(self),
            CHANNEL_REGS_START..=CHANNEL_REGS_END => {
                self.dma.channels[Dma::channel_index(reg)].read(reg)
            }
            APU_IO_START..=APU_IO_END => self.apu.cpu_read_port((reg - APU_IO_START) as usize),
            JOYPAD_IO_START..=JOYPAD_IO_END => self.joypads.read(reg),
            _ => OPEN_BUS,
        }
    }
}

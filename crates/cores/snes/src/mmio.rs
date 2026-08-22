pub mod addr;
pub mod cpu_io;
pub mod device;
pub mod dma;
pub mod map;
pub mod regs;
pub(crate) mod wram;

#[cfg(feature = "debug")]
mod access;
mod symbol;

#[cfg(feature = "debug")]
pub use access::{AccessTracker, MemoryAccess, TrackMode};

use crate::cartridge::Coprocessor;
use crate::mmio::wram::WramPort;
use crate::sa1::Sa1;
use crate::{Bus, Cartridge};
use serde::{Deserialize, Serialize};

pub use addr::Address;
pub use chapa::{EnumInfo, FieldInfo, FieldKind};
pub(crate) use device::{DeviceTable, RegEntry};
pub use device::{IO_BASE, IO_END};
pub use map::{CartridgeMap, MemoryMap};
pub use symbol::{MmioSymbol, address_of, is_symbol, symbol, symbols};

pub(crate) fn coprocessor_devices<M: MemoryMap>(cartridge: &Cartridge) -> Vec<RegEntry<M>> {
    match cartridge.coprocessor() {
        crate::cartridge::Coprocessor::SuperFx => crate::superfx::io::registers(),
        crate::cartridge::Coprocessor::Sa1 => crate::sa1::io::registers(),
        _ => Vec::new(),
    }
}

pub(crate) fn builtin_devices<M: MemoryMap>() -> Vec<RegEntry<M>> {
    crate::ppu::io::registers()
        .into_iter()
        .chain(crate::apu::io::registers())
        .chain(crate::mmio::wram::registers())
        .chain(crate::mmio::cpu_io::registers())
        .chain(crate::math::registers())
        .chain(crate::mmio::dma::registers())
        .chain(crate::joypad::registers())
        .collect()
}

pub(crate) const OPEN_BUS: u8 = 0xFF;
pub(crate) const WRIO: u16 = 0x4201;
pub(crate) const RDIO: u16 = 0x4213;

#[derive(Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Mmio<M: MemoryMap = CartridgeMap> {
    pub cartridge: Cartridge,
    pub wram: Box<[u8]>,
    #[serde(skip)]
    map: M,

    #[serde(skip)]
    devices: DeviceTable<M>,

    pub fastrom: bool,
    wrio: u8,
    pub(crate) wram_port: WramPort,
    pub clocks: u64,

    #[cfg(feature = "debug")]
    #[serde(skip)]
    pub access: AccessTracker,
}

pub enum AddressSpace {
    Wram(usize),
    Iram(usize), // SA-1
    Sram(usize),
    Rom(usize),
    Cart(CartRegion, usize),
    Io(u16),
    Unmapped,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CartRegion {
    Rom,
    Ram,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum MemKind {
    Wram,
    Iram,
    Sram,
    Rom,
    Io,
    Spc,
}

fn space_key(space: &AddressSpace) -> Option<(MemKind, u32)> {
    let (kind, index) = match space {
        AddressSpace::Wram(index) => (MemKind::Wram, *index),
        AddressSpace::Iram(index) => (MemKind::Iram, *index),
        AddressSpace::Sram(index) | AddressSpace::Cart(CartRegion::Ram, index) => {
            (MemKind::Sram, *index)
        }
        AddressSpace::Rom(index) | AddressSpace::Cart(CartRegion::Rom, index) => {
            (MemKind::Rom, *index)
        }
        AddressSpace::Io(reg) => return Some((MemKind::Io, u32::from(*reg))),
        AddressSpace::Unmapped => return None,
    };

    Some((kind, u32::try_from(index).ok()?))
}

impl Mmio {
    pub fn new(cartridge: Cartridge) -> Self {
        Self::with_map(cartridge, CartridgeMap)
    }
}

impl<M: MemoryMap> Mmio<M> {
    pub fn with_map(cartridge: Cartridge, map: M) -> Self {
        Self {
            devices: DeviceTable::for_cartridge(&cartridge),
            cartridge,
            wram: vec![0; M::WRAM_SIZE].into_boxed_slice(),
            map,
            fastrom: false,
            wrio: 0xFF,
            wram_port: WramPort::default(),
            clocks: 0,

            #[cfg(feature = "debug")]
            access: AccessTracker::default(),
        }
    }

    pub fn reset(&mut self) {
        self.wram.fill(0);
        self.fastrom = false;
        self.wrio = 0xFF;
        self.wram_port.reset();
        self.clocks = 0;

        #[cfg(feature = "debug")]
        {
            self.access = AccessTracker::default();
        }
    }

    /// Bit 7 of WRIO drives the PPU's external counter-latch pin.
    pub(crate) fn latch_pin_high(&self) -> bool {
        self.wrio & 0x80 != 0
    }

    /// Translate a CPU address to an address space (WRAM, ROM, I/O or unmapped).
    pub(crate) fn map(&self, sa1: &Sa1, addr: Address) -> AddressSpace {
        self.map.map_with_sa1(&self.cartridge, sa1, addr)
    }

    pub(crate) fn cart_read(&self, region: CartRegion, index: usize) -> u8 {
        match region {
            CartRegion::Rom => self.cartridge.rom[index],
            CartRegion::Ram => self.cartridge.sram[index],
        }
    }

    pub(crate) fn cart_write(&mut self, region: CartRegion, index: usize, value: u8) {
        match region {
            CartRegion::Ram => self.cartridge.sram[index] = value,
            CartRegion::Rom => tracing::warn!(index, "Write to cartridge ROM"),
        }
    }

    /// Waitstate clocks an access to `addr` adds beyond the fast cycle.
    pub(crate) fn waitstates(&self, addr: Address) -> u64 {
        self.map.speed(addr, self.fastrom) - map::WAITSTATES_FAST
    }
}

impl<M: MemoryMap> Bus<M> {
    pub(crate) fn rebuild_devices(&mut self) {
        self.mmu.devices = DeviceTable::for_cartridge(&self.mmu.cartridge);
    }

    fn cart_read(&self, region: CartRegion, index: usize, addr: Address) -> u8 {
        match self.mmu.cartridge.coprocessor() {
            Coprocessor::SuperFx => crate::superfx::snes_cart_read(self, region, index, addr),
            Coprocessor::Sa1 => crate::sa1::snes_cart_read(self, region, index, addr),
            _ => self.mmu.cart_read(region, index),
        }
    }

    fn cart_write(&mut self, region: CartRegion, index: usize, value: u8) {
        match self.mmu.cartridge.coprocessor() {
            Coprocessor::SuperFx => crate::superfx::snes_cart_write(self, region, index, value),
            Coprocessor::Sa1 => crate::sa1::snes_cart_write(self, region, index, value),
            _ => self.mmu.cart_write(region, index, value),
        }
    }

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
        if self.mmu.access.active()
            && let Some(key) = self::space_key(space)
        {
            self.mmu.access.record(key, write);
        }
    }

    /// A read issued by the CPU. Bills cycles and syncs the scheduler first.
    pub fn read8(&mut self, addr: Address) -> u8 {
        self.mmu.clocks += self.mmu.waitstates(addr);
        self.sync();
        self.read8_direct(addr)
    }

    /// Does not bill cycles or sync scheduler. Used by DMA.
    pub(crate) fn read8_direct(&mut self, addr: Address) -> u8 {
        let space = self.mmu.map(&self.sa1, addr);
        self.read8_mapped(addr, &space)
    }

    pub(crate) fn read8_mapped(&mut self, addr: Address, space: &AddressSpace) -> u8 {
        #[cfg(feature = "debug")]
        self.record_access(space, false);

        match space {
            AddressSpace::Wram(index) => self.mmu.wram[*index],
            AddressSpace::Iram(index) => self.sa1.iram[*index],
            AddressSpace::Sram(index) => self.mmu.cartridge.sram[*index],
            AddressSpace::Rom(index) => self.mmu.cartridge.rom[*index],
            AddressSpace::Cart(region, index) => self.cart_read(*region, *index, addr),
            AddressSpace::Io(reg) => self.read_io(*reg),
            AddressSpace::Unmapped => {
                tracing::error!(addr = format!("{addr:?}"), "Read from unmapped address");
                OPEN_BUS
            }
        }
    }

    pub fn classify(&self, addr: Address) -> Option<(MemKind, u32)> {
        self::space_key(&self.mmu.map(&self.sa1, addr))
    }

    pub fn peek8(&self, addr: Address) -> u8 {
        match self.mmu.map(&self.sa1, addr) {
            AddressSpace::Wram(index) => self.mmu.wram[index],
            AddressSpace::Iram(index) => self.sa1.iram[index],
            AddressSpace::Sram(index) => self.mmu.cartridge.sram[index],
            AddressSpace::Rom(index) => self.mmu.cartridge.rom[index],
            AddressSpace::Cart(region, index) => self.cart_read(region, index, addr),
            AddressSpace::Io(reg) => self.peek_io(reg),
            AddressSpace::Unmapped => OPEN_BUS,
        }
    }

    /// A write issued by the CPU. Bills cycles and syncs the scheduler first.
    pub fn write8(&mut self, addr: Address, value: u8) {
        self.mmu.clocks += self.mmu.waitstates(addr);
        self.sync();
        self.write8_direct(addr, value);
    }

    /// Does not bill cycles or sync scheduler. Used by DMA.
    pub(crate) fn write8_direct(&mut self, addr: Address, value: u8) {
        let space = self.mmu.map(&self.sa1, addr);
        self.write8_mapped(addr, &space, value);
    }

    pub(crate) fn write8_mapped(&mut self, addr: Address, space: &AddressSpace, value: u8) {
        #[cfg(feature = "debug")]
        self.record_access(space, true);

        match space {
            AddressSpace::Wram(index) => self.mmu.wram[*index] = value,
            AddressSpace::Iram(index) => self.sa1.write_iram_from_snes(*index, value),
            AddressSpace::Sram(index) => self.mmu.cartridge.sram[*index] = value,
            AddressSpace::Cart(region, index) => self.cart_write(*region, *index, value),
            AddressSpace::Rom(_) => {
                tracing::warn!(
                    addr = format!("{addr:?}"),
                    value = format!("{:02X}", value),
                    "Write to ROM"
                );
            }
            AddressSpace::Io(reg) => self.write_io(*reg, value),
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
        (self.mmu.devices.handler(reg).read)(self, reg)
    }

    fn write_io(&mut self, reg: u16, value: u8) {
        (self.mmu.devices.handler(reg).write)(self, reg, value);
    }

    fn peek_io(&self, reg: u16) -> u8 {
        (self.mmu.devices.handler(reg).peek)(self, reg)
    }

    /// Guaranteed to be side effect free.
    pub fn peek_mmio(&self, reg: u16) -> u8 {
        self.peek_io(reg)
    }
}

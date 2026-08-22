pub(crate) mod io;
pub(crate) mod map;
pub(crate) mod regs;

use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

use self::regs::{
    BMAP, BwramWriteEnable, BwramWriteProtectedArea, CBWE, CIWP, CharacterConversionDmaParameters,
    IramWriteProtection, ROM_BANK_START, Sa1BwramMapping, Sa1CpuControl, SnesBwramMapping,
    SnesFlags, SnesInterruptBits, SuperMmcBank, VECTOR_COUNT, VECTOR_START,
};
use crate::Cartridge;
use crate::cartridge::Coprocessor;
use crate::cpu::{Cpu, CpuBus};
use crate::mmio::{Address, AddressSpace, CartRegion, MemoryMap, OPEN_BUS};

pub const IRAM_SIZE: usize = 0x800;
pub(crate) const SNES_IRAM_START: u16 = 0x3000;
pub(crate) const SNES_IRAM_END: u16 = 0x37FF;
const IRAM_BLOCK_SIZE: usize = 0x100;
const IRAM_ZEROED: [u8; IRAM_SIZE] = [0; IRAM_SIZE];

fn vector_slot(reg: u16) -> Option<(usize, usize)> {
    let index = usize::from(reg.checked_sub(VECTOR_START)?);
    Some((index / 2, index % 2))
}

#[derive(Serialize, Deserialize)]
pub(crate) struct MemoryControl {
    rom_banks: [SuperMmcBank; 4],
    bwram_snes_mapping: SnesBwramMapping,
    bwram_sa1_mapping: Sa1BwramMapping,
    bwram_snes_write_enable: BwramWriteEnable,
    bwram_sa1_write_enable: BwramWriteEnable,
    bwram_protected_area: BwramWriteProtectedArea,
}

impl Default for MemoryControl {
    fn default() -> Self {
        Self {
            rom_banks: [
                SuperMmcBank::from_raw(0),
                SuperMmcBank::from_raw(1),
                SuperMmcBank::from_raw(2),
                SuperMmcBank::from_raw(3),
            ],
            bwram_snes_mapping: SnesBwramMapping::default(),
            bwram_sa1_mapping: Sa1BwramMapping::default(),
            bwram_snes_write_enable: BwramWriteEnable::default(),
            bwram_sa1_write_enable: BwramWriteEnable::default(),
            bwram_protected_area: BwramWriteProtectedArea::default(),
        }
    }
}

impl MemoryControl {
    pub(crate) fn rom_block(&self, window: u8) -> u8 {
        self.rom_banks
            .get(usize::from(window))
            .map_or(window, SuperMmcBank::rom_block)
    }

    pub(crate) fn lo_rom_enabled(&self, window: u8) -> bool {
        self.rom_banks
            .get(usize::from(window))
            .is_some_and(SuperMmcBank::lo_rom_enable)
    }

    fn set_rom_bank(&mut self, reg: u16, value: u8) {
        let Some(index) = reg.checked_sub(ROM_BANK_START).map(usize::from) else {
            return;
        };
        let Some(bank) = self.rom_banks.get_mut(index) else {
            return;
        };

        *bank = SuperMmcBank::from_raw(value & 0x87);
    }

    fn rom_bank_raw(&self, reg: u16) -> u8 {
        reg.checked_sub(ROM_BANK_START)
            .map(usize::from)
            .and_then(|index| self.rom_banks.get(index))
            .map_or(0, SuperMmcBank::raw)
    }

    pub(crate) fn bwram_snes_block(&self) -> usize {
        usize::from(self.bwram_snes_mapping.bwram_block())
    }

    fn set_bwram_snes_block(&mut self, value: u8) {
        self.bwram_snes_mapping = SnesBwramMapping::from_raw(value & 0x1F);
    }

    fn bwram_snes_block_raw(&self) -> u8 {
        self.bwram_snes_mapping.raw()
    }

    pub(crate) fn bwram_sa1_block(&self) -> usize {
        usize::from(self.bwram_sa1_mapping.bwram_block() & 0x1F)
    }

    pub(crate) fn bwram_sa1_bitmap(&self) -> bool {
        self.bwram_sa1_mapping.bitmap()
    }

    fn set_bwram_sa1_mapping(&mut self, value: u8) {
        self.bwram_sa1_mapping = Sa1BwramMapping::from_raw(value);
    }

    fn bwram_sa1_mapping_raw(&self) -> u8 {
        self.bwram_sa1_mapping.raw()
    }

    fn set_bwram_snes_write_enable(&mut self, value: u8) {
        self.bwram_snes_write_enable = BwramWriteEnable::from_raw(value & 0x80);
    }

    fn bwram_snes_write_enable_raw(&self) -> u8 {
        self.bwram_snes_write_enable.raw()
    }

    fn set_bwram_sa1_write_enable(&mut self, value: u8) {
        self.bwram_sa1_write_enable = BwramWriteEnable::from_raw(value & 0x80);
    }

    fn bwram_sa1_write_enable_raw(&self) -> u8 {
        self.bwram_sa1_write_enable.raw()
    }

    fn set_bwram_protected_area(&mut self, value: u8) {
        self.bwram_protected_area = BwramWriteProtectedArea::from_raw(value & 0x0F);
    }

    fn bwram_protected_area_raw(&self) -> u8 {
        self.bwram_protected_area.raw()
    }

    pub(crate) fn bwram_snes_write_allowed(&self, offset: usize) -> bool {
        self.bwram_snes_write_enable.write_enable()
            || offset >= 0x100usize << self.bwram_protected_area.protected_size_exponent()
    }

    fn bwram_sa1_write_allowed(&self, offset: usize) -> bool {
        self.bwram_sa1_write_enable.write_enable()
            || offset >= 0x100usize << self.bwram_protected_area.protected_size_exponent()
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Sa1 {
    pub cpu: Cpu,
    clocks: u64,
    clock_remainder: u64,
    #[serde(with = "BigArray")]
    pub iram: [u8; IRAM_SIZE] = IRAM_ZEROED,
    pub(crate) siwp: IramWriteProtection,
    pub(crate) ciwp: IramWriteProtection,
    pub(crate) ccnt: Sa1CpuControl,
    pub(crate) sie: SnesInterruptBits,
    vectors: [u16; VECTOR_COUNT],
    pub(crate) memory: MemoryControl,
    pub(crate) cdma: CharacterConversionDmaParameters,
    message_from_sa1: u8,
    snes_nmi_vector: bool,
    snes_irq_vector: bool,
    character_conversion_dma_irq_pending: bool,
    sa1_irq_pending: bool,
    sa1_nmi_pending: bool,
    snes_irq_to_sa1_pending: bool,
}

impl Sa1 {
    pub(crate) fn set_ccnt(&mut self, value: u8, clocks: u64) {
        let reset = self.ccnt.reset();
        let control = Sa1CpuControl::from_raw(value);
        self.sa1_nmi_pending |= control.nmi_request();
        self.snes_irq_to_sa1_pending |= control.irq_request();
        self.ccnt = control.with_nmi_request(false).with_irq_request(false);

        let cpu = if control.reset() {
            Some(Cpu::default())
        } else if reset {
            Some(Cpu::new(self.vectors[0]))
        } else {
            None
        };

        if let Some(cpu) = cpu {
            self.cpu = cpu;
            self.clocks = clocks;
            self.clock_remainder = 0;
        }
    }

    pub(crate) fn set_sie(&mut self, value: u8) {
        self.sie = SnesInterruptBits::from_raw(value & 0xA0);
    }

    pub(crate) fn clear_snes_interrupts(&mut self, value: u8) {
        let clear = SnesInterruptBits::from_raw(value);
        if clear.character_conversion_dma_irq() {
            self.character_conversion_dma_irq_pending = false;
        }
        if clear.sa1_irq() {
            self.sa1_irq_pending = false;
        }
    }

    pub(crate) fn set_vector_byte(&mut self, reg: u16, value: u8) {
        let Some((slot, byte)) = self::vector_slot(reg) else {
            return;
        };
        let Some(vector) = self.vectors.get_mut(slot) else {
            return;
        };

        let mut bytes = vector.to_le_bytes();
        bytes[byte] = value;
        *vector = u16::from_le_bytes(bytes);
    }

    pub(crate) fn vector_byte(&self, reg: u16) -> u8 {
        let Some((slot, byte)) = self::vector_slot(reg) else {
            return 0;
        };

        self.vectors
            .get(slot)
            .map_or(0, |vector| vector.to_le_bytes()[byte])
    }

    pub(crate) fn set_cdma(&mut self, value: u8) {
        self.cdma = CharacterConversionDmaParameters::from_raw(value & 0x9F);
    }

    pub(crate) fn sfr(&self) -> SnesFlags {
        SnesFlags::default()
            .with_message_from_sa1(self.message_from_sa1)
            .with_nmi_vector_from_port(self.snes_nmi_vector)
            .with_character_conversion_dma_irq(self.character_conversion_dma_irq_pending)
            .with_irq_vector_from_port(self.snes_irq_vector)
            .with_sa1_irq(self.sa1_irq_pending)
    }

    pub(crate) fn set_siwp(&mut self, value: u8) {
        self.siwp = IramWriteProtection::from_raw(value);
    }

    pub(crate) fn set_ciwp(&mut self, value: u8) {
        self.ciwp = IramWriteProtection::from_raw(value);
    }

    pub(crate) fn write_iram_from_snes(&mut self, index: usize, value: u8) {
        let block = index / IRAM_BLOCK_SIZE;
        if self.siwp.write_enabled(block) {
            self.iram[index] = value;
        }
    }

    pub(crate) fn cpu_and_bus<'a>(
        &'a mut self,
        cartridge: &'a mut Cartridge,
    ) -> (&'a mut Cpu, Bus<'a>) {
        let Self {
            cpu,
            iram,
            ciwp,
            memory,
            ..
        } = self;
        (
            cpu,
            Bus {
                cartridge,
                iram,
                ciwp,
                memory,
            },
        )
    }

    fn advance_clocks(&mut self, cycles: u64, master_clock_hz: u64) {
        const SA1_CLOCK_HZ: u64 = crate::scheduler::MASTER_CLOCK_HZ_NTSC / 2;

        let scaled = cycles * master_clock_hz + self.clock_remainder;
        self.clocks += scaled / SA1_CLOCK_HZ;
        self.clock_remainder = scaled % SA1_CLOCK_HZ;
    }
}

pub(crate) struct Bus<'a> {
    cartridge: &'a mut Cartridge,
    iram: &'a mut [u8; IRAM_SIZE],
    ciwp: &'a mut IramWriteProtection,
    memory: &'a mut MemoryControl,
}

impl Bus<'_> {
    fn space(&self, addr: Address) -> AddressSpace {
        map::cpu_space(self.cartridge, self.memory, addr.bank(), addr.address())
    }

    fn read_io(&self, reg: u16) -> u8 {
        match reg {
            BMAP | CBWE | CIWP => OPEN_BUS,
            _ => {
                tracing::error!(
                    cpu = "SA-1",
                    reg = format!("{reg:04X}"),
                    "Read from unmapped I/O register"
                );
                OPEN_BUS
            }
        }
    }

    fn write_io(&mut self, reg: u16, value: u8) {
        match reg {
            BMAP => self.memory.set_bwram_sa1_mapping(value),
            CBWE => self.memory.set_bwram_sa1_write_enable(value),
            CIWP => *self.ciwp = IramWriteProtection::from_raw(value),
            _ => tracing::error!(
                cpu = "SA-1",
                reg = format!("{reg:04X}"),
                value = format!("{value:02X}"),
                "Write to unmapped I/O register"
            ),
        }
    }

    fn peek_io(&self, reg: u16) -> u8 {
        match reg {
            BMAP => self.memory.bwram_sa1_mapping_raw(),
            CBWE => self.memory.bwram_sa1_write_enable_raw(),
            CIWP => self.ciwp.raw(),
            _ => OPEN_BUS,
        }
    }
}

impl CpuBus for Bus<'_> {
    fn read8(&mut self, addr: Address) -> u8 {
        match self.space(addr) {
            AddressSpace::Iram(index) => self.iram[index],
            AddressSpace::Cart(CartRegion::Ram, index) => self.cartridge.sram_read(index),
            AddressSpace::Rom(index) => self.cartridge.rom[index],
            AddressSpace::Io(reg) => self.read_io(reg),
            _ => {
                tracing::error!(
                    cpu = "SA-1",
                    addr = format!("{addr:?}"),
                    "Read from unmapped address"
                );
                OPEN_BUS
            }
        }
    }

    fn write8(&mut self, addr: Address, value: u8) {
        match self.space(addr) {
            AddressSpace::Iram(index) => {
                let block = index / IRAM_BLOCK_SIZE;
                if self.ciwp.write_enabled(block) {
                    self.iram[index] = value;
                }
            }
            AddressSpace::Cart(CartRegion::Ram, index)
                if self.memory.bwram_sa1_write_allowed(index) =>
            {
                self.cartridge.sram_write(index, value);
            }
            AddressSpace::Cart(CartRegion::Ram, _) => {}
            AddressSpace::Rom(_) => tracing::warn!(
                cpu = "SA-1",
                addr = format!("{addr:?}"),
                value = format!("{value:02X}"),
                "Write to ROM"
            ),
            AddressSpace::Io(reg) => self.write_io(reg, value),
            _ => tracing::error!(
                cpu = "SA-1",
                addr = format!("{addr:?}"),
                value = format!("{value:02X}"),
                "Write to unmapped address"
            ),
        }
    }

    fn peek8(&self, addr: Address) -> u8 {
        match self.space(addr) {
            AddressSpace::Iram(index) => self.iram[index],
            AddressSpace::Cart(CartRegion::Ram, index) => self.cartridge.sram_read(index),
            AddressSpace::Rom(index) => self.cartridge.rom[index],
            AddressSpace::Io(reg) => self.peek_io(reg),
            _ => OPEN_BUS,
        }
    }
}

pub(crate) fn run_until<M: MemoryMap>(snes: &mut crate::Bus<M>, target: u64) {
    if snes.mmu.cartridge.coprocessor() != Coprocessor::Sa1 {
        return;
    }

    if snes.sa1.ccnt.reset()
        || snes.sa1.ccnt.wait()
        || snes.sa1.cpu.execution != crate::cpu::ExecutionState::Running
    {
        snes.sa1.clocks = target;
        return;
    }

    let master_clock_hz = snes.scheduler.master_clock_hz();
    while snes.sa1.clocks < target {
        let cycles = {
            let (cpu, mut bus) = snes.sa1.cpu_and_bus(&mut snes.mmu.cartridge);
            crate::cpu::step_on_bus(cpu, &mut bus)
        };
        snes.sa1.advance_clocks(cycles, master_clock_hz);
    }
}

pub(crate) fn snes_cart_read<M: crate::mmio::MemoryMap>(
    snes: &crate::Bus<M>,
    region: crate::mmio::CartRegion,
    index: usize,
) -> u8 {
    match region {
        crate::mmio::CartRegion::Ram => snes.mmu.cartridge.sram_read(index),
        crate::mmio::CartRegion::Rom => snes.mmu.cart_read(region, index),
    }
}

pub(crate) fn snes_cart_write<M: crate::mmio::MemoryMap>(
    snes: &mut crate::Bus<M>,
    region: crate::mmio::CartRegion,
    index: usize,
    value: u8,
) {
    match region {
        crate::mmio::CartRegion::Ram if snes.sa1.memory.bwram_snes_write_allowed(index) => {
            snes.mmu.cartridge.sram_write(index, value);
        }
        crate::mmio::CartRegion::Ram => {}
        crate::mmio::CartRegion::Rom => snes.mmu.cart_write(region, index, value),
    }
}

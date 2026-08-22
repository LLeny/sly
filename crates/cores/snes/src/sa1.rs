pub(crate) mod io;
pub(crate) mod map;
mod math;
pub(crate) mod regs;

use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

use self::math::Math;
use self::regs::{
    BMAP, BwramWriteEnable, BwramWriteProtectedArea, CBWE, CIC, CIE, CIWP,
    CharacterConversionDmaParameters, IramWriteProtection, ROM_BANK_START, Sa1BwramMapping,
    Sa1CpuControl, Sa1Flags, Sa1InterruptBits, SnesBwramMapping, SnesCpuControl, SnesFlags,
    SnesInterruptBits, SuperMmcBank, VECTOR_COUNT, VECTOR_START,
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
struct InterruptState {
    enable: Sa1InterruptBits,
    nmi_from_snes_pending: bool,
    dma_irq_pending: bool,
    timer_irq_pending: bool,
    irq_from_snes_pending: bool,
    nmi_from_snes_delivered: bool,
    dma_irq_delivered: bool,
    timer_irq_delivered: bool,
    irq_from_snes_delivered: bool,
}

impl InterruptState {
    fn set_enable(&mut self, value: u8) {
        let enable = Sa1InterruptBits::from_raw(value & 0xF0);

        if !self.enable.nmi_from_snes() && enable.nmi_from_snes() && self.nmi_from_snes_pending {
            self.nmi_from_snes_delivered = false;
        }
        if !self.enable.dma_irq() && enable.dma_irq() && self.dma_irq_pending {
            self.dma_irq_delivered = false;
        }
        if !self.enable.timer_irq() && enable.timer_irq() && self.timer_irq_pending {
            self.timer_irq_delivered = false;
        }
        if !self.enable.irq_from_snes() && enable.irq_from_snes() && self.irq_from_snes_pending {
            self.irq_from_snes_delivered = false;
        }

        self.enable = enable;
    }

    fn clear(&mut self, value: u8) {
        let clear = Sa1InterruptBits::from_raw(value);
        if clear.nmi_from_snes() {
            self.nmi_from_snes_pending = false;
        }
        if clear.dma_irq() {
            self.dma_irq_pending = false;
        }
        if clear.timer_irq() {
            self.timer_irq_pending = false;
        }
        if clear.irq_from_snes() {
            self.irq_from_snes_pending = false;
        }
    }

    fn raise_nmi_from_snes(&mut self) {
        self.nmi_from_snes_pending = true;
        self.nmi_from_snes_delivered = false;
    }

    fn raise_irq_from_snes(&mut self) {
        self.irq_from_snes_pending = true;
        self.irq_from_snes_delivered = false;
    }

    fn take_nmi(&mut self) -> bool {
        if !self.enable.nmi_from_snes()
            || !self.nmi_from_snes_pending
            || self.nmi_from_snes_delivered
        {
            return false;
        }

        self.nmi_from_snes_delivered = true;
        true
    }

    fn irq_delivery_pending(&self) -> bool {
        (self.enable.timer_irq() && self.timer_irq_pending && !self.timer_irq_delivered)
            || (self.enable.dma_irq() && self.dma_irq_pending && !self.dma_irq_delivered)
            || (self.enable.irq_from_snes()
                && self.irq_from_snes_pending
                && !self.irq_from_snes_delivered)
    }

    fn take_irq(&mut self) -> bool {
        if self.enable.timer_irq() && self.timer_irq_pending && !self.timer_irq_delivered {
            self.timer_irq_delivered = true;
            return true;
        }
        if self.enable.dma_irq() && self.dma_irq_pending && !self.dma_irq_delivered {
            self.dma_irq_delivered = true;
            return true;
        }
        if self.enable.irq_from_snes()
            && self.irq_from_snes_pending
            && !self.irq_from_snes_delivered
        {
            self.irq_from_snes_delivered = true;
            return true;
        }

        false
    }

    fn flags(&self, message_from_snes: u8) -> Sa1Flags {
        Sa1Flags::default()
            .with_message_from_snes(message_from_snes)
            .with_nmi_from_snes(self.nmi_from_snes_pending)
            .with_dma_irq(self.dma_irq_pending)
            .with_timer_irq(self.timer_irq_pending)
            .with_irq_from_snes(self.irq_from_snes_pending)
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
    scnt: SnesCpuControl,
    interrupts: InterruptState,
    vectors: [u16; VECTOR_COUNT],
    pub(crate) memory: MemoryControl,
    pub(crate) cdma: CharacterConversionDmaParameters,
    math: Math,
    character_conversion_dma_irq_pending: bool,
    irq_from_sa1_pending: bool,
}

impl Sa1 {
    pub(crate) fn set_ccnt(&mut self, value: u8, clocks: u64) {
        let reset = self.ccnt.reset();
        let control = Sa1CpuControl::from_raw(value);
        if control.nmi_request() {
            self.interrupts.raise_nmi_from_snes();
        }
        if control.irq_request() {
            self.interrupts.raise_irq_from_snes();
        }
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

    fn snes_irq_active(&self) -> bool {
        (self.sie.character_conversion_dma_irq() && self.character_conversion_dma_irq_pending)
            || (self.sie.irq_from_sa1() && self.irq_from_sa1_pending)
    }

    pub(crate) fn clear_snes_interrupts(&mut self, value: u8) {
        let clear = SnesInterruptBits::from_raw(value);
        if clear.character_conversion_dma_irq() {
            self.character_conversion_dma_irq_pending = false;
        }
        if clear.irq_from_sa1() {
            self.irq_from_sa1_pending = false;
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

    pub(crate) fn sfr(&self) -> SnesFlags {
        SnesFlags::default()
            .with_message_from_sa1(self.scnt.message_to_snes())
            .with_nmi_vector_from_port(self.scnt.nmi_vector_from_port())
            .with_character_conversion_dma_irq(self.character_conversion_dma_irq_pending)
            .with_irq_vector_from_port(self.scnt.irq_vector_from_port())
            .with_irq_from_sa1(self.irq_from_sa1_pending)
    }

    pub(crate) fn set_siwp(&mut self, value: u8) {
        self.siwp = IramWriteProtection::from_raw(value);
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
            ccnt,
            scnt,
            interrupts,
            cdma,
            math,
            irq_from_sa1_pending,
            ..
        } = self;
        (
            cpu,
            Bus {
                cartridge,
                iram,
                ciwp,
                memory,
                ccnt,
                scnt,
                interrupts,
                cdma,
                math,
                irq_from_sa1_pending,
            },
        )
    }

    fn take_interrupt_vector(&mut self) -> Option<u16> {
        if self.interrupts.take_nmi() {
            return Some(self.vectors[1]);
        }
        if !self.cpu.flags.irq_disable() && self.interrupts.take_irq() {
            return Some(self.vectors[2]);
        }

        None
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
    ccnt: &'a mut Sa1CpuControl,
    scnt: &'a mut SnesCpuControl,
    interrupts: &'a mut InterruptState,
    cdma: &'a mut CharacterConversionDmaParameters,
    math: &'a mut Math,
    irq_from_sa1_pending: &'a mut bool,
}

impl Bus<'_> {
    fn space(&self, addr: Address) -> AddressSpace {
        map::cpu_space(self.cartridge, self.memory, addr.bank(), addr.address())
    }

    fn read_io(&self, reg: u16) -> u8 {
        if let Some(value) = self.math.read(reg) {
            return value;
        }
        if Math::is_write_only(reg) {
            return OPEN_BUS;
        }

        match reg {
            BMAP | CBWE | CIWP | CIE | CIC => OPEN_BUS,
            reg if reg == SnesCpuControl::ADDRESS => OPEN_BUS,
            reg if reg == CharacterConversionDmaParameters::ADDRESS => OPEN_BUS,
            reg if reg == Sa1Flags::ADDRESS => {
                self.interrupts.flags(self.ccnt.message_to_sa1()).raw()
            }
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
        if self.math.write(reg, value) {
            return;
        }

        match reg {
            reg if reg == SnesCpuControl::ADDRESS => {
                let control = SnesCpuControl::from_raw(value);
                if control.irq_request() {
                    *self.irq_from_sa1_pending = true;
                }
                *self.scnt = control.with_irq_request(false);
            }
            CIE => self.interrupts.set_enable(value),
            CIC => self.interrupts.clear(value),
            BMAP => self.memory.set_bwram_sa1_mapping(value),
            CBWE => self.memory.set_bwram_sa1_write_enable(value),
            CIWP => *self.ciwp = IramWriteProtection::from_raw(value),
            reg if reg == CharacterConversionDmaParameters::ADDRESS => {
                *self.cdma = CharacterConversionDmaParameters::from_raw(value & 0x9F);
            }
            _ => tracing::error!(
                cpu = "SA-1",
                reg = format!("{reg:04X}"),
                value = format!("{value:02X}"),
                "Write to unmapped I/O register"
            ),
        }
    }

    fn peek_io(&self, reg: u16) -> u8 {
        if let Some(value) = self.math.peek(reg) {
            return value;
        }

        match reg {
            reg if reg == SnesCpuControl::ADDRESS => self.scnt.raw(),
            CIE => self.interrupts.enable.raw(),
            CIC => 0,
            BMAP => self.memory.bwram_sa1_mapping_raw(),
            CBWE => self.memory.bwram_sa1_write_enable_raw(),
            CIWP => self.ciwp.raw(),
            reg if reg == CharacterConversionDmaParameters::ADDRESS => self.cdma.raw(),
            reg if reg == Sa1Flags::ADDRESS => {
                self.interrupts.flags(self.ccnt.message_to_sa1()).raw()
            }
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
        || snes.sa1.cpu.execution == crate::cpu::ExecutionState::Stopped
    {
        snes.sa1.clocks = target;
        return;
    }

    let master_clock_hz = snes.scheduler.master_clock_hz();
    while snes.sa1.clocks < target {
        let interrupt_vector = snes.sa1.take_interrupt_vector();
        let irq_wakes_waiting_cpu = interrupt_vector.is_none()
            && snes.sa1.cpu.execution == crate::cpu::ExecutionState::Waiting
            && snes.sa1.interrupts.irq_delivery_pending();

        if interrupt_vector.is_none()
            && snes.sa1.cpu.execution == crate::cpu::ExecutionState::Waiting
            && !irq_wakes_waiting_cpu
        {
            snes.sa1.clocks = target;
            break;
        }

        let cycles = {
            let (cpu, mut bus) = snes.sa1.cpu_and_bus(&mut snes.mmu.cartridge);
            if let Some(vector) = interrupt_vector {
                crate::cpu::service_interrupt_at(cpu, &mut bus, vector)
            } else {
                if irq_wakes_waiting_cpu {
                    cpu.execution = crate::cpu::ExecutionState::Running;
                }
                crate::cpu::step_on_bus(cpu, &mut bus)
            }
        };
        snes.sa1.advance_clocks(cycles, master_clock_hz);
    }

    self::update_snes_irq(snes);
}

pub(crate) fn update_snes_irq<M: MemoryMap>(snes: &mut crate::Bus<M>) {
    snes.irq.set_cart_irq(snes.sa1.snes_irq_active());
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

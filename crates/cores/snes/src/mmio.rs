pub mod addr;
pub mod dma;
pub mod map;
pub mod regs;

#[cfg(feature = "debug")]
mod access;
mod symbol;

#[cfg(feature = "debug")]
pub use access::{AccessTracker, MemoryAccess, TrackMode};

use crate::joypad::{JOYPAD_IO_END, JOYPAD_IO_START, JOYSER0, JOYSER1, JOYSER1_ALWAYS_SET};
use crate::math::{RDDIVH, RDDIVL, RDMPYH, RDMPYL, WRDIVB, WRDIVH, WRDIVL, WRMPYA, WRMPYB};
use crate::mmio::dma::{CHANNEL_REGS_END, CHANNEL_REGS_START, Dma};
use crate::mmio::regs::{
    APU_IO_END, APU_IO_START, HdmaTransferEnable, HvStatusJoypadBusy, MdmaTransferStart,
    NmiTimerEnable, TimerUp, VblankFlagCpuVersion,
};
use crate::ppu::mode7::{Mode7Registers, PRODUCT_HIGH, PRODUCT_LOW, PRODUCT_MIDDLE};
use crate::ppu::mosaic::MosaicRegister;
use crate::ppu::regs::{
    Bg12ChrBase, Bg34ChrBase, BgMode, BgScrollOffset, CGADD, CGDATA, COLDATA, ColorMathControl,
    ColorMathSelect, HTIMEH, HTIMEL, INIDISP, LayerWindowEnable, OAMADDH, OAMADDL, OAMDATA,
    OAMDATAREAD, OPHCT, OPVCT, Ppu2Status, RDCGRAM, RDVRAMH, RDVRAML, SETINI, SLHV,
    SpriteSizeAndTileBase, TM, TS, TilemapAddrAndSize, VMADDH, VMADDL, VMDATAH, VMDATAL, VTIMEH,
    VTIMEL, VramControl, WindowMaskSettings,
};
use crate::ppu::window::{LOGIC_END, LOGIC_START, POSITION_END, POSITION_START};
use crate::{Cartridge, Snes};

pub use addr::Address;
pub use chapa::{EnumInfo, FieldInfo, FieldKind};
pub use map::{CartridgeMap, MemoryMap};
pub(crate) use symbol::MmioReg;
pub use symbol::{MmioSymbol, address_of, is_symbol, symbol, symbols};

pub(crate) const OPEN_BUS: u8 = 0xFF;
pub(crate) const WRIO: u16 = 0x4201;
pub(crate) const MEMSEL: u16 = 0x420D;
pub(crate) const RDIO: u16 = 0x4213;

pub struct Mmio<M: MemoryMap = CartridgeMap> {
    pub cartridge: Cartridge,
    pub wram: Box<[u8]>,
    map: M,
    pub fastrom: bool,
    wrio: u8,
    pub clocks: u64,

    #[cfg(feature = "debug")]
    pub access: AccessTracker,
}

pub enum AddressSpace {
    Wram(usize),
    Sram(usize),
    Rom(usize),
    Io(u16),
    Unmapped,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum MemKind {
    Wram,
    Sram,
    Rom,
    Io,
    Spc,
}

fn space_key(space: &AddressSpace) -> Option<(MemKind, u32)> {
    match space {
        AddressSpace::Wram(index) => Some((MemKind::Wram, *index as u32)),
        AddressSpace::Sram(index) => Some((MemKind::Sram, *index as u32)),
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
            fastrom: false,
            wrio: 0xFF,
            clocks: 0,

            #[cfg(feature = "debug")]
            access: AccessTracker::default(),
        }
    }

    pub fn reset(&mut self) {
        self.wram.fill(0);
        self.fastrom = false;
        self.wrio = 0xFF;
        self.clocks = 0;

        #[cfg(feature = "debug")]
        {
            self.access = AccessTracker::default();
        }
    }

    pub(crate) fn wrio(&self) -> u8 {
        self.wrio
    }

    /// Translate a CPU address to an address space (WRAM, ROM, I/O or unmapped).
    pub(crate) fn map(&self, addr: Address) -> AddressSpace {
        self.map.map(&self.cartridge, addr)
    }

    /// Waitstate clocks an access to `addr` adds beyond the fast cycle.
    pub(crate) fn waitstates(&self, addr: Address) -> u64 {
        self.map.speed(addr, self.fastrom) - map::WAITSTATES_FAST
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
        if self.mmu.access.active()
            && let Some(key) = self::space_key(space)
        {
            self.mmu.access.record(key, write);
        }
    }

    pub fn read8(&mut self, addr: Address) -> u8 {
        let space = self.mmu.map(addr);
        self.mmu.clocks += self.mmu.waitstates(addr);

        #[cfg(feature = "debug")]
        self.record_access(&space, false);

        match space {
            AddressSpace::Wram(index) => self.mmu.wram[index],
            AddressSpace::Sram(index) => self.mmu.cartridge.sram[index],
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
            AddressSpace::Sram(index) => self.mmu.cartridge.sram[index],
            AddressSpace::Rom(index) => self.mmu.cartridge.rom[index],
            AddressSpace::Io(reg) => self.peek_io(reg),
            AddressSpace::Unmapped => OPEN_BUS,
        }
    }

    pub fn write8(&mut self, addr: Address, value: u8) {
        let space = self.mmu.map(addr);
        self.mmu.clocks += self.mmu.waitstates(addr);

        #[cfg(feature = "debug")]
        self.record_access(&space, true);

        match space {
            AddressSpace::Wram(index) => self.mmu.wram[index] = value,
            AddressSpace::Sram(index) => self.mmu.cartridge.sram[index] = value,
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
            RDIO => self.mmu.wrio,
            MEMSEL => self.mmu.fastrom as u8,
            RDDIVL => self.math.div_quotient as u8,
            RDDIVH => (self.math.div_quotient >> 8) as u8,
            RDMPYL => self.math.product_or_remainder as u8,
            RDMPYH => (self.math.product_or_remainder >> 8) as u8,
            BgMode::ADDRESS => BgMode::read(self),
            MosaicRegister::ADDRESS => MosaicRegister::read(self),
            TilemapAddrAndSize::START..=TilemapAddrAndSize::END => {
                TilemapAddrAndSize::peek(self, reg)
            }
            Bg12ChrBase::ADDRESS => Bg12ChrBase::read(self),
            Bg34ChrBase::ADDRESS => Bg34ChrBase::read(self),
            SpriteSizeAndTileBase::ADDRESS => SpriteSizeAndTileBase::read(self),
            OAMADDL => self.ppu.oam.word_addr() as u8,
            OAMADDH => (self.ppu.oam.word_addr() >> 8) as u8,
            OAMDATA => self.ppu.oam.peek_data(),
            OAMDATAREAD => self.ppu.oam.read_data(),
            CGADD => self.ppu.cgram.word_addr(),
            CGDATA => self.ppu.cgram.peek_data(),
            RDCGRAM => self.ppu.cgram.read_data(),
            VramControl::ADDRESS => self.ppu.vram.control().raw(),
            VMADDL => self.ppu.vram.word_addr() as u8,
            VMADDH => (self.ppu.vram.word_addr() >> 8) as u8,
            VMDATAL => self.ppu.vram.peek_low(),
            VMDATAH => self.ppu.vram.peek_high(),
            RDVRAML => self.ppu.vram.read_low(),
            RDVRAMH => self.ppu.vram.read_high(),
            PRODUCT_LOW => self.ppu.matrix.result as u8,
            PRODUCT_MIDDLE => (self.ppu.matrix.result >> 8) as u8,
            PRODUCT_HIGH => (self.ppu.matrix.result >> 16) as u8,
            SLHV => {
                if self.mmu.wrio & 0x80 != 0 {
                    let master_clock = self.scheduler.cycles();
                    let scanline = self.scheduler.scanline_at(master_clock) as u16;
                    self.ppu.latch_counters(master_clock, scanline);
                }
                OPEN_BUS
            }
            OPHCT => self.ppu.read_h_counter(),
            OPVCT => self.ppu.read_v_counter(),
            Ppu2Status::ADDRESS => Ppu2Status::read(self),
            NmiTimerEnable::ADDRESS => NmiTimerEnable::read(self),
            VblankFlagCpuVersion::ADDRESS => VblankFlagCpuVersion::read(self),
            TimerUp::ADDRESS => TimerUp::read(self),
            HvStatusJoypadBusy::ADDRESS => HvStatusJoypadBusy::read(self),
            MdmaTransferStart::ADDRESS => MdmaTransferStart::read(self),
            HdmaTransferEnable::ADDRESS => HdmaTransferEnable::read(self),
            CHANNEL_REGS_START..=CHANNEL_REGS_END => {
                self.dma.channels[Dma::channel_index(reg)].read(reg)
            }
            APU_IO_START..=APU_IO_END => self.apu.cpu_read_port((reg - APU_IO_START) as usize),
            JOYPAD_IO_START..=JOYPAD_IO_END => self.joypads.read(reg),
            JOYSER0 => 0,
            JOYSER1 => JOYSER1_ALWAYS_SET,
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
            WRIO => {
                let external_latch_falling_edge = self.mmu.wrio & 0x80 != 0 && value & 0x80 == 0;
                self.mmu.wrio = value;

                if external_latch_falling_edge {
                    let master_clock = self.scheduler.cycles();
                    let scanline = self.scheduler.scanline_at(master_clock) as u16;
                    self.ppu.latch_counters(master_clock, scanline);
                }
            }
            WRMPYA => self.math.mul_factor_a = value,
            WRMPYB => {
                self.math.mul_factor_b = value;
                self.math.process_mul();
            }
            WRDIVL => self.math.div_dividend = (self.math.div_dividend & 0xFF00) | value as u16,
            WRDIVH => {
                self.math.div_dividend = (self.math.div_dividend & 0x00FF) | ((value as u16) << 8)
            }
            WRDIVB => {
                self.math.div_divisor = value;
                self.math.process_div();
            }
            BgMode::ADDRESS => BgMode::write(self, value),
            MosaicRegister::ADDRESS => MosaicRegister::write(self, value),
            TilemapAddrAndSize::START..=TilemapAddrAndSize::END => {
                TilemapAddrAndSize::write(self, reg, value)
            }
            Bg12ChrBase::ADDRESS => Bg12ChrBase::write(self, value),
            Bg34ChrBase::ADDRESS => Bg34ChrBase::write(self, value),
            SpriteSizeAndTileBase::ADDRESS => SpriteSizeAndTileBase::write(self, value),
            OAMADDL => self.ppu.oam.set_addr_low(value),
            OAMADDH => self.ppu.oam.set_addr_high(value),
            OAMDATA => self.ppu.oam.write_data(value),
            OAMDATAREAD => {}
            CGADD => self.ppu.cgram.set_addr(value),
            CGDATA => self.ppu.cgram.write_data(value),
            RDCGRAM => {}
            VramControl::ADDRESS => self.ppu.vram.set_control(value),
            VMADDL => self.ppu.vram.set_addr_low(value),
            VMADDH => self.ppu.vram.set_addr_high(value),
            VMDATAL => self.ppu.vram.write_low(value),
            VMDATAH => self.ppu.vram.write_high(value),
            RDVRAML | RDVRAMH => {}
            WindowMaskSettings::START..=WindowMaskSettings::END => {
                WindowMaskSettings::write(self, reg, value)
            }
            POSITION_START..=POSITION_END => self.ppu.windows.write_position(reg, value),
            LOGIC_START..=LOGIC_END => self.ppu.windows.write_logic(reg, value),
            TM => self.ppu.main_screen = value.into(),
            TS => self.ppu.sub_screen = value.into(),
            LayerWindowEnable::START..=LayerWindowEnable::END => {
                LayerWindowEnable::write(self, reg, value)
            }
            ColorMathControl::ADDRESS => ColorMathControl::write(self, value),
            COLDATA => self.ppu.color_math.set_fixed_color(value),
            ColorMathSelect::ADDRESS => ColorMathSelect::write(self, value),
            SETINI => {
                if value & 0x40 != 0 {
                    tracing::error!(
                        value = format!("{:02X}", value),
                        "SETINI: Mode 7 EXTBG enabled (unimplemented)"
                    );
                }
            }
            Ppu2Status::ADDRESS => Ppu2Status::write(self, value),
            BgScrollOffset::START..=BgScrollOffset::END => BgScrollOffset::write(self, reg, value),
            Mode7Registers::START..=Mode7Registers::END => Mode7Registers::write(self, reg, value),
            HTIMEL => self.irq.h_timer.set_target_low(value),
            HTIMEH => self.irq.h_timer.set_target_high(value),
            VTIMEL => self.irq.v_timer.set_target_low(value),
            VTIMEH => self.irq.v_timer.set_target_high(value),
            MEMSEL => self.mmu.fastrom = value & 0x01 != 0,
            NmiTimerEnable::ADDRESS => NmiTimerEnable::write(self, value),
            VblankFlagCpuVersion::ADDRESS => VblankFlagCpuVersion::write(self, value),
            TimerUp::ADDRESS => TimerUp::write(self, value),
            HvStatusJoypadBusy::ADDRESS => HvStatusJoypadBusy::write(self, value),
            MdmaTransferStart::ADDRESS => MdmaTransferStart::write(self, value),
            HdmaTransferEnable::ADDRESS => HdmaTransferEnable::write(self, value),
            CHANNEL_REGS_START..=CHANNEL_REGS_END => {
                self.dma.channels[Dma::channel_index(reg)].write(reg, value)
            }
            APU_IO_START..=APU_IO_END => self
                .apu
                .cpu_write_port((reg - APU_IO_START) as usize, value),
            JOYSER0 => {}
            INIDISP => self.ppu.set_display_control(value),
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
            WRIO | RDIO => self.mmu.wrio,
            MEMSEL => self.mmu.fastrom as u8,
            RDDIVL => self.math.div_quotient as u8,
            RDDIVH => (self.math.div_quotient >> 8) as u8,
            RDMPYL => self.math.product_or_remainder as u8,
            RDMPYH => (self.math.product_or_remainder >> 8) as u8,
            BgMode::ADDRESS => BgMode::peek(self),
            MosaicRegister::ADDRESS => MosaicRegister::peek(self),
            TilemapAddrAndSize::START..=TilemapAddrAndSize::END => {
                TilemapAddrAndSize::peek(self, reg)
            }
            Bg12ChrBase::ADDRESS => Bg12ChrBase::peek(self),
            Bg34ChrBase::ADDRESS => Bg34ChrBase::peek(self),
            SpriteSizeAndTileBase::ADDRESS => SpriteSizeAndTileBase::peek(self),
            OAMADDL => self.ppu.oam.word_addr() as u8,
            OAMADDH => (self.ppu.oam.word_addr() >> 8) as u8,
            OAMDATA | OAMDATAREAD => self.ppu.oam.peek_data(),
            CGADD => self.ppu.cgram.word_addr(),
            CGDATA | RDCGRAM => self.ppu.cgram.peek_data(),
            VramControl::ADDRESS => self.ppu.vram.control().raw(),
            VMADDL => self.ppu.vram.word_addr() as u8,
            VMADDH => (self.ppu.vram.word_addr() >> 8) as u8,
            VMDATAL | RDVRAML => self.ppu.vram.peek_low(),
            VMDATAH | RDVRAMH => self.ppu.vram.peek_high(),
            WindowMaskSettings::START..=WindowMaskSettings::END => {
                WindowMaskSettings::peek(self, reg)
            }
            POSITION_START..=POSITION_END => self.ppu.windows.position(reg),
            LOGIC_START..=LOGIC_END => self.ppu.windows.logic(reg),
            PRODUCT_LOW => self.ppu.matrix.result as u8,
            PRODUCT_MIDDLE => (self.ppu.matrix.result >> 8) as u8,
            PRODUCT_HIGH => (self.ppu.matrix.result >> 16) as u8,
            SLHV => OPEN_BUS,
            OPHCT => self.ppu.peek_h_counter(),
            OPVCT => self.ppu.peek_v_counter(),
            Ppu2Status::ADDRESS => Ppu2Status::peek(self),
            TM => self.ppu.main_screen.raw(),
            TS => self.ppu.sub_screen.raw(),
            LayerWindowEnable::START..=LayerWindowEnable::END => LayerWindowEnable::peek(self, reg),
            HTIMEL => self.irq.h_timer.target_low(),
            HTIMEH => self.irq.h_timer.target_high(),
            VTIMEL => self.irq.v_timer.target_low(),
            VTIMEH => self.irq.v_timer.target_high(),
            ColorMathSelect::ADDRESS => ColorMathSelect::peek(self),
            ColorMathControl::ADDRESS => ColorMathControl::peek(self),
            NmiTimerEnable::ADDRESS => NmiTimerEnable::peek(self),
            VblankFlagCpuVersion::ADDRESS => VblankFlagCpuVersion::peek(self),
            TimerUp::ADDRESS => TimerUp::peek(self),
            HvStatusJoypadBusy::ADDRESS => HvStatusJoypadBusy::peek(self),
            MdmaTransferStart::ADDRESS => MdmaTransferStart::peek(self),
            HdmaTransferEnable::ADDRESS => HdmaTransferEnable::peek(self),
            CHANNEL_REGS_START..=CHANNEL_REGS_END => {
                self.dma.channels[Dma::channel_index(reg)].read(reg)
            }
            APU_IO_START..=APU_IO_END => self.apu.cpu_read_port((reg - APU_IO_START) as usize),
            JOYPAD_IO_START..=JOYPAD_IO_END => self.joypads.read(reg),
            JOYSER0 => 0,
            JOYSER1 => JOYSER1_ALWAYS_SET,
            _ => OPEN_BUS,
        }
    }
}

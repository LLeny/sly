use chapa::bitenum;

use crate::Snes;
use crate::macros::{register_mmio, register_mmio_family, register_mmio_raw};
use crate::mmio::{MemoryMap, MmioReg};

pub const APU_IO_START: u16 = 0x2140;
pub const APU_IO_END: u16 = APU_IO_START + 3;

register_mmio_raw! {
    0x2140 => "APUIO0",
    0x2141 => "APUIO1",
    0x2142 => "APUIO2",
    0x2143 => "APUIO3",
}

register_mmio_raw! {
    0x4016 => "JOYSER0",
    0x4017 => "JOYSER1",
    0x4218 => "JOY1L",
    0x4219 => "JOY1H",
    0x421A => "JOY2L",
    0x421B => "JOY2H",
    0x421C => "JOY3L",
    0x421D => "JOY3H",
    0x421E => "JOY4L",
    0x421F => "JOY4H",
}

register_mmio_raw! {
    0x4201 => "WRIO",
    0x4202 => "WRMPYA",
    0x4203 => "WRMPYB",
    0x4204 => "WRDIVL",
    0x4205 => "WRDIVH",
    0x4206 => "WRDIVB",
    0x4213 => "RDIO",
    0x4214 => "RDDIVL",
    0x4215 => "RDDIVH",
    0x4216 => "RDMPYL",
    0x4217 => "RDMPYH",
}

register_mmio! {
    /// MEMSEL
    pub struct MemorySelect @ 0x420D {
        #[bits(0)]
        pub fast_rom: bool,
    }
}

impl MmioReg for MemorySelect {
    fn write<M: MemoryMap>(snes: &mut Snes<M>, value: u8) {
        snes.mmu.fastrom = Self::from_raw(value).fast_rom();
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>) -> u8 {
        Self::default().with_fast_rom(snes.mmu.fastrom).raw()
    }
}

register_mmio! {
    /// NMITIMEN
    pub struct NmiTimerEnable @ 0x4200 {
        #[bits(0)]
        pub joypad_autoread_enable: bool,

        #[bits(4..=5)]
        pub hv_timer_condition: u8,

        #[bits(7)]
        pub vblank_nmi_enable: bool,
    }
}

register_mmio! {
    /// MDMAEN
    pub struct MdmaTransferStart @ 0x420B {
        #[bits(0)]
        pub channel0: bool,

        #[bits(1)]
        pub channel1: bool,

        #[bits(2)]
        pub channel2: bool,

        #[bits(3)]
        pub channel3: bool,

        #[bits(4)]
        pub channel4: bool,

        #[bits(5)]
        pub channel5: bool,

        #[bits(6)]
        pub channel6: bool,

        #[bits(7)]
        pub channel7: bool,
    }
}

register_mmio! {
    /// HDMAEN
    pub struct HdmaTransferEnable @ 0x420C {
        #[bits(0)]
        pub channel0: bool,

        #[bits(1)]
        pub channel1: bool,

        #[bits(2)]
        pub channel2: bool,

        #[bits(3)]
        pub channel3: bool,

        #[bits(4)]
        pub channel4: bool,

        #[bits(5)]
        pub channel5: bool,

        #[bits(6)]
        pub channel6: bool,

        #[bits(7)]
        pub channel7: bool,
    }
}

register_mmio! {
    /// HVBJOY
    pub struct HvStatusJoypadBusy @ 0x4212 {
        #[bits(0)]
        pub joypad_autoread_busy: bool,

        #[bits(6)]
        pub hblank: bool,

        #[bits(7)]
        pub vblank: bool,
    }
}

register_mmio! {
    /// RDNMI
    pub struct VblankFlagCpuVersion @ 0x4210 {
        #[bits(0..=3)]
        pub cpu_version: u8,

        #[bits(7)]
        pub vblank: bool,
    }
}

register_mmio! {
    /// TIMEUP
    pub struct TimerUp @ 0x4211 {
        #[bits(7)]
        pub flag: u8,
    }
}

#[bitenum]
pub enum TransferPattern {
    #[fallback]
    Pattern0 = 0,
    Pattern1 = 1,
    Pattern2 = 2,
    Pattern3 = 3,
    Pattern4 = 4,
    Pattern5 = 5,
    Pattern6 = 6,
    Pattern7 = 7,
}

#[bitenum]
pub enum AddressAdjustMode {
    Increment = 0,
    Decrement = 2,
    #[fallback]
    Fixed = 1, // includes 3
}

#[bitenum]
pub enum Direction {
    #[fallback]
    AToB = 0,
    BToA = 1,
}

register_mmio_family! {
    /// DMAPn
    pub struct DmaChannelParameters {
        #[bits(0..=2)]
        pub transfer_pattern: TransferPattern,

        #[bits(3..=4)]
        pub address_adjust_mode: AddressAdjustMode,

        #[bits(6)]
        pub indirect: bool,

        #[bits(7)]
        pub direction: Direction,
    }
    symbols {
        0x4300 => "DMAP0",
        0x4310 => "DMAP1",
        0x4320 => "DMAP2",
        0x4330 => "DMAP3",
        0x4340 => "DMAP4",
        0x4350 => "DMAP5",
        0x4360 => "DMAP6",
        0x4370 => "DMAP7",
    }
}

impl Default for DmaChannelParameters {
    fn default() -> Self {
        Self::from_raw(0xFF)
    }
}

#[chapa::bitfield(u32, order = lsb0, width = 24)]
pub struct DmaAddress {
    #[bits(0..=15, overlay = "mdma")]
    pub offset: u16,

    #[bits(16..=23, overlay = "mdma")]
    pub bank: u8,

    #[bits(0..=15, overlay = "hdma")]
    pub table_offset: u16,

    #[bits(16..=23, overlay = "hdma")]
    pub table_bank: u8,
}

impl Default for DmaAddress {
    fn default() -> Self {
        Self::from_raw(0xFF_FFFF)
    }
}

#[chapa::bitfield(u32, order = lsb0, width = 24)]
pub struct DmaByteCount {
    #[bits(0..=15, overlay = "mdma")]
    pub amount: u16,

    #[bits(0..=15, overlay = "hdma")]
    pub indirect_offset: u16,

    #[bits(16..=23, overlay = "hdma")]
    pub indirect_bank: u8,
}

impl DmaByteCount {
    pub fn count(&self) -> u32 {
        let count = self.amount() as u32;
        if count == 0 { 65536 } else { count }
    }
}

impl Default for DmaByteCount {
    fn default() -> Self {
        Self::from_raw(0xFF_FFFF)
    }
}

register_mmio_family! {
    /// NLTRn
    pub struct HdmaLineCounter {
        #[bits(0..=6)]
        pub line_count: u8,

        #[bits(7)]
        pub repeat: bool,
    }
    symbols {
        0x430A => "NLTR0",
        0x431A => "NLTR1",
        0x432A => "NLTR2",
        0x433A => "NLTR3",
        0x434A => "NLTR4",
        0x435A => "NLTR5",
        0x436A => "NLTR6",
        0x437A => "NLTR7",
    }
}

impl Default for HdmaLineCounter {
    fn default() -> Self {
        Self::from_raw(0xFF)
    }
}

macro_rules! register_dma_channel_symbols {
    ($($n:literal),+ $(,)?) => {
        $(register_mmio_raw! {
            0x4301 + ($n << 4) => concat!("BBAD", $n),
            0x4302 + ($n << 4) => concat!("A1T", $n, "L"),
            0x4303 + ($n << 4) => concat!("A1T", $n, "H"),
            0x4304 + ($n << 4) => concat!("A1B", $n),
            0x4305 + ($n << 4) => concat!("DAS", $n, "L"),
            0x4306 + ($n << 4) => concat!("DAS", $n, "H"),
            0x4307 + ($n << 4) => concat!("DASB", $n),
            0x4308 + ($n << 4) => concat!("A2A", $n, "L"),
            0x4309 + ($n << 4) => concat!("A2A", $n, "H"),
        })+
    };
}

register_dma_channel_symbols!(0, 1, 2, 3, 4, 5, 6, 7);

impl MmioReg for NmiTimerEnable {
    fn write<M: MemoryMap>(snes: &mut Snes<M>, value: u8) {
        snes.irq.write_nmitimen(value);
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>) -> u8 {
        snes.irq.nmitimen_raw()
    }
}

impl MmioReg for MdmaTransferStart {
    fn write<M: MemoryMap>(snes: &mut Snes<M>, value: u8) {
        snes.dma.mdmaen = Self::from_raw(value);
        tracing::trace!(
            channel0 = snes.dma.mdmaen.channel0(),
            channel1 = snes.dma.mdmaen.channel1(),
            channel2 = snes.dma.mdmaen.channel2(),
            channel3 = snes.dma.mdmaen.channel3(),
            channel4 = snes.dma.mdmaen.channel4(),
            channel5 = snes.dma.mdmaen.channel5(),
            channel6 = snes.dma.mdmaen.channel6(),
            channel7 = snes.dma.mdmaen.channel7(),
            "MDMA transfer start"
        );
        super::dma::start_mdma(snes);
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>) -> u8 {
        snes.dma.mdmaen.raw()
    }
}

impl MmioReg for HdmaTransferEnable {
    fn write<M: MemoryMap>(snes: &mut Snes<M>, value: u8) {
        snes.dma.hdmaen = Self::from_raw(value);
        tracing::trace!(
            channel0 = snes.dma.hdmaen.channel0(),
            channel1 = snes.dma.hdmaen.channel1(),
            channel2 = snes.dma.hdmaen.channel2(),
            channel3 = snes.dma.hdmaen.channel3(),
            channel4 = snes.dma.hdmaen.channel4(),
            channel5 = snes.dma.hdmaen.channel5(),
            channel6 = snes.dma.hdmaen.channel6(),
            channel7 = snes.dma.hdmaen.channel7(),
            "HDMA transfer enable"
        );
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>) -> u8 {
        snes.dma.hdmaen.raw()
    }
}

impl MmioReg for HvStatusJoypadBusy {
    fn write<M: MemoryMap>(_snes: &mut Snes<M>, _value: u8) {}

    fn peek<M: MemoryMap>(snes: &Snes<M>) -> u8 {
        // TODO
        let mut value = Self::default();
        value.set_vblank(crate::ppu::in_vblank(&snes.scheduler));
        value.set_hblank(crate::ppu::in_hblank(&snes.scheduler));
        value.raw()
    }
}

impl MmioReg for VblankFlagCpuVersion {
    fn read<M: MemoryMap>(snes: &mut Snes<M>) -> u8 {
        snes.irq.read_rdnmi()
    }

    fn write<M: MemoryMap>(_snes: &mut Snes<M>, _value: u8) {}

    fn peek<M: MemoryMap>(snes: &Snes<M>) -> u8 {
        snes.irq.rdnmi_raw()
    }
}

impl MmioReg for TimerUp {
    fn read<M: MemoryMap>(snes: &mut Snes<M>) -> u8 {
        snes.irq.read_timeup()
    }

    fn write<M: MemoryMap>(_snes: &mut Snes<M>, _value: u8) {}

    fn peek<M: MemoryMap>(snes: &Snes<M>) -> u8 {
        snes.irq.peek_timeup()
    }
}

crate::macros::impl_bitfield_serde!(
    TransferPattern,
    AddressAdjustMode,
    Direction,
    DmaAddress,
    DmaByteCount,
);

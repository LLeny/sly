use chapa::BitEnum;

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

#[derive(BitEnum, Clone, Copy)]
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

#[derive(BitEnum, Clone, Copy)]
pub enum AddressAdjustMode {
    Increment = 0,
    Decrement = 2,
    #[fallback]
    Fixed = 1, // includes 3
}

#[derive(BitEnum, Clone, Copy)]
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

use crate::Snes;
use crate::macros::{mmio_passthrough, register_mmio, register_mmio_raw};
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
    pub struct DmaTransferStart @ 0x420B {
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
    /// RDNMI
    pub struct VblankFlagCpuVersion @ 0x4210 {
        #[bits(0..=3)]
        pub cpu_version: u8,

        #[bits(7)]
        pub vblank: bool,
    }
}

mmio_passthrough!(HdmaTransferEnable => dma.hdmaen);

impl MmioReg for NmiTimerEnable {
    fn read<M: MemoryMap>(snes: &mut Snes<M>) -> u8 {
        snes.irq.nmitimen_raw()
    }

    fn write<M: MemoryMap>(snes: &mut Snes<M>, value: u8) {
        snes.irq.write_nmitimen(value);
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>) -> u8 {
        snes.irq.nmitimen_raw()
    }
}

impl MmioReg for DmaTransferStart {
    fn read<M: MemoryMap>(snes: &mut Snes<M>) -> u8 {
        snes.dma.mdmaen.raw()
    }

    fn write<M: MemoryMap>(snes: &mut Snes<M>, value: u8) {
        snes.dma.mdmaen = Self::from_raw(value);
        tracing::error!(
            value = format!("{:02X}", value),
            "MDMA start not implemented"
        );
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>) -> u8 {
        snes.dma.mdmaen.raw()
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

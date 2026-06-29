use crate::Snes;
use crate::macros::{mmio_passthrough, register_mmio};
use crate::mmio::MmioReg;

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

mmio_passthrough!(NmiTimerEnable => irq.nmitimen);
mmio_passthrough!(HdmaTransferEnable => dma.hdmaen);

impl MmioReg for DmaTransferStart {
    fn read(snes: &Snes) -> u8 {
        snes.dma.mdmaen.raw()
    }

    fn write(snes: &mut Snes, value: u8) {
        snes.dma.mdmaen = Self::from_raw(value);
        tracing::error!(
            value = format!("{:02X}", value),
            "MDMA start not implemented"
        );
    }

    fn peek(snes: &Snes) -> u8 {
        snes.dma.mdmaen.raw()
    }
}

use crate::macros::{register_mmio, register_mmio_raw};

pub(crate) const BRAMR: u16 = 0x3033;
pub(crate) const PBR: u16 = 0x3034;
pub(crate) const ROMBR: u16 = 0x3036;
pub(crate) const SCBR: u16 = 0x3038;
pub(crate) const VCR: u16 = 0x303B;
pub(crate) const RAMBR: u16 = 0x303C;
pub(crate) const CBR: u16 = 0x303E;

register_mmio! {
    /// SFR
    pub struct Sfr: u16 @ 0x3030 {
        #[bits(1)]
        pub zero: bool,

        #[bits(2)]
        pub carry: bool,

        #[bits(3)]
        pub sign: bool,

        #[bits(4)]
        pub overflow: bool,

        #[bits(5)]
        pub go: bool,

        #[bits(6)]
        pub rom_busy: bool,

        #[bits(8)]
        pub alt1: bool,

        #[bits(9)]
        pub alt2: bool,

        #[bits(10)]
        pub imm_low: bool,

        #[bits(11)]
        pub imm_high: bool,

        #[bits(12)]
        pub with: bool,

        #[bits(15)]
        pub irq: bool,
    }
}

register_mmio! {
    /// SCMR
    pub struct Scmr @ 0x303A {
        #[bits(0..=1)]
        pub color_depth: u8,

        #[bits(2)]
        pub height_low: bool,

        #[bits(3)]
        pub ram_to_gsu: bool,

        #[bits(4)]
        pub rom_to_gsu: bool,

        #[bits(5)]
        pub height_high: bool,
    }
}

register_mmio! {
    /// CFGR
    pub struct Cfgr @ 0x3037 {
        #[bits(5)]
        pub fast_multiply: bool,

        #[bits(7)]
        pub irq_masked: bool,
    }
}

register_mmio! {
    /// CLSR
    pub struct Clsr @ 0x3039 {
        /// 0 = 10.738 MHz, 1 = 21.477 MHz
        #[bits(0)]
        pub fast_clock: bool,
    }
}

register_mmio_raw! {
    0x3000 => "R0",
    0x3002 => "R1",
    0x3004 => "R2",
    0x3006 => "R3",
    0x3008 => "R4",
    0x300A => "R5",
    0x300C => "R6",
    0x300E => "R7",
    0x3010 => "R8",
    0x3012 => "R9",
    0x3014 => "R10",
    0x3016 => "R11",
    0x3018 => "R12",
    0x301A => "R13",
    0x301C => "R14",
    0x301E => "R15",
    BRAMR => "BRAMR",
    PBR => "PBR",
    ROMBR => "ROMBR",
    SCBR => "SCBR",
    VCR => "VCR",
    RAMBR => "RAMBR",
    CBR => "CBR",
}

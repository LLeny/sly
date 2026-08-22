use crate::macros::{register_mmio, register_mmio_family, register_mmio_raw};

pub(crate) const SIE: u16 = 0x2201;
pub(crate) const SIC: u16 = 0x2202;
pub(crate) const CIE: u16 = 0x220A;
pub(crate) const CIC: u16 = 0x220B;
pub(crate) const VECTOR_COUNT: usize = 3;
pub(crate) const VECTOR_START: u16 = 0x2203;
pub(crate) const VECTOR_END: u16 = 0x2208;
pub(crate) const SNES_VECTOR_COUNT: usize = 2;
pub(crate) const SNES_VECTOR_START: u16 = 0x220C;
pub(crate) const SNES_VECTOR_END: u16 = 0x220F;
pub(crate) const ROM_BANK_START: u16 = 0x2220;
pub(crate) const ROM_BANK_END: u16 = 0x2223;
pub(crate) const BMAPS: u16 = 0x2224;
pub(crate) const BMAP: u16 = 0x2225;
pub(crate) const SBWE: u16 = 0x2226;
pub(crate) const CBWE: u16 = 0x2227;
pub(crate) const BWPA: u16 = 0x2228;
pub(crate) const SIWP: u16 = 0x2229;
pub(crate) const CIWP: u16 = 0x222A;
pub(crate) const ARITHMETIC_PARAMETER_A_START: u16 = 0x2251;
pub(crate) const ARITHMETIC_PARAMETER_A_END: u16 = 0x2252;
pub(crate) const ARITHMETIC_PARAMETER_B_START: u16 = 0x2253;
pub(crate) const ARITHMETIC_PARAMETER_B_END: u16 = 0x2254;
pub(crate) const ARITHMETIC_RESULT_START: u16 = 0x2306;
pub(crate) const ARITHMETIC_RESULT_END: u16 = 0x230A;
pub(crate) const ARITHMETIC_OVERFLOW: u16 = 0x230B;

register_mmio! {
    /// CCNT
    pub struct Sa1CpuControl @ 0x2200 {
        #[bits(0..=3)]
        pub message_to_sa1: u8,

        #[bits(4)]
        pub nmi_request: bool,

        #[bits(5, default = true)]
        pub reset: bool,

        #[bits(6)]
        pub wait: bool,

        #[bits(7)]
        pub irq_request: bool,
    }
}

register_mmio! {
    /// MCNT
    pub struct ArithmeticControl @ 0x2250 {
        #[bits(0..=1)]
        pub mode: u8,
    }
}

register_mmio_raw! {
    ARITHMETIC_PARAMETER_A_START => "MAL",
    ARITHMETIC_PARAMETER_A_END => "MAH",
    ARITHMETIC_PARAMETER_B_START => "MBL",
    ARITHMETIC_PARAMETER_B_END => "MBH",
    ARITHMETIC_RESULT_START => "MR0",
    ARITHMETIC_RESULT_START + 1 => "MR1",
    ARITHMETIC_RESULT_START + 2 => "MR2",
    ARITHMETIC_RESULT_START + 3 => "MR3",
    ARITHMETIC_RESULT_END => "MR4",
    ARITHMETIC_OVERFLOW => "OF",
}

register_mmio_family! {
    #[derive(Default)]
    pub struct SnesInterruptBits {
        #[bits(5)]
        pub character_conversion_dma_irq: bool,

        #[bits(7)]
        pub irq_from_sa1: bool,
    }
    symbols {
        0x2201 => "SIE",
        0x2202 => "SIC",
    }
}

register_mmio! {
    /// SCNT
    pub struct SnesCpuControl @ 0x2209 {
        #[bits(0..=3)]
        pub message_to_snes: u8,

        #[bits(4)]
        pub nmi_vector_from_port: bool,

        #[bits(6)]
        pub irq_vector_from_port: bool,

        #[bits(7)]
        pub irq_request: bool,
    }
}

register_mmio_family! {
    #[derive(Default)]
    pub struct Sa1InterruptBits {
        #[bits(4)]
        pub nmi_from_snes: bool,

        #[bits(5)]
        pub dma_irq: bool,

        #[bits(6)]
        pub timer_irq: bool,

        #[bits(7)]
        pub irq_from_snes: bool,
    }
    symbols {
        0x220A => "CIE",
        0x220B => "CIC",
    }
}

register_mmio_raw! {
    VECTOR_START => "CRVL",
    VECTOR_START + 1 => "CRVH",
    VECTOR_START + 2 => "CNVL",
    VECTOR_START + 3 => "CNVH",
    VECTOR_START + 4 => "CIVL",
    VECTOR_START + 5 => "CIVH",
    SNES_VECTOR_START => "SNVL",
    SNES_VECTOR_START + 1 => "SNVH",
    SNES_VECTOR_START + 2 => "SIVL",
    SNES_VECTOR_END => "SIVH",
}

register_mmio_family! {
    #[derive(Default)]
    pub struct SuperMmcBank {
        #[bits(0..=2)]
        pub rom_block: u8,

        #[bits(7)]
        pub lo_rom_enable: bool,
    }
    symbols {
        0x2220 => "CXB",
        0x2221 => "DXB",
        0x2222 => "EXB",
        0x2223 => "FXB",
    }
}

register_mmio_family! {
    #[derive(Default)]
    pub struct SnesBwramMapping {
        #[bits(0..=4)]
        pub bwram_block: u8,
    }
    symbols {
        0x2224 => "BMAPS",
    }
}

register_mmio_family! {
    #[derive(Default)]
    pub struct Sa1BwramMapping {
        #[bits(0..=6)]
        pub bwram_block: u8,

        #[bits(7)]
        pub bitmap: bool,
    }
    symbols {
        0x2225 => "BMAP",
    }
}

register_mmio_family! {
    #[derive(Default)]
    pub struct BwramWriteEnable {
        #[bits(7)]
        pub write_enable: bool,
    }
    symbols {
        0x2226 => "SBWE",
        0x2227 => "CBWE",
    }
}

register_mmio_family! {
    pub struct BwramWriteProtectedArea {
        #[bits(0..=3)]
        pub protected_size_exponent: u8,
    }
    symbols {
        0x2228 => "BWPA",
    }
}

impl Default for BwramWriteProtectedArea {
    fn default() -> Self {
        Self::from_raw(0xFF)
    }
}

register_mmio! {
    /// CDMA
    pub struct CharacterConversionDmaParameters @ 0x2231 {
        #[bits(0..=1)]
        pub color_depth: u8,

        #[bits(2..=4)]
        pub virtual_vram_width: u8,

        #[bits(7)]
        pub terminate: bool,
    }
}

register_mmio! {
    /// CFR
    pub struct Sa1Flags @ 0x2301 {
        #[bits(0..=3)]
        pub message_from_snes: u8,

        #[bits(4)]
        pub nmi_from_snes: bool,

        #[bits(5)]
        pub dma_irq: bool,

        #[bits(6)]
        pub timer_irq: bool,

        #[bits(7)]
        pub irq_from_snes: bool,
    }
}

register_mmio! {
    /// SFR
    pub struct SnesFlags @ 0x2300 {
        #[bits(0..=3)]
        pub message_from_sa1: u8,

        #[bits(4)]
        pub nmi_vector_from_port: bool,

        #[bits(5)]
        pub character_conversion_dma_irq: bool,

        #[bits(6)]
        pub irq_vector_from_port: bool,

        #[bits(7)]
        pub irq_from_sa1: bool,
    }
}

register_mmio_family! {
    /// SIWP / CIWP
    #[derive(Default)]
    pub struct IramWriteProtection {
        #[bits(0)]
        pub block_3000: bool,

        #[bits(1)]
        pub block_3100: bool,

        #[bits(2)]
        pub block_3200: bool,

        #[bits(3)]
        pub block_3300: bool,

        #[bits(4)]
        pub block_3400: bool,

        #[bits(5)]
        pub block_3500: bool,

        #[bits(6)]
        pub block_3600: bool,

        #[bits(7)]
        pub block_3700: bool,
    }
    symbols {
        0x2229 => "SIWP",
        0x222A => "CIWP",
    }
}

impl IramWriteProtection {
    pub(crate) fn write_enabled(&self, block: usize) -> bool {
        self.raw() & (1 << block) != 0
    }
}

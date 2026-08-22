use crate::macros::{register_mmio, register_mmio_family, register_mmio_raw};

pub(crate) const SIE: u16 = 0x2201;
pub(crate) const SIC: u16 = 0x2202;
pub(crate) const VECTOR_COUNT: usize = 3;
pub(crate) const VECTOR_START: u16 = 0x2203;
pub(crate) const VECTOR_END: u16 = 0x2208;
pub(crate) const ROM_BANK_START: u16 = 0x2220;
pub(crate) const ROM_BANK_END: u16 = 0x2223;
pub(crate) const BMAPS: u16 = 0x2224;
pub(crate) const BMAP: u16 = 0x2225;
pub(crate) const SBWE: u16 = 0x2226;
pub(crate) const CBWE: u16 = 0x2227;
pub(crate) const BWPA: u16 = 0x2228;
pub(crate) const SIWP: u16 = 0x2229;
pub(crate) const CIWP: u16 = 0x222A;

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

register_mmio_family! {
    #[derive(Default)]
    pub struct SnesInterruptBits {
        #[bits(5)]
        pub character_conversion_dma_irq: bool,

        #[bits(7)]
        pub sa1_irq: bool,
    }
    symbols {
        0x2201 => "SIE",
        0x2202 => "SIC",
    }
}

register_mmio_raw! {
    VECTOR_START => "CRVL",
    VECTOR_START + 1 => "CRVH",
    VECTOR_START + 2 => "CNVL",
    VECTOR_START + 3 => "CNVH",
    VECTOR_START + 4 => "CIVL",
    VECTOR_START + 5 => "CIVH",
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
        pub sa1_irq: bool,
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

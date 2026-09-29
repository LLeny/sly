#[chapa::bitfield(u16, order = lsb0)]
pub struct DisplayStatus {
    #[bits(0)]
    pub vblank: bool,

    #[bits(1)]
    pub hblank: bool,

    #[bits(2)]
    pub vcounter: bool,

    #[bits(3)]
    pub vblank_irq: bool,

    #[bits(4)]
    pub hblank_irq: bool,

    #[bits(5)]
    pub vcounter_irq: bool,

    #[bits(8..=15)]
    pub vcount_compare: u8,
}

impl DisplayStatus {
    pub const WRITE_MASK: u16 = Self::VBLANK_IRQ_MASK
        | Self::HBLANK_IRQ_MASK
        | Self::VCOUNTER_IRQ_MASK
        | Self::VCOUNT_COMPARE_MASK;

    pub const READ_MASK: u16 =
        Self::WRITE_MASK | Self::VBLANK_MASK | Self::HBLANK_MASK | Self::VCOUNTER_MASK;
}

#[chapa::bitfield(u16, order = lsb0)]
pub struct VerticalCount {
    #[bits(0..=7)]
    pub scanline: u8,
}

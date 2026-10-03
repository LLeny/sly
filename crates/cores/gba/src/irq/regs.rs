#[chapa::bitfield(u32, order = lsb0)]
pub struct InterruptMasterEnable {
    #[bits(0)]
    pub enable_interrupts: bool,
}

#[chapa::bitfield(u16, order = lsb0)]
pub struct InterruptEnable {
    #[bits(0)]
    pub vblank: bool,

    #[bits(1)]
    pub hblank: bool,

    #[bits(2)]
    pub vcounter: bool,

    #[bits(3)]
    pub timer0: bool,

    #[bits(4)]
    pub timer1: bool,

    #[bits(5)]
    pub timer2: bool,

    #[bits(6)]
    pub timer3: bool,

    #[bits(7)]
    pub serial: bool,

    #[bits(8)]
    pub dma0: bool,

    #[bits(9)]
    pub dma1: bool,

    #[bits(10)]
    pub dma2: bool,

    #[bits(11)]
    pub dma3: bool,

    #[bits(12)]
    pub keypad: bool,

    #[bits(13)]
    pub gamepak: bool,
}

#[chapa::bitfield(u16, order = lsb0)]
pub struct InterruptRequest {
    #[bits(0)]
    pub vblank: bool,

    #[bits(1)]
    pub hblank: bool,

    #[bits(2)]
    pub vcounter: bool,

    #[bits(3)]
    pub timer0: bool,

    #[bits(4)]
    pub timer1: bool,

    #[bits(5)]
    pub timer2: bool,

    #[bits(6)]
    pub timer3: bool,

    #[bits(7)]
    pub serial: bool,

    #[bits(8)]
    pub dma0: bool,

    #[bits(9)]
    pub dma1: bool,

    #[bits(10)]
    pub dma2: bool,

    #[bits(11)]
    pub dma3: bool,

    #[bits(12)]
    pub keypad: bool,

    #[bits(13)]
    pub gamepak: bool,
}

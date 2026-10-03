#[chapa::bitfield(u32, order = lsb0)]
pub struct Address {
    #[bits(0..=27)]
    pub address: u32,
}

impl Address {
    pub const SOURCE_MASKS: [u32; 4] = [0x07FF_FFFF, 0x0FFF_FFFF, 0x0FFF_FFFF, 0x0FFF_FFFF];
    pub const DESTINATION_MASKS: [u32; 4] = [0x07FF_FFFF, 0x07FF_FFFF, 0x07FF_FFFF, 0x0FFF_FFFF];
}

#[chapa::bitfield(u16, order = lsb0)]
pub struct WordCount {
    #[bits(0..=13, overlay = "dma0-2")]
    pub count: u16,

    #[bits(0..=15, overlay = "dma3")]
    pub count3: u16,
}

#[chapa::bitenum]
pub enum DestinationAddressControl {
    Increment = 0,
    Decrement = 1,
    Fixed = 2,
    IncrementReload = 3,
    #[fallback]
    Unknown = 4,
}

impl std::fmt::Display for DestinationAddressControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            DestinationAddressControl::Increment => "Increment",
            DestinationAddressControl::Decrement => "Decrement",
            DestinationAddressControl::Fixed => "Fixed",
            DestinationAddressControl::IncrementReload => "Increment / reload",
            DestinationAddressControl::Unknown => "Unknown",
        };
        write!(f, "{}", s)
    }
}

#[chapa::bitenum]
pub enum SourceAddressControl {
    Increment = 0,
    Decrement = 1,
    Fixed = 2,
    #[fallback]
    Prohibited = 3,
}

impl std::fmt::Display for SourceAddressControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            SourceAddressControl::Increment => "Increment",
            SourceAddressControl::Decrement => "Decrement",
            SourceAddressControl::Fixed => "Fixed",
            SourceAddressControl::Prohibited => "Prohibited",
        };
        write!(f, "{}", s)
    }
}

#[chapa::bitenum]
pub enum TransferSize {
    HalfWord = 0,
    Word = 1,
    #[fallback]
    Unknown = 2,
}

impl std::fmt::Display for TransferSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            TransferSize::HalfWord => "16-bit",
            TransferSize::Word => "32-bit",
            TransferSize::Unknown => "Unknown",
        };
        write!(f, "{}", s)
    }
}

#[chapa::bitenum]
pub enum StartTiming {
    Immediate = 0,
    VBlank = 1,
    HBlank = 2,
    Special = 3,
    #[fallback]
    Unknown = 4,
}

impl std::fmt::Display for StartTiming {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            StartTiming::Immediate => "Immediate",
            StartTiming::VBlank => "VBlank",
            StartTiming::HBlank => "HBlank",
            StartTiming::Special => "Special",
            StartTiming::Unknown => "Unknown",
        };
        write!(f, "{}", s)
    }
}

#[chapa::bitfield(u16, order = lsb0)]
pub struct Control {
    #[bits(5..=6)]
    pub dest_addr_ctrl: DestinationAddressControl,

    #[bits(7..=8)]
    pub src_addr_ctrl: SourceAddressControl,

    #[bits(9)]
    pub repeat: bool,

    #[bits(10)]
    pub transfer_size: TransferSize,

    // DMA3 only! reserved on DMA0–2.
    #[bits(11)]
    pub gamepak_drq: bool,

    #[bits(12..=13)]
    pub start_timing: StartTiming,

    #[bits(14)]
    pub irq_on_completion: bool,

    #[bits(15)]
    pub enable: bool,
}

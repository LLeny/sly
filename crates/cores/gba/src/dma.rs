use crate::dma::regs::{Address, Control, WordCount};

pub(crate) mod io;
pub(crate) mod regs;

pub struct Dma {
    pub src_addr: [Address; 4],
    pub dest_addr: [Address; 4],
    pub word_count: [WordCount; 4],
    pub control: [Control; 4],
}

impl Dma {
    pub(crate) fn save_io(&self, io: &mut [u8]) {
        for channel in 0..4 {
            let base = 0xB0 + (channel * 12);
            io[base..(base + 4)].copy_from_slice(&self.src_addr[channel].raw().to_le_bytes());
            io[(base + 4)..(base + 8)]
                .copy_from_slice(&self.dest_addr[channel].raw().to_le_bytes());
            io[(base + 8)..(base + 10)]
                .copy_from_slice(&self.word_count[channel].raw().to_le_bytes());
            io[(base + 10)..(base + 12)]
                .copy_from_slice(&self.control[channel].raw().to_le_bytes());
        }
    }

    pub(crate) fn load_io(&mut self, io: &[u8]) {
        for channel in 0..4 {
            let base = 0xB0 + (channel * 12);
            self.src_addr[channel] = Address::from_raw(
                u32::from_le_bytes(io[base..(base + 4)].try_into().unwrap())
                    & Address::SOURCE_MASKS[channel],
            );
            self.dest_addr[channel] = Address::from_raw(
                u32::from_le_bytes(io[(base + 4)..(base + 8)].try_into().unwrap())
                    & Address::DESTINATION_MASKS[channel],
            );
            self.word_count[channel] = WordCount::from_raw(
                u16::from_le_bytes(io[(base + 8)..(base + 10)].try_into().unwrap())
                    & if channel == 3 { 0xFFFF } else { 0x3FFF },
            );
            self.control[channel] = Control::from_raw(
                u16::from_le_bytes(io[(base + 10)..(base + 12)].try_into().unwrap())
                    & io::control_mask(channel),
            );
        }
    }

    pub fn new() -> Self {
        Self {
            src_addr: [Address::zeroed(); 4],
            dest_addr: [Address::zeroed(); 4],
            word_count: [WordCount::zeroed(); 4],
            control: [Control::zeroed(); 4],
        }
    }
}

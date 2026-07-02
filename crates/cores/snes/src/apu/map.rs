pub trait ApuMap {
    fn region(&self, addr: u16) -> Region;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    Ram,
    Register,
    Ipl,
}

pub struct HardwareMap;

impl ApuMap for HardwareMap {
    fn region(&self, addr: u16) -> Region {
        match addr {
            0x00F0..=0x00F7 | 0x00FA..=0x00FF => Region::Register,
            0xFFC0..=0xFFFF => Region::Ipl,
            _ => Region::Ram,
        }
    }
}

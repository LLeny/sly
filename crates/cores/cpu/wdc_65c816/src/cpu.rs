pub const FLAG_M: u8 = 0x20;
pub const FLAG_X: u8 = 0x10;

pub struct Cpu {
    pub pc: u16,
    pub flags: u8,
}

impl Default for Cpu {
    fn default() -> Self {
        Cpu {
            pc: 0,
            flags: FLAG_M | FLAG_X,
        }
    }
}

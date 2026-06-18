pub const FLAG_M: u8 = 0x20;
pub const FLAG_X: u8 = 0x10;

pub struct Cpu {
    pub pc: u16,
    pub flags: u8,
    pub mem: Vec<u8>,
}

impl Default for Cpu {
    fn default() -> Self {
        Cpu {
            pc: 0,
            flags: FLAG_M | FLAG_X,
            mem: vec![0; 0x1_0000],
        }
    }
}

impl Cpu {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(&mut self, addr: u16, bytes: &[u8]) {
        for (i, b) in bytes.iter().enumerate() {
            self.mem[(addr as usize + i) & 0xffff] = *b;
        }

        self.pc = addr;
    }
}

use wdc_65c816::Bus;

pub struct Mmu {
    pub ram: Vec<u8>,
}

impl Mmu {
    pub fn load(&mut self, bank: u8, addr: u16, bytes: &[u8]) {
        let start = ((bank as usize) << 16) | (addr as usize);
        self.ram[start..(start + bytes.len())].copy_from_slice(bytes);
    }

    pub fn read16(&self, bank: u8, addr: u16) -> u16 {
        let start = ((bank as usize) << 16) | (addr as usize);
        let lo = self.ram[start] as u16;
        let hi = self.ram[start + 1] as u16;
        lo | (hi << 8)
    }
}

impl Bus for Mmu {
    fn read(&self, addr: u32) -> u8 {
        self.ram[(addr as usize) & 0xffff]
    }
}

impl Default for Mmu {
    fn default() -> Self {
        Self {
            ram: vec![0; 1 << 24],
        }
    }
}

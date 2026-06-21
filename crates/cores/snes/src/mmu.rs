use wdc_65c816::Bus;

pub struct Mmu {
    pub ram: Vec<u8>,
}

impl Default for Mmu {
    fn default() -> Self {
        Self {
            ram: vec![0; 0x1_0000],
        }
    }
}

impl Mmu {
    pub fn load(&mut self, addr: u16, bytes: &[u8]) {
        self.ram[(addr as usize)..(addr as usize + bytes.len())].copy_from_slice(bytes);
    }
}

impl Bus for Mmu {
    fn read(&self, addr: u32) -> u8 {
        self.ram[(addr as usize) & 0xffff]
    }
}

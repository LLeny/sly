use crate::Cartridge;

pub struct Bus {
    cartridge: Cartridge,
}

impl Bus {
    pub fn new(cartridge: Cartridge) -> Self {
        Self { cartridge }
    }

    pub fn cartridge(&self) -> &Cartridge {
        &self.cartridge
    }

    pub fn peek8(&self, _address: u32) -> u8 {
        todo!("bus inspection")
    }

    pub fn peek16(&self, _address: u32) -> u16 {
        todo!("bus inspection")
    }

    pub fn peek32(&self, _address: u32) -> u32 {
        todo!("bus inspection")
    }

    pub fn read8(&mut self, _address: u32) -> u8 {
        todo!("bus read")
    }

    pub fn read16(&mut self, _address: u32) -> u16 {
        todo!("bus read")
    }

    pub fn read32(&mut self, _address: u32) -> u32 {
        todo!("bus read")
    }

    pub fn write8(&mut self, _address: u32, _value: u8) {
        todo!("bus write")
    }

    pub fn write16(&mut self, _address: u32, _value: u16) {
        todo!("bus write")
    }

    pub fn write32(&mut self, _address: u32, _value: u32) {
        todo!("bus write")
    }

    pub fn fetch16(&mut self, address: u32) -> u16 {
        self.read16(address)
    }

    pub fn fetch32(&mut self, address: u32) -> u32 {
        self.read32(address)
    }
}

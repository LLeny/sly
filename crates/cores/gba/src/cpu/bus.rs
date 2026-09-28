pub trait Bus {
    fn read8(&mut self, address: u32) -> u8;
    fn read16(&mut self, address: u32) -> u16;
    fn read32(&mut self, address: u32) -> u32;

    fn write8(&mut self, address: u32, value: u8);
    fn write16(&mut self, address: u32, value: u16);
    fn write32(&mut self, address: u32, value: u32);

    fn fetch16(&mut self, address: u32) -> u16;
    fn fetch32(&mut self, address: u32) -> u32;
}

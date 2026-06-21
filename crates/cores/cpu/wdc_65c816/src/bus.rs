pub trait Bus {
    fn read(&self, addr: u32) -> u8;
}

use super::Width;

pub(super) struct Display;

impl Display {
    pub(super) fn new() -> Self {
        Self
    }

    pub(super) fn peek(&self, _address: u32, _width: Width) -> u32 {
        todo!("PPU inspection")
    }

    pub(super) fn read(&mut self, _address: u32, _width: Width) -> u32 {
        todo!("PPU read")
    }

    pub(super) fn write(&mut self, _address: u32, _width: Width, _value: u32) {
        todo!("PPU write")
    }
}

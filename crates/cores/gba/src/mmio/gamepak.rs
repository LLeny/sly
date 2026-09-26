use super::Width;
use crate::Cartridge;

pub(super) struct GamePak {
    cartridge: Cartridge,
    sram: Vec<u8>, // 0x10000
}

impl GamePak {
    pub(super) fn new(cartridge: Cartridge) -> Self {
        Self {
            cartridge,
            sram: vec![0; 0x10000],
        }
    }

    fn route(&self, address: u32, width: Width) -> Option<&[u8]> {
        let (memory, offset): (&[u8], _) = match address {
            0x0800_0000..=0x0DFF_FFFF => (self.cartridge.rom(), address & 0x01FF_FFFF),
            0x0E00_0000..=0x0E00_FFFF => (&self.sram, address & 0xFFFF),
            _ => return None,
        };
        memory.get(width.span(offset))
    }

    fn route_mut(&mut self, address: u32, width: Width) -> Option<&mut [u8]> {
        match address {
            0x0E00_0000..=0x0E00_FFFF => self.sram.get_mut(width.span(address & 0xFFFF)),
            _ => None,
        }
    }

    pub(super) fn cartridge(&self) -> &Cartridge {
        &self.cartridge
    }

    pub(super) fn peek(&self, address: u32, width: Width) -> u32 {
        self.route(address, width).map_or(0, super::load)
    }

    pub(super) fn read(&mut self, address: u32, width: Width) -> u32 {
        self.peek(address, width)
    }

    pub(super) fn write(&mut self, address: u32, width: Width, value: u32) {
        match self.route_mut(address, width) {
            Some(memory) => super::store(memory, value),
            None => tracing::error!(
                address = %format_args!("{address:#010X}"),
                "Attempted write to read-only GamePak memory"
            ),
        }
    }
}

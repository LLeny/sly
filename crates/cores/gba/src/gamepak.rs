pub(crate) mod io;

use crate::Cartridge;
use crate::bus::{Width, load, store};

const ROM_BASE: u32 = 0x0800_0000;
const ROM_END: u32 = 0x0DFF_FFFF;
const ROM_MASK: u32 = 0x01FF_FFFF;

pub(crate) const SRAM_BASE: u32 = 0x0E00_0000;
pub(crate) const SRAM_SIZE: u32 = 0x1_0000;
pub(crate) const SRAM_END: u32 = SRAM_BASE + SRAM_SIZE - 1;

pub struct GamePak {
    cartridge: Cartridge,
    pub(crate) sram: Vec<u8>,
}

impl GamePak {
    pub(crate) fn empty_cartridge() -> Self {
        Self {
            cartridge: Cartridge::default(),
            sram: vec![0; SRAM_SIZE as usize],
        }
    }

    pub fn new(cartridge: Cartridge) -> Self {
        Self {
            cartridge,
            sram: vec![0; SRAM_SIZE as usize],
        }
    }

    pub fn cartridge(&self) -> &Cartridge {
        &self.cartridge
    }

    pub(crate) fn peek(&self, address: u32, width: Width) -> u32 {
        self.route(address, width).map_or(0, load)
    }

    pub(crate) fn read(&mut self, address: u32, width: Width) -> u32 {
        self.peek(address, width)
    }

    pub(crate) fn write(&mut self, address: u32, width: Width, value: u32) {
        match self.route_mut(address, width) {
            Some(memory) => store(memory, value),
            None => tracing::error!(
                address = %format_args!("{address:#010X}"),
                "Attempted write to read-only GamePak memory"
            ),
        }
    }

    fn route(&self, address: u32, width: Width) -> Option<&[u8]> {
        let (memory, offset): (&[u8], _) = match address {
            ROM_BASE..=ROM_END => (self.cartridge.data(), address & ROM_MASK),
            SRAM_BASE..=SRAM_END => (&self.sram, address - SRAM_BASE),
            _ => return None,
        };

        memory.get(width.span(offset))
    }

    fn route_mut(&mut self, address: u32, width: Width) -> Option<&mut [u8]> {
        let (memory, offset): (&mut [u8], _) = match address {
            SRAM_BASE..=SRAM_END => (&mut self.sram, address - SRAM_BASE),
            _ => return None,
        };

        memory.get_mut(width.span(offset))
    }
}

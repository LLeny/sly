mod display;
mod gamepak;

use std::ops::Range;

use crate::Cartridge;
use display::Display;
use gamepak::GamePak;

// https://github.com/coolbho3k/emibios
const BIOS: &[u8] = include_bytes!("../../../../bins/gba/emibios.bin");

pub struct Mmio {
    bios: Vec<u8>,  // 0x4000
    ewram: Vec<u8>, // 0x40000
    iwram: Vec<u8>, // 0x8000
    io: Vec<u8>,    // 0x400
    display: Display,
    gamepak: GamePak,
}

impl Mmio {
    pub fn new(cartridge: Cartridge) -> Self {
        Self {
            bios: BIOS.to_vec(),
            ewram: vec![0; 0x40000],
            iwram: vec![0; 0x8000],
            io: vec![0; 0x400],
            display: Display::new(),
            gamepak: GamePak::new(cartridge),
        }
    }

    fn route(&self, address: u32, width: Width) -> Option<&[u8]> {
        let (memory, offset): (&[u8], _) = match address {
            0x0000_0000..=0x0000_3FFF => (&self.bios, address),
            0x0200_0000..=0x0203_FFFF => (&self.ewram, address & 0x3FFFF),
            0x0300_0000..=0x0300_7FFF => (&self.iwram, address & 0x7FFF),
            0x0400_0000..=0x0400_03FF => (&self.io, address & 0x3FF),
            _ => return None,
        };
        memory.get(width.span(offset))
    }

    fn route_mut(&mut self, address: u32, width: Width) -> Option<&mut [u8]> {
        let (memory, offset): (&mut [u8], _) = match address {
            0x0200_0000..=0x0203_FFFF => (&mut self.ewram, address & 0x3FFFF),
            0x0300_0000..=0x0300_7FFF => (&mut self.iwram, address & 0x7FFF),
            0x0400_0000..=0x0400_03FF => (&mut self.io, address & 0x3FF),
            _ => return None,
        };
        memory.get_mut(width.span(offset))
    }

    pub fn cartridge(&self) -> &Cartridge {
        self.gamepak.cartridge()
    }

    pub fn peek8(&self, address: u32) -> u8 {
        self.peek(address, Width::Byte) as u8
    }

    pub fn peek16(&self, address: u32) -> u16 {
        self.peek(address, Width::Halfword) as u16
    }

    pub fn peek32(&self, address: u32) -> u32 {
        self.peek(address, Width::Word)
    }

    pub fn read8(&mut self, address: u32) -> u8 {
        self.read(address, Width::Byte) as u8
    }

    pub fn read16(&mut self, address: u32) -> u16 {
        self.read(address, Width::Halfword) as u16
    }

    pub fn read32(&mut self, address: u32) -> u32 {
        self.read(address, Width::Word)
    }

    pub fn write8(&mut self, address: u32, value: u8) {
        self.write(address, Width::Byte, u32::from(value));
    }

    pub fn write16(&mut self, address: u32, value: u16) {
        self.write(address, Width::Halfword, u32::from(value));
    }

    pub fn write32(&mut self, address: u32, value: u32) {
        self.write(address, Width::Word, value);
    }

    pub fn fetch16(&mut self, address: u32) -> u16 {
        self.read16(address)
    }

    pub fn fetch32(&mut self, address: u32) -> u32 {
        self.read32(address)
    }

    fn peek(&self, address: u32, width: Width) -> u32 {
        match Self::target(address) {
            Target::Memory => self.route(address, width).map_or(0, load),
            Target::Display => self.display.peek(address, width),
            Target::GamePak => self.gamepak.peek(address, width),
        }
    }

    fn read(&mut self, address: u32, width: Width) -> u32 {
        match Self::target(address) {
            Target::Memory => self.route(address, width).map_or(0, load),
            Target::Display => self.display.read(address, width),
            Target::GamePak => self.gamepak.read(address, width),
        }
    }

    fn write(&mut self, address: u32, width: Width, value: u32) {
        match Self::target(address) {
            Target::Memory => match self.route_mut(address, width) {
                Some(memory) => store(memory, value),
                None => tracing::error!(
                    address = %format_args!("{address:#010X}"),
                    "Attempted write to read-only or unmapped memory"
                ),
            },
            Target::Display => self.display.write(address, width, value),
            Target::GamePak => self.gamepak.write(address, width, value),
        }
    }

    fn target(address: u32) -> Target {
        match address {
            0x0400_0000..=0x0400_0057 | 0x0500_0000..=0x07FF_FFFF => Target::Display,
            0x0800_0000..=0x0FFF_FFFF => Target::GamePak,
            _ => Target::Memory,
        }
    }
}

#[inline(always)]
fn load(memory: &[u8]) -> u32 {
    let mut bytes = [0; 4];
    bytes[..memory.len()].copy_from_slice(memory);
    u32::from_le_bytes(bytes)
}

#[inline(always)]
fn store(memory: &mut [u8], value: u32) {
    memory.copy_from_slice(&value.to_le_bytes()[..memory.len()]);
}

#[derive(Clone, Copy, Debug)]
enum Width {
    Byte,
    Halfword,
    Word,
}

impl Width {
    const fn bytes(self) -> usize {
        match self {
            Width::Byte => 1,
            Width::Halfword => 2,
            Width::Word => 4,
        }
    }

    fn span(self, offset: u32) -> Range<usize> {
        let start = offset as usize;
        start..start + self.bytes()
    }
}

#[derive(Clone, Copy, Debug)]
enum Target {
    Memory,
    Display,
    GamePak,
}

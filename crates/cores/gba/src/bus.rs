use std::ops::Range;

use crate::cpu::Bus;
use crate::gamepak::GamePak;
use crate::mmio::{Mmio, MmioMap, Target, mask};
use crate::ppu::Ppu;

pub struct CpuBus<'a> {
    pub(crate) ppu: &'a mut Ppu,
    pub(crate) gamepak: &'a mut GamePak,
    pub(crate) mmio: &'a mut Mmio,
}

pub struct BusView<'a> {
    pub(crate) ppu: &'a Ppu,
    pub(crate) gamepak: &'a GamePak,
    pub(crate) mmio: &'a Mmio,
}

impl CpuBus<'_> {
    pub fn view(&self) -> BusView<'_> {
        BusView {
            ppu: self.ppu,
            gamepak: self.gamepak,
            mmio: self.mmio,
        }
    }

    fn read(&mut self, address: u32, width: Width) -> u32 {
        let mut value = 0;
        let mut offset = 0;

        while offset < width.bytes() {
            let lane = Lane::new(&self.mmio.map, address, width, offset);
            let part = match lane.target {
                Target::Device(handler) => (handler.read)(self, lane.address, lane.width()),
                Target::Register(register) => {
                    register.read(self, lane.mask() << lane.shift) >> lane.shift
                }
            };

            value |= lane.place(part);
            offset += lane.bytes;
        }

        value
    }

    fn write(&mut self, address: u32, width: Width, value: u32) {
        let mut offset = 0;

        while offset < width.bytes() {
            let lane = Lane::new(&self.mmio.map, address, width, offset);
            let part = lane.extract(value);
            match lane.target {
                Target::Device(handler) => (handler.write)(self, lane.address, lane.width(), part),
                Target::Register(register) => {
                    register.write(self, part << lane.shift, lane.mask() << lane.shift);
                }
            }

            offset += lane.bytes;
        }
    }
}

impl BusView<'_> {
    pub fn peek8(&self, address: u32) -> u8 {
        self.peek(address, Width::Byte) as u8
    }

    pub fn peek16(&self, address: u32) -> u16 {
        self.peek(address, Width::Halfword) as u16
    }

    pub fn peek32(&self, address: u32) -> u32 {
        self.peek(address, Width::Word)
    }

    fn peek(&self, address: u32, width: Width) -> u32 {
        let mut value = 0;
        let mut offset = 0;

        while offset < width.bytes() {
            let lane = Lane::new(&self.mmio.map, address, width, offset);
            let part = match lane.target {
                Target::Device(handler) => (handler.peek)(self, lane.address, lane.width()),
                Target::Register(register) => register.peek(self) >> lane.shift,
            };

            value |= lane.place(part);
            offset += lane.bytes;
        }

        value
    }
}

impl Bus for CpuBus<'_> {
    fn read8(&mut self, address: u32) -> u8 {
        self.read(address, Width::Byte) as u8
    }

    fn read16(&mut self, address: u32) -> u16 {
        self.read(address, Width::Halfword) as u16
    }

    fn read32(&mut self, address: u32) -> u32 {
        self.read(address, Width::Word)
    }

    fn write8(&mut self, address: u32, value: u8) {
        self.write(address, Width::Byte, u32::from(value));
    }

    fn write16(&mut self, address: u32, value: u16) {
        self.write(address, Width::Halfword, u32::from(value));
    }

    fn write32(&mut self, address: u32, value: u32) {
        self.write(address, Width::Word, value);
    }

    fn fetch16(&mut self, address: u32) -> u16 {
        self.read16(address)
    }

    fn fetch32(&mut self, address: u32) -> u32 {
        self.read32(address)
    }
}

struct Lane {
    target: Target,
    address: u32,
    offset: u32,
    bytes: u32,
    shift: u32,
}

impl Lane {
    fn new(map: &MmioMap, address: u32, width: Width, offset: u32) -> Self {
        let address = address.wrapping_add(offset);
        let mapping = map.mapping(address);
        let bytes = (mapping.end - address).min(width.bytes() - offset - 1) + 1;
        let bytes = match mapping.target {
            Target::Device(_) => Width::fitting(bytes).bytes(),
            Target::Register(_) => bytes,
        };

        Self {
            target: mapping.target,
            address,
            offset,
            bytes,
            shift: (address - mapping.start) * 8,
        }
    }

    fn width(&self) -> Width {
        Width::fitting(self.bytes)
    }

    fn mask(&self) -> u32 {
        mask((self.bytes * 8) as u8)
    }

    fn place(&self, part: u32) -> u32 {
        (part & self.mask()) << (self.offset * 8)
    }

    fn extract(&self, value: u32) -> u32 {
        (value >> (self.offset * 8)) & self.mask()
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Width {
    Byte,
    Halfword,
    Word,
}

impl Width {
    const fn fitting(bytes: u32) -> Self {
        match bytes {
            1 => Self::Byte,
            2 | 3 => Self::Halfword,
            _ => Self::Word,
        }
    }

    pub(crate) const fn bytes(self) -> u32 {
        match self {
            Width::Byte => 1,
            Width::Halfword => 2,
            Width::Word => 4,
        }
    }

    pub(crate) const fn span(self, offset: u32) -> Range<usize> {
        let start = offset as usize;
        start..start + self.bytes() as usize
    }
}

#[inline(always)]
pub(crate) fn load(memory: &[u8]) -> u32 {
    let mut bytes = [0; 4];
    bytes[..memory.len()].copy_from_slice(memory);
    u32::from_le_bytes(bytes)
}

#[inline(always)]
pub(crate) fn store(memory: &mut [u8], value: u32) {
    memory.copy_from_slice(&value.to_le_bytes()[..memory.len()]);
}

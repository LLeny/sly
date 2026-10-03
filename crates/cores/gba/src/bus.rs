use std::ops::Range;

use crate::cpu::Bus;
use crate::dma::Dma;
use crate::gamepak::GamePak;
use crate::input::Input;
use crate::irq::Irq;
use crate::mmio::{Mapping, Mmio, MmioMap, Target, mask};
use crate::ppu::Ppu;

pub struct CpuBus<'a> {
    pub(crate) dma: &'a mut Dma,
    pub(crate) ppu: &'a mut Ppu,
    pub(crate) gamepak: &'a mut GamePak,
    pub(crate) input: &'a mut Input,
    pub(crate) mmio: &'a mut Mmio,
    pub(crate) irq: &'a mut Irq,
}

pub struct BusView<'a> {
    pub(crate) dma: &'a Dma,
    pub(crate) ppu: &'a Ppu,
    pub(crate) gamepak: &'a GamePak,
    pub(crate) input: &'a Input,
    pub(crate) mmio: &'a Mmio,
    pub(crate) irq: &'a Irq,
}

impl CpuBus<'_> {
    pub fn view(&self) -> BusView<'_> {
        BusView {
            dma: self.dma,
            ppu: self.ppu,
            gamepak: self.gamepak,
            input: self.input,
            mmio: self.mmio,
            irq: self.irq,
        }
    }

    #[inline]
    fn read(&mut self, address: u32, width: Width) -> u32 {
        let mapping = self.mmio.map.mapping(address);

        match mapping.device_covering(address, width) {
            Some(handler) => (handler.read)(self, address, width),
            None => self.read_lanes(mapping, address, width),
        }
    }

    #[inline]
    fn write(&mut self, address: u32, width: Width, value: u32) {
        let mapping = self.mmio.map.mapping(address);

        match mapping.device_covering(address, width) {
            Some(handler) => (handler.write)(self, address, width, value),
            None => self.write_lanes(mapping, address, width, value),
        }
    }

    #[inline(never)]
    fn read_lanes(&mut self, mapping: Mapping, address: u32, width: Width) -> u32 {
        let mut value = 0;
        let mut next = Some(Lane::new(mapping, address, width, 0));

        while let Some(lane) = next {
            let part = match lane.target {
                Target::Device(handler) => (handler.read)(self, lane.address, lane.width()),
                Target::Register(register) => {
                    register.read(self, lane.mask() << lane.shift) >> lane.shift
                }
            };

            value |= lane.place(part);
            next = lane.next(&self.mmio.map, width);
        }

        value
    }

    #[inline(never)]
    fn write_lanes(&mut self, mapping: Mapping, address: u32, width: Width, value: u32) {
        let mut next = Some(Lane::new(mapping, address, width, 0));

        while let Some(lane) = next {
            let part = lane.extract(value);

            match lane.target {
                Target::Device(handler) => (handler.write)(self, lane.address, lane.width(), part),
                Target::Register(register) => {
                    register.write(self, part << lane.shift, lane.mask() << lane.shift);
                }
            }

            next = lane.next(&self.mmio.map, width);
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

    #[inline]
    fn peek(&self, address: u32, width: Width) -> u32 {
        let mapping = self.mmio.map.mapping(address);

        match mapping.device_covering(address, width) {
            Some(handler) => (handler.peek)(self, address, width),
            None => self.peek_lanes(mapping, address, width),
        }
    }

    #[inline(never)]
    fn peek_lanes(&self, mapping: Mapping, address: u32, width: Width) -> u32 {
        let mut value = 0;
        let mut next = Some(Lane::new(mapping, address, width, 0));

        while let Some(lane) = next {
            let part = match lane.target {
                Target::Device(handler) => (handler.peek)(self, lane.address, lane.width()),
                Target::Register(register) => register.peek(self) >> lane.shift,
            };

            value |= lane.place(part);
            next = lane.next(&self.mmio.map, width);
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
    fn new(mapping: Mapping, address: u32, width: Width, offset: u32) -> Self {
        let bytes = (mapping.end - address).min(width.bytes() - offset - 1) + 1;
        let (bytes, shift) = match mapping.target {
            Target::Device(_) => (Width::fitting(bytes).bytes(), 0),
            Target::Register(_) => (bytes, (address - mapping.start) * 8),
        };

        Self {
            target: mapping.target,
            address,
            offset,
            bytes,
            shift,
        }
    }

    fn next(&self, map: &MmioMap, width: Width) -> Option<Self> {
        let offset = self.offset + self.bytes;

        (offset < width.bytes()).then(|| {
            let address = self.address.wrapping_add(self.bytes);
            Self::new(map.mapping(address), address, width, offset)
        })
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    match *memory {
        [byte] => u32::from(byte),
        [lo, hi] => u32::from_le_bytes([lo, hi, 0, 0]),
        [a, b, c, d] => u32::from_le_bytes([a, b, c, d]),
        _ => unreachable!("bus loads are 1, 2 or 4 bytes"),
    }
}

#[inline(always)]
pub(crate) fn store(memory: &mut [u8], value: u32) {
    memory.copy_from_slice(&value.to_le_bytes()[..memory.len()]);
}

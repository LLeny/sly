mod register;

pub use register::{Register, RegisterInfo};
pub(crate) use register::{RegisterAccess, mask};

use std::ops::RangeInclusive;

use crate::bus::{BusView, CpuBus, Width, load, store};

pub(crate) const IO_BASE: u32 = 0x0400_0000;
pub(crate) const IO_SIZE: u32 = 0x400;
pub(crate) const IO_END: u32 = IO_BASE + IO_SIZE - 1;

const BIOS_BASE: u32 = 0x0000_0000;
const BIOS_END: u32 = BIOS_BASE + BIOS.len() as u32 - 1;

pub(crate) const EWRAM_BASE: u32 = 0x0200_0000;
pub(crate) const EWRAM_SIZE: u32 = 0x4_0000;
pub(crate) const EWRAM_END: u32 = 0x02FF_FFFF;

pub(crate) const IWRAM_BASE: u32 = 0x0300_0000;
pub(crate) const IWRAM_SIZE: u32 = 0x8000;
pub(crate) const IWRAM_END: u32 = 0x03FF_FFFF;

const REGION_SHIFT: u32 = 24;
const REGION_COUNT: usize = 1 << (32 - REGION_SHIFT);

// https://github.com/coolbho3k/emibios
const BIOS: &[u8] = include_bytes!("../../../../bins/gba/emibios.bin");

pub(crate) struct Mmio {
    pub(crate) map: MmioMap,
    pub(crate) ewram: Vec<u8>,
    pub(crate) iwram: Vec<u8>,
    pub(crate) io: Vec<u8>, // Backing storage for unimplemented I/O registers.
}

impl Mmio {
    pub(crate) fn new(map: MmioMap) -> Self {
        Self {
            map,
            ewram: vec![0; EWRAM_SIZE as usize],
            iwram: vec![0; IWRAM_SIZE as usize],
            io: vec![0; IO_SIZE as usize],
        }
    }

    pub(crate) fn peek_memory(&self, address: u32, width: Width) -> u32 {
        self.route(address, width).map_or(0, load)
    }

    pub(crate) fn write_memory(&mut self, address: u32, width: Width, value: u32) {
        match self.route_mut(address, width) {
            Some(memory) => store(memory, value),
            None => tracing::error!(
                address = %format_args!("{address:#010X}"),
                "Attempted write to read-only or unmapped memory"
            ),
        }
    }

    fn route(&self, address: u32, width: Width) -> Option<&[u8]> {
        let (memory, offset): (&[u8], _) = match address {
            BIOS_BASE..=BIOS_END => (BIOS, address - BIOS_BASE),
            EWRAM_BASE..=EWRAM_END => (&self.ewram, address & (EWRAM_SIZE - 1)),
            IWRAM_BASE..=IWRAM_END => (&self.iwram, address & (IWRAM_SIZE - 1)),
            IO_BASE..=IO_END => (&self.io, address - IO_BASE),
            _ => return None,
        };

        memory.get(width.span(offset))
    }

    fn route_mut(&mut self, address: u32, width: Width) -> Option<&mut [u8]> {
        let (memory, offset): (&mut [u8], _) = match address {
            EWRAM_BASE..=EWRAM_END => (&mut self.ewram, address & (EWRAM_SIZE - 1)),
            IWRAM_BASE..=IWRAM_END => (&mut self.iwram, address & (IWRAM_SIZE - 1)),
            IO_BASE..=IO_END => (&mut self.io, address - IO_BASE),
            _ => return None,
        };

        memory.get_mut(width.span(offset))
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Handler {
    pub(crate) read: fn(&mut CpuBus<'_>, u32, Width) -> u32,
    pub(crate) write: fn(&mut CpuBus<'_>, u32, Width, u32),
    pub(crate) peek: fn(&BusView<'_>, u32, Width) -> u32,
}

impl Handler {
    pub(crate) const MEMORY: Self = Self {
        read: |bus, address, width| bus.mmio.peek_memory(address, width),
        write: |bus, address, width, value| bus.mmio.write_memory(address, width, value),
        peek: |bus, address, width| bus.mmio.peek_memory(address, width),
    };
}

#[derive(Clone, Copy)]
pub(crate) enum Target {
    Device(Handler),
    Register(&'static dyn RegisterAccess),
}

#[derive(Clone, Copy)]
pub(crate) struct Mapping {
    pub(crate) start: u32,
    pub(crate) end: u32,
    pub(crate) target: Target,
}

impl Mapping {
    pub(crate) fn device_covering(&self, address: u32, width: Width) -> Option<Handler> {
        match self.target {
            Target::Device(handler) if width.bytes() - 1 <= self.end - address => Some(handler),
            _ => None,
        }
    }
}

pub(crate) struct MmioMap {
    mappings: Vec<Mapping>,
    fallback: Handler,
    regions: [Option<Mapping>; REGION_COUNT],
}

impl MmioMap {
    pub(crate) const fn new(fallback: Handler) -> Self {
        Self {
            mappings: Vec::new(),
            fallback,
            regions: [None; REGION_COUNT],
        }
    }

    pub(crate) fn map(&mut self, range: RangeInclusive<u32>, handler: Handler) {
        self.insert(range, Target::Device(handler));
    }

    pub(crate) fn register(&mut self, register: &'static dyn RegisterAccess) {
        let info = register.info();
        assert!(
            matches!(info.bits, 8 | 16 | 32),
            "unsupported register width"
        );

        let bytes = u32::from(info.bits / 8);
        assert!(
            (IO_BASE..=IO_END + 1 - bytes).contains(&info.address),
            "register is outside the I/O window"
        );
        assert!(
            info.address.is_multiple_of(bytes),
            "unaligned register address"
        );

        self.insert(
            info.address..=info.address + bytes - 1,
            Target::Register(register),
        );
    }

    pub(crate) fn registers(&self) -> impl Iterator<Item = &'static dyn RegisterAccess> + '_ {
        self.mappings
            .iter()
            .filter_map(|mapping| match mapping.target {
                Target::Register(register) => Some(register),
                Target::Device(_) => None,
            })
    }

    fn insert(&mut self, range: RangeInclusive<u32>, target: Target) {
        let (start, end) = range.into_inner();
        assert!(
            start <= end,
            "Mapped range {start:#010X}..={end:#010X} is empty"
        );

        let index = self
            .mappings
            .partition_point(|mapping| mapping.start < start);
        let after_previous = index == 0 || self.mappings[index - 1].end < start;
        let before_next = self.mappings.get(index).is_none_or(|next| end < next.start);
        assert!(
            after_previous && before_next,
            "Mapped range {start:#010X}..={end:#010X} overlaps an existing mapping"
        );

        self.mappings.insert(index, Mapping { start, end, target });
        self.rebuild_regions();
    }

    fn rebuild_regions(&mut self) {
        self.regions = std::array::from_fn(|region| {
            let start = (region as u32) << REGION_SHIFT;
            let end = start | mask(REGION_SHIFT as u8);
            let mapping = self.mapping_slow(start);

            (mapping.end >= end).then_some(mapping)
        });
    }

    pub(crate) fn mapping(&self, address: u32) -> Mapping {
        self.regions[(address >> REGION_SHIFT) as usize]
            .unwrap_or_else(|| self.mapping_slow(address))
    }

    fn mapping_slow(&self, address: u32) -> Mapping {
        let index = self
            .mappings
            .partition_point(|mapping| mapping.start <= address);

        match self.mappings[..index].last() {
            Some(mapping) if address <= mapping.end => *mapping,
            _ => Mapping {
                start: address,
                end: self
                    .mappings
                    .get(index)
                    .map_or(u32::MAX, |next| next.start - 1),
                target: Target::Device(self.fallback),
            },
        }
    }
}

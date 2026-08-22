use crate::mmio::{MemoryMap, OPEN_BUS};
use crate::{Cartridge, SnesBus};

pub const IO_BASE: u16 = 0x2000;
pub const IO_SIZE: usize = 0x4000;
/// Last address in the window, inclusive.
pub const IO_END: u16 = IO_BASE + IO_SIZE as u16 - 1;

const HANDLER_SLOTS: usize = u8::MAX as usize + 1;

pub struct Handler<M: MemoryMap> {
    pub read: fn(&mut SnesBus<M>, u16) -> u8,
    pub write: fn(&mut SnesBus<M>, u16, u8),
    pub peek: fn(&SnesBus<M>, u16) -> u8, // side effect free
}

impl<M: MemoryMap> Clone for Handler<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M: MemoryMap> Copy for Handler<M> {}

pub(crate) fn unmapped_read(reg: u16) -> u8 {
    tracing::error!(
        reg = format!("{:04X}", reg),
        "Read from unmapped I/O register"
    );
    OPEN_BUS
}

pub(crate) fn unmapped_write(reg: u16, value: u8) {
    tracing::error!(
        reg = format!("{:04X}", reg),
        value = format!("{:02X}", value),
        "Write to unmapped I/O register"
    );
}

impl<M: MemoryMap> Handler<M> {
    pub const UNMAPPED: Self = Self {
        read: |_, reg| unmapped_read(reg),
        write: |_, reg, value| unmapped_write(reg, value),
        peek: |_, _| OPEN_BUS,
    };
}

pub struct RegEntry<M: MemoryMap> {
    pub start: u16,
    pub end: u16,
    pub handler: Handler<M>,
}

impl<M: MemoryMap> RegEntry<M> {
    pub const fn range(start: u16, end: u16, handler: Handler<M>) -> Self {
        Self {
            start,
            end,
            handler,
        }
    }

    pub const fn at(addr: u16, handler: Handler<M>) -> Self {
        Self::range(addr, addr, handler)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Access {
    Handled,
    Unmapped,
    Ignored,
}

pub(crate) trait IoRegister {
    const START: u16;
    const END: u16 = Self::START;

    const READ: Access = Access::Handled;
    const WRITE: Access = Access::Handled;

    /// Defaults to `peek`. Override only when reading has a side effect.
    fn read<M: MemoryMap>(snes: &mut SnesBus<M>, reg: u16) -> u8 {
        Self::peek(snes, reg)
    }

    /// Defaults to reporting itself, so a register with no write port needs no body.
    fn write<M: MemoryMap>(_snes: &mut SnesBus<M>, reg: u16, value: u8) {
        unmapped_write(reg, value);
    }

    fn peek<M: MemoryMap>(_snes: &SnesBus<M>, _reg: u16) -> u8 {
        OPEN_BUS
    }

    fn entry<M: MemoryMap>() -> RegEntry<M>
    where
        Self: Sized,
    {
        RegEntry::range(
            Self::START,
            Self::END,
            Handler {
                read: match Self::READ {
                    Access::Handled => Self::read::<M>,
                    Access::Unmapped => |_, reg| unmapped_read(reg),
                    Access::Ignored => |_, _| OPEN_BUS,
                },
                write: match Self::WRITE {
                    Access::Handled => Self::write::<M>,
                    Access::Unmapped => |_, reg, value| unmapped_write(reg, value),
                    Access::Ignored => |_, _, _| {},
                },
                peek: Self::peek::<M>,
            },
        )
    }
}

pub struct DeviceTable<M: MemoryMap> {
    index: Box<[u8; IO_SIZE]>,
    handlers: Box<[Handler<M>; HANDLER_SLOTS]>,
    next_slot: usize,
}

impl<M: MemoryMap> Default for DeviceTable<M> {
    fn default() -> Self {
        let mut table = Self {
            index: Box::new([0; IO_SIZE]),
            handlers: Box::new([Handler::UNMAPPED; HANDLER_SLOTS]),
            next_slot: 1,
        };
        table.install(&crate::mmio::builtin_devices::<M>());
        table
    }
}

impl<M: MemoryMap> DeviceTable<M> {
    pub fn for_cartridge(cartridge: &Cartridge) -> Self {
        let mut table = Self::default();
        table.install(&crate::mmio::coprocessor_devices(cartridge));
        table
    }

    pub fn install(&mut self, entries: &[RegEntry<M>]) {
        for entry in entries {
            assert!(
                entry.start >= IO_BASE && entry.end <= IO_END && entry.start <= entry.end,
                "range {:04X}..={:04X} is outside the I/O window",
                entry.start,
                entry.end
            );

            let slot = self.next_slot;
            debug_assert!(slot < HANDLER_SLOTS, "too many I/O handlers");

            self.handlers[slot] = entry.handler;
            self.next_slot += 1;

            for addr in entry.start..=entry.end {
                self.index[(addr - IO_BASE) as usize] = slot as u8;
            }
        }
    }

    #[inline(always)]
    pub fn handler(&self, reg: u16) -> &Handler<M> {
        let slot = match self
            .index
            .get((reg as usize).wrapping_sub(IO_BASE as usize))
        {
            Some(slot) => *slot as usize,
            None => 0,
        };
        &self.handlers[slot]
    }
}

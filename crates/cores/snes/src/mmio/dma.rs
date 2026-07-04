use crate::Snes;
use crate::mmio::regs::{
    AddressAdjustMode, Direction, DmaAddress, DmaByteCount, DmaChannelParameters, HdmaLineCounter,
    HdmaTransferEnable, MdmaTransferStart, TransferPattern,
};
use crate::mmio::{Address, MemoryMap, OPEN_BUS};

pub const CHANNEL_COUNT: usize = 8;
pub const CHANNEL_REGS_START: u16 = 0x4300;
pub const CHANNEL_REGS_END: u16 = 0x437F;

#[derive(Clone, Copy)]
pub struct DmaChannel {
    pub params: DmaChannelParameters,
    pub b_bus_addr: u8,
    pub a_bus_addr: DmaAddress,
    pub byte_count: DmaByteCount,
    pub table_addr: u16,
    pub line_counter: HdmaLineCounter,
}

impl Default for DmaChannel {
    fn default() -> Self {
        Self {
            params: DmaChannelParameters::default(),
            b_bus_addr: 0xFF,
            a_bus_addr: DmaAddress::default(),
            byte_count: DmaByteCount::default(),
            table_addr: 0xFFFF,
            line_counter: HdmaLineCounter::default(),
        }
    }
}

impl DmaChannel {
    pub fn read(&self, reg: u16) -> u8 {
        match reg & 0xF {
            0x0 => self.params.raw(),
            0x1 => self.b_bus_addr,
            0x2 => self.a_bus_addr.raw() as u8,
            0x3 => (self.a_bus_addr.raw() >> 8) as u8,
            0x4 => self.a_bus_addr.bank(),
            0x5 => self.byte_count.raw() as u8,
            0x6 => (self.byte_count.raw() >> 8) as u8,
            0x7 => self.byte_count.indirect_bank(),
            0x8 => self.table_addr as u8,
            0x9 => (self.table_addr >> 8) as u8,
            0xA => self.line_counter.raw(),
            _ => {
                tracing::error!(
                    reg = format!("{reg:?}"),
                    "Read from unmapped DMA channel register"
                );
                OPEN_BUS
            }
        }
    }

    pub fn write(&mut self, reg: u16, value: u8) {
        match reg & 0xF {
            0x0 => self.params = DmaChannelParameters::from_raw(value),
            0x1 => self.b_bus_addr = value,
            0x2 => self.a_bus_addr = (self.a_bus_addr & 0xFF_FF00) | value as u32,
            0x3 => self.a_bus_addr = (self.a_bus_addr & 0xFF_00FF) | ((value as u32) << 8),
            0x4 => self.a_bus_addr.set_bank(value),
            0x5 => self.byte_count = (self.byte_count & 0xFF_FF00) | value as u32,
            0x6 => self.byte_count = (self.byte_count & 0xFF_00FF) | ((value as u32) << 8),
            0x7 => self.byte_count.set_indirect_bank(value),
            0x8 => self.table_addr = (self.table_addr & 0xFF00) | value as u16,
            0x9 => self.table_addr = (self.table_addr & 0x00FF) | ((value as u16) << 8),
            0xA => self.line_counter = HdmaLineCounter::from_raw(value),
            _ => {
                tracing::error!(
                    reg = format!("{reg:?}"),
                    value = format!("{:02X}", value),
                    "Write to unmapped DMA channel register"
                );
            }
        }
    }
}

#[derive(Default)]
pub struct Dma {
    pub hdmaen: HdmaTransferEnable,
    pub mdmaen: MdmaTransferStart,
    pub channels: [DmaChannel; CHANNEL_COUNT],
}

impl Dma {
    pub fn channel_index(addr: u16) -> usize {
        ((addr >> 4) & 0x7) as usize
    }
}

pub(crate) fn start_mdma(snes: &mut Snes<impl MemoryMap>) {
    for index in 0..CHANNEL_COUNT {
        if snes.dma.mdmaen.raw() & (1 << index) != 0 {
            run_dma_channel(snes, index);
        }
    }

    snes.dma.mdmaen = MdmaTransferStart::from_raw(0);
}

fn transfer_pattern_offsets(pattern: TransferPattern) -> &'static [u8] {
    match pattern {
        TransferPattern::Pattern0 => &[0],
        TransferPattern::Pattern1 => &[0, 1],
        TransferPattern::Pattern2 | TransferPattern::Pattern6 => &[0, 0],
        TransferPattern::Pattern3 | TransferPattern::Pattern7 => &[0, 0, 1, 1],
        TransferPattern::Pattern4 => &[0, 1, 2, 3],
        TransferPattern::Pattern5 => &[0, 1, 0, 1],
    }
}

fn run_dma_channel(snes: &mut Snes<impl MemoryMap>, index: usize) {
    const B_BUS_BASE: u16 = 0x2100;

    let mut channel = snes.dma.channels[index];

    let pattern = transfer_pattern_offsets(channel.params.transfer_pattern());
    let direction = channel.params.direction();
    let step: i16 = match channel.params.address_adjust_mode() {
        AddressAdjustMode::Increment => 1,
        AddressAdjustMode::Decrement => -1,
        AddressAdjustMode::Fixed => 0,
    };
    let count = channel.byte_count.count() as usize;

    for &pattern_offset in pattern.iter().cycle().take(count) {
        let b_reg = channel.b_bus_addr.wrapping_add(pattern_offset);
        let b_bus = Address::from(B_BUS_BASE as u32 | b_reg as u32);
        let a_bus = Address::from(channel.a_bus_addr.raw());

        let (src, dst) = match direction {
            Direction::AToB => (a_bus, b_bus),
            Direction::BToA => (b_bus, a_bus),
        };

        let value = snes.read8(src);
        snes.write8(dst, value);

        channel
            .a_bus_addr
            .set_offset(channel.a_bus_addr.offset().wrapping_add_signed(step));
    }

    channel.byte_count.set_amount(0);
    snes.dma.channels[index] = channel;
}

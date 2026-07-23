use crate::Snes;
use crate::mmio::regs::{
    AddressAdjustMode, Direction, DmaAddress, DmaByteCount, DmaChannelParameters, HdmaLineCounter,
    HdmaTransferEnable, MdmaTransferStart, TransferPattern,
};
use crate::mmio::wram::WMDATA;
use crate::mmio::{Address, AddressSpace, MemoryMap, OPEN_BUS};

pub const CHANNEL_COUNT: usize = 8;
pub const CHANNEL_REGS_START: u16 = 0x4300;
pub const CHANNEL_REGS_END: u16 = 0x437F;

pub(crate) const B_BUS_BASE: u16 = 0x2100;
pub(crate) const HDMA_INIT_CLOCK: u64 = 6 * 4;
pub(crate) const HDMA_RUN_CLOCK: u64 = 1104;

const HDMA_BASE_CLOCKS: u64 = 18;
const HDMA_IDLE_CHANNEL_CLOCKS: u64 = 8;

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
    hdma_active: u8,
    hdma_completed: u8,
    hdma_do_transfer: u8,
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

    snes.dma.mdmaen = MdmaTransferStart::zeroed();
}

pub(crate) fn transfer_pattern_offsets(pattern: TransferPattern) -> &'static [u8] {
    match pattern {
        TransferPattern::Pattern0 => &[0],
        TransferPattern::Pattern1 => &[0, 1],
        TransferPattern::Pattern2 | TransferPattern::Pattern6 => &[0, 0],
        TransferPattern::Pattern3 | TransferPattern::Pattern7 => &[0, 0, 1, 1],
        TransferPattern::Pattern4 => &[0, 1, 2, 3],
        TransferPattern::Pattern5 => &[0, 1, 0, 1],
    }
}

pub(crate) fn init_hdma<M: MemoryMap + 'static>(snes: &mut Snes<M>, _deadline: u64) {
    let clocks_before = snes.mmu.clocks;
    let enabled = snes.dma.hdmaen.raw();

    snes.dma.hdma_active = enabled;
    snes.dma.hdma_completed = 0;
    snes.dma.hdma_do_transfer = 0;
    if enabled == 0 {
        return;
    }

    snes.mmu.clocks += HDMA_BASE_CLOCKS;

    for index in 0..CHANNEL_COUNT {
        if enabled & channel_bit(index) == 0 {
            continue;
        }

        let mut channel = snes.dma.channels[index];
        channel.table_addr = channel.a_bus_addr.table_offset();
        channel.line_counter = HdmaLineCounter::zeroed();
        reload_hdma_entry(snes, index, &mut channel);
        snes.dma.channels[index] = channel;
    }

    snes.scheduler.advance_by(snes.mmu.clocks - clocks_before);
}

/// Performs one HDMA step during HBLANK. A transfer at the end of line N
/// changes PPU state used to render line N + 1.
pub(crate) fn run_hdma<M: MemoryMap + 'static>(snes: &mut Snes<M>, deadline: u64) {
    if snes.scheduler.scanline_at(deadline) > crate::ppu::SCREEN_HEIGHT as u64 {
        return;
    }

    // A channel is active this line unless its table has already terminated;
    // `enabled & !completed` fully captures that, so no separate carry is kept.
    let enabled = snes.dma.hdmaen.raw();
    snes.dma.hdma_active = enabled & !snes.dma.hdma_completed;
    if snes.dma.hdma_active == 0 {
        return;
    }

    let clocks_before = snes.mmu.clocks;
    snes.mmu.clocks += HDMA_BASE_CLOCKS;

    for index in 0..CHANNEL_COUNT {
        if snes.dma.hdma_active & channel_bit(index) == 0 {
            continue;
        }

        transfer_hdma_channel(snes, index);
    }

    // Hardware transfers every active channel before it advances any line
    // counter or reloads any table entry.
    for index in 0..CHANNEL_COUNT {
        if snes.dma.hdma_active & channel_bit(index) == 0 {
            continue;
        }

        advance_hdma_channel(snes, index);
    }

    snes.scheduler.advance_by(snes.mmu.clocks - clocks_before);
}

fn transfer_hdma_channel(snes: &mut Snes<impl MemoryMap>, index: usize) {
    let bit = channel_bit(index);
    let mut channel = snes.dma.channels[index];

    if snes.dma.hdma_do_transfer & bit != 0 {
        transfer_hdma_unit(snes, &mut channel);
    }

    snes.dma.channels[index] = channel;
}

fn advance_hdma_channel(snes: &mut Snes<impl MemoryMap>, index: usize) {
    let bit = channel_bit(index);
    let mut channel = snes.dma.channels[index];

    channel.line_counter = channel.line_counter.wrapping_sub(1);
    if channel.line_counter.line_count() == 0 {
        reload_hdma_entry(snes, index, &mut channel);
    } else {
        snes.mmu.clocks += HDMA_IDLE_CHANNEL_CLOCKS;
        if channel.line_counter.repeat() {
            snes.dma.hdma_do_transfer |= bit;
        } else {
            snes.dma.hdma_do_transfer &= !bit;
        }
    }

    snes.dma.channels[index] = channel;
}

fn reload_hdma_entry(snes: &mut Snes<impl MemoryMap>, index: usize, channel: &mut DmaChannel) {
    let bit = channel_bit(index);
    let descriptor = read_hdma_table_byte(snes, channel);
    if descriptor == 0 {
        snes.dma.hdma_active &= !bit;
        snes.dma.hdma_completed |= bit;
        snes.dma.hdma_do_transfer &= !bit;
        channel.line_counter = HdmaLineCounter::zeroed();
        return;
    }

    channel.line_counter = HdmaLineCounter::from_raw(descriptor);
    snes.dma.hdma_do_transfer |= bit;

    if channel.params.indirect() {
        let low = read_hdma_table_byte(snes, channel);
        let high = read_hdma_table_byte(snes, channel);
        channel
            .byte_count
            .set_indirect_offset(u16::from_le_bytes([low, high]));
    }
}

fn transfer_hdma_unit(snes: &mut Snes<impl MemoryMap>, channel: &mut DmaChannel) {
    let pattern = transfer_pattern_offsets(channel.params.transfer_pattern());
    let indirect = channel.params.indirect();
    let direction = channel.params.direction();

    for &pattern_offset in pattern {
        let a_bus = if indirect {
            Address::new(
                channel.byte_count.indirect_bank(),
                channel.byte_count.indirect_offset(),
            )
        } else {
            Address::new(channel.a_bus_addr.table_bank(), channel.table_addr)
        };
        let b_reg = channel.b_bus_addr.wrapping_add(pattern_offset);
        let b_bus = Address::from(B_BUS_BASE as u32 | b_reg as u32);
        transfer_dma_byte(snes, a_bus, b_bus, direction);

        if indirect {
            channel
                .byte_count
                .set_indirect_offset(channel.byte_count.indirect_offset().wrapping_add(1));
        } else {
            channel.table_addr = channel.table_addr.wrapping_add(1);
        }
    }
}

/// Moves a single byte between the A-bus and B-bus, honoring the rule that the
/// A-bus cannot reach the B-bus/IO region (reads yield open bus, writes are
/// dropped). Costs 8 master clocks. Shared by GDMA and HDMA.
fn transfer_dma_byte(
    snes: &mut Snes<impl MemoryMap>,
    a_bus: Address,
    b_bus: Address,
    direction: Direction,
) {
    let clocks_before = snes.mmu.clocks;
    let a_bus_space = snes.mmu.map(a_bus);

    if b_bus.address() == WMDATA
        && let AddressSpace::Wram(index) = &a_bus_space
    {
        if matches!(direction, Direction::BToA) {
            snes.mmu.wram[*index] = OPEN_BUS;
        }
        snes.mmu.clocks = clocks_before + 8;
        return;
    }

    let (src, dst) = match direction {
        Direction::AToB => (a_bus, b_bus),
        Direction::BToA => (b_bus, a_bus),
    };

    let a_bus_blocked = matches!(a_bus_space, AddressSpace::Io(_));
    let value = if a_bus_blocked && matches!(direction, Direction::AToB) {
        OPEN_BUS
    } else {
        snes.read8(src)
    };

    if !(a_bus_blocked && matches!(direction, Direction::BToA)) {
        snes.write8(dst, value);
    }

    snes.mmu.clocks = clocks_before + 8;
}

fn read_hdma_table_byte(snes: &mut Snes<impl MemoryMap>, channel: &mut DmaChannel) -> u8 {
    let clocks_before = snes.mmu.clocks;
    let address = Address::new(channel.a_bus_addr.table_bank(), channel.table_addr);
    channel.table_addr = channel.table_addr.wrapping_add(1);

    let value = if matches!(snes.mmu.map(address), AddressSpace::Io(_)) {
        OPEN_BUS
    } else {
        snes.read8(address)
    };
    snes.mmu.clocks = clocks_before + 8;
    value
}

fn channel_bit(index: usize) -> u8 {
    1 << index
}

fn run_dma_channel(snes: &mut Snes<impl MemoryMap>, index: usize) {
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

        transfer_dma_byte(snes, a_bus, b_bus, direction);

        channel
            .a_bus_addr
            .set_offset(channel.a_bus_addr.offset().wrapping_add_signed(step));
    }

    channel.byte_count.set_amount(0);
    snes.dma.channels[index] = channel;
}

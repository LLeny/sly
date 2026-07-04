use crate::Snes;
use crate::mmio::regs::{DmaChannelParameters, HdmaTransferEnable, MdmaTransferStart};
use crate::mmio::{MemoryMap, OPEN_BUS};

pub const CHANNEL_COUNT: usize = 8;
pub const CHANNEL_REGS_START: u16 = 0x4300;
pub const CHANNEL_REGS_END: u16 = 0x437F;

#[derive(Default, Clone, Copy)]
pub struct DmaChannel {
    pub params: DmaChannelParameters,
    pub b_bus_addr: u8,
    pub a_bus_addr: u32,
    pub byte_count: u16,
    pub indirect_bank: u8,
    pub table_addr: u16,
    pub line_counter: u8,
}

impl DmaChannel {
    pub fn read(&self, reg: u16) -> u8 {
        match reg & 0xF {
            0x0 => self.params.raw(),
            0x1 => self.b_bus_addr,
            0x2 => self.a_bus_addr as u8,
            0x3 => (self.a_bus_addr >> 8) as u8,
            0x4 => (self.a_bus_addr >> 16) as u8,
            0x5 => self.byte_count as u8,
            0x6 => (self.byte_count >> 8) as u8,
            0x7 => self.indirect_bank,
            0x8 => self.table_addr as u8,
            0x9 => (self.table_addr >> 8) as u8,
            0xA => self.line_counter,
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
            0x4 => self.a_bus_addr = (self.a_bus_addr & 0x00_FFFF) | ((value as u32) << 16),
            0x5 => self.byte_count = (self.byte_count & 0xFF00) | value as u16,
            0x6 => self.byte_count = (self.byte_count & 0x00FF) | ((value as u16) << 8),
            0x7 => self.indirect_bank = value,
            0x8 => self.table_addr = (self.table_addr & 0xFF00) | value as u16,
            0x9 => self.table_addr = (self.table_addr & 0x00FF) | ((value as u16) << 8),
            0xA => self.line_counter = value,
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

fn run_dma_channel(snes: &mut Snes<impl MemoryMap>, index: usize) {
    let channel = &snes.dma.channels[index];
}

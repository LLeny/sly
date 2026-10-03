use serde::{Deserialize, Serialize};

use crate::bus::{CpuBus, Width};
use crate::dma::regs::{
    Address, Control, DestinationAddressControl, SourceAddressControl, StartTiming, TransferSize,
    WordCount,
};

pub(crate) mod io;
pub(crate) mod regs;

pub struct Dma {
    pub src_addr: [Address; 4],
    pub dest_addr: [Address; 4],
    pub word_count: [WordCount; 4],
    pub control: [Control; 4],
    pub(crate) latches: [LatchedTransfer; 4],
}

impl Dma {
    pub fn new() -> Self {
        Self {
            src_addr: [Address::zeroed(); 4],
            dest_addr: [Address::zeroed(); 4],
            word_count: [WordCount::zeroed(); 4],
            control: [Control::zeroed(); 4],
            latches: [LatchedTransfer::default(); 4],
        }
    }

    pub(crate) fn set_pending_for(&mut self, timing: StartTiming) {
        for (control, latch) in self.control.iter().zip(&mut self.latches) {
            if control.enable() && control.start_timing() == timing {
                latch.pending = true;
            }
        }
    }

    pub(crate) fn has_pending_transfers(&self) -> bool {
        self.latches.iter().any(|l| l.pending)
    }

    pub(crate) fn write_control(&mut self, channel: usize, control: Control) {
        let was_enabled = self.control[channel].enable();
        self.control[channel] = control;

        if control.enable() && !was_enabled {
            self.latches[channel] = LatchedTransfer {
                src: self.src_addr[channel].address(),
                dst: self.dest_addr[channel].address(),
                count: self.units(channel),
                pending: control.start_timing() == StartTiming::Immediate,
            };
        }
    }

    /// 0 == channel's maximum.
    pub fn units(&self, channel: usize) -> u32 {
        let count = if channel == 3 {
            self.word_count[channel].count3()
        } else {
            self.word_count[channel].count()
        };
        if count == 0 {
            max_units(channel)
        } else {
            u32::from(count)
        }
    }

    pub(crate) fn save_io(&self, io: &mut [u8]) {
        for channel in 0..4 {
            let base = io_base(channel);
            io[base..(base + 4)].copy_from_slice(&self.src_addr[channel].raw().to_le_bytes());
            io[(base + 4)..(base + 8)]
                .copy_from_slice(&self.dest_addr[channel].raw().to_le_bytes());
            io[(base + 8)..(base + 10)]
                .copy_from_slice(&self.word_count[channel].raw().to_le_bytes());
            io[(base + 10)..(base + 12)]
                .copy_from_slice(&self.control[channel].raw().to_le_bytes());
        }
    }

    pub(crate) fn load_io(&mut self, io: &[u8]) {
        for channel in 0..4 {
            let base = io_base(channel);
            self.src_addr[channel] = Address::from_raw(
                u32::from_le_bytes(io[base..(base + 4)].try_into().unwrap())
                    & Address::SOURCE_MASKS[channel],
            );
            self.dest_addr[channel] = Address::from_raw(
                u32::from_le_bytes(io[(base + 4)..(base + 8)].try_into().unwrap())
                    & Address::DESTINATION_MASKS[channel],
            );
            self.word_count[channel] = WordCount::from_raw(
                u16::from_le_bytes(io[(base + 8)..(base + 10)].try_into().unwrap())
                    & if channel == 3 { 0xFFFF } else { 0x3FFF },
            );
            self.control[channel] = control_from_io(io, channel);
        }
    }
}

const fn io_base(channel: usize) -> usize {
    0xB0 + (channel * 12)
}

const fn max_units(channel: usize) -> u32 {
    if channel == 3 { 0x10000 } else { 0x4000 }
}

pub(crate) fn control_from_io(io: &[u8], channel: usize) -> Control {
    let base = io_base(channel) + 10;
    Control::from_raw(
        u16::from_le_bytes(io[base..(base + 2)].try_into().unwrap()) & io::control_mask(channel),
    )
}

impl CpuBus<'_> {
    pub(crate) fn process_transfers(&mut self) -> u64 {
        let mut cycles = 0;

        for channel in 0..4 {
            let mut latch = self.dma.latches[channel];
            if !latch.pending {
                continue;
            }

            let control = self.dma.control[channel];
            let width = match control.transfer_size() {
                TransferSize::Word => Width::Word,
                _ => Width::Halfword,
            };
            let size = width.bytes();
            let src_step = match control.src_addr_ctrl() {
                SourceAddressControl::Increment => size,
                SourceAddressControl::Decrement => size.wrapping_neg(),
                SourceAddressControl::Fixed | SourceAddressControl::Prohibited => 0,
            };
            let dst_step = match control.dest_addr_ctrl() {
                DestinationAddressControl::Increment
                | DestinationAddressControl::IncrementReload => size,
                DestinationAddressControl::Decrement => size.wrapping_neg(),
                DestinationAddressControl::Fixed | DestinationAddressControl::Unknown => 0,
            };
            let src_mask = Address::SOURCE_MASKS[channel];
            let dst_mask = Address::DESTINATION_MASKS[channel];

            latch.src &= !(size - 1);
            latch.dst &= !(size - 1);

            let count = latch.count;
            for _ in 0..count {
                let value = self.read(latch.src, width);
                self.write(latch.dst, width, value);

                latch.src = latch.src.wrapping_add(src_step) & src_mask;
                latch.dst = latch.dst.wrapping_add(dst_step) & dst_mask;
            }

            latch.pending = false;

            if control.repeat() && control.start_timing() != StartTiming::Immediate {
                latch.count = self.dma.units(channel);
                if control.dest_addr_ctrl() == DestinationAddressControl::IncrementReload {
                    latch.dst = self.dma.dest_addr[channel].address();
                }
            } else {
                self.dma.control[channel].set_enable(false);
            }
            self.dma.latches[channel] = latch;

            if control.irq_on_completion() {
                self.irq.interrupt_request.request_dma(channel);
            }

            cycles += u64::from(count) * 2;
        }

        cycles
    }
}

#[derive(Copy, Clone, Default, Serialize, Deserialize)]
pub(crate) struct LatchedTransfer {
    pub src: u32,
    pub dst: u32,
    pub count: u32,
    pub pending: bool,
}

impl LatchedTransfer {
    pub(crate) fn is_valid(&self, channel: usize, control: Control) -> bool {
        self.src & !Address::SOURCE_MASKS[channel] == 0
            && self.dst & !Address::DESTINATION_MASKS[channel] == 0
            && self.count <= max_units(channel)
            && (!self.pending || control.enable())
    }
}

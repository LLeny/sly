use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::Snes;
use crate::apu::SPC700_CLOCK;
use crate::cartridge::TvSystem;
use crate::mmio::{CartridgeMap, MemoryMap};

pub(crate) const MASTER_CLOCKS_PER_SCANLINE: u64 = 1364;
pub(crate) const MASTER_CLOCK_HZ_NTSC: u64 = 21_477_270;
pub(crate) const MASTER_CLOCK_HZ_PAL: u64 = 21_281_370;

const SCANLINES_PER_FRAME_NTSC: u64 = 262;
const SCANLINES_PER_FRAME_PAL: u64 = 312;

pub(crate) const VBLANK_START_SCANLINE: u64 = 225;
pub(crate) const REFRESH_START_CLOCK: u64 = 534;
pub(crate) const REFRESH_MASTER_CLOCKS: u64 = 40;

pub(crate) const MASTER_CLOCKS_PER_FRAME_NTSC: u64 =
    MASTER_CLOCKS_PER_SCANLINE * SCANLINES_PER_FRAME_NTSC;
pub(crate) const MASTER_CLOCKS_PER_FRAME_PAL: u64 =
    MASTER_CLOCKS_PER_SCANLINE * SCANLINES_PER_FRAME_PAL;

pub type Callback<M = CartridgeMap> = fn(&mut Snes<M>, u64);

enum Period {
    Master(u64),
    Spc { spc_cycles: u64, remainder: u64 },
}

pub struct Event<M: MemoryMap = CartridgeMap> {
    deadline: u64,
    period: Option<Period>,
    callback: Callback<M>,
}

impl<M: MemoryMap> Eq for Event<M> {}

impl<M: MemoryMap> PartialEq for Event<M> {
    fn eq(&self, other: &Self) -> bool {
        self.deadline == other.deadline
    }
}

impl<M: MemoryMap> Ord for Event<M> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.deadline.cmp(&other.deadline)
    }
}

impl<M: MemoryMap> PartialOrd for Event<M> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub struct Scheduler<M: MemoryMap = CartridgeMap> {
    events: BinaryHeap<Reverse<Event<M>>>,
    cycles: u64,
    cycles_per_frame: u64,
    master_clock_hz: u64,
    spc_remainder: u64,
}

impl<M: MemoryMap> Scheduler<M> {
    pub fn new(region: TvSystem) -> Self {
        let (cycles_per_frame, master_clock_hz) = match region {
            TvSystem::Ntsc => (MASTER_CLOCKS_PER_FRAME_NTSC, MASTER_CLOCK_HZ_NTSC),
            TvSystem::Pal => (MASTER_CLOCKS_PER_FRAME_PAL, MASTER_CLOCK_HZ_PAL),
        };

        Self {
            events: BinaryHeap::new(),
            cycles: 0,
            cycles_per_frame,
            master_clock_hz,
            spc_remainder: 0,
        }
    }

    pub fn spc_master_clocks(&mut self, spc_cycles: u64) -> u64 {
        let scaled = spc_cycles * self.master_clock_hz + self.spc_remainder;
        self.spc_remainder = scaled % SPC700_CLOCK;
        scaled / SPC700_CLOCK
    }

    pub fn cycles(&self) -> u64 {
        self.cycles
    }

    pub fn cycles_per_frame(&self) -> u64 {
        self.cycles_per_frame
    }

    pub fn master_clock_hz(&self) -> u64 {
        self.master_clock_hz
    }

    pub fn frame_deadline(&self) -> u64 {
        (self.cycles / self.cycles_per_frame + 1) * self.cycles_per_frame
    }

    pub fn scanline_at(&self, deadline: u64) -> u64 {
        let lines_per_frame = self.cycles_per_frame / MASTER_CLOCKS_PER_SCANLINE;
        (deadline / MASTER_CLOCKS_PER_SCANLINE) % lines_per_frame
    }

    pub fn advance_by(&mut self, cycles: u64) {
        self.cycles += cycles;
    }

    pub fn pop_current_event(&mut self) -> Option<(u64, Callback<M>)> {
        match self.events.peek() {
            Some(next) if next.0.deadline <= self.cycles => {
                let event = self.events.pop().unwrap().0;
                if let Some(mut period) = event.period {
                    let delay = match &mut period {
                        Period::Master(period) => *period,
                        Period::Spc {
                            spc_cycles,
                            remainder,
                        } => {
                            let scaled = *spc_cycles * self.master_clock_hz + *remainder;
                            *remainder = scaled % SPC700_CLOCK;
                            scaled / SPC700_CLOCK
                        }
                    };
                    self.events.push(Reverse(Event {
                        deadline: event.deadline + delay,
                        period: Some(period),
                        callback: event.callback,
                    }));
                }
                Some((event.deadline, event.callback))
            }
            _ => None,
        }
    }

    pub fn schedule_event(&mut self, deadline: u64, callback: Callback<M>) {
        self.events.push(Reverse(Event {
            deadline,
            period: None,
            callback,
        }));
    }

    pub fn schedule_periodic(&mut self, period: u64, callback: Callback<M>) {
        self.schedule_periodic_at(period, period, callback);
    }

    pub fn schedule_periodic_at(&mut self, first: u64, period: u64, callback: Callback<M>) {
        self.events.push(Reverse(Event {
            deadline: self.cycles + first,
            period: Some(Period::Master(period)),
            callback,
        }));
    }

    /// Schedule a recurring event with a period given in SPC700 cycles.
    pub fn schedule_periodic_spc(&mut self, spc_cycles: u64, callback: Callback<M>) {
        let scaled = spc_cycles * self.master_clock_hz;
        self.events.push(Reverse(Event {
            deadline: self.cycles + scaled / SPC700_CLOCK,
            period: Some(Period::Spc {
                spc_cycles,
                remainder: scaled % SPC700_CLOCK,
            }),
            callback,
        }));
    }

    pub fn next_deadline(&self) -> Option<u64> {
        self.events.peek().map(|e| e.0.deadline)
    }
}

pub(crate) fn spc700_tick<M: MemoryMap + 'static>(snes: &mut Snes<M>, deadline: u64) {
    let cycles = crate::apu::spc700::step(&mut snes.apu);
    let delay = snes.scheduler.spc_master_clocks(cycles);
    snes.scheduler
        .schedule_event(deadline + delay, self::spc700_tick);
}

pub(crate) fn dram_refresh<M: MemoryMap + 'static>(snes: &mut Snes<M>, _deadline: u64) {
    snes.scheduler.advance_by(REFRESH_MASTER_CLOCKS);
}

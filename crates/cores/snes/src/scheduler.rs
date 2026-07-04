use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::Snes;
use crate::cartridge::TvSystem;
use crate::mmio::{CartridgeMap, MemoryMap};

pub(crate) const MASTER_CLOCKS_PER_SCANLINE: u64 = 1364;

const SCANLINES_PER_FRAME_NTSC: u64 = 262;
const SCANLINES_PER_FRAME_PAL: u64 = 312;

pub(crate) const VBLANK_START_SCANLINE: u64 = 225;

pub(crate) const MASTER_CLOCKS_PER_FRAME_NTSC: u64 =
    MASTER_CLOCKS_PER_SCANLINE * SCANLINES_PER_FRAME_NTSC;
pub(crate) const MASTER_CLOCKS_PER_FRAME_PAL: u64 =
    MASTER_CLOCKS_PER_SCANLINE * SCANLINES_PER_FRAME_PAL;

pub type Callback<M = CartridgeMap> = fn(&mut Snes<M>, u64);

pub struct Event<M: MemoryMap = CartridgeMap> {
    deadline: u64,
    period: Option<u64>,
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
}

impl<M: MemoryMap> Scheduler<M> {
    pub fn new(region: TvSystem) -> Self {
        let cycles_per_frame = match region {
            TvSystem::Ntsc => MASTER_CLOCKS_PER_FRAME_NTSC,
            TvSystem::Pal => MASTER_CLOCKS_PER_FRAME_PAL,
        };

        Self {
            events: BinaryHeap::new(),
            cycles: 0,
            cycles_per_frame,
        }
    }

    pub fn cycles(&self) -> u64 {
        self.cycles
    }

    pub fn cycles_per_frame(&self) -> u64 {
        self.cycles_per_frame
    }

    pub fn frame_deadline(&self) -> u64 {
        self.cycles + self.cycles_per_frame
    }

    pub fn advance_by(&mut self, cycles: u64) {
        self.cycles += cycles;
    }

    pub fn pop_current_event(&mut self) -> Option<(u64, Callback<M>)> {
        match self.events.peek() {
            Some(next) if next.0.deadline <= self.cycles => {
                let event = self.events.pop().unwrap().0;
                if let Some(period) = event.period {
                    self.events.push(Reverse(Event {
                        deadline: event.deadline + period,
                        period: event.period,
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
            period: Some(period),
            callback,
        }));
    }

    pub fn next_deadline(&self) -> Option<u64> {
        self.events.peek().map(|e| e.0.deadline)
    }
}

pub(crate) fn spc700_tick<M: MemoryMap + 'static>(snes: &mut Snes<M>, _deadline: u64) {
    #[cfg(feature = "debug")]
    {
        snes.executing = crate::Component::Spc;
    }

    crate::apu::spc700::step(&mut snes.apu);
}

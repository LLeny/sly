use std::cmp::Reverse;
use std::collections::BinaryHeap;

use serde::{Deserialize, Serialize};

use crate::Bus;
use crate::apu::SPC700_CLOCK;
use crate::cartridge_header::TvSystem;
use crate::mmio::MemoryMap;

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

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum EventKind {
    Spc700Tick,
    VblankStart,
    VblankEnd,
    DrawScanline,
    InitHdma,
    RunHdma,
    HvTimerLine,
    HvTimerFire,
    DramRefresh,
    Timer01Tick,
    Timer2Tick,
    DspTick,
    GsuTick,
}

impl EventKind {
    pub(crate) fn dispatch<M: MemoryMap>(self, snes: &mut Bus<M>, deadline: u64) {
        match self {
            EventKind::Spc700Tick => self::spc700_tick(snes, deadline),
            EventKind::VblankStart => crate::ppu::vblank_start(snes, deadline),
            EventKind::VblankEnd => crate::ppu::vblank_end(snes, deadline),
            EventKind::DrawScanline => crate::ppu::draw_scanline(snes, deadline),
            EventKind::InitHdma => crate::mmio::dma::init_hdma(snes, deadline),
            EventKind::RunHdma => crate::mmio::dma::run_hdma(snes, deadline),
            EventKind::HvTimerLine => crate::interrupt::hv_timer_line(snes, deadline),
            EventKind::HvTimerFire => crate::interrupt::hv_timer_fire(snes, deadline),
            EventKind::DramRefresh => self::dram_refresh(snes, deadline),
            EventKind::Timer01Tick => crate::apu::timer01_tick(snes, deadline),
            EventKind::Timer2Tick => crate::apu::timer2_tick(snes, deadline),
            EventKind::DspTick => crate::apu::dsp_tick(snes, deadline),
            EventKind::GsuTick => crate::superfx::gsu_tick(snes, deadline),
        }
    }
}

fn sorted_events<S: serde::Serializer>(
    heap: &BinaryHeap<Reverse<Event>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut events: Vec<&Event> = heap.iter().map(|entry| &entry.0).collect();
    events.sort_unstable();
    events.serialize(serializer)
}

fn heap_from_events<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<BinaryHeap<Reverse<Event>>, D::Error> {
    let events = Vec::<Event>::deserialize(deserializer)?;
    Ok(events.into_iter().map(Reverse).collect())
}

#[derive(Serialize, Deserialize)]
enum Period {
    Master(u64),
    Spc { spc_cycles: u64, remainder: u64 },
}

#[derive(Serialize, Deserialize)]
pub struct Event {
    deadline: u64,
    seq: u64,
    period: Option<Period>,
    kind: EventKind,
}

impl Eq for Event {}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        self.deadline == other.deadline && self.seq == other.seq
    }
}

impl Ord for Event {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.deadline
            .cmp(&other.deadline)
            .then_with(|| self.seq.cmp(&other.seq))
    }
}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Scheduler {
    #[serde(
        serialize_with = "sorted_events",
        deserialize_with = "heap_from_events"
    )]
    events: BinaryHeap<Reverse<Event>>,
    cycles: u64,
    cycles_per_frame: u64,
    master_clock_hz: u64,
    spc_remainder: u64,
    next_seq: u64,

    #[serde(skip)]
    cached_deadline: u64,
}

impl Scheduler {
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
            next_seq: 0,
            cached_deadline: u64::MAX,
        }
    }

    fn push(&mut self, deadline: u64, period: Option<Period>, kind: EventKind) {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.events.push(Reverse(Event {
            deadline,
            seq,
            period,
            kind,
        }));
        self.refresh_deadline();
    }

    fn refresh_deadline(&mut self) {
        self.cached_deadline = self.events.peek().map_or(u64::MAX, |next| next.0.deadline);
    }

    #[inline(always)]
    pub fn event_due(&self, clock: u64) -> bool {
        clock >= self.cached_deadline
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

    pub fn advance_to(&mut self, clock: u64) {
        debug_assert!(clock >= self.cycles, "scheduler ran ahead of the bus clock");
        self.cycles = clock;
    }

    pub fn pop_current_event(&mut self) -> Option<(u64, EventKind)> {
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
                        seq: event.seq,
                        period: Some(period),
                        kind: event.kind,
                    }));
                }
                self.refresh_deadline();
                Some((event.deadline, event.kind))
            }
            _ => {
                self.refresh_deadline();
                None
            }
        }
    }

    pub fn schedule_event(&mut self, deadline: u64, kind: EventKind) {
        self.push(deadline, None, kind);
    }

    pub fn schedule_periodic(&mut self, period: u64, kind: EventKind) {
        self.schedule_periodic_at(period, period, kind);
    }

    pub fn schedule_periodic_at(&mut self, first: u64, period: u64, kind: EventKind) {
        self.push(self.cycles + first, Some(Period::Master(period)), kind);
    }

    /// Schedule a recurring event with a period given in SPC700 cycles.
    pub fn schedule_periodic_spc(&mut self, spc_cycles: u64, kind: EventKind) {
        let scaled = spc_cycles * self.master_clock_hz;
        self.push(
            self.cycles + scaled / SPC700_CLOCK,
            Some(Period::Spc {
                spc_cycles,
                remainder: scaled % SPC700_CLOCK,
            }),
            kind,
        );
    }

    pub fn next_deadline(&self) -> Option<u64> {
        self.events.peek().map(|e| e.0.deadline)
    }
}

pub(crate) fn spc700_tick<M: MemoryMap>(snes: &mut Bus<M>, deadline: u64) {
    let cycles = crate::apu::spc700::step(&mut snes.apu);
    let delay = snes.scheduler.spc_master_clocks(cycles);
    snes.scheduler
        .schedule_event(deadline + delay, EventKind::Spc700Tick);
}

pub(crate) fn dram_refresh<M: MemoryMap>(snes: &mut Bus<M>, _deadline: u64) {
    snes.mmu.clocks += REFRESH_MASTER_CLOCKS;
    snes.scheduler.advance_by(REFRESH_MASTER_CLOCKS);
}

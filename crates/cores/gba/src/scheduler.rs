use std::cmp::Reverse;
use std::collections::BinaryHeap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum EventKind {
    HBlankStart,
    ScanlineStart,
    VBlankStart,
    VBlankEnd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Event {
    pub(crate) deadline: u64,
    pub(crate) kind: EventKind,
    pub(crate) period: u64,
}

#[derive(Default)]
pub(crate) struct Scheduler {
    pub(crate) events: BinaryHeap<Reverse<Event>>,
    pub(crate) cycles: u64,
}

impl Scheduler {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn advance_by(&mut self, cycles: u64) {
        self.cycles += cycles;
    }

    pub(crate) fn schedule_periodic_at(&mut self, deadline: u64, period: u64, kind: EventKind) {
        assert!(period > 0, "event period must be nonzero");

        self.events.push(Reverse(Event {
            deadline,
            kind,
            period,
        }));
    }

    pub(crate) fn pop_due(&mut self) -> Option<Event> {
        if self.events.peek()?.0.deadline > self.cycles {
            return None;
        }

        let Reverse(event) = self.events.pop()?;
        self.events.push(Reverse(Event {
            deadline: event.deadline + event.period,
            ..event
        }));

        Some(event)
    }
}

#[cfg(feature = "debug")]
impl EventKind {
    pub(crate) fn bit(self) -> u8 {
        1 << self as u8
    }

    pub(crate) fn debug_id(self) -> &'static str {
        match self {
            Self::HBlankStart => "gba.hblank",
            Self::ScanlineStart => "gba.scanline",
            Self::VBlankStart => "gba.vblank",
            Self::VBlankEnd => "gba.vblank_end",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::HBlankStart => "HBLANK",
            Self::ScanlineStart => "Scanline",
            Self::VBlankStart => "VBLANK",
            Self::VBlankEnd => "VBLANK end",
        }
    }
}

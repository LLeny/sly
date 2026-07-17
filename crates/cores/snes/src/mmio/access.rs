use std::cell::{Cell, RefCell};
use std::collections::HashSet;

use super::MemKind;

#[derive(Default, Clone)]
pub struct MemoryAccess {
    pub reads: HashSet<(MemKind, u32)>,
    pub writes: HashSet<(MemKind, u32)>,
    pub last: Option<(MemKind, u32)>,
}

impl MemoryAccess {
    pub fn merge(&mut self, other: &MemoryAccess) {
        self.reads.extend(other.reads.iter().copied());
        self.writes.extend(other.writes.iter().copied());
        if other.last.is_some() {
            self.last = other.last;
        }
    }
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum TrackMode {
    #[default]
    Off,
    LastOnly,
    Full,
}

#[derive(Default)]
pub struct AccessTracker {
    mode: Cell<TrackMode>,
    gated: Cell<bool>,
    reads: RefCell<HashSet<(MemKind, u32)>>,
    writes: RefCell<HashSet<(MemKind, u32)>>,
    last: Cell<Option<(MemKind, u32)>>,
}

impl AccessTracker {
    pub(crate) fn begin(&self, mode: TrackMode) {
        self.mode.set(mode);
        self.gated.set(false);
        self.last.set(None);
        if mode == TrackMode::Full {
            self.reads.borrow_mut().clear();
            self.writes.borrow_mut().clear();
        }
    }

    pub(crate) fn stop(&self) {
        self.mode.set(TrackMode::Off);
        self.gated.set(false);
    }

    pub(crate) fn resume(&self) {
        if self.mode.get() != TrackMode::Off {
            self.gated.set(true);
        }
    }

    pub(crate) fn pause(&self) {
        self.gated.set(false);
    }

    pub(super) fn active(&self) -> bool {
        self.gated.get()
    }

    pub(crate) fn record(&self, key: (MemKind, u32), write: bool) {
        if !self.gated.get() {
            return;
        }

        self.last.set(Some(key));

        if self.mode.get() == TrackMode::Full {
            if write {
                self.writes.borrow_mut().insert(key);
            } else {
                self.reads.borrow_mut().insert(key);
            }
        }
    }

    pub fn take(&self) -> MemoryAccess {
        MemoryAccess {
            reads: self.reads.take(),
            writes: self.writes.take(),
            last: self.last.take(),
        }
    }
}

use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

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
#[repr(u8)]
pub enum TrackMode {
    #[default]
    Off = 0,
    LastOnly = 1,
    Full = 2,
}

#[derive(Default)]
pub struct AccessTracker {
    mode: AtomicU8,
    gated: AtomicBool,
    reads: Mutex<HashSet<(MemKind, u32)>>,
    writes: Mutex<HashSet<(MemKind, u32)>>,
    last: Mutex<Option<(MemKind, u32)>>,
}

impl AccessTracker {
    pub(crate) fn begin(&self, mode: TrackMode) {
        self.mode.store(mode as u8, Ordering::Relaxed);
        self.gated.store(false, Ordering::Relaxed);
        *self.last.lock().unwrap() = None;
        if mode == TrackMode::Full {
            self.reads.lock().unwrap().clear();
            self.writes.lock().unwrap().clear();
        }
    }

    pub(crate) fn stop(&self) {
        self.mode.store(TrackMode::Off as u8, Ordering::Relaxed);
        self.gated.store(false, Ordering::Relaxed);
    }

    pub(crate) fn resume(&self) {
        if self.mode.load(Ordering::Relaxed) != TrackMode::Off as u8 {
            self.gated.store(true, Ordering::Relaxed);
        }
    }

    pub(crate) fn pause(&self) {
        self.gated.store(false, Ordering::Relaxed);
    }

    pub(super) fn active(&self) -> bool {
        self.gated.load(Ordering::Relaxed)
    }

    pub(crate) fn record(&self, key: (MemKind, u32), write: bool) {
        if !self.gated.load(Ordering::Relaxed) {
            return;
        }

        *self.last.lock().unwrap() = Some(key);

        if self.mode.load(Ordering::Relaxed) == TrackMode::Full as u8 {
            if write {
                self.writes.lock().unwrap().insert(key);
            } else {
                self.reads.lock().unwrap().insert(key);
            }
        }
    }

    pub fn take(&self) -> MemoryAccess {
        MemoryAccess {
            reads: std::mem::take(&mut *self.reads.lock().unwrap()),
            writes: std::mem::take(&mut *self.writes.lock().unwrap()),
            last: self.last.lock().unwrap().take(),
        }
    }
}

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::MemKind;

#[derive(Clone, Copy, Debug)]
pub struct AccessEvent {
    pub key: (MemKind, u32),
    pub address: u32,
    pub value: u8,
    pub write: bool,
    pub pc: u32,
    pub dma: bool,
    pub sequence: u64,
}

#[derive(Default, Clone)]
pub struct MemoryAccess {
    pub reads: HashSet<(MemKind, u32)>,
    pub writes: HashSet<(MemKind, u32)>,
    last: Option<((MemKind, u32), u64)>,
    pub events: Vec<AccessEvent>,
}

impl MemoryAccess {
    pub fn last(&self) -> Option<(MemKind, u32)> {
        self.last.map(|(key, _)| key)
    }

    pub fn merge(&mut self, other: &MemoryAccess) {
        self.merge_highlights(other);
        self.events.extend_from_slice(&other.events);
        self.events.sort_unstable_by_key(|event| event.sequence);
    }

    pub fn merge_highlights(&mut self, other: &MemoryAccess) {
        self.reads.extend(other.reads.iter().copied());
        self.writes.extend(other.writes.iter().copied());
        if other.last.map(|(_, seq)| seq) > self.last.map(|(_, seq)| seq) {
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
    Transactions = 3,
}

#[derive(Default)]
pub struct AccessTracker {
    mode: AtomicU8,
    pc: AtomicU32,
    order: Arc<AtomicU64>,
    events: Mutex<Vec<AccessEvent>>,
    gated: AtomicBool,
    reads: Mutex<HashSet<(MemKind, u32)>>,
    writes: Mutex<HashSet<(MemKind, u32)>>,
    last: Mutex<Option<((MemKind, u32), u64)>>,
}

impl AccessTracker {
    pub(crate) fn share_order(&mut self, other: &Self) {
        self.order = other.order.clone();
    }

    pub(crate) fn instruction(&self, pc: u32) {
        self.pc.store(pc, Ordering::Relaxed);
    }

    pub(crate) fn begin(&self, mode: TrackMode) {
        self.mode.store(mode as u8, Ordering::Relaxed);
        self.gated.store(false, Ordering::Relaxed);
        *self.last.lock().unwrap() = None;
        self.events.lock().unwrap().clear();
        if matches!(mode, TrackMode::Full | TrackMode::Transactions) {
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
        self.mode.load(Ordering::Relaxed) != TrackMode::Off as u8
    }

    pub(crate) fn record(
        &self,
        key: (MemKind, u32),
        address: u32,
        write: bool,
        value: u8,
        dma: bool,
    ) {
        if !self.active() || (!dma && !self.gated.load(Ordering::Relaxed)) {
            return;
        }

        let sequence = self.order.fetch_add(1, Ordering::Relaxed);
        *self.last.lock().unwrap() = Some((key, sequence));

        let mode = self.mode.load(Ordering::Relaxed);
        if mode == TrackMode::Transactions as u8 {
            self.events.lock().unwrap().push(AccessEvent {
                key,
                address,
                value,
                write,
                dma,
                pc: self.pc.load(Ordering::Relaxed),
                sequence,
            });
        }

        if mode >= TrackMode::Full as u8 {
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
            events: std::mem::take(&mut *self.events.lock().unwrap()),
        }
    }
}

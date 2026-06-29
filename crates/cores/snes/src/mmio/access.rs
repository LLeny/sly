use std::cell::{Cell, RefCell};
use std::collections::HashSet;

use super::MemKind;

#[derive(Default, Clone)]
pub struct MemoryAccess {
    pub reads: HashSet<(MemKind, u32)>,
    pub writes: HashSet<(MemKind, u32)>,
    pub last: Option<(MemKind, u32)>,
}

#[derive(Default)]
pub struct AccessTracker {
    tracing: Cell<bool>,
    capturing: Cell<bool>,
    reads: RefCell<HashSet<(MemKind, u32)>>,
    writes: RefCell<HashSet<(MemKind, u32)>>,
    last: Cell<Option<(MemKind, u32)>>,
}

impl AccessTracker {
    pub(crate) fn start(&self) {
        self.tracing.set(true);
        self.capturing.set(false);
        self.reads.borrow_mut().clear();
        self.writes.borrow_mut().clear();
        self.last.set(None);
    }

    pub(crate) fn stop(&self) {
        self.tracing.set(false);
        self.capturing.set(false);
    }

    pub(crate) fn resume(&self) {
        if self.tracing.get() {
            self.capturing.set(true);
        }
    }

    pub(crate) fn pause(&self) {
        self.capturing.set(false);
    }

    pub(super) fn capturing(&self) -> bool {
        self.capturing.get()
    }

    pub(super) fn record_read(&self, key: (MemKind, u32)) {
        self.reads.borrow_mut().insert(key);
        self.last.set(Some(key));
    }

    pub(super) fn record_write(&self, key: (MemKind, u32)) {
        self.writes.borrow_mut().insert(key);
        self.last.set(Some(key));
    }

    pub fn snapshot(&self) -> MemoryAccess {
        MemoryAccess {
            reads: self.reads.borrow().clone(),
            writes: self.writes.borrow().clone(),
            last: self.last.get(),
        }
    }
}

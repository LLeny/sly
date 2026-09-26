use std::cell::{Cell, RefCell, RefMut};
use std::collections::VecDeque;
use std::sync::Arc;

use debugger_model::{
    BreakpointKind, BreakpointMark, ConsoleLevel, ConsoleLine, CpuDescriptor, CpuId, DebugCore,
    EventHit, HookHit, MemoryRegion,
};

use piccolo::StashedFunction;

const CONSOLE_LIMIT: usize = 4096;

#[derive(Clone, PartialEq, Eq)]
pub enum Trigger {
    Access(BreakpointKind),
    Frame,
    Event { id: Arc<str>, value: Option<u32> },
}

pub struct Hook {
    pub id: u64,
    pub trigger: Trigger,
    pub start: u32,
    pub end: u32,
    pub cpu: CpuId,
    pub region: Arc<str>,
    pub func: StashedFunction,
}

impl Hook {
    pub fn mark(&self) -> Option<BreakpointMark> {
        let Trigger::Access(kind) = self.trigger else {
            return None;
        };

        Some(BreakpointMark {
            start: self.start,
            end: self.end,
            kind,
            cpu: self.cpu.clone(),
            region: self.region.clone(),
        })
    }

    pub fn matches_event(&self, event: &EventHit) -> bool {
        match &self.trigger {
            Trigger::Event { id, value } => {
                *id == event.id && value.is_none_or(|wanted| wanted == event.value)
            }
            _ => false,
        }
    }

    pub fn matches(&self, hit: &HookHit) -> bool {
        self.mark().is_some_and(|mark| mark.covers_hit(hit))
    }
}

#[derive(Default)]
pub struct HostState {
    pub cpus: Vec<CpuDescriptor>,
    pub events: Vec<Arc<str>>,
    pub regions: Vec<MemoryRegion>,
    pub hooks: Vec<Hook>,
    pub output: VecDeque<ConsoleLine>,
    pub status: Option<Arc<str>>,
    pub pause: Option<bool>,
    pub frame: u64,
    next_hook: u64,
}

impl HostState {
    pub fn cpu(&self, name: Option<&str>) -> Option<&CpuDescriptor> {
        match name {
            None => self.cpus.first(),
            Some(name) => self.cpus.iter().find(|cpu| {
                cpu.id.as_str().eq_ignore_ascii_case(name) || cpu.title.eq_ignore_ascii_case(name)
            }),
        }
    }

    pub fn region(&self, cpu: Option<&str>, region: Option<&str>) -> Option<Arc<str>> {
        match region {
            Some(region) => self.region_descriptor(region).map(|known| known.id.clone()),
            None => self.cpu(cpu).map(|cpu| cpu.space.clone()),
        }
    }

    pub fn region_names(&self) -> impl Iterator<Item = &Arc<str>> {
        self.regions.iter().map(|region| &region.id)
    }

    pub fn region_descriptor(&self, region: &str) -> Option<&MemoryRegion> {
        self.regions
            .iter()
            .find(|known| known.id.eq_ignore_ascii_case(region))
    }

    pub fn add_hook(
        &mut self,
        trigger: Trigger,
        start: u32,
        end: u32,
        cpu: CpuId,
        region: Arc<str>,
        func: StashedFunction,
    ) -> u64 {
        self.next_hook += 1;
        let id = self.next_hook;
        self.hooks.push(Hook {
            id,
            trigger,
            start,
            end,
            cpu,
            region,
            func,
        });
        id
    }

    pub fn remove_hook(&mut self, id: u64) -> bool {
        let before = self.hooks.len();
        self.hooks.retain(|hook| hook.id != id);
        self.hooks.len() != before
    }

    pub fn print(&mut self, level: ConsoleLevel, text: impl Into<Arc<str>>) {
        if self.output.len() == CONSOLE_LIMIT {
            self.output.pop_front();
        }
        self.output.push_back(ConsoleLine::new(level, text));
    }
}

pub struct Host {
    core: Cell<Option<*mut (dyn DebugCore + 'static)>>,
    state: RefCell<HostState>,
}

impl Host {
    pub fn new(core: &dyn DebugCore) -> Self {
        let host = Self {
            core: Cell::new(None),
            state: RefCell::new(HostState::default()),
        };
        host.describe(core);
        host
    }

    pub fn describe(&self, core: &dyn DebugCore) {
        let mut state = self.state.borrow_mut();
        state.cpus = core.cpus();
        state.regions = core.memory_regions();
        state.events = core.events().into_iter().map(|event| event.id).collect();
    }

    pub fn state(&self) -> RefMut<'_, HostState> {
        self.state.borrow_mut()
    }

    pub fn print(&self, level: ConsoleLevel, text: impl Into<Arc<str>>) {
        self.state().print(level, text);
    }

    pub fn enter<'a>(&'a self, core: &'a mut dyn DebugCore) -> CoreGuard<'a> {
        let ptr: *mut dyn DebugCore = core;
        let ptr: *mut (dyn DebugCore + 'static) = unsafe { std::mem::transmute(ptr) };
        self.core.set(Some(ptr));
        CoreGuard { host: self }
    }

    pub fn with_core<R>(&self, f: impl FnOnce(&mut dyn DebugCore) -> R) -> Option<R> {
        let ptr = self.core.get()?;
        Some(f(unsafe { &mut *ptr }))
    }
}

pub struct CoreGuard<'a> {
    host: &'a Host,
}

impl Drop for CoreGuard<'_> {
    fn drop(&mut self) {
        self.host.core.set(None);
    }
}

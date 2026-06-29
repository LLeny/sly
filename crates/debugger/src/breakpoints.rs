use cirsium::components::ComboOption;
use cirsium::widgets::{BreakpointEntry, BreakpointKind, BreakpointsView};
use gpui::{Pixels, Point, SharedString};

use crate::target::DebugTarget;

#[derive(Clone)]
pub struct BreakpointMark {
    pub start: u32,
    pub end: u32,
    pub kind: BreakpointKind,
    pub region: SharedString,
}

pub fn mark_at(marks: &[BreakpointMark], addr: u32, mask: u32) -> Option<BreakpointKind> {
    let addr = addr & mask;
    marks
        .iter()
        .find(|m| (m.start & mask..=m.end & mask).contains(&addr))
        .map(|m| m.kind)
}

struct Breakpoint {
    id: usize,
    start: u32,
    end: u32,
    kind: BreakpointKind,
    region: SharedString,
    enabled: bool,
    label: SharedString,
    note: SharedString,
}

pub struct Breakpoints {
    pub combo_open: Option<Point<Pixels>>,
    entries: Vec<Breakpoint>,
    marks: Vec<BreakpointMark>,
    draft_kind: BreakpointKind,
    next_id: usize,
}

impl Default for Breakpoints {
    fn default() -> Self {
        Self {
            combo_open: None,
            entries: Vec::new(),
            marks: Vec::new(),
            draft_kind: BreakpointKind::Execute,
            next_id: 0,
        }
    }
}

impl Breakpoints {
    pub fn draft_kind(&self) -> BreakpointKind {
        self.draft_kind
    }

    pub fn set_draft_kind(&mut self, kind: BreakpointKind) {
        self.draft_kind = kind;
        self.combo_open = None;
    }

    pub fn options(&self) -> Vec<ComboOption> {
        BreakpointKind::ALL
            .iter()
            .map(|kind| ComboOption::new(kind.id(), kind.tag()))
            .collect()
    }

    pub fn add(&mut self, addr: u32, target: &dyn DebugTarget) {
        self.add_kind(addr, addr, self.draft_kind, target);
    }

    pub fn add_kind(
        &mut self,
        start: u32,
        end: u32,
        kind: BreakpointKind,
        target: &dyn DebugTarget,
    ) {
        let label = format!("${}", target.format_address("bus", start)).into();
        let note = target.annotate(start);
        self.add_labeled(start, end, kind, "bus".into(), label, note);
    }

    pub fn add_labeled(
        &mut self,
        start: u32,
        end: u32,
        kind: BreakpointKind,
        region: SharedString,
        label: SharedString,
        note: SharedString,
    ) {
        let (start, end) = (start.min(end), start.max(end));
        if self
            .entries
            .iter()
            .any(|b| b.start == start && b.end == end && b.kind == kind && b.region == region)
        {
            return;
        }
        self.entries.push(Breakpoint {
            id: self.next_id,
            start,
            end,
            kind,
            region,
            enabled: true,
            label,
            note,
        });
        self.next_id += 1;
        self.rebuild_marks();
    }

    pub fn remove(&mut self, id: usize) {
        self.entries.retain(|b| b.id != id);
        self.rebuild_marks();
    }

    pub fn toggle(&mut self, id: usize) {
        if let Some(b) = self.entries.iter_mut().find(|b| b.id == id) {
            b.enabled = !b.enabled;
            self.rebuild_marks();
        }
    }

    pub fn open_combo(&mut self, position: Point<Pixels>) {
        self.combo_open = Some(position);
    }

    pub fn close_combo(&mut self) -> bool {
        self.combo_open.take().is_some()
    }

    pub fn marks(&self) -> &[BreakpointMark] {
        &self.marks
    }

    fn rebuild_marks(&mut self) {
        self.marks = self
            .entries
            .iter()
            .filter(|b| b.enabled)
            .map(|b| BreakpointMark {
                start: b.start,
                end: b.end,
                kind: b.kind,
                region: b.region.clone(),
            })
            .collect();
    }

    pub fn view(&self) -> BreakpointsView {
        BreakpointsView {
            entries: self
                .entries
                .iter()
                .map(|b| BreakpointEntry {
                    id: b.id,
                    label: b.label.clone(),
                    note: b.note.clone(),
                    kind: b.kind,
                    enabled: b.enabled,
                })
                .collect(),
        }
    }
}

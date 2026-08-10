use std::sync::Arc;

use cirsium::components::ComboOption;
use cirsium::widgets::{BreakpointEntry, BreakpointsView};
use debugger_model::{BreakpointKind, BreakpointMark, CpuId, DebugCore};
use gpui::{Pixels, Point, SharedString};

struct Breakpoint {
    id: usize,
    start: u32,
    end: u32,
    kind: BreakpointKind,
    cpu: CpuId,
    region: Arc<str>,
    pc_mask: u32,
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
    cpu_titles: Vec<(CpuId, SharedString)>,
}

impl Default for Breakpoints {
    fn default() -> Self {
        Self {
            combo_open: None,
            entries: Vec::new(),
            marks: Vec::new(),
            draft_kind: BreakpointKind::Execute,
            next_id: 0,
            cpu_titles: Vec::new(),
        }
    }
}

impl Breakpoints {
    pub fn set_cpu_titles(&mut self, titles: Vec<(CpuId, SharedString)>) {
        self.cpu_titles = titles;
    }

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

    pub fn add(&mut self, addr: u32, cpu: CpuId, region: Arc<str>, target: &dyn DebugCore) {
        self.add_kind(addr, addr, self.draft_kind, cpu, region, target);
    }

    pub fn add_kind(
        &mut self,
        start: u32,
        end: u32,
        kind: BreakpointKind,
        cpu: CpuId,
        region: Arc<str>,
        target: &dyn DebugCore,
    ) {
        let label = format!("${}", target.format_address(&region, start)).into();
        let note = target.annotate(start).into();
        self.add_labeled(start, end, kind, cpu, region, label, note, target);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_labeled(
        &mut self,
        start: u32,
        end: u32,
        kind: BreakpointKind,
        cpu: CpuId,
        region: Arc<str>,
        label: SharedString,
        note: SharedString,
        target: &dyn DebugCore,
    ) {
        let (start, end) = (start.min(end), start.max(end));
        if self.entries.iter().any(|b| {
            b.start == start && b.end == end && b.kind == kind && b.cpu == cpu && b.region == region
        }) {
            return;
        }

        let pc_mask = target
            .cpus()
            .into_iter()
            .find(|desc| desc.id == cpu)
            .map_or(u32::MAX, |desc| desc.pc_mask());

        self.entries.push(Breakpoint {
            id: self.next_id,
            start,
            end,
            kind,
            cpu,
            region,
            pc_mask,
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
                cpu: b.cpu.clone(),
                region: b.region.clone(),
                pc_mask: b.pc_mask,
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
                    cpu: self
                        .cpu_titles
                        .iter()
                        .find(|(id, _)| id == &b.cpu)
                        .map(|(_, title)| title.clone())
                        .unwrap_or_default(),
                    kind: b.kind,
                    enabled: b.enabled,
                })
                .collect(),
        }
    }
}

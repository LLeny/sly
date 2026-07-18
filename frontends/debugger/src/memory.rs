use std::ops::Range;

use cirsium::components::ComboOption;
use cirsium::widgets::{MemoryCell, MemoryRow, ascii_glyph};
use debugger_model::{BreakpointKind, BreakpointMark, DebugCore, MemoryRegion, mark_at};
use gpui::{Pixels, Point, ScrollStrategy, SharedString, UniformListScrollHandle};

pub const BYTES_PER_ROW: usize = 16;

pub struct MemMenu {
    pub position: Point<Pixels>,
    pub range: (u32, u32),
    pub address: SharedString,
    pub hex: SharedString,
    pub raw: SharedString,
    pub ascii: SharedString,
}

pub struct Memory {
    pub scroll: UniformListScrollHandle,
    pub combo_open: Option<Point<Pixels>>,
    pub menu: Option<MemMenu>,
    regions: Vec<MemoryRegion>,
    current: MemoryRegion,
    selection: Option<(u32, u32)>,
    drag_anchor: Option<u32>,
    follow: bool,
    breakpoints: Vec<BreakpointMark>,
}

impl Memory {
    pub fn new(target: &dyn DebugCore) -> Self {
        let regions = target.memory_regions();
        let current = regions
            .first()
            .cloned()
            .unwrap_or_else(|| MemoryRegion::new("", "", 0, 0));
        Self {
            scroll: UniformListScrollHandle::new(),
            combo_open: None,
            menu: None,
            regions,
            current,
            selection: None,
            drag_anchor: None,
            follow: false,
            breakpoints: Vec::new(),
        }
    }

    pub fn set_breakpoints(&mut self, breakpoints: Vec<BreakpointMark>) {
        self.breakpoints = breakpoints;
    }

    pub fn breakpoint_ranges(&self) -> Vec<(u32, u32, BreakpointKind)> {
        self.breakpoints
            .iter()
            .map(|m| (m.start, m.end, m.kind))
            .collect()
    }

    pub fn region_id(&self) -> &str {
        &self.current.id
    }

    pub fn region_label(&self) -> SharedString {
        self.current.name.clone().into()
    }

    pub fn selection(&self) -> Option<(u32, u32)> {
        self.selection
    }

    pub fn options(&self) -> Vec<ComboOption> {
        self.regions
            .iter()
            .map(|r| ComboOption::new(r.id.clone(), r.name.clone()))
            .collect()
    }

    pub fn row_count(&self) -> usize {
        (self.current.size as usize).div_ceil(BYTES_PER_ROW)
    }

    pub fn rows_for(&self, range: Range<usize>, target: &dyn DebugCore) -> Vec<MemoryRow> {
        let region = self.current.id.as_ref();
        range
            .map(|row_ix| {
                let offset = (row_ix * BYTES_PER_ROW) as u32;
                let addr = self.current.base.wrapping_add(offset);
                let len = (self.current.size.saturating_sub(offset) as usize).min(BYTES_PER_ROW);
                let bytes = target.read_memory(region, addr, len);
                let highlights = target.memory_highlights(region, addr, len);
                let cells = bytes
                    .iter()
                    .zip(highlights)
                    .enumerate()
                    .map(|(i, (value, highlight))| {
                        let cell_addr = addr.wrapping_add(i as u32);
                        let breakpoint = mark_at(&self.breakpoints, cell_addr, !0);
                        MemoryCell {
                            value: *value,
                            highlight,
                            breakpoint,
                        }
                    })
                    .collect();
                MemoryRow {
                    addr,
                    label: target.format_address(region, addr).into(),
                    cells,
                }
            })
            .collect()
    }

    fn normalized_selection(&self) -> Option<(u32, u32)> {
        self.selection.map(|(a, b)| (a.min(b), a.max(b)))
    }

    fn read_span(&self, target: &dyn DebugCore, lo: u32, hi: u32) -> Vec<u8> {
        target.read_memory(&self.current.id, lo, (hi - lo + 1) as usize)
    }

    pub fn dump(&self, target: &dyn DebugCore) -> Option<(String, Vec<u8>)> {
        if self.current.size == 0 {
            return None;
        }

        let (lo, hi) = self
            .normalized_selection()
            .unwrap_or((self.current.base, self.current.base + self.current.size - 1));
        let bytes = self.read_span(target, lo, hi);
        let name = format!("{}_{lo:06X}-{hi:06X}.bin", self.current.id);

        Some((name, bytes))
    }

    fn scroll_to_addr(&mut self, addr: u32) -> Option<u32> {
        let base = self.current.base;
        if self.current.size == 0 {
            return None;
        }

        let clamped = addr.clamp(base, base + self.current.size - 1);
        let row = ((clamped - base) as usize) / BYTES_PER_ROW;
        self.scroll.scroll_to_item_strict(row, ScrollStrategy::Top);

        Some(clamped)
    }

    pub fn jump_to(&mut self, addr: u32) {
        if let Some(clamped) = self.scroll_to_addr(addr) {
            self.selection = Some((clamped, clamped));
        }
    }

    pub fn follow_enabled(&self) -> bool {
        self.follow
    }

    pub fn toggle_follow(&mut self) {
        self.follow = !self.follow;
    }

    pub fn follow_to(&mut self, region: &str, addr: u32) {
        if self.current.id.as_ref() == region {
            self.scroll_to_addr(addr);
        }
    }

    pub fn set_region(&mut self, id: &str) {
        if let Some(region) = self.regions.iter().find(|r| r.id.as_ref() == id) {
            self.current = region.clone();
            self.selection = None;
            self.drag_anchor = None;
            self.scroll.scroll_to_item_strict(0, ScrollStrategy::Top);
        }

        self.combo_open = None;
    }

    pub fn begin(&mut self, addr: u32, shift: bool) {
        let anchor = if shift {
            self.selection.map(|(a, _)| a).unwrap_or(addr)
        } else {
            addr
        };
        self.selection = Some((anchor, addr));
        self.drag_anchor = Some(anchor);
    }

    pub fn drag(&mut self, addr: u32) -> bool {
        if let Some(anchor) = self.drag_anchor {
            let next = Some((anchor, addr));
            if self.selection != next {
                self.selection = next;
                return true;
            }
        }
        false
    }

    pub fn open_combo(&mut self, position: Point<Pixels>) {
        self.combo_open = Some(position);
        self.menu = None;
    }

    pub fn close_combo(&mut self) -> bool {
        self.combo_open.take().is_some()
    }

    pub fn open_menu(&mut self, addr: u32, position: Point<Pixels>, target: &dyn DebugCore) {
        let (lo, hi) = match self.normalized_selection() {
            Some((lo, hi)) if (lo..=hi).contains(&addr) => (lo, hi),
            _ => {
                self.selection = Some((addr, addr));
                (addr, addr)
            }
        };

        let bytes = self.read_span(target, lo, hi);
        let octets: Vec<String> = bytes.iter().map(|b| format!("{b:02X}")).collect();
        let hex = octets.join(" ");
        let raw = octets.concat();
        let ascii = bytes.iter().map(|b| ascii_glyph(*b)).collect::<String>();

        self.menu = Some(MemMenu {
            position,
            range: (lo, hi),
            address: target.format_address(&self.current.id, lo).into(),
            hex: hex.into(),
            raw: raw.into(),
            ascii: ascii.into(),
        });
        self.combo_open = None;
    }

    pub fn close_menu(&mut self) -> bool {
        self.menu.take().is_some()
    }
}

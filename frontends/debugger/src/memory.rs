use std::collections::VecDeque;
use std::ops::Range;
use std::sync::Arc;

use cirsium::components::ComboOption;
use cirsium::widgets::{MemoryCell, MemoryRow, ascii_glyph};
use debugger_model::{BreakpointKind, BreakpointMark, DebugCore, MemoryRegion};
use gpui::{Pixels, Point, ScrollStrategy, SharedString, UniformListScrollHandle, point, px};

pub const BYTES_PER_ROW: usize = 16;
pub const DEBUG_PAGE_SIZE: usize = 0x1000;
const PAGE_ROWS: usize = DEBUG_PAGE_SIZE / BYTES_PER_ROW;
const MAX_CACHED_PAGES: usize = 8;
const SCROLL_WINDOW_ROWS: usize = 4096;
const SCROLL_WINDOW_MARGIN: usize = 512;

fn window_start(row: usize, total: usize) -> usize {
    row.saturating_sub(SCROLL_WINDOW_ROWS / 2)
        .min(total.saturating_sub(SCROLL_WINDOW_ROWS))
}

struct MemoryPage {
    region: Arc<str>,
    first_row: usize,
    rows: Arc<[MemoryRow]>,
}

pub struct MemMenu {
    pub position: Point<Pixels>,
    pub range: (u32, u32),
    pub address: SharedString,
    pub end_address: SharedString,
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
    pages: VecDeque<MemoryPage>,
    first_scroll_row: usize,
}

impl Memory {
    pub fn new(target: &dyn DebugCore) -> Self {
        let regions = target.memory_regions();
        let current = regions
            .first()
            .cloned()
            .unwrap_or_else(|| MemoryRegion::new("", "", 0, 0));
        let mut memory = Self {
            scroll: UniformListScrollHandle::new(),
            combo_open: None,
            menu: None,
            regions,
            current,
            selection: None,
            drag_anchor: None,
            follow: false,
            breakpoints: Vec::new(),
            pages: VecDeque::new(),
            first_scroll_row: 0,
        };
        memory.load_page(0, target);
        memory
    }

    pub fn supports_watchpoints(&self) -> bool {
        !self.current.watch_sources.is_empty()
    }

    pub fn set_breakpoints(&mut self, breakpoints: Vec<BreakpointMark>) {
        self.breakpoints = breakpoints;
    }

    pub fn breakpoint_ranges(&self) -> Vec<(u32, u32, BreakpointKind)> {
        self.breakpoints
            .iter()
            .filter(|m| m.region == self.current.id)
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
        ((self.current.end() - u64::from(self.current.base)) as usize).div_ceil(BYTES_PER_ROW)
    }

    pub fn row_window(&self) -> Range<usize> {
        self.first_scroll_row..(self.first_scroll_row + SCROLL_WINDOW_ROWS).min(self.row_count())
    }

    pub fn recenter_target(&self, expected_start: usize, row_height: Pixels) -> Option<usize> {
        if self.first_scroll_row != expected_start || row_height <= px(0.) {
            return None;
        }

        let scroll = self.scroll.0.borrow();
        if scroll.deferred_scroll_to_item.is_some() {
            return None;
        }

        let offset = scroll.base_handle.offset();
        let local_row = (f32::from(-offset.y) / f32::from(row_height)).max(0.) as usize;
        let window = self.row_window();
        if (SCROLL_WINDOW_MARGIN..window.len().saturating_sub(SCROLL_WINDOW_MARGIN))
            .contains(&local_row)
        {
            return None;
        }

        let total = self.row_count();
        let absolute_row = (window.start + local_row).min(total.saturating_sub(1));
        let new_start = window_start(absolute_row, total);
        (new_start != window.start).then_some(new_start)
    }

    pub fn recenter_scroll(&mut self, expected_start: usize, row_height: Pixels) -> bool {
        let Some(new_start) = self.recenter_target(expected_start, row_height) else {
            return false;
        };

        let delta = new_start as i64 - self.first_scroll_row as i64;
        let scroll = self.scroll.0.borrow();
        let offset = scroll.base_handle.offset();
        scroll
            .base_handle
            .set_offset(point(offset.x, offset.y + row_height * delta as f32));

        self.first_scroll_row = new_start;
        true
    }

    fn build_rows(&self, range: Range<usize>, target: &dyn DebugCore) -> Vec<MemoryRow> {
        let region = self.current.id.as_ref();
        range
            .map(|row_ix| {
                let offset = (row_ix * BYTES_PER_ROW) as u32;
                let addr = self.current.base + offset;
                let len = (self.current.end() - u64::from(addr)).min(BYTES_PER_ROW as u64) as usize;
                let bytes = target.peek_memory(region, addr, len);
                let highlights = target.memory_highlights(region, addr, len);
                let cells = bytes
                    .iter()
                    .zip(highlights)
                    .enumerate()
                    .map(|(i, (value, highlight))| {
                        let cell_addr = addr + i as u32;
                        let breakpoint = self
                            .breakpoints
                            .iter()
                            .find(|mark| mark.covers(mark.kind, &mark.cpu, region, cell_addr, 1))
                            .map(|mark| mark.kind);

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

    pub fn cached_rows(&self, range: Range<usize>) -> Vec<MemoryRow> {
        range
            .map(|row| {
                let page = self.pages.iter().find(|page| {
                    page.region == self.current.id
                        && (page.first_row..page.first_row + page.rows.len()).contains(&row)
                })?;
                page.rows.get(row - page.first_row).cloned()
            })
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default()
    }

    pub fn missing_pages(&self, range: Range<usize>) -> Vec<usize> {
        if range.is_empty() {
            return Vec::new();
        }
        let mut pages = Vec::new();
        let first = range.start / PAGE_ROWS * PAGE_ROWS;
        let last = range.end.saturating_sub(1) / PAGE_ROWS * PAGE_ROWS;
        for first_row in (first..=last).step_by(PAGE_ROWS) {
            let cached = self
                .pages
                .iter()
                .any(|page| page.region == self.current.id && page.first_row == first_row);
            if !cached {
                pages.push(first_row);
            }
        }
        pages
    }

    pub fn load_page(&mut self, first_row: usize, target: &dyn DebugCore) {
        let first_row = first_row / PAGE_ROWS * PAGE_ROWS;
        let end = (first_row + PAGE_ROWS).min(self.row_count());
        if first_row >= end {
            return;
        }

        self.pages
            .retain(|page| page.region != self.current.id || page.first_row != first_row);
        let rows = self.build_rows(first_row..end, target).into();
        self.pages.push_front(MemoryPage {
            region: self.current.id.clone(),
            first_row,
            rows,
        });
        self.pages.truncate(MAX_CACHED_PAGES);
    }

    pub fn refresh_cached(&mut self, target: &dyn DebugCore) {
        let pages: Vec<usize> = self
            .pages
            .iter()
            .filter(|page| page.region == self.current.id)
            .map(|page| page.first_row)
            .collect();
        for first_row in pages.into_iter().rev() {
            self.load_page(first_row, target);
        }
    }

    fn normalized_selection(&self) -> Option<(u32, u32)> {
        self.selection.map(|(a, b)| (a.min(b), a.max(b)))
    }

    fn read_span(&self, target: &dyn DebugCore, lo: u32, hi: u32) -> Vec<u8> {
        target.peek_memory(&self.current.id, lo, (hi - lo + 1) as usize)
    }

    pub fn dump(&self, target: &dyn DebugCore) -> Option<(String, Vec<u8>)> {
        if self.current.size == 0 {
            return None;
        }

        let (lo, hi) = self
            .normalized_selection()
            .unwrap_or((self.current.base, (self.current.end() - 1) as u32));
        let bytes = self.read_span(target, lo, hi);
        let name = format!("{}_{lo:06X}-{hi:06X}.bin", self.current.id);

        Some((name, bytes))
    }

    fn scroll_to_addr(&mut self, addr: u32) -> Option<u32> {
        let base = self.current.base;
        if self.current.size == 0 {
            return None;
        }

        let clamped = addr.clamp(base, (self.current.end() - 1) as u32);
        let row = ((clamped - base) as usize) / BYTES_PER_ROW;
        self.first_scroll_row = window_start(row, self.row_count());
        self.scroll
            .scroll_to_item_strict(row - self.first_scroll_row, ScrollStrategy::Top);

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
            self.first_scroll_row = 0;
            self.scroll.scroll_to_item_strict(0, ScrollStrategy::Top);
            self.pages.clear();
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
            end_address: target.format_address(&self.current.id, hi).into(),
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

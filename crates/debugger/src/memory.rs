use std::ops::Range;

use cirsium::components::ComboOption;
use cirsium::widgets::{MemoryCell, MemoryRegion, MemoryRow, ascii_glyph};
use gpui::{Pixels, Point, ScrollStrategy, SharedString, UniformListScrollHandle};

use crate::target::DebugTarget;

pub const BYTES_PER_ROW: usize = 16;

pub struct MemMenu {
    pub position: Point<Pixels>,
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
}

impl Memory {
    pub fn new(target: &dyn DebugTarget) -> Self {
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
        }
    }

    pub fn region_id(&self) -> &str {
        &self.current.id
    }

    pub fn region_label(&self) -> SharedString {
        self.current.name.clone()
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

    pub fn rows_for(&self, range: Range<usize>, target: &dyn DebugTarget) -> Vec<MemoryRow> {
        let base = self.current.base;
        let size = self.current.size;
        let region = self.current.id.as_ref();
        range
            .map(|row_ix| {
                let offset = (row_ix * BYTES_PER_ROW) as u32;
                let addr = base.wrapping_add(offset);
                let len = (size.saturating_sub(offset) as usize).min(BYTES_PER_ROW);
                let bytes = target.read_memory(region, addr, len);
                let highlights = target.memory_highlights(region, addr, len);
                let cells = bytes
                    .iter()
                    .zip(highlights)
                    .map(|(value, highlight)| MemoryCell {
                        value: *value,
                        highlight,
                    })
                    .collect();
                MemoryRow {
                    addr,
                    label: target.format_address(region, addr),
                    cells,
                }
            })
            .collect()
    }

    fn scroll_to_addr(&mut self, addr: u32) -> Option<u32> {
        let base = self.current.base;
        let size = self.current.size;
        if size == 0 {
            return None;
        }

        let clamped = addr.clamp(base, base + size - 1);
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
        if self.current.id.as_ref() != region
            && let Some(r) = self.regions.iter().find(|r| r.id.as_ref() == region)
        {
            self.current = r.clone();
            self.selection = None;
            self.drag_anchor = None;
        }

        self.scroll_to_addr(addr);
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

    pub fn open_menu(&mut self, addr: u32, position: Point<Pixels>, target: &dyn DebugTarget) {
        let (lo, hi) = match self.selection {
            Some((a, b)) if (a.min(b)..=a.max(b)).contains(&addr) => (a.min(b), a.max(b)),
            _ => {
                self.selection = Some((addr, addr));
                (addr, addr)
            }
        };

        let len = (hi - lo + 1) as usize;
        let bytes = target.read_memory(&self.current.id, lo, len);
        let octets: Vec<String> = bytes.iter().map(|b| format!("{b:02X}")).collect();
        let hex = octets.join(" ");
        let raw = octets.concat();
        let ascii = bytes.iter().map(|b| ascii_glyph(*b)).collect::<String>();

        self.menu = Some(MemMenu {
            position,
            address: target.format_address(&self.current.id, lo),
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

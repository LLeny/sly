use std::ops::Range;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{
    AnyElement, App, ClickEvent, Pixels, Point, SharedString, UniformListScrollHandle, Window, div,
    px, uniform_list,
};

use crate::assets::icon;
use crate::components::{ClickHandler, Panel, combobox_button, uniform_vertical_scroll_area};
use crate::diff::Highlight;
use crate::theme::Theme;
use crate::widgets::BreakpointKind;

mod row;

const MIN_LIST_HEIGHT: f32 = 200.;

type ByteDownCallback = Rc<dyn Fn(u32, bool, &mut Window, &mut App) + 'static>;
type ByteDragCallback = Rc<dyn Fn(u32, &mut Window, &mut App) + 'static>;
type ContextCallback = Rc<dyn Fn(u32, Point<Pixels>, &mut Window, &mut App) + 'static>;

pub type RowProvider = Rc<dyn Fn(Range<usize>, &mut App) -> Vec<MemoryRow> + 'static>;

#[derive(Clone)]
pub struct MemoryCell {
    pub value: u8,
    pub highlight: Highlight,
    pub breakpoint: Option<BreakpointKind>,
}

#[derive(Clone)]
pub struct MemoryRow {
    pub addr: u32,
    pub label: SharedString,
    pub cells: Vec<MemoryCell>,
}

#[derive(IntoElement)]
pub struct MemoryWidget {
    rows: RowProvider,
    row_count: usize,
    bytes_per_row: usize,
    selection: Option<(u32, u32)>,
    breakpoints: Vec<(u32, u32, BreakpointKind)>,
    scroll: UniformListScrollHandle,
    region_label: SharedString,
    header_extra: Option<AnyElement>,
    collapsed: bool,
    on_toggle: Option<ClickHandler>,
    on_region_open: Option<ClickHandler>,
    on_byte_down: Option<ByteDownCallback>,
    on_byte_drag: Option<ByteDragCallback>,
    on_context: Option<ContextCallback>,
}

impl MemoryWidget {
    pub const ID: &'static str = debugger_model::panel::MEMORY;

    pub fn new(
        rows: RowProvider,
        row_count: usize,
        bytes_per_row: usize,
        scroll: UniformListScrollHandle,
        region_label: impl Into<SharedString>,
    ) -> Self {
        Self {
            rows,
            row_count,
            bytes_per_row: bytes_per_row.max(1),
            selection: None,
            breakpoints: Vec::new(),
            scroll,
            region_label: region_label.into(),
            header_extra: None,
            collapsed: false,
            on_toggle: None,
            on_region_open: None,
            on_byte_down: None,
            on_byte_drag: None,
            on_context: None,
        }
    }

    pub fn selection(mut self, selection: Option<(u32, u32)>) -> Self {
        self.selection = selection;
        self
    }

    /// Enabled breakpoint ranges, used to merge adjacent breakpoint cells.
    pub fn breakpoints(mut self, breakpoints: Vec<(u32, u32, BreakpointKind)>) -> Self {
        self.breakpoints = breakpoints;
        self
    }

    pub fn header_extra(mut self, element: impl IntoElement) -> Self {
        self.header_extra = Some(element.into_any_element());
        self
    }

    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    pub fn on_toggle(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Box::new(handler));
        self
    }

    pub fn on_region_open(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_region_open = Some(Box::new(handler));
        self
    }

    pub fn on_byte_down(
        mut self,
        handler: impl Fn(u32, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_byte_down = Some(Rc::new(handler));
        self
    }

    pub fn on_byte_drag(mut self, handler: impl Fn(u32, &mut Window, &mut App) + 'static) -> Self {
        self.on_byte_drag = Some(Rc::new(handler));
        self
    }

    pub fn on_context(
        mut self,
        handler: impl Fn(u32, Point<Pixels>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_context = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for MemoryWidget {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;

        let bar = div()
            .flex()
            .items_center()
            .gap_2()
            .pb_2()
            .child(combobox_button(
                t,
                "mem-region",
                self.region_label.clone(),
                move |e, window, cx| {
                    if let Some(handler) = &self.on_region_open {
                        handler(e, window, cx);
                    }
                },
            ))
            .children(self.header_extra);

        let provider = self.rows;
        let selection = self.selection.map(|(a, b)| (a.min(b), a.max(b)));
        let breakpoints = Rc::new(self.breakpoints);
        let scroll = self.scroll.clone();

        let list = uniform_list("mem-list", self.row_count, move |range, _window, cx| {
            let rows = provider(range, cx);
            let theme = Theme::global(cx);
            let t = &*theme;
            rows.iter()
                .enumerate()
                .map(|(i, row)| {
                    row::mem_row(
                        t,
                        row,
                        i.checked_sub(1).and_then(|j| rows.get(j)),
                        rows.get(i + 1),
                        self.bytes_per_row,
                        selection,
                        &breakpoints,
                        self.on_byte_down.clone(),
                        self.on_byte_drag.clone(),
                        self.on_context.clone(),
                    )
                })
                .collect::<Vec<_>>()
        })
        .track_scroll(&scroll)
        .flex_1()
        .min_h(px(MIN_LIST_HEIGHT));
        let list = uniform_vertical_scroll_area(list, &scroll, t).flex_1();

        let body = div()
            .flex()
            .flex_col()
            .h_full()
            .child(bar)
            .child(row::column_header(t, self.bytes_per_row))
            .child(list);

        let mut panel = Panel::new(Self::ID, "Memory")
            .glyph(icon::CARTRIDGE)
            .collapsed(self.collapsed)
            .grow(true)
            .on_toggle_opt(self.on_toggle)
            .child(body);
        if let Some((lo, hi)) = selection {
            panel = panel.meta(format!("${lo:06X}–${hi:06X} ({})", hi - lo + 1));
        }
        panel
    }
}

pub fn ascii_glyph(byte: u8) -> char {
    if (0x20..=0x7e).contains(&byte) {
        byte as char
    } else {
        '.'
    }
}

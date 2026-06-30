use std::rc::Rc;

use gpui::prelude::*;
use gpui::{AnyElement, App, ClickEvent, Hsla, SharedString, Window, div, px, svg};

use crate::assets::icon;
use crate::components::{ClickHandler, Panel, badge, combobox_button, field_button};
use crate::theme::Theme;

type RowCallback = Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BreakpointKind {
    Execute,
    Read,
    Write,
}

impl BreakpointKind {
    pub const ALL: [BreakpointKind; 3] = [
        BreakpointKind::Execute,
        BreakpointKind::Read,
        BreakpointKind::Write,
    ];

    pub fn id(self) -> &'static str {
        match self {
            BreakpointKind::Execute => "execute",
            BreakpointKind::Read => "read",
            BreakpointKind::Write => "write",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "execute" => Some(BreakpointKind::Execute),
            "read" => Some(BreakpointKind::Read),
            "write" => Some(BreakpointKind::Write),
            _ => None,
        }
    }

    pub fn tag(self) -> &'static str {
        match self {
            BreakpointKind::Execute => "PC",
            BreakpointKind::Read => "Read",
            BreakpointKind::Write => "Write",
        }
    }

    pub fn color(self, theme: &Theme) -> Hsla {
        match self {
            BreakpointKind::Execute => theme.danger,
            BreakpointKind::Read => theme.read_border,
            BreakpointKind::Write => theme.write_border,
        }
    }

    pub fn colors(self, theme: &Theme) -> (Hsla, Hsla) {
        match self {
            BreakpointKind::Execute => (theme.exec_fill, theme.exec_border),
            BreakpointKind::Read => (theme.read_fill, theme.read_border),
            BreakpointKind::Write => (theme.write_fill, theme.write_border),
        }
    }
}

#[derive(Clone)]
pub struct BreakpointEntry {
    pub id: usize,
    pub label: SharedString,
    pub note: SharedString,
    pub kind: BreakpointKind,
    pub enabled: bool,
}

#[derive(Clone, Default)]
pub struct BreakpointsView {
    pub entries: Vec<BreakpointEntry>,
}

#[derive(IntoElement)]
pub struct BreakpointsWidget {
    view: BreakpointsView,
    draft_kind: BreakpointKind,
    address_input: Option<AnyElement>,
    collapsed: bool,
    on_toggle: Option<ClickHandler>,
    on_kind_open: Option<ClickHandler>,
    on_add: Option<ClickHandler>,
    on_remove: Option<RowCallback>,
    on_toggle_entry: Option<RowCallback>,
    scrollable: bool,
}

impl BreakpointsWidget {
    pub const ID: &'static str = "breakpoints";

    pub fn new(view: BreakpointsView, draft_kind: BreakpointKind) -> Self {
        Self {
            view,
            draft_kind,
            address_input: None,
            collapsed: false,
            on_toggle: None,
            on_kind_open: None,
            on_add: None,
            on_remove: None,
            on_toggle_entry: None,
            scrollable: false,
        }
    }

    pub fn scrollable(mut self, scrollable: bool) -> Self {
        self.scrollable = scrollable;
        self
    }

    pub fn address_input(mut self, input: impl IntoElement) -> Self {
        self.address_input = Some(input.into_any_element());
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

    pub fn on_kind_open(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_kind_open = Some(Box::new(handler));
        self
    }

    pub fn on_add(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_add = Some(Box::new(handler));
        self
    }

    pub fn on_remove(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_remove = Some(Rc::new(handler));
        self
    }

    pub fn on_toggle_entry(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle_entry = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for BreakpointsWidget {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;

        let kind_combo = combobox_button(t, "bp-kind", self.draft_kind.tag(), move |e, w, cx| {
            if let Some(handler) = self.on_kind_open.as_ref() {
                handler(e, w, cx);
            }
        });

        let add_button = field_button(t, "bp-add", icon::PLUS, move |e, w, cx| {
            if let Some(handler) = self.on_add.as_ref() {
                handler(e, w, cx);
            }
        });

        let add_row = div()
            .flex()
            .items_center()
            .gap_1()
            .child(kind_combo)
            .children(self.address_input)
            .child(add_button);

        let count = self.view.entries.len();

        let mut list = div()
            .id("bp-list")
            .flex()
            .flex_col()
            .when(self.scrollable, |d| {
                d.flex_1().min_h(px(0.)).overflow_y_scroll()
            });
        if count == 0 {
            list = list
                .flex_1()
                .min_h(px(0.))
                .items_center()
                .justify_center()
                .child(
                    div()
                        .px_1()
                        .py_1()
                        .text_xs()
                        .text_color(t.text_dim)
                        .child("No breakpoints"),
                );
        } else {
            list = list.children(self.view.entries.into_iter().map(|entry| {
                breakpoint_row(
                    t,
                    entry,
                    self.on_toggle_entry.clone(),
                    self.on_remove.clone(),
                )
            }));
        }

        let body = div()
            .flex()
            .flex_col()
            .gap_2()
            .when(self.scrollable, |d| d.flex_1().min_h(px(0.)))
            .child(add_row)
            .child(list);

        Panel::new(Self::ID, "Breakpoints")
            .glyph(icon::BREAKPOINT)
            .grow(self.scrollable)
            .collapsed(self.collapsed)
            .meta(format!("{count}"))
            .on_toggle_opt(self.on_toggle)
            .child(body)
    }
}

fn breakpoint_row(
    theme: &Theme,
    entry: BreakpointEntry,
    on_toggle_entry: Option<RowCallback>,
    on_remove: Option<RowCallback>,
) -> impl IntoElement {
    let BreakpointEntry {
        id,
        label,
        note,
        kind,
        enabled,
    } = entry;

    let kind_color = kind.color(theme);

    let mut dot = div()
        .id(("bp-dot", id))
        .size(px(13.))
        .flex_none()
        .rounded_sm()
        .border_1();
    if enabled {
        dot = dot.bg(kind_color).border_color(kind_color);
    } else {
        dot = dot.border_color(theme.border_strong);
    }

    if let Some(handler) = on_toggle_entry {
        dot = dot
            .cursor_pointer()
            .on_click(move |_, window, cx| handler(id, window, cx));
    }

    let address = div()
        .font_family(theme.mono_font)
        .text_xs()
        .text_color(if enabled {
            theme.text_primary
        } else {
            theme.text_dim
        })
        .child(label);

    let tag = badge(kind.tag(), kind_color).rounded_sm();

    let mut row = div()
        .flex()
        .items_center()
        .gap_2()
        .px_1()
        .py(px(3.))
        .child(dot)
        .child(address);

    if !note.is_empty() {
        row = row.child(div().text_xs().text_color(theme.text_muted).child(note));
    }

    row = row.child(div().flex_1()).child(tag);

    if let Some(handler) = on_remove {
        let remove = div()
            .id(("bp-remove", id))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .size(px(16.))
            .cursor_pointer()
            .on_click(move |_, window, cx| handler(id, window, cx))
            .child(
                svg()
                    .path(icon::TRASH)
                    .size(px(13.))
                    .text_color(theme.danger)
                    .hover(|s| s.text_color(theme.danger_strong)),
            );
        row = row.child(remove);
    }

    row
}

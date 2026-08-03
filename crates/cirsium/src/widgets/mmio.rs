use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{App, ClickEvent, Div, FontWeight, SharedString, Stateful, Window, div, px};

use crate::assets::icon;
use crate::components::{
    ClickHandler, DETAIL_INDENT, INDENT_STEP, Panel, RowCallback, chevron_slot, hex, label_text,
    row_toggle,
};
use crate::diff::{Highlight, highlight_color};
use crate::theme::Theme;

#[derive(Clone)]
pub struct MmioFieldView {
    pub name: SharedString,
    pub bit_label: SharedString,
    pub value: u32,
    pub display: SharedString,
    pub is_bool: bool,
    pub children: Vec<MmioFieldView>,
}

#[derive(Clone)]
pub struct MmioRegisterView {
    pub id: SharedString,
    pub name: SharedString,
    pub addr: SharedString,
    pub value: u32,
    pub bits: u8,
    pub highlight: Highlight,
    pub fields: Vec<MmioFieldView>,
}

impl MmioRegisterView {
    fn hex(&self) -> String {
        hex(self.value, self.bits)
    }
}

#[derive(Clone)]
pub struct MmioGroup {
    pub name: SharedString,
    pub registers: Vec<MmioRegisterView>,
}

#[derive(Clone, Default)]
pub struct MmioView {
    pub groups: Vec<MmioGroup>,
}

impl From<debugger_model::MmioFieldView> for MmioFieldView {
    fn from(field: debugger_model::MmioFieldView) -> Self {
        Self {
            name: field.name.into(),
            bit_label: field.bit_label.into(),
            value: field.value,
            display: field.display.into(),
            is_bool: field.is_bool,
            children: field.children.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<debugger_model::MmioRegisterView> for MmioRegisterView {
    fn from(reg: debugger_model::MmioRegisterView) -> Self {
        Self {
            id: reg.id.into(),
            name: reg.name.into(),
            addr: reg.addr.into(),
            value: reg.value,
            bits: reg.bits,
            highlight: reg.highlight,
            fields: reg.fields.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<debugger_model::MmioGroup> for MmioGroup {
    fn from(group: debugger_model::MmioGroup) -> Self {
        Self {
            name: group.name.into(),
            registers: group.registers.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<debugger_model::MmioView> for MmioView {
    fn from(view: debugger_model::MmioView) -> Self {
        Self {
            groups: view.groups.into_iter().map(Into::into).collect(),
        }
    }
}

impl MmioView {
    fn register_count(&self) -> usize {
        self.groups.iter().map(|g| g.registers.len()).sum()
    }
}

#[derive(IntoElement)]
pub struct MmioWidget {
    view: Arc<MmioView>,
    expanded: HashSet<SharedString>,
    collapsed: bool,
    priority: Option<SharedString>,
    on_toggle: Option<ClickHandler>,
    on_toggle_register: Option<RowCallback>,
}

impl MmioWidget {
    pub const ID: &'static str = "mmio";

    pub fn new(view: Arc<MmioView>) -> Self {
        Self {
            view,
            expanded: HashSet::new(),
            collapsed: false,
            priority: None,
            on_toggle: None,
            on_toggle_register: None,
        }
    }

    pub fn expanded(mut self, expanded: HashSet<SharedString>) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    pub fn priority(mut self, group: Option<SharedString>) -> Self {
        self.priority = group;
        self
    }

    pub fn on_toggle(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Box::new(handler));
        self
    }

    pub fn on_toggle_register(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle_register = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for MmioWidget {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;

        let count = self.view.register_count();

        let mut list = div()
            .id("mmio-list")
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll();

        if count == 0 {
            list = list.items_center().justify_center().child(
                div()
                    .px_1()
                    .py_1()
                    .text_xs()
                    .text_color(t.text_dim)
                    .child("No registers"),
            );
        } else {
            let mut groups: Vec<_> = self.view.groups.iter().collect();
            if let Some(priority) = &self.priority
                && groups.iter().any(|g| g.name == *priority)
            {
                groups.sort_by_key(|g| g.name != *priority);
            }

            for group in groups {
                list = list.child(self::group_header(t, group.name.clone()));
                for reg in &group.registers {
                    let is_expanded = self.expanded.contains(&reg.id);
                    list = list.child(self::register_row(
                        t,
                        reg,
                        is_expanded,
                        self.on_toggle_register.clone(),
                    ));
                    if is_expanded {
                        list = list.children(self::field_rows(t, &reg.fields, 0));
                    }
                }
            }
        }

        Panel::new(Self::ID, "MMIO")
            .glyph(icon::LIST)
            .grow(true)
            .collapsed(self.collapsed)
            .meta(format!("{count}"))
            .on_toggle_opt(self.on_toggle)
            .child(list)
    }
}

fn group_header(theme: &Theme, name: SharedString) -> Div {
    div()
        .flex()
        .items_center()
        .px_1()
        .py(px(2.))
        .bg(theme.bg_panel_alt)
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.text_muted)
        .child(name.to_uppercase())
}

fn register_row(
    theme: &Theme,
    reg: &MmioRegisterView,
    expanded: bool,
    on_toggle: Option<RowCallback>,
) -> Stateful<Div> {
    let expandable = !reg.fields.is_empty();
    let chevron = chevron_slot(theme, expandable, expanded);

    let name = div()
        .text_xs()
        .text_color(theme.text_secondary)
        .child(reg.name.clone());

    let addr = div()
        .font_family(theme.mono_font)
        .text_xs()
        .text_color(theme.text_dim)
        .child(reg.addr.clone());

    let value = div()
        .font_family(theme.mono_font)
        .text_xs()
        .text_color(highlight_color(theme, reg.highlight))
        .child(reg.hex());

    let mut row = div()
        .id(reg.id.clone())
        .flex()
        .items_center()
        .gap_2()
        .px_1()
        .py(px(3.))
        .child(chevron)
        .child(name)
        .child(addr)
        .child(div().flex_1())
        .child(value);

    if expandable && let Some(handler) = on_toggle {
        row = row_toggle(row, theme, reg.id.clone(), handler);
    }

    row
}

fn field_rows(theme: &Theme, fields: &[MmioFieldView], depth: usize) -> Vec<Div> {
    let mut rows = Vec::new();
    for field in fields {
        rows.push(self::field_row(theme, field, depth));
        if !field.children.is_empty() {
            rows.extend(self::field_rows(theme, &field.children, depth + 1));
        }
    }
    rows
}

fn field_row(theme: &Theme, field: &MmioFieldView, depth: usize) -> Div {
    let indent = px(DETAIL_INDENT + depth as f32 * INDENT_STEP);

    let bit_label = div()
        .font_family(theme.mono_font)
        .text_xs()
        .text_color(theme.text_dim)
        .w(px(44.))
        .flex_none()
        .child(field.bit_label.clone());

    let name = label_text(theme, field.name.clone());

    let value_color = match (field.is_bool, field.value != 0) {
        (true, true) => theme.accent,
        (true, false) => theme.text_muted,
        (false, _) => theme.text_secondary,
    };
    let value = div()
        .font_family(theme.mono_font)
        .text_xs()
        .text_color(value_color)
        .child(field.display.clone());

    div()
        .flex()
        .items_center()
        .gap_2()
        .pl(indent)
        .pr_1()
        .py(px(2.))
        .child(bit_label)
        .child(name)
        .child(div().flex_1())
        .child(value)
}

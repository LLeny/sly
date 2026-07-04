use std::collections::HashSet;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{App, ClickEvent, Div, SharedString, Stateful, Window, div, px};

use crate::assets::icon;
use crate::components::{ClickHandler, Panel, RowCallback, chevron_slot, label_text, row_toggle};
use crate::theme::Theme;
use crate::widgets::field::Field;

pub type PpuProperty = Field;

#[derive(Clone)]
pub struct PpuGroup {
    pub id: SharedString,
    pub name: SharedString,
    pub summary: SharedString,
    pub properties: Vec<PpuProperty>,
}

#[derive(Clone, Default)]
pub struct PpuView {
    pub properties: Vec<PpuProperty>,
    pub groups: Vec<PpuGroup>,
}

impl From<debugger_model::PpuGroup> for PpuGroup {
    fn from(group: debugger_model::PpuGroup) -> Self {
        Self {
            id: group.id.into(),
            name: group.name.into(),
            summary: group.summary.into(),
            properties: group.properties.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<debugger_model::PpuView> for PpuView {
    fn from(view: debugger_model::PpuView) -> Self {
        Self {
            properties: view.properties.into_iter().map(Into::into).collect(),
            groups: view.groups.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(IntoElement)]
pub struct PpuWidget {
    view: PpuView,
    expanded: HashSet<SharedString>,
    collapsed: bool,
    on_toggle: Option<ClickHandler>,
    on_toggle_group: Option<RowCallback>,
}

impl PpuWidget {
    pub const ID: &'static str = "ppu";

    pub fn new(view: PpuView) -> Self {
        Self {
            view,
            expanded: HashSet::new(),
            collapsed: false,
            on_toggle: None,
            on_toggle_group: None,
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

    pub fn on_toggle(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Box::new(handler));
        self
    }

    pub fn on_toggle_group(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle_group = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for PpuWidget {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;

        let mut list = div()
            .id("ppu-list")
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll();

        if self.view.properties.is_empty() && self.view.groups.is_empty() {
            list = list.items_center().justify_center().child(
                div()
                    .px_1()
                    .py_1()
                    .text_xs()
                    .text_color(t.text_dim)
                    .child("No data"),
            );
        } else {
            list = list.children(
                self.view
                    .properties
                    .iter()
                    .map(|property| self::property_row(t, property)),
            );

            for group in &self.view.groups {
                let is_expanded = self.expanded.contains(&group.id);
                list = list.child(self::group_row(
                    t,
                    group,
                    is_expanded,
                    self.on_toggle_group.clone(),
                ));
                if is_expanded {
                    list = list.children(group.properties.iter().map(|property| {
                        crate::components::detail_row(
                            t,
                            property.label.clone(),
                            self::value_text(t, property),
                        )
                    }));
                }
            }
        }

        Panel::new(Self::ID, "PPU")
            .glyph(icon::MONITOR)
            .grow(true)
            .collapsed(self.collapsed)
            .on_toggle_opt(self.on_toggle)
            .child(list)
    }
}

fn value_text(theme: &Theme, property: &PpuProperty) -> Div {
    let value_color = if property.accent {
        theme.accent
    } else {
        theme.text_secondary
    };

    div()
        .font_family(theme.mono_font)
        .text_xs()
        .text_color(value_color)
        .child(property.value.clone())
}

fn property_row(theme: &Theme, property: &PpuProperty) -> Div {
    div()
        .flex()
        .items_center()
        .gap_2()
        .px_1()
        .py(px(3.))
        .child(label_text(theme, property.label.clone()))
        .child(div().flex_1())
        .child(self::value_text(theme, property))
}

fn group_row(
    theme: &Theme,
    group: &PpuGroup,
    expanded: bool,
    on_toggle: Option<RowCallback>,
) -> Stateful<Div> {
    let expandable = !group.properties.is_empty();

    let name = div()
        .text_xs()
        .text_color(theme.text_secondary)
        .child(group.name.clone());

    let summary = div()
        .font_family(theme.mono_font)
        .text_xs()
        .text_color(theme.text_dim)
        .child(group.summary.clone());

    let mut row = div()
        .id(group.id.clone())
        .flex()
        .items_center()
        .gap_2()
        .px_1()
        .py(px(3.))
        .child(chevron_slot(theme, expandable, expanded))
        .child(name)
        .child(div().flex_1())
        .child(summary);

    if expandable && let Some(handler) = on_toggle {
        row = row_toggle(row, theme, group.id.clone(), handler);
    }

    row
}

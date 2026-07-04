use std::collections::HashSet;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{App, ClickEvent, Div, SharedString, Stateful, Window, div, px, svg};

use crate::assets::icon;
use crate::components::{ClickHandler, Panel, label_text};
use crate::theme::Theme;

type GroupCallback = Rc<dyn Fn(SharedString, &mut Window, &mut App) + 'static>;

#[derive(Clone)]
pub struct PpuProperty {
    pub label: SharedString,
    pub value: SharedString,
    pub accent: bool,
}

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

impl From<debugger_model::PpuProperty> for PpuProperty {
    fn from(property: debugger_model::PpuProperty) -> Self {
        Self {
            label: property.label.into(),
            value: property.value.into(),
            accent: property.accent,
        }
    }
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
    on_toggle_group: Option<GroupCallback>,
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
                    list = list.children(
                        group
                            .properties
                            .iter()
                            .map(|property| self::detail_row(t, property)),
                    );
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
    on_toggle: Option<GroupCallback>,
) -> Stateful<Div> {
    let expandable = !group.properties.is_empty();

    let chevron = div().flex_none().size(px(12.)).when(expandable, |d| {
        d.child(
            svg()
                .path(if expanded {
                    icon::CHEVRON_DOWN
                } else {
                    icon::CHEVRON_RIGHT
                })
                .size(px(12.))
                .text_color(theme.text_muted),
        )
    });

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
        .child(chevron)
        .child(name)
        .child(div().flex_1())
        .child(summary);

    if expandable && let Some(handler) = on_toggle {
        let id = group.id.clone();
        row = row
            .cursor_pointer()
            .hover(|s| s.bg(theme.bg_panel_alt))
            .on_click(move |_, window, cx| handler(id.clone(), window, cx));
    }

    row
}

fn detail_row(theme: &Theme, property: &PpuProperty) -> Div {
    div()
        .flex()
        .items_center()
        .gap_2()
        .pl(px(24.))
        .pr_1()
        .py(px(2.))
        .child(label_text(theme, property.label.clone()))
        .child(div().flex_1())
        .child(self::value_text(theme, property))
}

use std::collections::HashSet;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{App, ClickEvent, Div, SharedString, Stateful, Window, div, px};

use crate::assets::icon;
use crate::components::{
    ClickHandler, Panel, RowCallback, badge, chevron_slot, mono_text, row_toggle,
};
use crate::theme::Theme;

#[derive(Clone)]
pub struct DmaChannelDetail {
    pub label: SharedString,
    pub value: SharedString,
}

#[derive(Clone)]
pub struct DmaChannelView {
    pub id: SharedString,
    pub name: SharedString,
    pub dma_enabled: bool,
    pub hdma_enabled: bool,
    pub source: SharedString,
    pub dest: SharedString,
    pub count: SharedString,
    pub details: Vec<DmaChannelDetail>,
}

#[derive(Clone, Default)]
pub struct DmaView {
    pub channels: Vec<DmaChannelView>,
}

impl From<debugger_model::DmaChannelDetail> for DmaChannelDetail {
    fn from(detail: debugger_model::DmaChannelDetail) -> Self {
        Self {
            label: detail.label.into(),
            value: detail.value.into(),
        }
    }
}

impl From<debugger_model::DmaChannelView> for DmaChannelView {
    fn from(channel: debugger_model::DmaChannelView) -> Self {
        Self {
            id: channel.id.into(),
            name: channel.name.into(),
            dma_enabled: channel.dma_enabled,
            hdma_enabled: channel.hdma_enabled,
            source: channel.source.into(),
            dest: channel.dest.into(),
            count: channel.count.into(),
            details: channel.details.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<debugger_model::DmaView> for DmaView {
    fn from(view: debugger_model::DmaView) -> Self {
        Self {
            channels: view.channels.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(IntoElement)]
pub struct DmaWidget {
    view: DmaView,
    expanded: HashSet<SharedString>,
    collapsed: bool,
    on_toggle: Option<ClickHandler>,
    on_toggle_channel: Option<RowCallback>,
}

impl DmaWidget {
    pub const ID: &'static str = "dma";

    pub fn new(view: DmaView) -> Self {
        Self {
            view,
            expanded: HashSet::new(),
            collapsed: false,
            on_toggle: None,
            on_toggle_channel: None,
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

    pub fn on_toggle_channel(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle_channel = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for DmaWidget {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;

        let active = self
            .view
            .channels
            .iter()
            .filter(|c| c.dma_enabled || c.hdma_enabled)
            .count();

        let mut list = div()
            .id("dma-list")
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll();

        if self.view.channels.is_empty() {
            list = list.items_center().justify_center().child(
                div()
                    .px_1()
                    .py_1()
                    .text_xs()
                    .text_color(t.text_dim)
                    .child("No channels"),
            );
        } else {
            for channel in &self.view.channels {
                let is_expanded = self.expanded.contains(&channel.id);
                list = list.child(self::channel_row(
                    t,
                    channel,
                    is_expanded,
                    self.on_toggle_channel.clone(),
                ));
                if is_expanded {
                    list = list.children(
                        channel
                            .details
                            .iter()
                            .map(|detail| self::detail_row(t, detail)),
                    );
                }
            }
        }

        Panel::new(Self::ID, "DMA")
            .glyph(icon::GRAPH)
            .grow(true)
            .collapsed(self.collapsed)
            .meta(format!("{active} active"))
            .on_toggle_opt(self.on_toggle)
            .child(list)
    }
}

fn channel_row(
    theme: &Theme,
    channel: &DmaChannelView,
    expanded: bool,
    on_toggle: Option<RowCallback>,
) -> Stateful<Div> {
    let expandable = !channel.details.is_empty();
    let chevron = chevron_slot(theme, expandable, expanded);

    let enabled = channel.dma_enabled || channel.hdma_enabled;
    let name = badge(
        channel.name.clone(),
        if enabled {
            theme.accent
        } else {
            theme.text_muted
        },
    );

    let transfer = div()
        .font_family(theme.mono_font)
        .text_xs()
        .text_color(theme.text_secondary)
        .child(format!("{} \u{2192} {}", channel.source, channel.dest));

    let count = div()
        .font_family(theme.mono_font)
        .text_xs()
        .text_color(theme.text_dim)
        .child(channel.count.clone());

    let mut row = div()
        .id(channel.id.clone())
        .flex()
        .items_center()
        .gap_2()
        .px_1()
        .py(px(3.))
        .child(chevron)
        .child(name)
        .child(transfer)
        .child(div().flex_1())
        .child(count);

    if channel.hdma_enabled {
        row = row.child(badge("HDMA", theme.accent));
    }

    if expandable && let Some(handler) = on_toggle {
        row = row_toggle(row, theme, channel.id.clone(), handler);
    }

    row
}

fn detail_row(theme: &Theme, detail: &DmaChannelDetail) -> Div {
    crate::components::detail_row(
        theme,
        detail.label.clone(),
        mono_text(theme, detail.value.clone()),
    )
}

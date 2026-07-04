use std::rc::Rc;

use gpui::prelude::*;
use gpui::{App, SharedString, Window, div, px, svg};

use crate::theme::Theme;

type SelectHandler = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;

struct Segment {
    key: SharedString,
    icon: SharedString,
    label: SharedString,
}

#[derive(IntoElement)]
pub struct Segmented {
    id: SharedString,
    options: Vec<Segment>,
    selected: SharedString,
    on_select: Option<SelectHandler>,
}

impl Segmented {
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            options: Vec::new(),
            selected: SharedString::default(),
            on_select: None,
        }
    }

    pub fn option(
        mut self,
        key: impl Into<SharedString>,
        icon: impl Into<SharedString>,
        label: impl Into<SharedString>,
    ) -> Self {
        self.options.push(Segment {
            key: key.into(),
            icon: icon.into(),
            label: label.into(),
        });
        self
    }

    pub fn option_label(
        self,
        key: impl Into<SharedString>,
        label: impl Into<SharedString>,
    ) -> Self {
        self.option(key, "", label)
    }

    pub fn selected(mut self, key: impl Into<SharedString>) -> Self {
        self.selected = key.into();
        self
    }

    pub fn on_select(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Segmented {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let theme = &*theme;

        let mut row = div()
            .flex()
            .items_center()
            .gap(px(2.))
            .p(px(2.))
            .rounded_md()
            .bg(theme.bg_panel_alt)
            .border_1()
            .border_color(theme.border_subtle);

        for option in self.options {
            let active = option.key == self.selected;

            let icon_color = if active {
                theme.on_accent
            } else {
                theme.text_secondary
            };

            let mut button = div()
                .id(SharedString::from(format!("{}-{}", self.id, option.key)))
                .flex()
                .items_center()
                .gap_1()
                .px_2()
                .py(px(1.))
                .rounded_sm()
                .cursor_pointer()
                .text_xs()
                .when(!option.icon.is_empty(), |b| {
                    b.child(svg().path(option.icon).size(px(12.)).text_color(icon_color))
                })
                .child(option.label);

            if active {
                button = button.bg(theme.accent).text_color(theme.on_accent);
            } else {
                let hover = theme.bg_header;
                button = button
                    .text_color(theme.text_secondary)
                    .hover(move |s| s.bg(hover));
            }

            if let Some(handler) = self.on_select.clone() {
                let key = option.key.clone();

                button = button.on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    handler(key.as_ref(), window, cx);
                });
            }

            row = row.child(button);
        }

        row
    }
}

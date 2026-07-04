use gpui::prelude::*;
use gpui::{App, ClickEvent, FontWeight, SharedString, Window, div, px, svg};

use super::ClickHandler;
use crate::assets::icon;
use crate::theme::Theme;

const HEADER_HEIGHT: f32 = 36.;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Sizing {
    #[default]
    Fixed,
    Grow,
    Scroll,
}

#[derive(IntoElement)]
pub struct Panel {
    id: SharedString,
    title: SharedString,
    meta: Option<SharedString>,
    glyph: Option<SharedString>,
    collapsed: bool,
    sizing: Sizing,
    on_toggle: Option<ClickHandler>,
    header_action: Option<gpui::AnyElement>,
    body: Option<gpui::AnyElement>,
}

impl Panel {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            meta: None,
            glyph: None,
            collapsed: false,
            sizing: Sizing::Fixed,
            on_toggle: None,
            header_action: None,
            body: None,
        }
    }

    pub fn meta(mut self, meta: impl Into<SharedString>) -> Self {
        self.meta = Some(meta.into());
        self
    }

    pub fn glyph(mut self, icon_path: impl Into<SharedString>) -> Self {
        self.glyph = Some(icon_path.into());
        self
    }

    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    pub fn grow(mut self, grow: bool) -> Self {
        self.sizing = if grow { Sizing::Grow } else { Sizing::Fixed };
        self
    }

    pub fn scrollable(mut self, scrollable: bool) -> Self {
        self.sizing = if scrollable {
            Sizing::Scroll
        } else {
            Sizing::Fixed
        };
        self
    }

    pub fn on_toggle(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Box::new(handler));
        self
    }

    pub fn on_toggle_opt(mut self, handler: Option<ClickHandler>) -> Self {
        self.on_toggle = handler;
        self
    }

    pub fn header_action(mut self, action: impl IntoElement) -> Self {
        self.header_action = Some(action.into_any_element());
        self
    }

    pub fn child(mut self, body: impl IntoElement) -> Self {
        self.body = Some(body.into_any_element());
        self
    }
}

impl RenderOnce for Panel {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;
        let chevron = if self.collapsed {
            icon::CHEVRON_RIGHT
        } else {
            icon::CHEVRON_DOWN
        };

        let mut left = div().flex().items_center().gap_2();
        if let Some(glyph) = self.glyph.clone() {
            left = left.child(svg().path(glyph).size(px(12.)).text_color(t.text_muted));
        }

        left = left.child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.text_secondary)
                .child(self.title.to_uppercase()),
        );

        let mut right = div().flex().items_center().gap_2();
        if let Some(action) = self.header_action {
            right = right.child(action);
        }
        if let Some(meta) = self.meta.clone() {
            right = right.child(
                div()
                    .font_family(t.mono_font)
                    .text_xs()
                    .text_color(t.text_muted)
                    .child(meta),
            );
        }
        right = right.child(svg().path(chevron).size(px(12.)).text_color(t.text_muted));

        let hover_bg = t.bg_panel_alt;
        let mut header = div()
            .id(self.id.clone())
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .h(px(HEADER_HEIGHT))
            .px_3()
            .bg(t.bg_header)
            .border_b_1()
            .border_color(t.border_subtle)
            .hover(move |s| s.bg(hover_bg))
            .child(left)
            .child(right);
        if let Some(toggle) = self.on_toggle {
            header = header.on_click(toggle);
        }

        let fill = self.sizing == Sizing::Grow && !self.collapsed;
        let bounded = self.sizing != Sizing::Fixed && !self.collapsed;

        let mut card = div()
            .flex()
            .flex_col()
            .bg(t.bg_panel)
            .border_1()
            .border_color(t.border_subtle)
            .overflow_hidden()
            .when(fill, |c| c.flex_1())
            .when(bounded, |c| c.min_h(px(0.)))
            .child(header);

        if !self.collapsed
            && let Some(body) = self.body
        {
            card = card.child(
                div()
                    .flex()
                    .flex_col()
                    .p_2()
                    .when(bounded, |b| b.flex_1().min_h(px(0.)))
                    .child(body),
            );
        }

        card
    }
}

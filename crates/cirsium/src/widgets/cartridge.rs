use std::sync::Arc;

use gpui::prelude::*;
use gpui::{App, ClickEvent, Div, FontWeight, ObjectFit, SharedString, Window, div, img, px, svg};

use crate::assets::icon;
use crate::components::{ClickHandler, Copyable, Panel, label_text};
use crate::theme::Theme;

/// One labeled cartridge field (e.g. "Mapper" / "LoROM").
pub type CartridgeField = super::field::Field;

#[derive(Clone)]
pub struct CartridgeView {
    pub title: SharedString,
    pub fields: Vec<CartridgeField>,
    pub cover: Option<SharedString>,
}

impl From<debugger_model::CartridgeView> for CartridgeView {
    fn from(view: debugger_model::CartridgeView) -> Self {
        Self {
            title: view.title.into(),
            fields: view.fields.into_iter().map(Into::into).collect(),
            cover: view.cover.map(Into::into),
        }
    }
}

#[derive(IntoElement)]
pub struct CartridgeWidget {
    view: Arc<CartridgeView>,
    collapsed: bool,
    on_toggle: Option<ClickHandler>,
}

impl CartridgeWidget {
    pub const ID: &'static str = "cartridge";

    pub fn new(view: Arc<CartridgeView>) -> Self {
        Self {
            view,
            collapsed: false,
            on_toggle: None,
        }
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
}

impl RenderOnce for CartridgeWidget {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;

        let title = self.view.title.clone();
        let title_block = Copyable::new("cart-title", title.clone()).child(
            div()
                .font_family(t.mono_font)
                .text_color(t.text_primary)
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        );

        let cover = self.view.cover.clone();

        let art_box = div()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .size(px(56.))
            .bg(t.bg_panel_alt)
            .border_1()
            .border_color(t.border_subtle)
            .rounded_md()
            .overflow_hidden();
        let art_box = match cover {
            Some(url) => art_box.child(img(url).size_full().object_fit(ObjectFit::Contain)),
            None => art_box.child(
                svg()
                    .path(icon::CARTRIDGE)
                    .size(px(26.))
                    .text_color(t.accent),
            ),
        };

        let header_row = div()
            .flex()
            .items_center()
            .gap_2()
            .pb_2()
            .border_b_1()
            .border_color(t.border_subtle)
            .child(art_box)
            .child(title_block);

        let fields = div()
            .flex()
            .flex_col()
            .gap_1()
            .pt_1()
            .children(self.view.fields.iter().map(|f| self::field_row(t, f)));

        let body = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(header_row)
            .child(fields);

        Panel::new(Self::ID, "Cartridge")
            .glyph(icon::CARTRIDGE)
            .collapsed(self.collapsed)
            .on_toggle_opt(self.on_toggle)
            .child(body)
    }
}

fn field_row(theme: &Theme, f: &CartridgeField) -> Div {
    let value_color = if f.accent {
        theme.accent
    } else {
        theme.text_secondary
    };

    div()
        .flex()
        .items_center()
        .justify_between()
        .px_1()
        .child(label_text(theme, f.label.clone()))
        .child(
            Copyable::new(format!("cart-{}", f.label), f.value.clone()).child(
                div()
                    .font_family(theme.mono_font)
                    .text_xs()
                    .text_color(value_color)
                    .child(f.value.clone()),
            ),
        )
}

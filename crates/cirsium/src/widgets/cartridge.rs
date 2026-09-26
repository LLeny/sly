use std::sync::Arc;

use gpui::prelude::*;
use gpui::{App, ClickEvent, FontWeight, ObjectFit, Window, div, img, px, svg};

use crate::assets::icon;
use crate::components::{ClickHandler, Copyable, Panel};
use crate::theme::Theme;

pub use debugger_model::CartridgeView;

#[derive(IntoElement)]
pub struct CartridgeWidget {
    view: Arc<CartridgeView>,
    collapsed: bool,
    on_toggle: Option<ClickHandler>,
}

impl CartridgeWidget {
    pub const ID: &'static str = debugger_model::panel::CARTRIDGE;

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

        let title = gpui::SharedString::from(self.view.title.clone());
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
            Some(url) => art_box.child(
                img(gpui::SharedString::from(url))
                    .size_full()
                    .object_fit(ObjectFit::Contain),
            ),
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

        let fields = super::structured::content(t, &self.view.details, &Default::default(), &None);

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

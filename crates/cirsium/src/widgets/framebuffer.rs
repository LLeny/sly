use std::sync::Arc;

use gpui::prelude::*;
use gpui::{App, ClickEvent, ImageFiltering, ObjectFit, RenderImage, Window, div, img, px};

use crate::assets::icon;
use crate::components::{ClickHandler, Panel, empty_state};
use crate::theme::Theme;

#[derive(Clone)]
pub struct FramebufferView {
    pub width: u32,
    pub height: u32,
    pub image: Option<Arc<RenderImage>>,
}

const SCALE: u32 = 1;

impl From<debugger_model::Framebuffer> for FramebufferView {
    fn from(fb: debugger_model::Framebuffer) -> Self {
        let len = fb.width as usize * fb.height as usize;
        let image = (len > 0 && fb.pixels.len() == len)
            .then(|| crate::widgets::grid::build_image(&fb.pixels, fb.width, fb.height, SCALE));

        Self {
            width: fb.width,
            height: fb.height,
            image,
        }
    }
}

#[derive(IntoElement)]
pub struct FramebufferWidget {
    view: Option<FramebufferView>,
    collapsed: bool,
    on_toggle: Option<ClickHandler>,
}

impl FramebufferWidget {
    pub const ID: &'static str = "framebuffer";

    pub fn new(view: Option<FramebufferView>) -> Self {
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

impl RenderOnce for FramebufferWidget {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;

        let body = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .items_center()
            .justify_center()
            .overflow_hidden();
        let body = match self.view.as_ref().and_then(|view| view.image.clone()) {
            Some(image) => body.child(
                img(image)
                    .size_full()
                    .object_fit(ObjectFit::Contain)
                    .image_filtering(ImageFiltering::Nearest),
            ),
            None => body.child(empty_state(t, "No frame")),
        };

        let mut panel = Panel::new(Self::ID, "Screen")
            .glyph(icon::MONITOR)
            .grow(true)
            .collapsed(self.collapsed)
            .on_toggle_opt(self.on_toggle);
        if let Some(view) = &self.view {
            panel = panel.meta(format!("{}×{}", view.width, view.height));
        }

        panel.child(body)
    }
}

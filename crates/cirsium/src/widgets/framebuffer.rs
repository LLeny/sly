use std::sync::Arc;

use gpui::prelude::*;
use gpui::{
    App, Bounds, ClickEvent, Corners, ImageFiltering, Pixels, RenderImage, Window, canvas, div,
    point, px, size,
};

use crate::assets::icon;
use crate::components::{ClickHandler, Panel, empty_state};
use crate::theme::Theme;

#[derive(Clone)]
pub struct FramebufferView {
    pub width: u32,
    pub height: u32,
    pub image: Option<Arc<RenderImage>>,
}

impl From<core_model::SoftwareFrame<'_>> for FramebufferView {
    fn from(fb: core_model::SoftwareFrame<'_>) -> Self {
        let image = fb
            .is_valid()
            .then(|| crate::widgets::grid::build_image(fb.pixels, fb.width, fb.height));

        Self {
            width: fb.width,
            height: fb.height,
            image,
        }
    }
}

impl FramebufferView {
    pub fn matches(&self, fb: &core_model::SoftwareFrame<'_>) -> bool {
        self.image.is_some() && self.width == fb.width && self.height == fb.height && fb.is_valid()
    }

    pub fn rebuild_from_staged(&mut self, staged: &[u8]) -> Option<Arc<RenderImage>> {
        let len = self.width as usize * self.height as usize * 4;
        if len == 0 || staged.len() != len {
            return None;
        }

        let buffer = image::RgbaImage::from_raw(self.width, self.height, staged.to_vec())?;
        self.image
            .replace(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
    }
}

pub fn stage_bgra(fb: &core_model::SoftwareFrame<'_>, out: &mut Vec<u8>) {
    out.resize(fb.pixels.len() * 4, 0);
    for (dst, &px) in out.chunks_exact_mut(4).zip(fb.pixels) {
        dst.copy_from_slice(&crate::widgets::grid::bgra(px));
    }
}

fn fitted_bounds(
    bounds: Bounds<Pixels>,
    image_width: u32,
    image_height: u32,
    display_scale: f32,
) -> Bounds<Pixels> {
    let image_width = image_width as f32;
    let image_height = image_height as f32;
    let fit = (f32::from(bounds.size.width) * display_scale / image_width)
        .min(f32::from(bounds.size.height) * display_scale / image_height);
    let scale = if fit >= 1.0 { fit.floor() } else { fit };
    let width = px(image_width * scale / display_scale);
    let height = px(image_height * scale / display_scale);

    Bounds {
        origin: point(
            bounds.origin.x + (bounds.size.width - width) / 2.0,
            bounds.origin.y + (bounds.size.height - height) / 2.0,
        ),
        size: size(width, height),
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
        let body = match self.view.as_ref().and_then(|view| {
            view.image
                .clone()
                .map(|image| (image, view.width, view.height))
        }) {
            Some((image, width, height)) => body.child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let bounds = fitted_bounds(bounds, width, height, window.scale_factor());
                        window
                            .paint_image(
                                bounds,
                                Corners::all(px(0.)),
                                image.clone(),
                                0,
                                false,
                                ImageFiltering::Nearest,
                            )
                            .ok();
                    },
                )
                .size_full(),
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

use crate::assets::icon;
use crate::components::{footer_row, mono_text};
use crate::widgets::grid::{GridContent, GridPanel, build_image};
use gpui::RenderImage;
use gpui::prelude::*;
use std::sync::Arc;

#[derive(Clone)]
pub struct RasterView {
    pub data: debugger_model::RasterImage,
    pub image: Option<Arc<RenderImage>>,
}

impl From<debugger_model::RasterImage> for RasterView {
    fn from(data: debugger_model::RasterImage) -> Self {
        let image = data
            .is_valid()
            .then(|| build_image(&data.pixels, data.width, data.height));
        Self { data, image }
    }
}

pub struct RasterWidget;

impl RasterWidget {
    pub const ID: &'static str = debugger_model::panel::RASTER;

    pub fn panel(view: Option<RasterView>) -> GridPanel {
        let content = view
            .as_ref()
            .filter(|v| v.image.is_some())
            .map(|v| GridContent {
                image: v.image.clone(),
                cols: v.data.width as usize,
                rows: v.data.height as usize,
                tile_px: 1,
                count: v.data.pixels.len(),
                empty_message: "No raster",
                overlay: None,
                footprint: None,
                window_overlays: vec![],
            });

        let mut panel = GridPanel::new(Self::ID, "Raster", icon::MONITOR, content)
            .placeholder("Select a pixel to inspect");

        if let Some(view) = view {
            panel = panel
                .meta(format!("{}x{}", view.data.width, view.data.height))
                .detail_rows(view.data.details.len() + 1)
                .detail(move |t, index| {
                    let (x, y) = (
                        index % view.data.width as usize,
                        index / view.data.width as usize,
                    );

                    let [r, g, b, a] = view.data.pixels[index];
                    gpui::div()
                        .flex()
                        .flex_col()
                        .child(footer_row(
                            t,
                            format!("({x}, {y})"),
                            mono_text(t, format!("RGBA #{r:02X}{g:02X}{b:02X}{a:02X}")),
                        ))
                        .child(super::tileset::details(t, &view.data.details))
                });
        }

        panel
    }
}

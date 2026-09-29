use crate::assets::icon;
use crate::components::{footer_row, mono_text};
use crate::widgets::grid::{GridContent, GridPanel, build_image};
use debugger_model::hex;
use gpui::RenderImage;
use gpui::prelude::*;
use std::sync::Arc;

#[derive(Clone)]
pub struct BitmapView {
    pub data: debugger_model::BitmapImage,
    pub image: Option<Arc<RenderImage>>,
}

impl From<debugger_model::BitmapImage> for BitmapView {
    fn from(data: debugger_model::BitmapImage) -> Self {
        let image = data
            .is_valid()
            .then(|| build_image(&data.pixels, data.width, data.height));
        Self { data, image }
    }
}

pub struct BitmapWidget;

impl BitmapWidget {
    pub const ID: &'static str = debugger_model::panel::BITMAP;

    pub fn panel(view: Option<BitmapView>) -> GridPanel {
        let content = view
            .as_ref()
            .filter(|v| v.image.is_some())
            .map(|v| GridContent {
                image: v.image.clone(),
                cols: v.data.width as usize,
                rows: v.data.height as usize,
                tile_px: 1,
                count: v.data.pixels.len(),
                empty_message: "No bitmap",
                overlay: None,
                footprint: None,
                window_overlays: vec![],
            });

        let mut panel = GridPanel::new(Self::ID, "Bitmap", icon::MONITOR, content)
            .placeholder("Select a pixel to inspect");

        if let Some(view) = view {
            let data = &view.data;
            let rows = 2 + usize::from(data.indices.is_some()) + data.details.len();

            panel = panel
                .meta(format!("{}x{}", data.width, data.height))
                .detail_rows(rows)
                .detail(move |t, index| {
                    let data = &view.data;
                    let (x, y) = (index % data.width as usize, index / data.width as usize);
                    let address = data
                        .pixel_address(index)
                        .map(|address| format!("{:08X}", address.addr))
                        .unwrap_or_default();
                    let [r, g, b, _] = data.pixels[index];

                    gpui::div()
                        .flex()
                        .flex_col()
                        .child(footer_row(t, format!("({x}, {y})"), mono_text(t, address)))
                        .children(data.indices.as_ref().and_then(|indices| {
                            let index = u32::from(*indices.get(index)?);
                            Some(footer_row(t, "Palette index", mono_text(t, hex(index, 8))))
                        }))
                        .child(footer_row(
                            t,
                            "Color",
                            mono_text(t, format!("#{r:02X}{g:02X}{b:02X}")),
                        ))
                        .child(super::tileset::details(t, &data.details))
                });
        }

        panel
    }
}

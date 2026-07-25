use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{
    App, ImageFiltering, MouseButton, MouseDownEvent, ObjectFit, RenderImage, SharedString, Window,
    div, img, px, svg,
};

use crate::assets::icon;
use crate::components::{ClickHandler, Panel, empty_state};
use crate::theme::Theme;

#[derive(Clone)]
pub struct SlotThumbnail {
    pub image: Arc<RenderImage>,
    pub width: u32,
    pub height: u32,
}

impl SlotThumbnail {
    fn aspect(&self) -> f32 {
        if self.height == 0 {
            1.0
        } else {
            self.width as f32 / self.height as f32
        }
    }
}

#[derive(Clone)]
pub struct SavestateSlot {
    pub id: SharedString,
    pub label: SharedString,
    pub detail: SharedString,
    pub occupied: bool,
    pub thumbnail: Option<SlotThumbnail>,
}

#[derive(Clone, Default)]
pub struct SavestatesView {
    pub slots: Vec<SavestateSlot>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlotAction {
    Select,
    Save,
    Load,
    Delete,
}

type SlotHandler = Rc<dyn Fn(SlotAction, SharedString, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct SavestatesWidget {
    view: SavestatesView,
    collapsed: bool,
    on_toggle: Option<ClickHandler>,
    selected: Option<SharedString>,
    on_action: Option<SlotHandler>,
}

impl SavestatesWidget {
    pub const ID: &'static str = "savestates";

    pub fn new(view: SavestatesView) -> Self {
        Self {
            view,
            collapsed: false,
            on_toggle: None,
            selected: None,
            on_action: None,
        }
    }

    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    pub fn on_toggle(
        mut self,
        handler: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Box::new(handler));
        self
    }

    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }

    pub fn on_action(
        mut self,
        handler: impl Fn(SlotAction, SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for SavestatesWidget {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;

        let body = if self.view.slots.is_empty() {
            div()
                .child(empty_state(t, "No save slots"))
                .into_any_element()
        } else {
            let on_action = self.on_action;
            let selected = self.selected;

            div()
                .id("savestate-list")
                .flex()
                .flex_col()
                .gap_1()
                .max_h(px(LIST_MAX_HEIGHT))
                .overflow_y_scroll()
                .children(self.view.slots.into_iter().map(|slot| {
                    let is_selected = selected.as_ref() == Some(&slot.id);
                    row(t, slot, is_selected, on_action.clone())
                }))
                .into_any_element()
        };

        Panel::new(Self::ID, "Save States")
            .glyph(icon::BOOKMARK)
            .collapsed(self.collapsed)
            .on_toggle_opt(self.on_toggle)
            .child(body)
    }
}

pub fn thumbnail_image(width: u32, height: u32, rgb: &[u8]) -> Option<SlotThumbnail> {
    if width == 0 || height == 0 || rgb.len() != width as usize * height as usize * 3 {
        return None;
    }

    let mut rgba = image::RgbaImage::new(width, height);
    for (pixel, chunk) in rgba.pixels_mut().zip(rgb.as_chunks::<3>().0) {
        *pixel = image::Rgba([chunk[2], chunk[1], chunk[0], 0xFF]);
    }

    Some(SlotThumbnail {
        image: Arc::new(RenderImage::new(vec![image::Frame::new(rgba)])),
        width,
        height,
    })
}

const THUMB_HEIGHT: f32 = 56.;
const PLACEHOLDER_ASPECT: f32 = 4. / 3.;
const LIST_MAX_HEIGHT: f32 = 210.;

fn row(
    t: &Theme,
    slot: SavestateSlot,
    selected: bool,
    on_action: Option<SlotHandler>,
) -> impl IntoElement {
    let id = slot.id.clone();

    let aspect = slot
        .thumbnail
        .as_ref()
        .map_or(PLACEHOLDER_ASPECT, SlotThumbnail::aspect);
    let thumb_width = (THUMB_HEIGHT * aspect).round();

    let thumb = div()
        .flex_none()
        .w(px(thumb_width))
        .h(px(THUMB_HEIGHT))
        .rounded_sm()
        .border_1()
        .border_color(t.border_subtle)
        .bg(t.bg_panel_alt)
        .flex()
        .items_center()
        .justify_center()
        .child(match slot.thumbnail {
            Some(thumb) => img(thumb.image)
                .w(px(thumb_width - 2.))
                .h(px(THUMB_HEIGHT - 2.))
                .object_fit(ObjectFit::Contain)
                .image_filtering(ImageFiltering::Nearest)
                .into_any_element(),
            None => svg()
                .path(icon::BOOKMARK)
                .size(px(16.))
                .text_color(t.text_dim)
                .into_any_element(),
        });

    let label_color = if slot.occupied {
        t.text_primary
    } else {
        t.text_dim
    };

    let text = div()
        .flex()
        .flex_col()
        .gap_0()
        .flex_grow(1.0)
        .child(
            div()
                .font_family(t.ui_font)
                .text_size(px(11.))
                .text_color(label_color)
                .child(slot.label),
        )
        .child(
            div()
                .font_family(t.ui_font)
                .text_size(px(10.))
                .text_color(t.text_dim)
                .child(slot.detail),
        );

    let actions = div()
        .flex()
        .items_center()
        .gap_1()
        .child(action(
            t,
            &id,
            SlotAction::Save,
            icon::SAVE,
            true,
            on_action.clone(),
        ))
        .child(action(
            t,
            &id,
            SlotAction::Load,
            icon::RESTORE,
            slot.occupied,
            on_action.clone(),
        ))
        .child(action(
            t,
            &id,
            SlotAction::Delete,
            icon::TRASH,
            slot.occupied,
            on_action.clone(),
        ));

    let (bg, border) = if selected {
        (t.bg_header, t.accent)
    } else {
        (t.bg_panel_alt, t.border_subtle)
    };

    let mut container = div()
        .id(SharedString::from(format!("slot-{id}")))
        .flex()
        .flex_none()
        .items_center()
        .gap_2()
        .p_1()
        .rounded_md()
        .border_1()
        .border_color(border)
        .bg(bg)
        .child(thumb)
        .child(text)
        .child(actions);

    if let Some(handler) = on_action {
        container = container.cursor_pointer().on_mouse_down(
            MouseButton::Left,
            move |_: &MouseDownEvent, window, cx| {
                handler(SlotAction::Select, id.clone(), window, cx)
            },
        );
    }

    container
}

fn action(
    t: &Theme,
    slot: &SharedString,
    kind: SlotAction,
    icon_path: &'static str,
    enabled: bool,
    on_action: Option<SlotHandler>,
) -> impl IntoElement {
    let live = enabled && on_action.is_some();
    let color = if live { t.text_secondary } else { t.text_dim };

    let mut button = div()
        .id(SharedString::from(format!("slot-{kind:?}-{slot}")))
        .flex()
        .items_center()
        .justify_center()
        .size(px(20.))
        .rounded_sm()
        .border_1()
        .border_color(t.border_subtle)
        .bg(t.bg_panel)
        .child(svg().path(icon_path).size(px(11.)).text_color(color));

    if live && let Some(handler) = on_action {
        let hover = t.bg_header;
        let slot = slot.clone();
        button = button
            .hover(move |s| s.bg(hover))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |_: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                handler(kind, slot.clone(), window, cx)
            });
    }

    button
}

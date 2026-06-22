use std::time::Duration;

use gpui::{
    AnyElement, App, ClickEvent, ClipboardItem, Div, Entity, SharedString, Window, div, prelude::*,
    px, svg,
};

use crate::assets::icon;
use crate::theme::Theme;

const COPY_FLASH_MS: u64 = 1000;

fn copy_handler(
    flashed: Entity<bool>,
    payload: SharedString,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    move |_, window, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string(payload.to_string()));

        flashed.update(cx, |f, cx| {
            *f = true;
            cx.notify();
        });

        let flashed = flashed.clone();
        window
            .spawn(cx, async move |cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(COPY_FLASH_MS))
                    .await;
                cx.update(|_, cx| {
                    flashed.update(cx, |f, cx| {
                        *f = false;
                        cx.notify();
                    });
                })
                .ok();
            })
            .detach();
    }
}

fn copied_indicator(theme: &Theme) -> Div {
    div()
        .flex()
        .items_center()
        .gap_1()
        .text_color(theme.accent)
        .child(
            svg()
                .path(icon::CHECK)
                .size(px(12.))
                .text_color(theme.accent),
        )
        .child(div().text_xs().child("Copied"))
}

#[derive(Clone, Copy, PartialEq)]
enum CopyStyle {
    Inline,
    Cell,
}

#[derive(IntoElement)]
pub struct Copyable {
    id: SharedString,
    payload: SharedString,
    child: Option<AnyElement>,
    style: CopyStyle,
}

impl Copyable {
    pub fn new(id: impl Into<SharedString>, payload: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            payload: payload.into(),
            child: None,
            style: CopyStyle::Inline,
        }
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.child = Some(child.into_any_element());
        self
    }

    pub fn cell(mut self) -> Self {
        self.style = CopyStyle::Cell;
        self
    }
}

impl RenderOnce for Copyable {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;
        let flashed = window.use_keyed_state(self.id.clone(), cx, |_, _| false);
        let is_flashed = *flashed.read(cx);
        let accent_soft = t.accent_soft;

        let base = div().id(self.id).cursor_pointer();
        let styled = match self.style {
            CopyStyle::Inline => base
                .px_1p5()
                .py_0p5()
                .rounded_md()
                .when(is_flashed, |d| d.bg(accent_soft))
                .hover(move |s| s.bg(accent_soft)),
            CopyStyle::Cell => {
                let accent = t.accent;
                base.flex_1()
                    .min_w(px(0.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .px_2()
                    .py_1()
                    .bg(t.bg_panel_alt)
                    .border_1()
                    .border_color(t.border_subtle)
                    .rounded_sm()
                    .when(is_flashed, |d| d.bg(accent_soft).border_color(accent))
                    .hover(move |s| s.bg(accent_soft).border_color(accent))
            }
        };

        styled
            .on_click(copy_handler(flashed, self.payload))
            .map(|d| {
                if is_flashed {
                    d.child(copied_indicator(t))
                } else {
                    d.children(self.child)
                }
            })
    }
}

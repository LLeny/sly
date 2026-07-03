use gpui::prelude::*;
use gpui::{App, ClickEvent, SharedString, Window, div, px};

use crate::assets::icon;
use crate::components::{ClickHandler, Copyable, Panel};
use crate::theme::Theme;

#[derive(Clone)]
pub struct StackFrameView {
    pub kind: SharedString,
    pub location: SharedString,
    pub symbol: SharedString,
    pub caller: SharedString,
}

#[derive(Clone, Default)]
pub struct CallStackView {
    pub frames: Vec<StackFrameView>,
}

impl From<debugger_model::StackFrame> for StackFrameView {
    fn from(frame: debugger_model::StackFrame) -> Self {
        Self {
            kind: SharedString::from(frame.kind.as_ref()),
            location: SharedString::from(frame.location.as_ref()),
            symbol: SharedString::from(frame.symbol.as_ref()),
            caller: SharedString::from(frame.caller.as_ref()),
        }
    }
}

impl From<debugger_model::CallStackView> for CallStackView {
    fn from(view: debugger_model::CallStackView) -> Self {
        Self {
            frames: view.frames.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(IntoElement)]
pub struct CallStackWidget {
    view: CallStackView,
    id: SharedString,
    title: SharedString,
    collapsed: bool,
    scrollable: bool,
    on_toggle: Option<ClickHandler>,
}

impl CallStackWidget {
    pub const ID: &'static str = "callstack";

    pub fn new(view: CallStackView) -> Self {
        Self {
            view,
            id: Self::ID.into(),
            title: "Call Stack".into(),
            collapsed: false,
            scrollable: false,
            on_toggle: None,
        }
    }

    pub fn id(mut self, id: impl Into<SharedString>) -> Self {
        self.id = id.into();
        self
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    pub fn scrollable(mut self, scrollable: bool) -> Self {
        self.scrollable = scrollable;
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

impl RenderOnce for CallStackWidget {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;

        // Depth counts only the real call frames, not the synthetic PC row.
        let depth = self.view.frames.len().saturating_sub(1);

        let mut list = div()
            .id(SharedString::from(format!("{}-list", self.id)))
            .flex()
            .flex_col()
            .when(self.scrollable, |d| {
                d.flex_1().min_h(px(0.)).overflow_y_scroll()
            });

        if self.view.frames.is_empty() {
            list = list
                .flex_1()
                .min_h(px(0.))
                .items_center()
                .justify_center()
                .child(
                    div()
                        .px_1()
                        .py_1()
                        .text_xs()
                        .text_color(t.text_dim)
                        .child("Empty call stack"),
                );
        } else {
            list = list.children(
                self.view
                    .frames
                    .into_iter()
                    .enumerate()
                    .map(|(ix, frame)| self::frame_row(t, &self.id, ix, frame)),
            );
        }

        Panel::new(self.id.clone(), self.title.clone())
            .glyph(icon::LIST)
            .grow(self.scrollable)
            .collapsed(self.collapsed)
            .meta(format!("{depth}"))
            .on_toggle_opt(self.on_toggle)
            .child(list)
    }
}

fn frame_row(theme: &Theme, widget_id: &str, ix: usize, frame: StackFrameView) -> impl IntoElement {
    let StackFrameView {
        kind,
        location,
        symbol,
        caller,
    } = frame;

    let index = div()
        .flex_none()
        .w(px(16.))
        .flex()
        .justify_end()
        .font_family(theme.mono_font)
        .text_xs()
        .text_color(theme.text_dim)
        .child(format!("{ix}"));

    let address = Copyable::new(format!("{widget_id}-frame-{ix}"), location.clone()).child(
        div()
            .font_family(theme.mono_font)
            .text_xs()
            .text_color(theme.text_primary)
            .child(location),
    );

    let pill = div()
        .flex_none()
        .px(px(3.))
        .font_family(theme.mono_font)
        .text_size(px(9.))
        .text_color(theme.text_muted)
        .bg(theme.bg_panel_alt)
        .rounded_sm()
        .child(kind);

    let mut row = div()
        .flex()
        .items_center()
        .gap_2()
        .px_1()
        .py(px(3.))
        .child(index)
        .child(address)
        .child(pill);

    if !symbol.is_empty() {
        row = row.child(div().text_xs().text_color(theme.text_muted).child(symbol));
    }

    row = row.child(div().flex_1());

    if !caller.is_empty() {
        row = row.child(
            div()
                .font_family(theme.mono_font)
                .text_xs()
                .text_color(theme.text_dim)
                .child(caller),
        );
    }

    row
}

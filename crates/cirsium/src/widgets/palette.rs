use std::rc::Rc;

use gpui::prelude::*;
use gpui::{App, ClickEvent, Div, Rgba, SharedString, Window, div, px, rgb};

use crate::assets::icon;
use crate::components::{ClickHandler, Panel, empty_state, footer_row, hex, mono_text};
use crate::theme::Theme;

type SelectCallback = Rc<dyn Fn(usize, usize, &mut Window, &mut App) + 'static>;

pub use debugger_model::PaletteColor;

#[derive(Clone, Default)]
pub struct PaletteView {
    pub palettes: Vec<Vec<PaletteColor>>,
    pub meta: SharedString,
}

impl From<debugger_model::PaletteView> for PaletteView {
    fn from(view: debugger_model::PaletteView) -> Self {
        Self {
            palettes: view.palettes,
            meta: view.meta.into(),
        }
    }
}

fn swatch_color(color: &PaletteColor) -> Rgba {
    let (r, g, b) = color.rgb;
    rgb(((r as u32) << 16) | ((g as u32) << 8) | b as u32)
}

#[derive(IntoElement)]
pub struct PaletteWidget {
    view: PaletteView,
    selected: Option<(usize, usize)>,
    collapsed: bool,
    on_toggle: Option<ClickHandler>,
    on_select: Option<SelectCallback>,
}

impl PaletteWidget {
    pub const ID: &'static str = "palette";

    pub fn new(view: PaletteView) -> Self {
        Self {
            view,
            selected: None,
            collapsed: false,
            on_toggle: None,
            on_select: None,
        }
    }

    pub fn selected(mut self, selected: Option<(usize, usize)>) -> Self {
        self.selected = selected;
        self
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

    pub fn on_select(
        mut self,
        handler: impl Fn(usize, usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for PaletteWidget {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;

        let active = self.selected.map(|(palette, _)| palette);

        let mut grid = div()
            .id("palette-grid")
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .gap(px(2.))
            .overflow_y_scroll();

        if self.view.palettes.is_empty() {
            grid = grid
                .items_center()
                .justify_center()
                .child(empty_state(t, "No palette colors available"));
        } else {
            for (palette_ix, palette) in self.view.palettes.iter().enumerate() {
                let mut cells = div()
                    .id(palette_ix)
                    .flex()
                    .gap(px(2.))
                    .p(px(1.))
                    .rounded_sm()
                    .border_1()
                    .border_color(if active == Some(palette_ix) {
                        t.accent
                    } else {
                        gpui::transparent_black()
                    });
                for (color_ix, color) in palette.iter().enumerate() {
                    cells = cells.child(self::swatch(
                        t,
                        palette_ix,
                        color_ix,
                        color,
                        self.selected == Some((palette_ix, color_ix)),
                        self.on_select.clone(),
                    ));
                }
                grid = grid.child(cells);
            }
        }

        let mut body = div().flex().flex_col().flex_1().min_h(px(0.)).child(grid);
        if let Some((palette_ix, color_ix)) = self.selected
            && let Some(color) = self
                .view
                .palettes
                .get(palette_ix)
                .and_then(|palette| palette.get(color_ix))
        {
            body = body.child(self::detail_row(t, palette_ix, color_ix, color));
        }

        let mut panel = Panel::new(Self::ID, "Palettes")
            .glyph(icon::PALETTE)
            .grow(true)
            .collapsed(self.collapsed)
            .on_toggle_opt(self.on_toggle);
        if !self.view.meta.is_empty() {
            panel = panel.meta(self.view.meta.clone());
        }
        panel.child(body)
    }
}

fn swatch(
    theme: &Theme,
    palette_ix: usize,
    color_ix: usize,
    color: &PaletteColor,
    selected: bool,
    on_select: Option<SelectCallback>,
) -> impl IntoElement {
    let mut cell = div()
        .id(color_ix)
        .flex_1()
        .h(px(18.))
        .bg(self::swatch_color(color))
        .border_1()
        .border_color(if selected {
            theme.accent
        } else {
            theme.border_subtle
        });

    if let Some(handler) = on_select {
        cell = cell
            .cursor_pointer()
            .hover(|s| s.border_color(theme.accent_hover))
            .on_click(move |_, window, cx| handler(palette_ix, color_ix, window, cx));
    }

    cell
}

fn detail_row(theme: &Theme, palette_ix: usize, color_ix: usize, color: &PaletteColor) -> Div {
    let (r, g, b) = color.rgb;

    footer_row(
        theme,
        format!("Palette {palette_ix}, color {color_ix}"),
        mono_text(
            theme,
            format!(
                "{} \u{2192} #{r:02X}{g:02X}{b:02X}",
                hex(color.raw as u32, 16)
            ),
        ),
    )
}

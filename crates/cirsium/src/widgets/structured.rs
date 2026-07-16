use std::sync::Arc;

use gpui::prelude::*;
use gpui::{App, ClickEvent, Div, FontWeight, Window, div, px};

use crate::components::{ClickHandler, Panel, label_text, meter_bar, mono_text, status_flag};
use crate::theme::Theme;

pub use debugger_model::{Field, FlagView, Row, Section, StructuredView, Tone};

#[derive(IntoElement)]
pub struct StructuredPanel {
    id: gpui::SharedString,
    title: gpui::SharedString,
    view: Arc<StructuredView>,
    glyph: Option<gpui::SharedString>,
    collapsed: bool,
    on_toggle: Option<ClickHandler>,
}

impl StructuredPanel {
    pub fn new(
        id: impl Into<gpui::SharedString>,
        title: impl Into<gpui::SharedString>,
        view: Arc<StructuredView>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            view,
            glyph: None,
            collapsed: false,
            on_toggle: None,
        }
    }

    pub fn glyph(mut self, glyph: impl Into<gpui::SharedString>) -> Self {
        self.glyph = Some(glyph.into());
        self
    }

    pub fn glyph_opt(mut self, glyph: Option<gpui::SharedString>) -> Self {
        self.glyph = glyph;
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
}

impl RenderOnce for StructuredPanel {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;
        let view = &*self.view;

        let mut blocks: Vec<Vec<&Section>> = Vec::new();
        for section in &view.sections {
            let merge = section.group != 0
                && blocks
                    .last()
                    .and_then(|b| b.first())
                    .is_some_and(|f| f.group == section.group);

            if merge {
                blocks.last_mut().unwrap().push(section);
            } else {
                blocks.push(vec![section]);
            }
        }

        let mut body = div().flex().flex_col();
        for block in blocks {
            if block.len() == 1 && block[0].group == 0 {
                body = body.child(section_column(t, block[0]));
            } else {
                let row =
                    block
                        .into_iter()
                        .fold(div().flex().items_start().gap_3(), |row, section| {
                            let col = section_column(t, section).min_w(px(0.)).overflow_hidden();
                            let col = match section.basis {
                                Some(fraction) => col.w(gpui::relative(fraction)).flex_none(),
                                None => col.flex_1(),
                            };
                            row.child(col)
                        });

                body = body.child(row);
            }
        }

        Panel::new(self.id, self.title)
            .collapsed(self.collapsed)
            .on_toggle_opt(self.on_toggle)
            .glyph_opt(self.glyph)
            .meta_opt(view.meta.clone().map(Into::into))
            .child(body)
    }
}

fn section_column(t: &Theme, section: &Section) -> Div {
    let mut col = div().flex().flex_col();
    if let Some(title) = &section.title {
        col = col.child(heading(t, title, section.leading));
    }

    section
        .rows
        .iter()
        .fold(col, |col, row| col.child(render_row(t, row)))
}

fn heading(t: &Theme, text: &str, leading: bool) -> Div {
    div()
        .px_1()
        .pt(px(if leading { 0. } else { 6. }))
        .pb(px(3.))
        .mb(px(3.))
        .border_b_1()
        .border_color(t.border_subtle)
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(t.text_muted)
        .whitespace_nowrap()
        .child(text.to_uppercase())
}

fn render_row(t: &Theme, row: &Row) -> Div {
    match row {
        Row::Field(field) => kv(t, field),
        Row::Flags { flags, note } => flag_row(t, flags, note),
        Row::BitHeader { bits } => bit_header(t, *bits),
        Row::BitGrid {
            label,
            bits,
            on_mask,
        } => bit_grid(t, label.clone(), *bits, *on_mask),
        Row::Meter {
            label,
            value,
            max,
            accent,
        } => meter(t, label.clone(), *value, *max, *accent),
        Row::Tags { tags } => tag_row(t, tags),
    }
}

fn kv(t: &Theme, field: &Field) -> Div {
    let value = mono_text(t, field.value.clone())
        .whitespace_nowrap()
        .when(field.accent, |d| d.text_color(t.accent));
    div()
        .flex()
        .items_center()
        .justify_between()
        .px_1()
        .h(px(16.))
        .gap_2()
        .child(label_text(t, field.label.clone()).whitespace_nowrap())
        .child(value)
}

fn flag_row(t: &Theme, flags: &[FlagView], note: &Option<(Arc<str>, Tone)>) -> Div {
    let mut row = flags.iter().fold(
        div().flex().items_center().gap_2().px_1().h(px(17.)),
        |row, flag| row.child(status_flag(t, flag.name.clone(), flag.set)),
    );

    if let Some((text, tone)) = note {
        row = row.child(
            mono_text(t, text.clone())
                .ml_auto()
                .text_color(tone_color(t, *tone)),
        );
    }

    row
}

fn tone_color(t: &Theme, tone: Tone) -> gpui::Hsla {
    match tone {
        Tone::Normal => t.text_secondary,
        Tone::Muted => t.text_muted,
        Tone::Warning => t.warning,
    }
}

fn bit_header(t: &Theme, bits: u8) -> Div {
    let nums = (0..bits).fold(div().flex().items_center().gap_1(), |d, i| {
        d.child(
            div()
                .w(px(6.))
                .flex()
                .justify_center()
                .font_family(t.mono_font)
                .text_xs()
                .text_color(t.text_dim)
                .child(i.to_string()),
        )
    });

    div()
        .flex()
        .justify_between()
        .items_center()
        .px_1()
        .h(px(12.))
        .child(div())
        .child(nums)
}

fn bit_grid(t: &Theme, label: Arc<str>, bits: u8, on_mask: u8) -> Div {
    let dots = (0..bits).fold(div().flex().items_center().gap_1(), |d, i| {
        let on = on_mask & (1 << i) != 0;
        d.child(
            div()
                .size(px(6.))
                .rounded_full()
                .bg(if on { t.accent } else { t.bg_panel_alt }),
        )
    });

    div()
        .flex()
        .justify_between()
        .items_center()
        .px_1()
        .h(px(15.))
        .child(label_text(t, label).whitespace_nowrap())
        .child(dots)
}

fn meter(t: &Theme, label: Arc<str>, value: i32, max: i32, accent: bool) -> Div {
    let magnitude = if max > 0 {
        value.unsigned_abs() as f32 / max as f32
    } else {
        0.0
    };

    div()
        .flex()
        .items_center()
        .gap_2()
        .px_1()
        .h(px(17.))
        .child(label_text(t, label).w(px(52.)).whitespace_nowrap())
        .child(meter_bar(t, magnitude, accent))
        .child(
            mono_text(t, format!("{value:+6}"))
                .w(px(50.))
                .text_color(if value == 0 {
                    t.text_dim
                } else {
                    t.text_secondary
                })
                .whitespace_nowrap(),
        )
}

fn tag_row(t: &Theme, tags: &[Arc<str>]) -> Div {
    tags.iter().fold(
        div().flex().items_center().gap(px(8.)).px_1().h(px(16.)),
        |row, tag| row.child(mono_text(t, tag.clone()).whitespace_nowrap()),
    )
}

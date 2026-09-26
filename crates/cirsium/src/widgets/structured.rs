use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{App, Div, ScrollHandle, SharedString, Window, div, px};

use crate::components::{
    PanelChrome, RowCallback, chevron_slot, label_text, meter_bar_scaled, mono_text, row_toggle,
    section_header, status_flag, vertical_scroll_area,
};

use crate::theme::Theme;

pub use debugger_model::{Field, FlagView, Row, Section, StructuredView, Tone};

#[derive(IntoElement)]
pub struct StructuredPanel {
    chrome: PanelChrome,
    view: Arc<StructuredView>,
    scroll: ScrollHandle,
    expanded: HashSet<SharedString>,
    on_toggle_group: Option<RowCallback>,
}

impl StructuredPanel {
    pub fn new(chrome: PanelChrome, view: Arc<StructuredView>) -> Self {
        Self {
            chrome,
            view,
            scroll: ScrollHandle::new(),
            expanded: HashSet::new(),
            on_toggle_group: None,
        }
    }

    pub fn expanded(mut self, expanded: HashSet<SharedString>) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn on_toggle_group(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle_group = Some(Rc::new(handler));
        self
    }

    pub fn scroll(mut self, scroll: ScrollHandle) -> Self {
        self.scroll = scroll;
        self
    }
}

impl RenderOnce for StructuredPanel {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;
        let view = &*self.view;

        let scroll_id = SharedString::from(format!("{}.structured", self.chrome.id()));
        let scroll = self.scroll;
        let mut body = div()
            .id(scroll_id)
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .track_scroll(&scroll);

        body = body.child(content(t, view, &self.expanded, &self.on_toggle_group));

        let body = vertical_scroll_area(body, scroll, t).flex_1();

        self.chrome
            .panel()
            .grow(true)
            .meta_opt(view.meta.clone().map(Into::into))
            .child(body)
    }
}

pub fn content(
    t: &Theme,
    view: &StructuredView,
    expanded: &HashSet<SharedString>,
    toggle: &Option<RowCallback>,
) -> Div {
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
            body = body.child(section_column(t, block[0], expanded, toggle));
        } else {
            let row = block
                .into_iter()
                .fold(div().flex().items_start().gap_3(), |row, section| {
                    let col = section_column(t, section, expanded, toggle)
                        .min_w(px(0.))
                        .overflow_hidden();
                    let col = match section.basis {
                        Some(fraction) => col.w(gpui::relative(fraction)).flex_none(),
                        None => col.flex_1(),
                    };

                    row.child(col)
                });

            body = body.child(row);
        }
    }

    body
}

fn section_column(
    t: &Theme,
    section: &Section,
    expanded: &HashSet<SharedString>,
    toggle: &Option<RowCallback>,
) -> Div {
    let mut col = div().flex().flex_col();
    if let Some(title) = &section.title {
        col = col.child(heading(t, title, section.leading));
    }

    section.rows.iter().fold(col, |col, row| {
        col.child(render_row(t, row, expanded, toggle))
    })
}

fn heading(t: &Theme, text: &str, leading: bool) -> Div {
    section_header(t, text).pt(px(if leading { 0. } else { 6. }))
}

fn render_row(
    t: &Theme,
    row: &Row,
    expanded: &HashSet<SharedString>,
    toggle: &Option<RowCallback>,
) -> Div {
    match row {
        Row::Field(field) => kv(t, field),
        Row::Group {
            id,
            title,
            summary,
            active,
            rows,
        } => {
            let id = SharedString::from(id.clone());
            let open = toggle.is_none() || expanded.contains(&id);
            let mut header = div()
                .id(id.clone())
                .flex()
                .items_center()
                .gap_2()
                .px_1()
                .h(px(18.))
                .child(chevron_slot(t, !rows.is_empty(), open))
                .child(label_text(t, title.clone()).when(*active, |d| d.text_color(t.accent)))
                .child(div().flex_1())
                .child(mono_text(t, summary.clone()).text_color(t.text_muted));

            if let Some(toggle) = toggle {
                header = row_toggle(header, t, id, toggle.clone());
            }

            let mut group = div().flex().flex_col().child(header);
            if open {
                group = group.child(
                    rows.iter()
                        .fold(div().flex().flex_col().pl(px(12.)), |d, r| {
                            d.child(render_row(t, r, expanded, toggle))
                        }),
                );
            }

            group
        }
        Row::Flags { flags, note } => flag_row(t, flags, note),
        Row::BitHeader { bits } => bit_header(t, *bits),
        Row::BitGrid { label, mask } => bit_grid(t, label.clone(), *mask),
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
    let nums = (0..bits.min(32)).fold(div().flex().items_center().gap_1(), |d, i| {
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

fn bit_grid(t: &Theme, label: Arc<str>, mask: debugger_model::BitMask) -> Div {
    let dots = (0..mask.bits()).fold(div().flex().items_center().gap_1(), |d, i| {
        let on = mask.is_set(i);
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
    div()
        .flex()
        .items_center()
        .gap_2()
        .px_1()
        .h(px(17.))
        .child(label_text(t, label).w(px(52.)).whitespace_nowrap())
        .child(meter_bar_scaled(t, value, max, accent))
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

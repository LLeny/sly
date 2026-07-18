use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{App, Div, Window, div, px};

use crate::components::{
    PanelChrome, empty_state_centered, header_rule, label_text, meter_bar_scaled, mono_text,
    status_flag,
};
use crate::theme::Theme;

pub use debugger_model::{Cell, ColWidth, Column, TableRow, TableView};

use crate::widgets::grid::SelectCallback;

const ROW_HEIGHT: f32 = 18.;

#[derive(IntoElement)]
pub struct TablePanel {
    chrome: PanelChrome,
    view: Arc<TableView>,
    selected: Option<usize>,
    on_select: Option<SelectCallback>,
}

impl TablePanel {
    pub fn new(chrome: PanelChrome, view: Arc<TableView>) -> Self {
        Self {
            chrome,
            view,
            selected: None,
            on_select: None,
        }
    }

    pub fn selected(mut self, row: usize) -> Self {
        self.selected = Some(row);
        self
    }

    pub fn on_select(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for TablePanel {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let t = &*theme;
        let view = &*self.view;

        let body = if view.rows.is_empty() {
            empty_state_centered(t, "No rows")
        } else {
            let mut list = div().flex().flex_col().child(header_row(t, &view.columns));
            for (ix, row) in view.rows.iter().enumerate() {
                list = list.child(data_row(
                    t,
                    &view.columns,
                    ix,
                    row,
                    self.selected == Some(ix),
                    self.on_select.clone(),
                ));
            }

            list
        };

        self.chrome
            .panel()
            .meta_opt(view.meta.clone().map(Into::into))
            .child(body)
    }
}

fn cell_frame(column: &Column, content: impl IntoElement) -> Div {
    let frame = div().flex().items_center();
    let frame = match column.width {
        ColWidth::Fixed(w) => frame.w(px(w)).flex_shrink_0(),
        ColWidth::Flex => frame.flex_1().min_w(px(0.)),
    };
    frame.overflow_hidden().child(content.into_any_element())
}

fn header_row(t: &Theme, columns: &[Column]) -> Div {
    columns.iter().fold(
        header_rule(t).flex().items_center().gap(px(3.)),
        |row, column| {
            row.child(cell_frame(
                column,
                label_text(t, column.label.clone()).whitespace_nowrap(),
            ))
        },
    )
}

fn data_row(
    t: &Theme,
    columns: &[Column],
    index: usize,
    row: &TableRow,
    selected: bool,
    on_select: Option<SelectCallback>,
) -> impl IntoElement {
    let active = row.active;
    let mut el = row.cells.iter().enumerate().fold(
        div()
            .id(index)
            .flex()
            .items_center()
            .gap(px(3.))
            .px_1()
            .h(px(ROW_HEIGHT))
            .when(selected, |d| {
                d.bg(t.accent_soft)
                    .border_l_1()
                    .border_color(t.selection_border)
            })
            .when(!selected && active, |d| d.bg(t.accent_faint)),
        |el, (col_ix, cell)| {
            let column = &columns[col_ix.min(columns.len() - 1)];
            el.child(cell_frame(
                column,
                render_cell(t, cell, col_ix == 0, active),
            ))
        },
    );

    if let Some(handler) = on_select {
        let hover_bg = t.accent_soft;
        el = el
            .cursor_pointer()
            .hover(move |style| style.bg(hover_bg))
            .on_click(move |_, window, cx| handler(index, window, cx));
    }

    el
}

fn render_cell(t: &Theme, cell: &Cell, first: bool, active: bool) -> Div {
    match cell {
        Cell::Text(text) => {
            let color = if first && active {
                t.accent
            } else if first {
                t.text_dim
            } else {
                t.text_secondary
            };
            mono_text(t, text.clone())
                .text_color(color)
                .whitespace_nowrap()
        }
        Cell::Meter { value, max, label } => {
            let mut row = div().flex().flex_1().min_w(px(0.)).items_center().gap_1();
            if let Some(label) = label {
                row = row.child(
                    mono_text(t, label.clone())
                        .w(px(30.))
                        .flex_shrink_0()
                        .whitespace_nowrap(),
                );
            }
            row.child(meter_bar_scaled(t, *value, *max, active))
        }
        Cell::FlagChips(flags) => flags.iter().fold(
            div().flex().items_center().gap(px(2.)).whitespace_nowrap(),
            |row, flag| row.child(status_flag(t, flag.name.clone(), flag.set).whitespace_nowrap()),
        ),
    }
}

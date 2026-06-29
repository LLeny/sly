use gpui::prelude::*;
use gpui::{Div, FontWeight, Hsla, MouseButton, MouseDownEvent, MouseMoveEvent, div, px};

use crate::diff::{Highlight, highlight_color};
use crate::theme::Theme;
use crate::widgets::BreakpointKind;

use super::{
    ByteDownCallback, ByteDragCallback, ContextCallback, MemoryCell, MemoryRow, ascii_glyph,
};

const ROW_HEIGHT: f32 = 20.;
const ADDR_WIDTH: f32 = 76.;
const HEX_CELL_WIDTH: f32 = 21.;
const ASCII_CELL_WIDTH: f32 = 9.;
const GROUP_GAP: f32 = 8.;
const HEX_ASCII_GAP: f32 = 14.;

#[allow(clippy::too_many_arguments)]
pub(super) fn mem_row(
    theme: &Theme,
    row: &MemoryRow,
    bytes_per_row: usize,
    selection: Option<(u32, u32)>,
    breakpoints: &[(u32, u32, BreakpointKind)],
    on_byte_down: Option<ByteDownCallback>,
    on_byte_drag: Option<ByteDragCallback>,
    on_context: Option<ContextCallback>,
) -> Div {
    let in_selection = |addr: u32| selection.is_some_and(|(lo, hi)| addr >= lo && addr <= hi);
    let breakpoint_at = |addr: u32| {
        breakpoints
            .iter()
            .find(|(lo, hi, _)| addr >= *lo && addr <= *hi)
            .map(|&(_, _, kind)| kind)
    };

    let address = div()
        .font_family(theme.mono_font)
        .text_sm()
        .w(px(ADDR_WIDTH))
        .flex_none()
        .text_color(theme.code_address)
        .child(row.label.clone());

    let half = bytes_per_row / 2;
    let last = row.cells.len().saturating_sub(1);
    let fills: Vec<Fill> = row
        .cells
        .iter()
        .enumerate()
        .map(|(i, cell)| cell_fill(cell, in_selection(row.addr + i as u32)))
        .collect();

    let mut hex = div().flex().items_center().flex_none();
    let mut ascii = div().flex().items_center().flex_none();

    let stride = bytes_per_row as u32;
    for (i, cell) in row.cells.iter().enumerate() {
        let addr = row.addr + i as u32;
        let colors = fill_colors(theme, fills[i]);
        let dashed = matches!(fills[i], Fill::Breakpoint(_));
        let text = match colors {
            Some((_, border)) if dashed => border,
            _ => highlight_color(theme, cell.highlight),
        };

        let hex_left = i == 0 || i == half || fills[i - 1] != fills[i];
        let hex_right = i == last || i + 1 == half || fills[i + 1] != fills[i];
        let ascii_left = i == 0 || fills[i - 1] != fills[i];
        let ascii_right = i == last || fills[i + 1] != fills[i];

        let (top, bottom) = match fills[i] {
            Fill::Selection => (
                !in_selection(addr.wrapping_sub(stride)),
                !in_selection(addr + stride),
            ),
            Fill::Breakpoint(kind) => (
                breakpoint_at(addr.wrapping_sub(stride)) != Some(kind),
                breakpoint_at(addr.wrapping_add(stride)) != Some(kind),
            ),
            _ => (true, true),
        };

        if i == half {
            hex = hex.child(div().w(px(GROUP_GAP)).flex_none());
        }

        let hex_cell = byte_cell(
            fill_cell(
                theme,
                cell_box(HEX_CELL_WIDTH),
                text,
                colors,
                hex_left,
                hex_right,
                top,
                bottom,
                dashed,
            )
            .child(format!("{:02X}", cell.value)),
            addr,
            on_byte_down.clone(),
            on_byte_drag.clone(),
            on_context.clone(),
        );
        hex = hex.child(hex_cell);

        let ascii_cell = byte_cell(
            fill_cell(
                theme,
                cell_box(ASCII_CELL_WIDTH),
                text,
                colors,
                ascii_left,
                ascii_right,
                top,
                bottom,
                dashed,
            )
            .child(ascii_glyph(cell.value).to_string()),
            addr,
            on_byte_down.clone(),
            on_byte_drag.clone(),
            on_context.clone(),
        );
        ascii = ascii.child(ascii_cell);
    }

    div()
        .flex()
        .items_center()
        .gap_2()
        .h(px(ROW_HEIGHT))
        .px_1()
        .child(address)
        .child(hex)
        .child(div().w(px(HEX_ASCII_GAP)).flex_none())
        .child(ascii)
}

pub(super) fn column_header(theme: &Theme, bytes_per_row: usize) -> Div {
    let mut hex = div().flex().items_center().flex_none();
    let mut ascii = div().flex().items_center().flex_none();
    for col in 0..bytes_per_row {
        if col == bytes_per_row / 2 {
            hex = hex.child(div().w(px(GROUP_GAP)).flex_none());
        }

        hex = hex.child(
            cell_box(HEX_CELL_WIDTH)
                .font_family(theme.mono_font)
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(theme.text_muted)
                .child(format!("{col:02X}")),
        );
        ascii = ascii.child(
            cell_box(ASCII_CELL_WIDTH)
                .font_family(theme.mono_font)
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(theme.text_dim)
                .child(format!("{col:X}")),
        );
    }

    div()
        .flex()
        .items_center()
        .gap_2()
        .h(px(ROW_HEIGHT))
        .px_1()
        .border_b_1()
        .border_color(theme.border_subtle)
        .child(
            div()
                .w(px(ADDR_WIDTH))
                .flex_none()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(theme.text_muted)
                .child("ADDRESS"),
        )
        .child(hex)
        .child(div().w(px(HEX_ASCII_GAP)).flex_none())
        .child(ascii)
}

fn cell_box(width: f32) -> Div {
    div()
        .w(px(width))
        .h(px(ROW_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fill {
    None,
    Read,
    Write,
    Selection,
    Breakpoint(BreakpointKind),
}

fn cell_fill(cell: &MemoryCell, selected: bool) -> Fill {
    if let Some(kind) = cell.breakpoint {
        return Fill::Breakpoint(kind);
    }

    match cell.highlight {
        Highlight::Read => Fill::Read,
        Highlight::Written | Highlight::Changed => Fill::Write,
        Highlight::None if selected => Fill::Selection,
        Highlight::None => Fill::None,
    }
}

fn fill_colors(theme: &Theme, fill: Fill) -> Option<(Hsla, Hsla)> {
    match fill {
        Fill::None => None,
        Fill::Read => Some((theme.read_fill, theme.read_border)),
        Fill::Write => Some((theme.write_fill, theme.write_border)),
        Fill::Selection => Some((theme.selection, theme.selection_border)),
        Fill::Breakpoint(kind) => Some(kind.colors(theme)),
    }
}

#[allow(clippy::too_many_arguments)]
fn fill_cell(
    theme: &Theme,
    base: Div,
    text: Hsla,
    fill: Option<(Hsla, Hsla)>,
    left: bool,
    right: bool,
    top: bool,
    bottom: bool,
    dashed: bool,
) -> Div {
    let mut cell = base.font_family(theme.mono_font).text_sm().text_color(text);
    if let Some((bg, border)) = fill {
        cell = cell
            .when(dashed, |d| d.border_dashed())
            .bg(bg)
            .border_color(border)
            .when(top, |d| d.border_t_1())
            .when(bottom, |d| d.border_b_1())
            .when(left, |d| d.border_l_1())
            .when(right, |d| d.border_r_1());
    }
    cell
}

fn byte_cell(
    cell: Div,
    addr: u32,
    on_byte_down: Option<ByteDownCallback>,
    on_byte_drag: Option<ByteDragCallback>,
    on_context: Option<ContextCallback>,
) -> Div {
    let mut cell = cell.cursor_pointer();

    if let Some(down) = on_byte_down {
        cell = cell.on_mouse_down(MouseButton::Left, move |e: &MouseDownEvent, window, cx| {
            down(addr, e.modifiers.shift, window, cx);
        });
    }
    if let Some(drag) = on_byte_drag {
        cell = cell.on_mouse_move(move |e: &MouseMoveEvent, window, cx| {
            if e.dragging() {
                drag(addr, window, cx);
            }
        });
    }
    if let Some(context) = on_context {
        cell = cell.on_mouse_down(MouseButton::Right, move |e: &MouseDownEvent, window, cx| {
            context(addr, e.position, window, cx);
        });
    }

    cell
}

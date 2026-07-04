mod combobox;
mod copyable;
mod menu;
mod panel;
mod primitives;
mod rows;
mod segmented;
mod status_bar;
mod text_input;
mod window;

pub use combobox::{ComboOption, combobox_button, combobox_menu};
pub use copyable::Copyable;
pub use menu::{MenuItem, context_menu, popover};
pub use panel::Panel;
pub use primitives::{
    CheckboxSize, badge, bool_chip, checkbox, empty_state, field_button, hex, label_text,
    mono_text, separator,
};
pub use rows::{
    DETAIL_INDENT, INDENT_STEP, RowCallback, chevron_slot, detail_row, footer_row, row_toggle,
};
pub use segmented::Segmented;
pub use status_bar::{StatusView, console_pill, status_bar};
pub use text_input::{TextInput, TextInputEvent, register_text_input_keys};
pub use window::{icon_button, title_bar, window_button};

use gpui::{App, ClickEvent, Window};

pub type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

pub const CONTROL_HEIGHT: f32 = 24.;

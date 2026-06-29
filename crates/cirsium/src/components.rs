mod combobox;
mod copyable;
mod menu;
mod panel;
mod primitives;
mod status_bar;
mod text_input;
mod window;

pub use combobox::{ComboOption, combobox_button, combobox_menu};
pub use copyable::Copyable;
pub use menu::{MenuItem, context_menu, popover};
pub use panel::Panel;
pub use primitives::{CheckboxSize, badge, checkbox, field_button, label_text, separator};
pub use status_bar::{StatusView, console_pill, status_bar};
pub use text_input::{TextInput, TextInputEvent, register_text_input_keys};
pub use window::{icon_button, title_bar, window_button};

use gpui::{App, ClickEvent, Window};

pub type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

pub const CONTROL_HEIGHT: f32 = 24.;

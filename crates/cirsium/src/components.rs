mod copyable;
mod menu;
mod panel;
mod primitives;
mod status_bar;
mod window;

pub use copyable::Copyable;
pub use menu::{MenuItem, context_menu, popover};
pub use panel::Panel;
pub use primitives::{badge, checkbox, label_text, separator};
pub use status_bar::{StatusView, console_pill, status_bar};
pub use window::{icon_button, title_bar, window_button};

use gpui::{App, ClickEvent, Window};

pub type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

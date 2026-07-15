pub mod assets;
pub mod components;
pub mod diff;
pub mod theme;
pub mod widgets;

pub use assets::Assets;
pub use diff::{ChangeTracker, Highlight, highlight_color};
pub use theme::{ACCENT_PRESETS, GUEST_COLORS, Theme, ThemeMode, set_theme};

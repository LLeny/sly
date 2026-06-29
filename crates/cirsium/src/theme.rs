use std::sync::Arc;

use gpui::{App, Global, Hsla, rgb, rgba};

pub const UI_FONT: &str = "IBM Plex Sans";
pub const MONO_FONT: &str = "IBM Plex Mono";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

impl ThemeMode {
    pub fn toggled(self) -> Self {
        match self {
            ThemeMode::Dark => ThemeMode::Light,
            ThemeMode::Light => ThemeMode::Dark,
        }
    }
}

#[derive(Clone)]
pub struct Theme {
    pub mode: ThemeMode,

    pub ui_font: &'static str,
    pub mono_font: &'static str,

    pub bg_root: Hsla,
    pub bg_panel: Hsla,
    pub bg_panel_alt: Hsla,
    pub bg_header: Hsla,
    pub border_subtle: Hsla,
    pub border_strong: Hsla,

    pub text_primary: Hsla,
    pub text_secondary: Hsla,
    pub text_muted: Hsla,
    pub text_dim: Hsla,

    pub accent: Hsla,
    pub accent_soft: Hsla,
    pub accent_hover: Hsla,
    pub selection: Hsla,
    pub selection_border: Hsla,

    pub success: Hsla,
    pub warning: Hsla,
    pub danger: Hsla,
    pub danger_strong: Hsla,
    pub info: Hsla,

    pub read_fill: Hsla,
    pub read_border: Hsla,
    pub write_fill: Hsla,
    pub write_border: Hsla,
    pub exec_fill: Hsla,
    pub exec_border: Hsla,

    pub code_opcode: Hsla,
    pub code_address: Hsla,
    pub code_comment: Hsla,
    pub code_number: Hsla,

    pub code_flow: Hsla,
    pub code_memory: Hsla,
    pub code_alu: Hsla,
    pub code_transfer: Hsla,
    pub code_system: Hsla,
    pub code_register: Hsla,
    pub code_immediate: Hsla,
    pub code_sigil: Hsla,
    pub code_symbol: Hsla,
    pub code_punctuation: Hsla,
}

impl Theme {
    pub fn new(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self::dark(),
            ThemeMode::Light => Self::light(),
        }
    }

    pub fn global(cx: &App) -> Arc<Theme> {
        cx.global::<GlobalTheme>().0.clone()
    }

    pub fn dark() -> Self {
        Self {
            mode: ThemeMode::Dark,

            ui_font: UI_FONT,
            mono_font: MONO_FONT,

            bg_root: rgb(0x000000).into(),
            bg_panel: rgb(0x0a0a0c).into(),
            bg_panel_alt: rgb(0x111114).into(),
            bg_header: rgb(0x0e0e11).into(),
            border_subtle: rgb(0x232328).into(),
            border_strong: rgb(0x3a3a42).into(),

            text_primary: rgb(0xf0f0f3).into(),
            text_secondary: rgb(0xababb2).into(),
            text_muted: rgb(0x6c6c74).into(),
            text_dim: rgb(0x4a4a52).into(),

            accent: rgb(0xa55cff).into(),
            accent_soft: rgba(0xa55cff3a).into(),
            accent_hover: rgb(0xb985ff).into(),
            selection: rgb(0x4a2f7a).into(),
            selection_border: rgb(0x7d5bb8).into(),

            success: rgb(0x6cc98a).into(),
            warning: rgb(0xd9b46a).into(),
            danger: rgb(0xee8a86).into(),
            danger_strong: rgb(0xe5484d).into(),
            info: rgb(0x7bb8f4).into(),

            read_fill: rgb(0x1d3a5c).into(),
            read_border: rgb(0x3f6fa8).into(),
            write_fill: rgb(0x1e3d2b).into(),
            write_border: rgb(0x4f9e6a).into(),
            exec_fill: rgb(0x5c2329).into(),
            exec_border: rgb(0xa84f54).into(),

            code_opcode: rgb(0x86c98f).into(),
            code_address: rgb(0x86868f).into(),
            code_comment: rgb(0x64645f).into(),
            code_number: rgb(0xd7b87e).into(),

            code_flow: rgb(0xe6a0c0).into(),
            code_memory: rgb(0x8fbde6).into(),
            code_alu: rgb(0xe6b58f).into(),
            code_transfer: rgb(0xc6a8ec).into(),
            code_system: rgb(0x8fe0cf).into(),
            code_register: rgb(0x7fd4d4).into(),
            code_immediate: rgb(0xa79bcf).into(),
            code_sigil: rgb(0x9c8a5e).into(),
            code_symbol: rgb(0xf0a995).into(),
            code_punctuation: rgb(0x6c6c74).into(),
        }
    }

    pub fn light() -> Self {
        Self {
            mode: ThemeMode::Light,

            ui_font: UI_FONT,
            mono_font: MONO_FONT,

            bg_root: rgb(0xeaeaec).into(),
            bg_panel: rgb(0xfbfbfc).into(),
            bg_panel_alt: rgb(0xf1f1f3).into(),
            bg_header: rgb(0xf3f3f5).into(),
            border_subtle: rgb(0xdadade).into(),
            border_strong: rgb(0xbcbcc2).into(),

            text_primary: rgb(0x18181c).into(),
            text_secondary: rgb(0x4c4c54).into(),
            text_muted: rgb(0x8a8a92).into(),
            text_dim: rgb(0xa6a6ae).into(),

            accent: rgb(0x7c3aed).into(),
            accent_soft: rgba(0x7c3aed26).into(),
            accent_hover: rgb(0x6a28d9).into(),
            selection: rgb(0xcdb4f5).into(),
            selection_border: rgb(0x9d7fd0).into(),

            success: rgb(0x1f9d57).into(),
            warning: rgb(0xb5800f).into(),
            danger: rgb(0xd23b39).into(),
            danger_strong: rgb(0xdc2626).into(),
            info: rgb(0x356fa8).into(),

            read_fill: rgb(0xc3dbf5).into(),
            read_border: rgb(0x5a8fc0).into(),
            write_fill: rgb(0xc8ecd0).into(),
            write_border: rgb(0x4f9e63).into(),
            exec_fill: rgb(0xf3c7c4).into(),
            exec_border: rgb(0xc2706c).into(),

            code_opcode: rgb(0x1f8a3b).into(),
            code_address: rgb(0x6c6c74).into(),
            code_comment: rgb(0x82826a).into(),
            code_number: rgb(0x9a5a12).into(),

            code_flow: rgb(0xb83a78).into(),
            code_memory: rgb(0x2f6fb0).into(),
            code_alu: rgb(0xb5651f).into(),
            code_transfer: rgb(0x7a3ac4).into(),
            code_system: rgb(0x12806a).into(),
            code_register: rgb(0x1f8a8a).into(),
            code_immediate: rgb(0x5f4fa0).into(),
            code_sigil: rgb(0x8a7340).into(),
            code_symbol: rgb(0xc26a4f).into(),
            code_punctuation: rgb(0x8a8a92).into(),
        }
    }
}

pub fn with_alpha(mut color: Hsla, alpha: f32) -> Hsla {
    color.a = alpha;
    color
}

pub struct GlobalTheme(pub Arc<Theme>);

impl Global for GlobalTheme {}

pub fn set_theme(cx: &mut App, theme: Theme) {
    cx.set_global(GlobalTheme(Arc::new(theme)));
}

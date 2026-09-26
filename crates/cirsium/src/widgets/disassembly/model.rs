use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Linear,
    Graph,
}

impl ViewMode {
    pub(super) fn key(self) -> &'static str {
        match self {
            ViewMode::Linear => "linear",
            ViewMode::Graph => "graph",
        }
    }

    pub(super) fn from_key(key: &str) -> Self {
        match key {
            "graph" => ViewMode::Graph,
            _ => ViewMode::Linear,
        }
    }
}

pub use debugger_model::{DisasmLine, DisasmToken, FlowKind, TokenKind};

pub(super) fn token_color(theme: &Theme, kind: TokenKind) -> Hsla {
    match kind {
        TokenKind::Mnemonic => theme.code_opcode,
        TokenKind::MnemonicFlow => theme.code_flow,
        TokenKind::MnemonicMemory => theme.code_memory,
        TokenKind::MnemonicAlu => theme.code_alu,
        TokenKind::MnemonicTransfer => theme.code_transfer,
        TokenKind::MnemonicSystem => theme.code_system,
        TokenKind::Number => theme.code_number,
        TokenKind::ImmPrefix => theme.code_immediate,
        TokenKind::HexPrefix => theme.code_sigil,
        TokenKind::Register => theme.code_register,
        TokenKind::Symbol => theme.code_symbol,
        TokenKind::Punctuation => theme.code_punctuation,
        TokenKind::Plain => theme.text_secondary,
    }
}

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

pub use debugger_model::{FlowKind, TokenKind};

#[derive(Clone)]
pub struct DisasmToken {
    pub text: SharedString,
    pub kind: TokenKind,
}

impl DisasmToken {
    pub fn new(text: impl Into<SharedString>, kind: TokenKind) -> Self {
        Self {
            text: text.into(),
            kind,
        }
    }
}

impl From<debugger_model::DisasmToken> for DisasmToken {
    fn from(token: debugger_model::DisasmToken) -> Self {
        Self {
            text: SharedString::from(token.text.as_ref()),
            kind: token.kind,
        }
    }
}

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

#[derive(Clone, Default)]
pub struct DisasmLine {
    pub addr: u32,
    pub address: SharedString,
    pub bytes: SharedString,
    pub text: SharedString,
    pub tokens: Vec<DisasmToken>,
    /// Muted trailing annotation, e.g. `NMITIMEN = $4200` or a resolved branch target.
    pub note: SharedString,
    pub target: Option<u32>,
    pub flow: FlowKind,
}

impl DisasmLine {
    pub fn new(
        addr: u32,
        address: impl Into<SharedString>,
        bytes: impl Into<SharedString>,
        text: impl Into<SharedString>,
        tokens: Vec<DisasmToken>,
    ) -> Self {
        Self {
            addr,
            address: address.into(),
            bytes: bytes.into(),
            text: text.into(),
            tokens,
            note: SharedString::default(),
            target: None,
            flow: FlowKind::None,
        }
    }

    pub fn target(mut self, target: Option<u32>) -> Self {
        self.target = target;
        self
    }

    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = note.into();
        self
    }
}

impl From<debugger_model::DisasmLine> for DisasmLine {
    fn from(line: debugger_model::DisasmLine) -> Self {
        Self {
            addr: line.addr,
            address: SharedString::from(line.address.as_ref()),
            bytes: SharedString::from(line.bytes.as_ref()),
            text: SharedString::from(line.text.as_ref()),
            tokens: line.tokens.into_iter().map(Into::into).collect(),
            note: SharedString::from(line.note.as_ref()),
            target: line.target,
            flow: line.flow,
        }
    }
}

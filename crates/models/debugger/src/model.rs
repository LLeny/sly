use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Highlight {
    #[default]
    None,
    Read,
    Written,
    Changed,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TokenKind {
    Mnemonic,
    MnemonicFlow,
    MnemonicMemory,
    MnemonicAlu,
    MnemonicTransfer,
    MnemonicSystem,
    Number,
    ImmPrefix,
    HexPrefix,
    Register,
    Symbol,
    Punctuation,
    Plain,
}

#[derive(Clone)]
pub struct DisasmToken {
    pub text: Arc<str>,
    pub kind: TokenKind,
}

impl DisasmToken {
    pub fn new(text: impl Into<Arc<str>>, kind: TokenKind) -> Self {
        Self {
            text: text.into(),
            kind,
        }
    }
}

#[derive(Clone)]
pub struct DisasmLine {
    pub addr: u32,
    pub address: Arc<str>,
    pub bytes: Arc<str>,
    pub text: Arc<str>,
    pub tokens: Vec<DisasmToken>,
    /// Muted trailing annotation, e.g. `NMITIMEN = $4200` or a resolved branch target.
    pub note: Arc<str>,
    pub target: Option<u32>,
    pub current: bool,
}

impl DisasmLine {
    pub fn new(
        addr: u32,
        address: impl Into<Arc<str>>,
        bytes: impl Into<Arc<str>>,
        text: impl Into<Arc<str>>,
        tokens: Vec<DisasmToken>,
        current: bool,
    ) -> Self {
        Self {
            addr,
            address: address.into(),
            bytes: bytes.into(),
            text: text.into(),
            tokens,
            note: Arc::from(""),
            target: None,
            current,
        }
    }

    pub fn target(mut self, target: Option<u32>) -> Self {
        self.target = target;
        self
    }

    pub fn note(mut self, note: impl Into<Arc<str>>) -> Self {
        self.note = note.into();
        self
    }
}

#[derive(Clone)]
pub struct RegisterView {
    pub name: Arc<str>,
    pub value: u32,
    pub bits: u8,
    pub highlight: Highlight,
}

impl RegisterView {
    pub fn new(name: impl Into<Arc<str>>, value: u32, bits: u8) -> Self {
        Self {
            name: name.into(),
            value,
            bits,
            highlight: Highlight::None,
        }
    }

    pub fn hex(&self) -> String {
        let width = (self.bits as usize).div_ceil(4);
        format!("${:0width$X}", self.value, width = width)
    }
}

#[derive(Clone)]
pub struct FlagView {
    pub name: Arc<str>,
    pub set: bool,
}

impl FlagView {
    pub fn new(name: impl Into<Arc<str>>, set: bool) -> Self {
        Self {
            name: name.into(),
            set,
        }
    }
}

#[derive(Clone)]
pub struct CpuView {
    pub isa: Arc<str>,
    pub registers: Vec<RegisterView>,
    pub flags: Vec<FlagView>,
}

impl Default for CpuView {
    fn default() -> Self {
        Self {
            isa: Arc::from(""),
            registers: Vec::new(),
            flags: Vec::new(),
        }
    }
}

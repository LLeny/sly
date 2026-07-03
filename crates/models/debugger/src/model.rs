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

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum FlowKind {
    #[default]
    None,
    CondBranch,
    Branch,
    Jump,
    Call,
    Return,
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
    pub flow: FlowKind,
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
            flow: FlowKind::None,
        }
    }

    pub fn target(mut self, target: Option<u32>) -> Self {
        self.target = target;
        self
    }

    pub fn flow(mut self, flow: FlowKind) -> Self {
        self.flow = flow;
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
        self::hex(self.value, self.bits)
    }
}

pub fn hex(value: u32, bits: u8) -> String {
    let width = (bits as usize).div_ceil(4);
    format!("${value:0width$X}")
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

#[derive(Clone)]
pub struct CartridgeField {
    pub label: Arc<str>,
    pub value: Arc<str>,
    pub accent: bool,
}

impl CartridgeField {
    pub fn new(label: impl Into<Arc<str>>, value: impl Into<Arc<str>>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            accent: false,
        }
    }

    pub fn accented(mut self) -> Self {
        self.accent = true;
        self
    }
}

#[derive(Clone)]
pub struct CartridgeView {
    pub title: Arc<str>,
    pub fields: Vec<CartridgeField>,
    pub cover: Option<Arc<str>>,
}

#[derive(Clone)]
pub struct MmioFieldView {
    pub name: Arc<str>,
    pub bit_label: Arc<str>,
    pub value: u32,
    pub display: Arc<str>,
    pub is_bool: bool,
    pub children: Vec<MmioFieldView>,
}

#[derive(Clone)]
pub struct MmioRegisterView {
    pub id: Arc<str>,
    pub name: Arc<str>,
    pub addr: Arc<str>,
    pub value: u32,
    pub bits: u8,
    pub highlight: Highlight,
    pub fields: Vec<MmioFieldView>,
}

#[derive(Clone)]
pub struct MmioGroup {
    pub name: Arc<str>,
    pub registers: Vec<MmioRegisterView>,
}

#[derive(Clone, Default)]
pub struct MmioView {
    pub groups: Vec<MmioGroup>,
}

#[derive(Clone)]
pub struct MemoryRegion {
    pub id: Arc<str>,
    pub name: Arc<str>,
    pub base: u32,
    pub size: u32,
}

impl MemoryRegion {
    pub fn new(id: impl Into<Arc<str>>, name: impl Into<Arc<str>>, base: u32, size: u32) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            base,
            size,
        }
    }
}

#[derive(Clone, Default)]
pub struct StatusView {
    pub system: Arc<str>,
    pub session: Arc<str>,
    pub segments: Vec<Arc<str>>,
    pub state: Arc<str>,
}

#[derive(Clone)]
pub struct StackFrame {
    pub kind: Arc<str>,
    pub location: Arc<str>,
    pub symbol: Arc<str>,
    pub caller: Arc<str>,
}

#[derive(Clone, Default)]
pub struct CallStackView {
    pub frames: Vec<StackFrame>,
}

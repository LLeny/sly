use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Highlight {
    #[default]
    None,
    Read,
    Written,
    Changed,
    Execution,
    Pipeline,
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

#[derive(Clone, Default)]
pub struct DisasmLine {
    pub addr: u32,
    pub length: u8,
    pub conditional: bool,
    pub address: Arc<str>,
    pub bytes: Arc<str>,
    pub text: Arc<str>,
    pub tokens: Vec<DisasmToken>,
    /// Muted trailing annotation, e.g. `NMITIMEN = $4200` or a resolved branch target.
    pub note: Arc<str>,
    pub target: Option<u32>,
    pub flow: FlowKind,
}

impl DisasmLine {
    pub fn new(
        addr: u32,
        length: u8,
        address: impl Into<Arc<str>>,
        bytes: impl Into<Arc<str>>,
        text: impl Into<Arc<str>>,
        tokens: Vec<DisasmToken>,
    ) -> Self {
        Self {
            addr,
            length,
            conditional: false,
            address: address.into(),
            bytes: bytes.into(),
            text: text.into(),
            tokens,
            note: Arc::from(""),
            target: None,
            flow: FlowKind::None,
        }
    }

    pub fn conditional(mut self, conditional: bool) -> Self {
        self.conditional = conditional;
        self
    }

    pub fn has_fallthrough(&self) -> bool {
        self.conditional || matches!(self.flow, FlowKind::None | FlowKind::Call)
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
    let mut width = (bits as usize).div_ceil(4);
    if (value as u64) >> (width * 4) != 0 {
        let nibbles = ((32 - value.leading_zeros()) as usize).div_ceil(4);
        width = nibbles.next_multiple_of(2);
    }
    format!("${value:0width$X}")
}

pub fn format_bytes(len: u8, mut byte_at: impl FnMut(u16) -> u8) -> String {
    use std::fmt::Write;

    let mut out = String::with_capacity(len as usize * 3);
    for i in 0..len {
        if i != 0 {
            out.push(' ');
        }
        let _ = write!(out, "{:02X}", byte_at(u16::from(i)));
    }
    out
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

/// A labeled value with optional accent, shared by field-list views.
#[derive(Clone)]
pub struct Field {
    pub label: Arc<str>,
    pub value: Arc<str>,
    pub accent: bool,
}

impl Field {
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
    pub details: StructuredView,
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

#[derive(Clone, Default)]
pub struct AudioChannel {
    pub label: Arc<str>,
    pub active: bool,
    pub waveform: Arc<[f32]>,
}

#[derive(Clone, Default)]
pub struct AudioScopesView {
    pub channels: Vec<AudioChannel>,
}

#[derive(Clone, Default)]
pub struct AudioScopeView {
    pub samples: Arc<[f32]>,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Tone {
    #[default]
    Normal,
    Muted,
    Warning,
}

#[derive(Clone, Default)]
pub struct StructuredView {
    pub sections: Vec<Section>,
    pub meta: Option<Arc<str>>,
}

#[derive(Clone, Default)]
pub struct Section {
    pub title: Option<Arc<str>>,
    pub leading: bool,
    pub group: u8,
    pub basis: Option<f32>,
    pub rows: Vec<Row>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BitMask {
    bits: u8,
    value: u32,
}

impl BitMask {
    pub const fn new(bits: u8, value: u32) -> Option<Self> {
        if bits == 0 || bits > 32 || (bits < 32 && value >> bits != 0) {
            None
        } else {
            Some(Self { bits, value })
        }
    }

    pub const fn bits(self) -> u8 {
        self.bits
    }

    pub const fn is_set(self, bit: u8) -> bool {
        bit < self.bits && self.value & (1u32 << bit) != 0
    }
}

impl From<u8> for BitMask {
    fn from(value: u8) -> Self {
        Self {
            bits: 8,
            value: value.into(),
        }
    }
}

#[derive(Clone)]
pub enum Row {
    Field(Field),
    Group {
        id: Arc<str>,
        title: Arc<str>,
        summary: Arc<str>,
        active: bool,
        rows: Vec<Row>,
    },
    Flags {
        flags: Vec<FlagView>,
        note: Option<(Arc<str>, Tone)>,
    },
    BitHeader {
        bits: u8,
    },
    BitGrid {
        label: Arc<str>,
        mask: BitMask,
    },
    Meter {
        label: Arc<str>,
        value: i32,
        max: i32,
        accent: bool,
    },
    Tags {
        tags: Vec<Arc<str>>,
    },
}

#[derive(Clone, Copy)]
pub enum ColWidth {
    Fixed(f32),
    Flex,
}

#[derive(Clone)]
pub struct Column {
    pub label: Arc<str>,
    pub width: ColWidth,
}

#[derive(Clone)]
pub enum Cell {
    Text(Arc<str>),
    Meter {
        value: i32,
        max: i32,
        label: Option<Arc<str>>,
    },
    FlagChips(Vec<FlagView>),
}

#[derive(Clone, Default)]
pub struct TableRow {
    pub active: bool,
    pub cells: Vec<Cell>,
}

#[derive(Clone, Default)]
pub struct TableView {
    pub columns: Vec<Column>,
    pub rows: Vec<TableRow>,
    pub meta: Option<Arc<str>>,
}

#[derive(Clone)]
pub struct MemoryRegion {
    pub id: Arc<str>,
    pub name: Arc<str>,
    pub base: u32,
    pub size: u32,
    /// Empty means data watchpoints aren't supported here.
    pub watch_sources: Vec<crate::AccessSource>,
}

impl MemoryRegion {
    pub fn watch_sources(mut self, sources: Vec<crate::AccessSource>) -> Self {
        self.watch_sources = sources;
        self
    }

    pub fn end(&self) -> u64 {
        (u64::from(self.base) + u64::from(self.size)).min(1_u64 << 32)
    }

    pub fn addresses(&self) -> impl Iterator<Item = u32> + '_ {
        (u64::from(self.base)..self.end()).map(|addr| addr as u32)
    }

    pub fn contains(&self, addr: u32) -> bool {
        u64::from(addr) >= u64::from(self.base) && u64::from(addr) < self.end()
    }

    pub fn new(id: impl Into<Arc<str>>, name: impl Into<Arc<str>>, base: u32, size: u32) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            base,
            size,
            watch_sources: Vec::new(),
        }
    }
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

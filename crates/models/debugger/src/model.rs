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

pub type CartridgeField = Field;

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
pub struct DmaChannelDetail {
    pub label: Arc<str>,
    pub value: Arc<str>,
}

impl DmaChannelDetail {
    pub fn new(label: impl Into<Arc<str>>, value: impl Into<Arc<str>>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

#[derive(Clone)]
pub struct DmaChannelView {
    pub id: Arc<str>,
    pub name: Arc<str>,
    pub dma_enabled: bool,
    pub hdma_enabled: bool,
    pub source: Arc<str>,
    pub dest: Arc<str>,
    pub count: Arc<str>,
    pub details: Vec<DmaChannelDetail>,
}

#[derive(Clone, Default)]
pub struct DmaView {
    pub channels: Vec<DmaChannelView>,
}

pub type PpuProperty = Field;

#[derive(Clone)]
pub struct PpuGroup {
    pub id: Arc<str>,
    pub name: Arc<str>,
    pub summary: Arc<str>,
    pub properties: Vec<PpuProperty>,
}

impl PpuGroup {
    pub fn new(
        id: impl Into<Arc<str>>,
        name: impl Into<Arc<str>>,
        properties: Vec<PpuProperty>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            summary: Arc::from(""),
            properties,
        }
    }

    pub fn summary(mut self, summary: impl Into<Arc<str>>) -> Self {
        self.summary = summary.into();
        self
    }
}

#[derive(Clone, Default)]
pub struct PpuView {
    pub properties: Vec<PpuProperty>,
    pub groups: Vec<PpuGroup>,
}

#[derive(Clone)]
pub struct TilesetTile {
    pub addr: u32,
    pub pixels: Vec<(u8, u8, u8)>,
}

#[derive(Clone)]
pub struct Tileset {
    pub bpp: u8,
    pub tile_bytes: u32,
    pub tiles: Vec<TilesetTile>,
}

#[derive(Clone)]
pub struct TilesetLayer {
    pub name: Arc<str>,
    pub enabled: bool,
}

#[derive(Clone)]
pub struct TilemapEntry {
    pub addr: u32,
    pub raw: u16,
    pub tile_index: u16,
    pub palette: u8,
    pub priority: bool,
    pub flip_h: bool,
    pub flip_v: bool,
    pub pixels: Vec<(u8, u8, u8)>,
}

#[derive(Clone)]
pub struct Tilemap {
    pub width: usize,
    pub height: usize,
    pub tile_px: u8,
    pub entries: Vec<TilemapEntry>,
}

#[derive(Clone, Copy)]
pub struct PaletteColor {
    pub raw: u16,
    pub rgb: (u8, u8, u8),
}

#[derive(Clone, Default)]
pub struct PaletteView {
    pub palettes: Vec<Vec<PaletteColor>>,
    pub meta: Arc<str>,
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

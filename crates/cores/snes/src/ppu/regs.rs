use chapa::BitEnum;

use crate::Snes;
use crate::macros::{register_mmio, register_mmio_family, register_mmio_raw};
use crate::mmio::{MemoryMap, MmioReg};

pub(crate) const INIDISP: u16 = 0x2100;

pub(crate) const OAMADDL: u16 = 0x2102;
pub(crate) const OAMADDH: u16 = 0x2103;
pub(crate) const OAMDATA: u16 = 0x2104;
pub(crate) const OAMDATAREAD: u16 = 0x2138;

pub(crate) const CGADD: u16 = 0x2121;
pub(crate) const CGDATA: u16 = 0x2122;
pub(crate) const RDCGRAM: u16 = 0x213B;

pub(crate) const TM: u16 = 0x212C;
pub(crate) const TS: u16 = 0x212D;
pub(crate) const COLDATA: u16 = 0x2132;

// TODO
pub(crate) const SETINI: u16 = 0x2133;

pub(crate) const VMADDL: u16 = 0x2116;
pub(crate) const VMADDH: u16 = 0x2117;
pub(crate) const VMDATAL: u16 = 0x2118;
pub(crate) const VMDATAH: u16 = 0x2119;
pub(crate) const RDVRAML: u16 = 0x2139;
pub(crate) const RDVRAMH: u16 = 0x213A;

pub(crate) const M7HOFS: u16 = 0x210D;
pub(crate) const M7VOFS: u16 = 0x210E;
pub(crate) const M7_MATRIX_A: u16 = 0x211B;
pub(crate) const M7_MATRIX_B: u16 = 0x211C;
pub(crate) const M7_MATRIX_C: u16 = 0x211D;
pub(crate) const M7_MATRIX_D: u16 = 0x211E;
pub(crate) const M7_CENTER_X: u16 = 0x211F;
pub(crate) const M7_CENTER_Y: u16 = 0x2120;
pub(crate) const MPYL: u16 = 0x2134;
pub(crate) const MPYM: u16 = 0x2135;
pub(crate) const MPYH: u16 = 0x2136;

pub(crate) const HTIMEL: u16 = 0x4207;
pub(crate) const HTIMEH: u16 = 0x4208;
pub(crate) const VTIMEL: u16 = 0x4209;
pub(crate) const VTIMEH: u16 = 0x420A;

register_mmio_raw! {
    OAMADDL => "OAMADDL",
    OAMADDH => "OAMADDH",
    OAMDATA => "OAMDATA",
    OAMDATAREAD => "OAMDATAREAD",
    CGADD => "CGADD",
    CGDATA => "CGDATA",
    RDCGRAM => "RDCGRAM",
    VMADDL => "VMADDL",
    VMADDH => "VMADDH",
    VMDATAL => "VMDATAL",
    VMDATAH => "VMDATAH",
    RDVRAML => "RDVRAML",
    RDVRAMH => "RDVRAMH",
    TM => "TM",
    TS => "TS",
    COLDATA => "COLDATA",
    SETINI => "SETINI",
    0x210D => "BG1HOFS",
    0x210E => "BG1VOFS",
    0x210F => "BG2HOFS",
    0x2110 => "BG2VOFS",
    0x2111 => "BG3HOFS",
    0x2112 => "BG3VOFS",
    0x2113 => "BG4HOFS",
    0x2114 => "BG4VOFS",
    M7HOFS => "M7HOFS",
    M7VOFS => "M7VOFS",
    M7_MATRIX_A => "M7A",
    M7_MATRIX_B => "M7B",
    M7_MATRIX_C => "M7C",
    M7_MATRIX_D => "M7D",
    M7_CENTER_X => "M7X",
    M7_CENTER_Y => "M7Y",
    MPYL => "MPYL",
    MPYM => "MPYM",
    MPYH => "MPYH",
    HTIMEL => "HTIMEL",
    HTIMEH => "HTIMEH",
    VTIMEL => "VTIMEL",
    VTIMEH => "VTIMEH",
}

pub(crate) struct BgScrollOffset;

impl BgScrollOffset {
    pub(crate) const START: u16 = 0x210D; // BG1HOFS
    pub(crate) const END: u16 = 0x2114; // BG4VOFS

    pub(crate) fn write<M: MemoryMap>(snes: &mut Snes<M>, reg: u16, value: u8) {
        let matrix = &mut snes.ppu.matrix;
        match reg {
            M7HOFS => matrix.hofs = Signed13::from_raw(matrix.latch(value)).value(),
            M7VOFS => matrix.vofs = Signed13::from_raw(matrix.latch(value)).value(),
            _ => {}
        }

        let index = (reg - Self::START) as usize;
        let layer = &mut snes.ppu.bg_layers[index / 2];

        let prev = snes.ppu.bgofs_latch.exchange(value);
        if index % 2 == 0 {
            let prev_h = snes.ppu.hofs_latch.exchange(value);
            layer.hofs = (value as u16) << 8 | (prev & !7) as u16 | (prev_h & 7) as u16;
        } else {
            layer.vofs = (value as u16) << 8 | prev as u16;
        }
    }
}

#[chapa::bitfield(u16, order = lsb0)]
struct Signed13 {
    #[bits(0..=12)]
    value: i16,
}

pub(crate) struct Mode7;

impl Mode7 {
    pub(crate) const START: u16 = M7_MATRIX_A;
    pub(crate) const END: u16 = M7_CENTER_Y;

    pub(crate) fn write<M: MemoryMap>(snes: &mut Snes<M>, reg: u16, value: u8) {
        let matrix = &mut snes.ppu.matrix;
        let word = matrix.latch(value);

        match reg {
            M7_MATRIX_A => matrix.a = word as i16,
            M7_MATRIX_B => matrix.b = word as i16,
            M7_MATRIX_C => matrix.c = word as i16,
            M7_MATRIX_D => matrix.d = word as i16,
            M7_CENTER_X => matrix.center_x = Signed13::from_raw(word).value(),
            M7_CENTER_Y => matrix.center_y = Signed13::from_raw(word).value(),
            _ => unreachable!(),
        }

        if matches!(reg, M7_MATRIX_A | M7_MATRIX_B) {
            matrix.update_result();
        }
    }
}

#[derive(BitEnum, Clone, Copy, Default)]
pub enum IncrementMode {
    #[fallback]
    #[default]
    AfterLow = 0,
    AfterHigh = 1,
}

#[derive(BitEnum, Clone, Copy, Default)]
pub enum IncrementAmount {
    #[default]
    One = 0,
    ThirtyTwo = 1,
    #[fallback]
    OneTwentyEight = 2,
}

impl IncrementAmount {
    pub fn words(self) -> u16 {
        match self {
            Self::One => 1,
            Self::ThirtyTwo => 32,
            Self::OneTwentyEight => 128,
        }
    }
}

#[derive(BitEnum, Clone, Copy, Default)]
pub enum AddressRemapping {
    #[fallback]
    #[default]
    None = 0,
    Bpp2 = 1,
    Bpp4 = 2,
    Bpp8 = 3,
}

impl AddressRemapping {
    pub fn apply(self, addr: u16) -> u16 {
        match self {
            Self::None => addr,
            Self::Bpp2 => (addr & 0xFF00) | ((addr << 3) & 0x00F8) | ((addr >> 5) & 0x0007),
            Self::Bpp4 => (addr & 0xFE00) | ((addr << 3) & 0x01F8) | ((addr >> 6) & 0x0007),
            Self::Bpp8 => (addr & 0xFC00) | ((addr << 3) & 0x03F8) | ((addr >> 7) & 0x0007),
        }
    }
}

register_mmio! {
    /// VMAIN
    pub struct VramControl @ 0x2115 {
        #[bits(0..=1)]
        pub increment_amount: IncrementAmount,

        #[bits(2..=3)]
        pub address_remapping: AddressRemapping,

        #[bits(7)]
        pub increment_mode: IncrementMode,
    }
}

register_mmio! {
    /// BG12NBA
    pub struct Bg12ChrBase @ 0x210B {
        #[bits(0..=3)]
        pub bg1: u8,

        #[bits(4..=7)]
        pub bg2: u8,
    }
}

register_mmio! {
    /// BG34NBA
    pub struct Bg34ChrBase @ 0x210C {
        #[bits(0..=3)]
        pub bg3: u8,

        #[bits(4..=7)]
        pub bg4: u8,
    }
}

#[derive(BitEnum, Clone, Copy)]
pub enum BackgroundMode {
    Mode0 = 0,
    Mode1 = 1,
    Mode2 = 2,
    Mode3 = 3,
    Mode4 = 4,
    Mode5 = 5,
    Mode6 = 6,
    Mode7 = 7,
    #[fallback]
    Invalid,
}

impl BackgroundMode {
    pub fn layer_bpp(self) -> [Option<u8>; 4] {
        match self {
            Self::Mode0 => [Some(2), Some(2), Some(2), Some(2)],
            Self::Mode1 => [Some(4), Some(4), Some(2), None],
            Self::Mode2 => [Some(4), Some(4), None, None],
            Self::Mode3 => [Some(8), Some(4), None, None],
            Self::Mode4 => [Some(8), Some(2), None, None],
            Self::Mode5 => [Some(4), Some(2), None, None],
            Self::Mode6 => [Some(4), None, None, None],
            Self::Mode7 => [Some(8), None, None, None],
            Self::Invalid => [None; 4],
        }
    }

    pub fn bpp(self, layer: usize) -> Option<u8> {
        self.layer_bpp().get(layer).copied().flatten()
    }
}

#[derive(BitEnum, Clone, Copy, Default, PartialEq, Eq)]
pub enum Priority {
    #[fallback]
    #[default]
    Normal = 0,
    High = 1,
}

#[derive(BitEnum, Clone, Copy, Default)]
pub enum TileSize {
    #[fallback]
    #[default]
    Size8x8 = 0,
    Size16x16 = 1,
}

impl TileSize {
    pub fn pixels(self) -> u32 {
        match self {
            Self::Size8x8 => 8,
            Self::Size16x16 => 16,
        }
    }
}

register_mmio! {
    /// BGMODE
    pub struct BgMode @ 0x2105 {
        #[bits(0..=2)]
        pub mode: BackgroundMode,

        #[bits(3)]
        pub mode1_bg3_priority: Priority,

        #[bits(4)]
        pub bg1_tile_size: TileSize,

        #[bits(5)]
        pub bg2_tile_size: TileSize,

        #[bits(6)]
        pub bg3_tile_size: TileSize,

        #[bits(7)]
        pub bg4_tile_size: TileSize,
    }
}

register_mmio! {
    /// MOSAIC
    pub struct Mosaic @ 0x2106 {
        #[bits(0)]
        pub enable_bg1: bool,

        #[bits(1)]
        pub enable_bg2: bool,

        #[bits(2)]
        pub enable_bg3: bool,

        #[bits(3)]
        pub enable_bg4: bool,

        #[bits(4..=7)]
        pub size: u8,
    }
}

#[derive(BitEnum, Clone, Copy, Default)]
pub enum TilemapCount {
    #[fallback]
    #[default]
    One = 0,
    Two = 1,
}

impl TilemapCount {
    pub fn count(self) -> u8 {
        match self {
            Self::One => 1,
            Self::Two => 2,
        }
    }
}

register_mmio_family! {
    /// BGnSC
    pub struct TilemapAddrAndSize {
        #[bits(0)]
        pub horizontal_tilemap_count: TilemapCount,

        #[bits(1)]
        pub vertical_tilemap_count: TilemapCount,

        #[bits(2..=7)]
        pub tilemap_addr: u8,
    }
    symbols {
        0x2107 => "BG1SC",
        0x2108 => "BG2SC",
        0x2109 => "BG3SC",
        0x210A => "BG4SC",
    }
}

impl TilemapAddrAndSize {
    pub fn address(&self) -> u32 {
        (self.tilemap_addr() as u32) << 10
    }

    pub(crate) const START: u16 = 0x2107;
    pub(crate) const END: u16 = 0x210A;

    pub(crate) fn write<M: MemoryMap>(snes: &mut Snes<M>, addr: u16, value: u8) {
        let reg = Self::from_raw(value);
        let layer = &mut snes.ppu.bg_layers[(addr - Self::START) as usize];
        layer.tilemap_addr = reg.address();
        layer.horizontal_tilemap_count = reg.horizontal_tilemap_count();
        layer.vertical_tilemap_count = reg.vertical_tilemap_count();
    }

    pub(crate) fn peek<M: MemoryMap>(snes: &Snes<M>, addr: u16) -> u8 {
        let layer = &snes.ppu.bg_layers[(addr - Self::START) as usize];
        let mut reg = Self::zeroed();
        reg.set_tilemap_addr((layer.tilemap_addr >> 10) as u8);
        reg.set_horizontal_tilemap_count(layer.horizontal_tilemap_count);
        reg.set_vertical_tilemap_count(layer.vertical_tilemap_count);
        reg.raw()
    }
}

#[derive(Clone, Copy, Default)]
pub struct LayerMask(u8);

impl LayerMask {
    pub const OBJ: u8 = 4;
    pub const BACKDROP: u8 = 5;

    pub fn layer(self, index: u8) -> bool {
        self.0 & 1 << index != 0
    }

    pub fn bg(self, layer: usize) -> bool {
        self.layer(layer as u8)
    }

    pub fn obj(self) -> bool {
        self.layer(Self::OBJ)
    }

    pub fn raw(self) -> u8 {
        self.0
    }
}

impl From<u8> for LayerMask {
    fn from(value: u8) -> Self {
        Self(value & 0x3F)
    }
}

#[derive(BitEnum, Clone, Copy, Default)]
pub enum ColorMathOperation {
    #[fallback]
    #[default]
    Add = 0,
    Subtract = 1,
}

#[derive(BitEnum, Clone, Copy, Default)]
pub enum ColorMathIntensity {
    #[fallback]
    #[default]
    Full = 0,
    Half = 1,
}

#[derive(BitEnum, Clone, Copy, Default)]
pub enum ColorSource {
    #[fallback]
    #[default]
    Palette = 0,
    Direct = 1,
}

#[derive(BitEnum, Clone, Copy, Default, PartialEq, Eq)]
pub enum ColorMathAddend {
    #[fallback]
    #[default]
    FixedColor = 0,
    SubScreen = 1,
}

#[derive(BitEnum, Clone, Copy, Default)]
pub enum WindowRegion {
    #[fallback]
    #[default]
    Nowhere = 0,
    OutsideWindow = 1,
    InsideWindow = 2,
    Everywhere = 3,
}

impl WindowRegion {
    pub fn contains(self, in_window: bool) -> bool {
        match self {
            Self::Nowhere => false,
            Self::OutsideWindow => !in_window,
            Self::InsideWindow => in_window,
            Self::Everywhere => true,
        }
    }
}

register_mmio! {
    /// CGWSEL
    pub struct ColorMathSelect @ 0x2130 {
        #[bits(0)]
        pub color_source: ColorSource,

        #[bits(1)]
        pub addend: ColorMathAddend,

        #[bits(4..=5)]
        pub sub_screen_region: WindowRegion,

        #[bits(6..=7)]
        pub main_screen_region: WindowRegion,
    }
}

impl MmioReg for ColorMathSelect {
    fn write<M: MemoryMap>(snes: &mut Snes<M>, value: u8) {
        let reg = Self::from_raw(value);
        let math = &mut snes.ppu.color_math;
        math.color_source = reg.color_source();
        math.addend = reg.addend();
        math.sub_screen_region = reg.sub_screen_region();
        math.main_screen_region = reg.main_screen_region();
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>) -> u8 {
        let math = &snes.ppu.color_math;
        let mut reg = Self::default();
        reg.set_color_source(math.color_source);
        reg.set_addend(math.addend);
        reg.set_sub_screen_region(math.sub_screen_region);
        reg.set_main_screen_region(math.main_screen_region);
        reg.raw()
    }
}

register_mmio! {
    /// CGADSUB
    pub struct ColorMathControl @ 0x2131 {
        #[bits(0..=5)]
        pub layers: u8,

        #[bits(6)]
        pub intensity: ColorMathIntensity,

        #[bits(7)]
        pub operation: ColorMathOperation,
    }
}

impl MmioReg for ColorMathControl {
    fn write<M: MemoryMap>(snes: &mut Snes<M>, value: u8) {
        let reg = Self::from_raw(value);
        let math = &mut snes.ppu.color_math;
        math.layers = reg.layers().into();
        math.intensity = reg.intensity();
        math.operation = reg.operation();
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>) -> u8 {
        let math = &snes.ppu.color_math;
        let mut reg = Self::default();
        reg.set_layers(math.layers.raw());
        reg.set_intensity(math.intensity);
        reg.set_operation(math.operation);
        reg.raw()
    }
}

#[derive(BitEnum, Clone, Copy, Default)]
pub enum SpriteSize {
    #[fallback]
    #[default]
    Size8x8_16x16 = 0,
    Size8x8_32x32 = 1,
    Size8x8_64x64 = 2,
    Size16x16_32x32 = 3,
    Size16x16_64x64 = 4,
    Size32x32_64x64 = 5,
    Size16x32_32x64 = 6,
    Size16x32_32x32 = 7,
}

impl SpriteSize {
    pub fn small(self) -> (u32, u32) {
        match self {
            Self::Size8x8_16x16 | Self::Size8x8_32x32 | Self::Size8x8_64x64 => (8, 8),
            Self::Size16x16_32x32 | Self::Size16x16_64x64 => (16, 16),
            Self::Size32x32_64x64 => (32, 32),
            Self::Size16x32_32x64 | Self::Size16x32_32x32 => (16, 32),
        }
    }

    pub fn large(self) -> (u32, u32) {
        match self {
            Self::Size8x8_16x16 => (16, 16),
            Self::Size8x8_32x32 | Self::Size16x16_32x32 | Self::Size16x32_32x32 => (32, 32),
            Self::Size8x8_64x64 | Self::Size16x16_64x64 | Self::Size32x32_64x64 => (64, 64),
            Self::Size16x32_32x64 => (32, 64),
        }
    }

    pub fn dimensions(self, large: bool) -> (u32, u32) {
        if large { self.large() } else { self.small() }
    }
}

register_mmio! {
    /// OBJSEL
    pub struct SpriteSizeAndTileBase @ 0x2101 {
        #[bits(0..=2)]
        pub name_base_addr: u8,

        #[bits(3..=4)]
        pub name_select: u8,

        #[bits(5..=7)]
        pub sprite_size: SpriteSize,
    }
}

impl SpriteSizeAndTileBase {
    pub fn base_address(&self) -> u32 {
        (self.name_base_addr() as u32) << 13
    }

    pub fn offset_address(&self) -> u32 {
        (self.name_select() as u32) << 12
    }
}

impl MmioReg for SpriteSizeAndTileBase {
    fn write<M: MemoryMap>(snes: &mut Snes<M>, value: u8) {
        let reg = Self::from_raw(value);
        let obj = &mut snes.ppu.obj;
        obj.tileset_addr = reg.base_address();
        obj.name_offset = reg.offset_address();
        obj.sprite_size = reg.sprite_size();
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>) -> u8 {
        let obj = &snes.ppu.obj;
        let mut reg = Self::default();
        reg.set_name_base_addr((obj.tileset_addr >> 13) as u8);
        reg.set_name_select((obj.name_offset >> 12) as u8);
        reg.set_sprite_size(obj.sprite_size);
        reg.raw()
    }
}

impl MmioReg for BgMode {
    fn write<M: MemoryMap>(snes: &mut Snes<M>, value: u8) {
        let reg = Self::from_raw(value);
        snes.ppu.bg_mode = reg.mode();
        snes.ppu.bg3_priority = reg.mode1_bg3_priority();
        snes.ppu.bg_layers[0].tile_size = reg.bg1_tile_size();
        snes.ppu.bg_layers[1].tile_size = reg.bg2_tile_size();
        snes.ppu.bg_layers[2].tile_size = reg.bg3_tile_size();
        snes.ppu.bg_layers[3].tile_size = reg.bg4_tile_size();
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>) -> u8 {
        let mut reg = Self::default();
        reg.set_mode(snes.ppu.bg_mode);
        reg.set_mode1_bg3_priority(snes.ppu.bg3_priority);
        reg.set_bg1_tile_size(snes.ppu.bg_layers[0].tile_size);
        reg.set_bg2_tile_size(snes.ppu.bg_layers[1].tile_size);
        reg.set_bg3_tile_size(snes.ppu.bg_layers[2].tile_size);
        reg.set_bg4_tile_size(snes.ppu.bg_layers[3].tile_size);
        reg.raw()
    }
}

impl MmioReg for Bg12ChrBase {
    fn write<M: MemoryMap>(snes: &mut Snes<M>, value: u8) {
        let reg = Self::from_raw(value);
        snes.ppu.bg_layers[0].tileset_addr = (reg.bg1() as u32) << 12;
        snes.ppu.bg_layers[1].tileset_addr = (reg.bg2() as u32) << 12;
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>) -> u8 {
        let mut reg = Self::default();
        reg.set_bg1((snes.ppu.bg_layers[0].tileset_addr >> 12) as u8);
        reg.set_bg2((snes.ppu.bg_layers[1].tileset_addr >> 12) as u8);
        reg.raw()
    }
}

impl MmioReg for Bg34ChrBase {
    fn write<M: MemoryMap>(snes: &mut Snes<M>, value: u8) {
        let reg = Self::from_raw(value);
        snes.ppu.bg_layers[2].tileset_addr = (reg.bg3() as u32) << 12;
        snes.ppu.bg_layers[3].tileset_addr = (reg.bg4() as u32) << 12;
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>) -> u8 {
        let mut reg = Self::default();
        reg.set_bg3((snes.ppu.bg_layers[2].tileset_addr >> 12) as u8);
        reg.set_bg4((snes.ppu.bg_layers[3].tileset_addr >> 12) as u8);
        reg.raw()
    }
}

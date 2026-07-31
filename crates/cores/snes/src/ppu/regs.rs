use crate::mmio::device::{Access, IoRegister};
use chapa::bitenum;

use crate::Snes;
use crate::macros::{register_mmio, register_mmio_family, register_mmio_raw};
use crate::mmio::MemoryMap;
use crate::scheduler::VBLANK_START_SCANLINE;

pub(crate) const OAMADDL: u16 = 0x2102;
pub(crate) const OAMADDH: u16 = 0x2103;
pub(crate) const OAMDATA: u16 = 0x2104;
pub(crate) const OAMDATAREAD: u16 = 0x2138;

pub(crate) const CGADD: u16 = 0x2121;
pub(crate) const CGDATA: u16 = 0x2122;
pub(crate) const RDCGRAM: u16 = 0x213B;

pub(crate) const TM: u16 = 0x212C;
pub(crate) const TS: u16 = 0x212D;
pub(crate) const W12SEL: u16 = 0x2123;
pub(crate) const W34SEL: u16 = 0x2124;
pub(crate) const WOBJSEL: u16 = 0x2125;
pub(crate) const WH0: u16 = 0x2126;
pub(crate) const WH1: u16 = 0x2127;
pub(crate) const WH2: u16 = 0x2128;
pub(crate) const WH3: u16 = 0x2129;
pub(crate) const WBGLOG: u16 = 0x212A;
pub(crate) const WOBJLOG: u16 = 0x212B;
pub(crate) const TMW: u16 = 0x212E;
pub(crate) const TSW: u16 = 0x212F;
pub(crate) const COLDATA: u16 = 0x2132;

pub(crate) const VMADDL: u16 = 0x2116;
pub(crate) const VMADDH: u16 = 0x2117;
pub(crate) const VMDATAL: u16 = 0x2118;
pub(crate) const VMDATAH: u16 = 0x2119;
pub(crate) const RDVRAML: u16 = 0x2139;
pub(crate) const RDVRAMH: u16 = 0x213A;

pub(crate) const SLHV: u16 = 0x2137;
pub(crate) const OPHCT: u16 = 0x213C;
pub(crate) const OPVCT: u16 = 0x213D;

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
    SLHV => "SLHV",
    OPHCT => "OPHCT",
    OPVCT => "OPVCT",
    WH0 => "WH0",
    WH1 => "WH1",
    WH2 => "WH2",
    WH3 => "WH3",
    COLDATA => "COLDATA",
    0x210D => "BG1HOFS",
    0x210E => "BG1VOFS",
    0x210F => "BG2HOFS",
    0x2110 => "BG2VOFS",
    0x2111 => "BG3HOFS",
    0x2112 => "BG3VOFS",
    0x2113 => "BG4HOFS",
    0x2114 => "BG4VOFS",
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
        snes.ppu.matrix.write_scroll(reg, value);

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

#[bitenum]
#[derive(Default)]
pub enum IncrementMode {
    #[fallback]
    #[default]
    AfterLow = 0,
    AfterHigh = 1,
}

#[bitenum]
#[derive(Default)]
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

#[bitenum]
#[derive(Default)]
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
    /// INIDISP
    pub struct DisplayControl @ 0x2100 {
        #[bits(0..=3)]
        pub brightness: u8,

        #[bits(7)]
        pub force_blank: bool,
    }
}

impl IoRegister for DisplayControl {
    const START: u16 = Self::ADDRESS;

    fn write<M: MemoryMap>(snes: &mut Snes<M>, _reg: u16, value: u8) {
        let control = Self::from_raw(value);
        let was_blanked = snes.ppu.force_blank;
        snes.ppu.set_display_control(control);

        if was_blanked
            && !control.force_blank()
            && snes.scheduler.scanline_at(snes.scheduler.cycles()) == VBLANK_START_SCANLINE
        {
            snes.ppu.oam.reload();
        }
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>, _reg: u16) -> u8 {
        Self::default()
            .with_brightness(snes.ppu.brightness)
            .with_force_blank(snes.ppu.force_blank)
            .raw()
    }
}

register_mmio! {
    /// SETINI
    pub struct DisplayControl2 @ 0x2133 {
        #[bits(0)]
        pub interlace: bool,

        #[bits(1)]
        pub obj_interlace: bool,

        #[bits(2)]
        pub overscan: bool,

        #[bits(3)]
        pub pseudo_hires: bool,

        #[bits(6)]
        pub extbg: bool,

        #[bits(7)]
        pub external_sync: bool,
    }
}

impl IoRegister for DisplayControl2 {
    const START: u16 = Self::ADDRESS;

    fn write<M: MemoryMap>(snes: &mut Snes<M>, _reg: u16, value: u8) {
        let control = Self::from_raw(value);
        snes.ppu.pseudo_hires = control.pseudo_hires();
        snes.ppu.extbg = control.extbg();
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>, _reg: u16) -> u8 {
        Self::default()
            .with_pseudo_hires(snes.ppu.pseudo_hires)
            .with_extbg(snes.ppu.extbg)
            .raw()
    }
}

register_mmio! {
    /// STAT78
    pub struct Ppu2Status @ 0x213F {
        #[bits(4)]
        pub pal: bool,

        #[bits(6)]
        pub counter_latch: bool,
    }
}

impl IoRegister for Ppu2Status {
    const START: u16 = Self::ADDRESS;
    const WRITE: Access = Access::Ignored;

    fn read<M: MemoryMap>(snes: &mut Snes<M>, _reg: u16) -> u8 {
        let value = Self::peek(snes, _reg);
        snes.ppu.reset_counter_reads();
        if snes.mmu.latch_pin_high() {
            snes.ppu.counter_latch = false;
        }
        value
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>, _reg: u16) -> u8 {
        let mut status = Self::default();
        status.set_pal(snes.mmu.cartridge.region().tv_system() == crate::cartridge::TvSystem::Pal);
        status.set_counter_latch(!snes.mmu.latch_pin_high() || snes.ppu.counter_latch);
        status.raw()
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

#[bitenum]
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

    /// Modes 5 and 6 render at doubled (512px) horizontal resolution.
    pub fn is_hires(self) -> bool {
        matches!(self, Self::Mode5 | Self::Mode6)
    }
}

#[bitenum]
#[derive(Default, PartialEq, Eq)]
pub enum Priority {
    #[fallback]
    #[default]
    Normal = 0,
    High = 1,
}

#[bitenum]
#[derive(Default)]
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

#[bitenum]
#[derive(Default)]
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

register_mmio_family! {
    /// TMW / TSW
    #[derive(Clone, Copy, Default)]
    pub struct LayerWindowEnable {
        #[bits(0)]
        pub enabled_bg1: bool,

        #[bits(1)]
        pub enabled_bg2: bool,

        #[bits(2)]
        pub enabled_bg3: bool,

        #[bits(3)]
        pub enabled_bg4: bool,

        #[bits(4)]
        pub enabled_obj: bool,
    }
    symbols {
        0x212E => "TMW",
        0x212F => "TSW",
    }
}

register_mmio_family! {
    /// W12SEL / W34SEL / WOBJSEL
    #[derive(Clone, Copy, Default)]
    pub struct WindowMaskSettings {
        #[bits(0, alias = ["invert_win1_bg3", "invert_win1_obj"])]
        pub invert_win1_bg1: bool,

        #[bits(1, alias = ["enable_win1_bg3", "enable_win1_obj"])]
        pub enable_win1_bg1: bool,

        #[bits(2, alias = ["invert_win2_bg3", "invert_win2_obj"])]
        pub invert_win2_bg1: bool,

        #[bits(3, alias = ["enable_win2_bg3", "enable_win2_obj"])]
        pub enable_win2_bg1: bool,

        #[bits(4, alias = ["invert_win1_bg4", "invert_win1_color"])]
        pub invert_win1_bg2: bool,

        #[bits(5, alias = ["enable_win1_bg4", "enable_win1_color"])]
        pub enable_win1_bg2: bool,

        #[bits(6, alias = ["invert_win2_bg4", "invert_win2_color"])]
        pub invert_win2_bg2: bool,

        #[bits(7, alias = ["enable_win2_bg4", "enable_win2_color"])]
        pub enable_win2_bg2: bool,
    }
    symbols {
        0x2123 => "W12SEL",
        0x2124 => "W34SEL",
        0x2125 => "WOBJSEL",
    }
}

impl WindowMaskSettings {
    pub(crate) const START: u16 = W12SEL;
    pub(crate) const END: u16 = WOBJSEL;

    pub(crate) fn write<M: MemoryMap>(snes: &mut Snes<M>, addr: u16, value: u8) {
        snes.ppu.windows.write_selector(addr, Self::from_raw(value));
    }

    pub(crate) fn peek<M: MemoryMap>(snes: &Snes<M>, addr: u16) -> u8 {
        snes.ppu.windows.selector(addr).raw()
    }
}

impl LayerWindowEnable {
    pub(crate) const START: u16 = TMW;
    pub(crate) const END: u16 = TSW;

    pub(crate) fn enabled(&self, layer: usize) -> bool {
        match layer {
            0 => self.enabled_bg1(),
            1 => self.enabled_bg2(),
            2 => self.enabled_bg3(),
            3 => self.enabled_bg4(),
            4 => self.enabled_obj(),
            _ => false,
        }
    }

    pub(crate) fn write<M: MemoryMap>(snes: &mut Snes<M>, addr: u16, value: u8) {
        let reg = Self::from_raw(value & 0x1F);
        match addr {
            TMW => snes.ppu.main_screen_window = reg,
            TSW => snes.ppu.sub_screen_window = reg,
            _ => unreachable!(),
        }
    }

    pub(crate) fn peek<M: MemoryMap>(snes: &Snes<M>, addr: u16) -> u8 {
        match addr {
            TMW => snes.ppu.main_screen_window.raw(),
            TSW => snes.ppu.sub_screen_window.raw(),
            _ => unreachable!(),
        }
    }
}

#[bitenum]
#[derive(Default)]
pub enum MaskLogic {
    #[fallback]
    #[default]
    Or = 0,
    And = 1,
    Xor = 2,
    Xnor = 3,
}

register_mmio_family! {
    #[derive(Default)]
    pub struct WindowMaskLogic {
        #[bits(0..=1, alias = "obj")]
        pub bg1: MaskLogic,

        #[bits(2..=3, alias = "color")]
        pub bg2: MaskLogic,

        #[bits(4..=5)]
        pub bg3: MaskLogic,

        #[bits(6..=7)]
        pub bg4: MaskLogic,
    }
    symbols {
        0x212A => "WBGLOG",
        0x212B => "WOBJLOG",
    }
}

register_mmio_family! {
    #[derive(Default)]
    pub struct LayerMask {
        #[bits(0)]
        pub bg1: bool,

        #[bits(1)]
        pub bg2: bool,

        #[bits(2)]
        pub bg3: bool,

        #[bits(3)]
        pub bg4: bool,

        #[bits(4)]
        pub obj: bool,

        #[bits(5)]
        pub backdrop: bool,
    }
    symbols {
        0x212C => "TM",
        0x212D => "TS",
    }
}

impl LayerMask {
    pub const OBJ: u8 = 4;
    pub const BACKDROP: u8 = 5;

    pub fn layer(self, index: u8) -> bool {
        self.raw() & 1 << index != 0
    }

    pub fn bg(self, layer: usize) -> bool {
        self.layer(layer as u8)
    }

    pub fn from_bits(value: u8) -> Self {
        Self::from_raw(value & 0x3F)
    }
}

#[bitenum]
#[derive(Default)]
pub enum ColorMathOperation {
    #[fallback]
    #[default]
    Add = 0,
    Subtract = 1,
}

#[bitenum]
#[derive(Default)]
pub enum ColorMathIntensity {
    #[fallback]
    #[default]
    Full = 0,
    Half = 1,
}

#[bitenum]
#[derive(Default)]
pub enum ColorSource {
    #[fallback]
    #[default]
    Palette = 0,
    Direct = 1,
}

#[bitenum]
#[derive(Default, PartialEq, Eq)]
pub enum ColorMathAddend {
    #[fallback]
    #[default]
    FixedColor = 0,
    SubScreen = 1,
}

#[bitenum]
#[derive(Default)]
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

impl IoRegister for ColorMathSelect {
    const START: u16 = Self::ADDRESS;

    fn write<M: MemoryMap>(snes: &mut Snes<M>, _reg: u16, value: u8) {
        let reg = Self::from_raw(value);
        let math = &mut snes.ppu.color_math;
        math.color_source = reg.color_source();
        math.addend = reg.addend();
        math.sub_screen_region = reg.sub_screen_region();
        math.main_screen_region = reg.main_screen_region();
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>, _reg: u16) -> u8 {
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

impl IoRegister for ColorMathControl {
    const START: u16 = Self::ADDRESS;

    fn write<M: MemoryMap>(snes: &mut Snes<M>, _reg: u16, value: u8) {
        let reg = Self::from_raw(value);
        let math = &mut snes.ppu.color_math;
        math.layers = LayerMask::from_bits(reg.layers());
        math.intensity = reg.intensity();
        math.operation = reg.operation();
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>, _reg: u16) -> u8 {
        let math = &snes.ppu.color_math;
        let mut reg = Self::default();
        reg.set_layers(math.layers.raw());
        reg.set_intensity(math.intensity);
        reg.set_operation(math.operation);
        reg.raw()
    }
}

#[bitenum]
#[derive(Default)]
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

impl IoRegister for SpriteSizeAndTileBase {
    const START: u16 = Self::ADDRESS;

    fn write<M: MemoryMap>(snes: &mut Snes<M>, _reg: u16, value: u8) {
        let reg = Self::from_raw(value);
        let obj = &mut snes.ppu.obj;
        obj.tileset_addr = reg.base_address();
        obj.name_offset = reg.offset_address();
        obj.sprite_size = reg.sprite_size();
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>, _reg: u16) -> u8 {
        let obj = &snes.ppu.obj;
        let mut reg = Self::default();
        reg.set_name_base_addr((obj.tileset_addr >> 13) as u8);
        reg.set_name_select((obj.name_offset >> 12) as u8);
        reg.set_sprite_size(obj.sprite_size);
        reg.raw()
    }
}

impl IoRegister for BgMode {
    const START: u16 = Self::ADDRESS;

    fn write<M: MemoryMap>(snes: &mut Snes<M>, _reg: u16, value: u8) {
        let reg = Self::from_raw(value);
        snes.ppu.bg_mode = reg.mode();
        snes.ppu.bg3_priority = reg.mode1_bg3_priority();
        snes.ppu.bg_layers[0].tile_size = reg.bg1_tile_size();
        snes.ppu.bg_layers[1].tile_size = reg.bg2_tile_size();
        snes.ppu.bg_layers[2].tile_size = reg.bg3_tile_size();
        snes.ppu.bg_layers[3].tile_size = reg.bg4_tile_size();
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>, _reg: u16) -> u8 {
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

impl IoRegister for Bg12ChrBase {
    const START: u16 = Self::ADDRESS;

    fn write<M: MemoryMap>(snes: &mut Snes<M>, _reg: u16, value: u8) {
        let reg = Self::from_raw(value);
        snes.ppu.bg_layers[0].tileset_addr = (reg.bg1() as u32) << 12;
        snes.ppu.bg_layers[1].tileset_addr = (reg.bg2() as u32) << 12;
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>, _reg: u16) -> u8 {
        let mut reg = Self::default();
        reg.set_bg1((snes.ppu.bg_layers[0].tileset_addr >> 12) as u8);
        reg.set_bg2((snes.ppu.bg_layers[1].tileset_addr >> 12) as u8);
        reg.raw()
    }
}

impl IoRegister for Bg34ChrBase {
    const START: u16 = Self::ADDRESS;

    fn write<M: MemoryMap>(snes: &mut Snes<M>, _reg: u16, value: u8) {
        let reg = Self::from_raw(value);
        snes.ppu.bg_layers[2].tileset_addr = (reg.bg3() as u32) << 12;
        snes.ppu.bg_layers[3].tileset_addr = (reg.bg4() as u32) << 12;
    }

    fn peek<M: MemoryMap>(snes: &Snes<M>, _reg: u16) -> u8 {
        let mut reg = Self::default();
        reg.set_bg3((snes.ppu.bg_layers[2].tileset_addr >> 12) as u8);
        reg.set_bg4((snes.ppu.bg_layers[3].tileset_addr >> 12) as u8);
        reg.raw()
    }
}

crate::macros::impl_bitfield_serde!(
    IncrementMode,
    IncrementAmount,
    AddressRemapping,
    BackgroundMode,
    Priority,
    TileSize,
    TilemapCount,
    ColorMathOperation,
    ColorMathIntensity,
    ColorSource,
    ColorMathAddend,
    WindowRegion,
    SpriteSize,
);

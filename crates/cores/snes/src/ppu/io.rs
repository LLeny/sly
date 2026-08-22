use crate::SnesBus;
use crate::mmio::device::{Access, IoRegister, RegEntry};
use crate::mmio::{MemoryMap, OPEN_BUS};
use crate::ppu::mode7::{Mode7Registers, Mode7Settings, PRODUCT_HIGH, PRODUCT_LOW, PRODUCT_MIDDLE};
use crate::ppu::mosaic::MosaicRegister;
use crate::ppu::regs::{
    Bg12ChrBase, Bg34ChrBase, BgMode, BgScrollOffset, CGADD, CGDATA, COLDATA, ColorMathControl,
    ColorMathSelect, DisplayControl, DisplayControl2, LayerMask, LayerWindowEnable, OAMADDH,
    OAMADDL, OAMDATA, OAMDATAREAD, OPHCT, OPVCT, Ppu2Status, RDCGRAM, RDVRAMH, RDVRAML, SLHV,
    SpriteSizeAndTileBase, TM, TS, TilemapAddrAndSize, VMADDH, VMADDL, VMDATAH, VMDATAL,
    VramControl, WindowMaskSettings,
};
use crate::ppu::window::{LOGIC_END, LOGIC_START, POSITION_END, POSITION_START};

pub fn registers<M: MemoryMap>() -> Vec<RegEntry<M>> {
    vec![
        Inidisp::entry(),
        SpriteSizeAndTileBase::entry(),
        OamAddrLow::entry(),
        OamAddrHigh::entry(),
        OamData::entry(),
        BgMode::entry(),
        MosaicRegister::entry(),
        TilemapSetup::entry(),
        Bg12ChrBase::entry(),
        Bg34ChrBase::entry(),
        BgScroll::entry(),
        VramSetup::entry(),
        VramAddrLow::entry(),
        VramAddrHigh::entry(),
        VramDataLow::entry(),
        VramDataHigh::entry(),
        Mode7Settings::entry(),
        Mode7Matrix::entry(),
        CgramAddr::entry(),
        CgramData::entry(),
        WindowMask::entry(),
        WindowPosition::entry(),
        WindowLogic::entry(),
        MainScreen::entry(),
        SubScreen::entry(),
        WindowScreenMask::entry(),
        ColorMathCtrl::entry(),
        ColorMathSel::entry(),
        FixedColor::entry(),
        ScreenMode::entry(),
        ProductLow::entry(),
        ProductMid::entry(),
        ProductHigh::entry(),
        CounterLatch::entry(),
        OamDataRead::entry(),
        VramReadLow::entry(),
        VramReadHigh::entry(),
        CgramRead::entry(),
        HCounter::entry(),
        VCounter::entry(),
        Ppu2Status::entry(),
    ]
}

struct Inidisp;

impl IoRegister for Inidisp {
    const START: u16 = DisplayControl::ADDRESS;
    const READ: Access = Access::Unmapped;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, reg: u16, value: u8) {
        DisplayControl::write(snes, reg, value);
    }
}

struct OamAddrLow;

impl IoRegister for OamAddrLow {
    const START: u16 = OAMADDL;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16, value: u8) {
        snes.ppu.oam.set_addr_low(value);
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        snes.ppu.oam.word_addr() as u8
    }
}

struct OamAddrHigh;

impl IoRegister for OamAddrHigh {
    const START: u16 = OAMADDH;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16, value: u8) {
        snes.ppu.oam.set_addr_high(value);
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        (snes.ppu.oam.word_addr() >> 8) as u8
    }
}

struct OamData;

impl IoRegister for OamData {
    const START: u16 = OAMDATA;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16, value: u8) {
        snes.ppu.oam.write_data(value);
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        snes.ppu.oam.peek_data()
    }
}

struct OamDataRead;

impl IoRegister for OamDataRead {
    const START: u16 = OAMDATAREAD;
    const WRITE: Access = Access::Ignored;

    fn read<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16) -> u8 {
        snes.ppu.oam.read_data()
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        snes.ppu.oam.peek_data()
    }
}

struct TilemapSetup;

impl IoRegister for TilemapSetup {
    const START: u16 = TilemapAddrAndSize::START;
    const END: u16 = TilemapAddrAndSize::END;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, reg: u16, value: u8) {
        TilemapAddrAndSize::write(snes, reg, value);
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, reg: u16) -> u8 {
        TilemapAddrAndSize::peek(snes, reg)
    }
}

struct BgScroll;

impl IoRegister for BgScroll {
    const START: u16 = BgScrollOffset::START;
    const END: u16 = BgScrollOffset::END;
    const READ: Access = Access::Unmapped;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, reg: u16, value: u8) {
        BgScrollOffset::write(snes, reg, value);
    }
}

struct VramSetup;

impl IoRegister for VramSetup {
    const START: u16 = VramControl::ADDRESS;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16, value: u8) {
        snes.ppu.vram.set_control(value);
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        snes.ppu.vram.control().raw()
    }
}

struct VramAddrLow;

impl IoRegister for VramAddrLow {
    const START: u16 = VMADDL;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16, value: u8) {
        snes.ppu.vram.set_addr_low(value);
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        snes.ppu.vram.word_addr() as u8
    }
}

struct VramAddrHigh;

impl IoRegister for VramAddrHigh {
    const START: u16 = VMADDH;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16, value: u8) {
        snes.ppu.vram.set_addr_high(value);
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        (snes.ppu.vram.word_addr() >> 8) as u8
    }
}

struct VramDataLow;

impl IoRegister for VramDataLow {
    const START: u16 = VMDATAL;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16, value: u8) {
        snes.ppu.vram.write_low(value);
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        snes.ppu.vram.peek_low()
    }
}

struct VramDataHigh;

impl IoRegister for VramDataHigh {
    const START: u16 = VMDATAH;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16, value: u8) {
        snes.ppu.vram.write_high(value);
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        snes.ppu.vram.peek_high()
    }
}

struct VramReadLow;

impl IoRegister for VramReadLow {
    const START: u16 = RDVRAML;
    const WRITE: Access = Access::Ignored;

    fn read<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16) -> u8 {
        snes.ppu.vram.read_low()
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        snes.ppu.vram.peek_low()
    }
}

struct VramReadHigh;

impl IoRegister for VramReadHigh {
    const START: u16 = RDVRAMH;
    const WRITE: Access = Access::Ignored;

    fn read<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16) -> u8 {
        snes.ppu.vram.read_high()
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        snes.ppu.vram.peek_high()
    }
}

struct Mode7Matrix;

impl IoRegister for Mode7Matrix {
    const START: u16 = Mode7Registers::START;
    const END: u16 = Mode7Registers::END;
    const READ: Access = Access::Unmapped;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, reg: u16, value: u8) {
        Mode7Registers::write(snes, reg, value);
    }
}

struct CgramAddr;

impl IoRegister for CgramAddr {
    const START: u16 = CGADD;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16, value: u8) {
        snes.ppu.cgram.set_addr(value);
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        snes.ppu.cgram.word_addr()
    }
}

struct CgramData;

impl IoRegister for CgramData {
    const START: u16 = CGDATA;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16, value: u8) {
        snes.ppu.cgram.write_data(value);
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        snes.ppu.cgram.peek_data()
    }
}

struct CgramRead;

impl IoRegister for CgramRead {
    const START: u16 = RDCGRAM;
    const WRITE: Access = Access::Ignored;

    fn read<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16) -> u8 {
        snes.ppu.cgram.read_data()
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        snes.ppu.cgram.peek_data()
    }
}

struct WindowMask;

impl IoRegister for WindowMask {
    const START: u16 = WindowMaskSettings::START;
    const END: u16 = WindowMaskSettings::END;
    const READ: Access = Access::Unmapped;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, reg: u16, value: u8) {
        WindowMaskSettings::write(snes, reg, value);
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, reg: u16) -> u8 {
        WindowMaskSettings::peek(snes, reg)
    }
}

struct WindowPosition;

impl IoRegister for WindowPosition {
    const START: u16 = POSITION_START;
    const END: u16 = POSITION_END;
    const READ: Access = Access::Unmapped;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, reg: u16, value: u8) {
        snes.ppu.windows.write_position(reg, value);
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, reg: u16) -> u8 {
        snes.ppu.windows.position(reg)
    }
}

struct WindowLogic;

impl IoRegister for WindowLogic {
    const START: u16 = LOGIC_START;
    const END: u16 = LOGIC_END;
    const READ: Access = Access::Unmapped;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, reg: u16, value: u8) {
        snes.ppu.windows.write_logic(reg, value);
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, reg: u16) -> u8 {
        snes.ppu.windows.logic(reg)
    }
}

struct MainScreen;

impl IoRegister for MainScreen {
    const START: u16 = TM;
    const READ: Access = Access::Unmapped;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16, value: u8) {
        snes.ppu.main_screen = LayerMask::from_bits(value);
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        snes.ppu.main_screen.raw()
    }
}

struct SubScreen;

impl IoRegister for SubScreen {
    const START: u16 = TS;
    const READ: Access = Access::Unmapped;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16, value: u8) {
        snes.ppu.sub_screen = LayerMask::from_bits(value);
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        snes.ppu.sub_screen.raw()
    }
}

struct WindowScreenMask;

impl IoRegister for WindowScreenMask {
    const START: u16 = LayerWindowEnable::START;
    const END: u16 = LayerWindowEnable::END;
    const READ: Access = Access::Unmapped;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, reg: u16, value: u8) {
        LayerWindowEnable::write(snes, reg, value);
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, reg: u16) -> u8 {
        LayerWindowEnable::peek(snes, reg)
    }
}

struct ColorMathCtrl;

impl IoRegister for ColorMathCtrl {
    const START: u16 = ColorMathControl::ADDRESS;
    const READ: Access = Access::Unmapped;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, reg: u16, value: u8) {
        ColorMathControl::write(snes, reg, value);
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, reg: u16) -> u8 {
        ColorMathControl::peek(snes, reg)
    }
}

struct ColorMathSel;

impl IoRegister for ColorMathSel {
    const START: u16 = ColorMathSelect::ADDRESS;
    const READ: Access = Access::Unmapped;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, reg: u16, value: u8) {
        ColorMathSelect::write(snes, reg, value);
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, reg: u16) -> u8 {
        ColorMathSelect::peek(snes, reg)
    }
}

struct FixedColor;

impl IoRegister for FixedColor {
    const START: u16 = COLDATA;
    const READ: Access = Access::Unmapped;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16, value: u8) {
        snes.ppu.color_math.set_fixed_color(value);
    }
}

struct ScreenMode;

impl IoRegister for ScreenMode {
    const START: u16 = DisplayControl2::ADDRESS;
    const READ: Access = Access::Unmapped;

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, reg: u16, value: u8) {
        DisplayControl2::write(snes, reg, value);
    }
}

struct ProductLow;

impl IoRegister for ProductLow {
    const START: u16 = PRODUCT_LOW;

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        snes.ppu.matrix.result as u8
    }
}

struct ProductMid;

impl IoRegister for ProductMid {
    const START: u16 = PRODUCT_MIDDLE;

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        (snes.ppu.matrix.result >> 8) as u8
    }
}

struct ProductHigh;

impl IoRegister for ProductHigh {
    const START: u16 = PRODUCT_HIGH;

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        (snes.ppu.matrix.result >> 16) as u8
    }
}

struct CounterLatch;

impl IoRegister for CounterLatch {
    const START: u16 = SLHV;
    const WRITE: Access = Access::Ignored;

    fn read<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16) -> u8 {
        if snes.mmu.latch_pin_high() {
            crate::ppu::latch_hv_counters(snes);
        }
        OPEN_BUS
    }
}

struct HCounter;

impl IoRegister for HCounter {
    const START: u16 = OPHCT;
    const WRITE: Access = Access::Ignored;

    fn read<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16) -> u8 {
        snes.ppu.read_h_counter()
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        snes.ppu.peek_h_counter()
    }
}

struct VCounter;

impl IoRegister for VCounter {
    const START: u16 = OPVCT;
    const WRITE: Access = Access::Ignored;

    fn read<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16) -> u8 {
        snes.ppu.read_v_counter()
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        snes.ppu.peek_v_counter()
    }
}

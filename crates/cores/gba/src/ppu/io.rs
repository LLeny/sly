use super::regs::*;
use super::{OAM_BASE, OAM_END, PALETTE_BASE, PALETTE_END, VRAM_BASE, VRAM_END};
use crate::mmio::{Handler, MmioMap, Register};

const DISPSTAT: Register<DisplayStatus> = Register::new(
    0x0400_0004,
    "DISPSTAT",
    "PPU",
    |bus| bus.ppu.dispstat,
    |bus| &mut bus.ppu.dispstat,
)
.read_mask(DisplayStatus::READ_MASK)
.write_mask(DisplayStatus::WRITE_MASK)
.write_with(|bus, value, mask| {
    let status = &mut bus.ppu.dispstat;
    *status = DisplayStatus::from_raw((status.raw() & !mask) | (value.raw() & mask));
    status.set_vcounter(status.vcount_compare() == bus.ppu.vcount.scanline());
});

const DISPCTL: Register<DisplayControl> = Register::new(
    0x0400_0000,
    "DISPCTL",
    "PPU",
    |bus| bus.ppu.dispctrl,
    |bus| &mut bus.ppu.dispctrl,
);

const VCOUNT: Register<VerticalCount> = Register::new(
    0x0400_0006,
    "VCOUNT",
    "PPU",
    |bus| bus.ppu.vcount,
    |bus| &mut bus.ppu.vcount,
)
.read_mask(VerticalCount::SCANLINE_MASK)
.read_only();

const fn bgcnt<const I: usize>(address: u32, name: &'static str) -> Register<BackgroundControl> {
    Register::new(
        address,
        name,
        "PPU",
        |bus| bus.ppu.bgcnt[I],
        |bus| &mut bus.ppu.bgcnt[I],
    )
    .write_mask(BackgroundControl::WRITE_MASK)
}

const BG0CNT: Register<BackgroundControl> = bgcnt::<0>(0x0400_0008, "BG0CNT");
const BG1CNT: Register<BackgroundControl> = bgcnt::<1>(0x0400_000A, "BG1CNT");
const BG2CNT: Register<BackgroundControl> = bgcnt::<2>(0x0400_000C, "BG2CNT");
const BG3CNT: Register<BackgroundControl> = bgcnt::<3>(0x0400_000E, "BG3CNT");

const fn bgofs<const I: usize, const VERTICAL: bool>(
    address: u32,
    name: &'static str,
) -> Register<BackgroundOffset> {
    Register::new(
        address,
        name,
        "PPU",
        |bus| {
            if VERTICAL {
                bus.ppu.bgvofs[I]
            } else {
                bus.ppu.bghofs[I]
            }
        },
        |bus| {
            if VERTICAL {
                &mut bus.ppu.bgvofs[I]
            } else {
                &mut bus.ppu.bghofs[I]
            }
        },
    )
    .read_mask(0)
    .write_mask(BackgroundOffset::OFFSET_MASK)
}

const BG0HOFS: Register<BackgroundOffset> = bgofs::<0, false>(0x0400_0010, "BG0HOFS");
const BG0VOFS: Register<BackgroundOffset> = bgofs::<0, true>(0x0400_0012, "BG0VOFS");
const BG1HOFS: Register<BackgroundOffset> = bgofs::<1, false>(0x0400_0014, "BG1HOFS");
const BG1VOFS: Register<BackgroundOffset> = bgofs::<1, true>(0x0400_0016, "BG1VOFS");
const BG2HOFS: Register<BackgroundOffset> = bgofs::<2, false>(0x0400_0018, "BG2HOFS");
const BG2VOFS: Register<BackgroundOffset> = bgofs::<2, true>(0x0400_001A, "BG2VOFS");
const BG3HOFS: Register<BackgroundOffset> = bgofs::<3, false>(0x0400_001C, "BG3HOFS");
const BG3VOFS: Register<BackgroundOffset> = bgofs::<3, true>(0x0400_001E, "BG3VOFS");

const fn winh<const I: usize>(address: u32, name: &'static str) -> Register<WindowXOffset> {
    Register::new(
        address,
        name,
        "PPU",
        |bus| bus.ppu.winh[I],
        |bus| &mut bus.ppu.winh[I],
    )
    .read_mask(0)
}

const fn winv<const I: usize>(address: u32, name: &'static str) -> Register<WindowYOffset> {
    Register::new(
        address,
        name,
        "PPU",
        |bus| bus.ppu.winv[I],
        |bus| &mut bus.ppu.winv[I],
    )
    .read_mask(0)
}

const WIN0H: Register<WindowXOffset> = winh::<0>(0x0400_0040, "WIN0H");
const WIN1H: Register<WindowXOffset> = winh::<1>(0x0400_0042, "WIN1H");
const WIN0V: Register<WindowYOffset> = winv::<0>(0x0400_0044, "WIN0V");
const WIN1V: Register<WindowYOffset> = winv::<1>(0x0400_0046, "WIN1V");

const WININ: Register<WindowControl> = Register::new(
    0x0400_0048,
    "WININ",
    "PPU",
    |bus| bus.ppu.winin,
    |bus| &mut bus.ppu.winin,
)
.read_mask(0x3F3F)
.write_mask(0x3F3F);

const WINOUT: Register<WindowControl> = Register::new(
    0x0400_004A,
    "WINOUT",
    "PPU",
    |bus| bus.ppu.winout,
    |bus| &mut bus.ppu.winout,
)
.read_mask(0x3F3F)
.write_mask(0x3F3F);

const PALETTE: Handler = Handler {
    read: |bus, address, width| bus.ppu.read_palette(address, width),
    write: |bus, address, width, value| bus.ppu.write_palette(address, width, value),
    peek: |bus, address, width| bus.ppu.read_palette(address, width),
};

const VRAM: Handler = Handler {
    read: |bus, address, width| bus.ppu.read_vram(address, width),
    write: |bus, address, width, value| bus.ppu.write_vram(address, width, value),
    peek: |bus, address, width| bus.ppu.read_vram(address, width),
};

const OAM: Handler = Handler {
    read: |bus, address, width| bus.ppu.read_oam(address, width),
    write: |bus, address, width, value| bus.ppu.write_oam(address, width, value),
    peek: |bus, address, width| bus.ppu.read_oam(address, width),
};

pub(crate) fn register(map: &mut MmioMap) {
    map.register(&DISPCTL);
    map.register(&DISPSTAT);
    map.register(&VCOUNT);
    map.register(&BG0CNT);
    map.register(&BG1CNT);
    map.register(&BG2CNT);
    map.register(&BG3CNT);
    map.register(&BG0HOFS);
    map.register(&BG0VOFS);
    map.register(&BG1HOFS);
    map.register(&BG1VOFS);
    map.register(&BG2HOFS);
    map.register(&BG2VOFS);
    map.register(&BG3HOFS);
    map.register(&BG3VOFS);
    map.register(&WIN0H);
    map.register(&WIN1H);
    map.register(&WIN0V);
    map.register(&WIN1V);
    map.register(&WININ);
    map.register(&WINOUT);
    map.map(PALETTE_BASE..=PALETTE_END, PALETTE);
    map.map(VRAM_BASE..=VRAM_END, VRAM);
    map.map(OAM_BASE..=OAM_END, OAM);
}

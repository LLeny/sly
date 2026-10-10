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
    map.map(PALETTE_BASE..=PALETTE_END, PALETTE);
    map.map(VRAM_BASE..=VRAM_END, VRAM);
    map.map(OAM_BASE..=OAM_END, OAM);
}

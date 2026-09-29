use super::regs::{DisplayStatus, VerticalCount};
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

const VCOUNT: Register<VerticalCount> = Register::new(
    0x0400_0006,
    "VCOUNT",
    "PPU",
    |bus| bus.ppu.vcount,
    |bus| &mut bus.ppu.vcount,
)
.read_mask(VerticalCount::SCANLINE_MASK)
.read_only();

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
    map.register(&DISPSTAT);
    map.register(&VCOUNT);
    map.map(0x0500_0000..=0x05FF_FFFF, PALETTE);
    map.map(0x0600_0000..=0x06FF_FFFF, VRAM);
    map.map(0x0700_0000..=0x07FF_FFFF, OAM);
}

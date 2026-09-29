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

const HANDLER: Handler = Handler {
    read: |bus, address, width| bus.ppu.read(address, width),
    write: |bus, address, width, value| bus.ppu.write(address, width, value),
    peek: |bus, address, width| bus.ppu.peek(address, width),
};

pub(crate) fn register(map: &mut MmioMap) {
    map.register(&DISPSTAT);
    map.register(&VCOUNT);
    map.map(0x0500_0000..=0x07FF_FFFF, HANDLER);
}

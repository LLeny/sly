use super::regs::{Address, Control, WordCount};
use crate::mmio::{MmioMap, Register};

const fn source<const CHANNEL: usize>() -> Register<Address> {
    Register::new(
        0x0400_00B0 + ((CHANNEL as u32) * 12),
        ["DMA0SAD", "DMA1SAD", "DMA2SAD", "DMA3SAD"][CHANNEL],
        "DMA",
        |bus| bus.dma.src_addr[CHANNEL],
        |bus| &mut bus.dma.src_addr[CHANNEL],
    )
    .read_mask(0)
    .write_mask(Address::SOURCE_MASKS[CHANNEL])
}

const fn destination<const CHANNEL: usize>() -> Register<Address> {
    Register::new(
        0x0400_00B4 + ((CHANNEL as u32) * 12),
        ["DMA0DAD", "DMA1DAD", "DMA2DAD", "DMA3DAD"][CHANNEL],
        "DMA",
        |bus| bus.dma.dest_addr[CHANNEL],
        |bus| &mut bus.dma.dest_addr[CHANNEL],
    )
    .read_mask(0)
    .write_mask(Address::DESTINATION_MASKS[CHANNEL])
}

const fn count<const CHANNEL: usize>() -> Register<WordCount> {
    Register::new(
        0x0400_00B8 + ((CHANNEL as u32) * 12),
        ["DMA0CNT_L", "DMA1CNT_L", "DMA2CNT_L", "DMA3CNT_L"][CHANNEL],
        "DMA",
        |bus| bus.dma.word_count[CHANNEL],
        |bus| &mut bus.dma.word_count[CHANNEL],
    )
    .read_mask(0)
    .write_mask(if CHANNEL == 3 { 0xFFFF } else { 0x3FFF })
}

const fn control<const CHANNEL: usize>() -> Register<Control> {
    Register::new(
        0x0400_00BA + ((CHANNEL as u32) * 12),
        ["DMA0CNT_H", "DMA1CNT_H", "DMA2CNT_H", "DMA3CNT_H"][CHANNEL],
        "DMA",
        |bus| bus.dma.control[CHANNEL],
        |bus| &mut bus.dma.control[CHANNEL],
    )
    .read_mask(control_mask(CHANNEL))
    .write_mask(control_mask(CHANNEL))
    .write_with(|bus, value, mask| {
        let old = bus.dma.control[CHANNEL].raw();
        let control = Control::from_raw((old & !mask) | (value.raw() & mask));
        bus.dma.write_control(CHANNEL, control);
    })
}

pub(super) const fn control_mask(channel: usize) -> u16 {
    if channel == 3 { 0xFFE0 } else { 0xF7E0 }
}

#[rustfmt::skip]
const SOURCES: [Register<Address>; 4] = [
    source::<0>(),
    source::<1>(),
    source::<2>(),
    source::<3>()
];

#[rustfmt::skip]
const DESTINATIONS: [Register<Address>; 4] = [
    destination::<0>(),
    destination::<1>(),
    destination::<2>(),
    destination::<3>(),
];

#[rustfmt::skip]
const COUNTS: [Register<WordCount>; 4] = [
    count::<0>(),
    count::<1>(),
    count::<2>(),
    count::<3>()
];

#[rustfmt::skip]
const CONTROLS: [Register<Control>; 4] = [
    control::<0>(),
    control::<1>(),
    control::<2>(),
    control::<3>(),
];

pub(crate) fn register(map: &mut MmioMap) {
    for channel in 0..4 {
        map.register(&SOURCES[channel]);
        map.register(&DESTINATIONS[channel]);
        map.register(&COUNTS[channel]);
        map.register(&CONTROLS[channel]);
    }
}

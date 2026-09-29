use super::ROM_BASE;
use crate::mmio::{Handler, MmioMap};

const HANDLER: Handler = Handler {
    read: |bus, address, width| bus.gamepak.read(address, width),
    write: |bus, address, width, value| bus.gamepak.write(address, width, value),
    peek: |bus, address, width| bus.gamepak.peek(address, width),
};

pub(crate) fn register(map: &mut MmioMap) {
    map.map(ROM_BASE..=0x0FFF_FFFF, HANDLER);
}

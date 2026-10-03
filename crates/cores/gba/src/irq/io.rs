use crate::irq::regs::{InterruptEnable, InterruptMasterEnable, InterruptRequest};
use crate::mmio::{MmioMap, Register};

const IME: Register<InterruptMasterEnable> = Register::new(
    0x0400_0208,
    "IME",
    "IRQ",
    |bus| bus.irq.ime,
    |bus| &mut bus.irq.ime,
);

const IE: Register<InterruptEnable> = Register::new(
    0x0400_0200,
    "IE",
    "IRQ",
    |bus| bus.irq.interrupt_enable,
    |bus| &mut bus.irq.interrupt_enable,
);

const IF: Register<InterruptRequest> = Register::new(
    0x0400_0202,
    "IF",
    "IRQ",
    |bus| bus.irq.interrupt_request,
    |bus| &mut bus.irq.interrupt_request,
)
.write_with(|bus, value, mask| {
    bus.irq.interrupt_request = bus.irq.interrupt_request & !(value & mask);
});

pub(crate) fn register(map: &mut MmioMap) {
    map.register(&IME);
    map.register(&IE);
    map.register(&IF);
}

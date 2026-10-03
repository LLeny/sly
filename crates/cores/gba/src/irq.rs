pub(crate) mod io;
pub(crate) mod regs;

use regs::{InterruptEnable, InterruptMasterEnable, InterruptRequest};

pub struct Irq {
    pub ime: InterruptMasterEnable,
    pub interrupt_enable: InterruptEnable,
    pub interrupt_request: InterruptRequest,
}

impl Irq {
    pub(crate) fn pending(&self) -> bool {
        self.ime.enable_interrupts()
            && (self.interrupt_enable.raw() & self.interrupt_request.raw() & 0x3FFF) != 0
    }

    pub fn new() -> Self {
        Self {
            ime: InterruptMasterEnable::zeroed(),
            interrupt_enable: InterruptEnable::zeroed(),
            interrupt_request: InterruptRequest::zeroed(),
        }
    }
}

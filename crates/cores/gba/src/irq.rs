pub(crate) mod io;
pub(crate) mod regs;

use regs::{InterruptEnable, InterruptMasterEnable, InterruptRequest};

pub(crate) struct Irq {
    pub ime: InterruptMasterEnable,
    pub interrupt_enable: InterruptEnable,
    pub interrupt_request: InterruptRequest,
}

impl Irq {
    pub fn new() -> Self {
        Self {
            ime: InterruptMasterEnable::zeroed(),
            interrupt_enable: InterruptEnable::zeroed(),
            interrupt_request: InterruptRequest::zeroed(),
        }
    }
}

use crate::mmio::regs::NmiTimerEnable;

#[derive(Default)]
pub struct Irq {
    pub nmitimen: NmiTimerEnable,
}

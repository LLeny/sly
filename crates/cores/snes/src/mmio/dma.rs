use crate::mmio::regs::HdmaTransferEnable;

#[derive(Default)]
pub struct Dma {
    pub hdmaen: HdmaTransferEnable,
}

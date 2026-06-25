use crate::mmio::regs::{DmaTransferStart, HdmaTransferEnable};

#[derive(Default)]
pub struct Dma {
    pub hdmaen: HdmaTransferEnable,
    pub mdmaen: DmaTransferStart,
}

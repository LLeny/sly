mod mmio;

use std::sync::Arc;

use debugger_model::{
    BreakpointMark, CartridgeView, CpuDescriptor, CpuId, CpuView, DisasmLine, DisassemblyRequest,
    Highlight, MemoryRegion, MmioView, RunOutcome,
};

use crate::{Cartridge, Gba};

pub struct GbaDebug {
    core: Gba,
}

impl GbaDebug {
    pub fn new(cartridge: Cartridge, _name: String, _cover: Option<Arc<str>>) -> Self {
        Self {
            core: Gba::new(cartridge),
        }
    }
}

impl core_model::Core for GbaDebug {
    fn software_frame(&self) -> Option<core_model::SoftwareFrame<'_>> {
        self.core.software_frame()
    }

    fn reset(&mut self) {
        self.core.reset();
    }

    fn step(&mut self) {
        self.core.step();
    }

    fn run_frame(&mut self) {
        self.core.run_frame();
    }
}

impl debugger_model::DebugCore for GbaDebug {
    fn cpus(&self) -> Vec<CpuDescriptor> {
        todo!()
    }

    fn pc(&self, _cpu: &CpuId) -> u32 {
        todo!()
    }

    fn set_pc(&mut self, _cpu: &CpuId, _addr: u32) {
        todo!()
    }

    fn cpu_view(&self, _cpu: &CpuId) -> CpuView {
        todo!()
    }

    fn disassemble(&self, _cpu: &CpuId, _request: &DisassemblyRequest) -> Vec<DisasmLine> {
        todo!()
    }

    fn debug_step(&mut self) {
        todo!()
    }

    fn debug_run_frame(&mut self, _breakpoints: &[BreakpointMark]) -> RunOutcome {
        todo!()
    }

    fn cartridge(&self) -> &CartridgeView {
        todo!()
    }

    fn mmio_view(&self) -> MmioView {
        mmio::view(&self.core)
    }

    fn caption(&self, _label: &str) -> String {
        todo!()
    }

    fn memory_regions(&self) -> Vec<MemoryRegion> {
        todo!()
    }

    fn peek_memory(&self, _region: &str, _addr: u32, _len: usize) -> Vec<u8> {
        todo!()
    }

    fn format_address(&self, _region: &str, _addr: u32) -> String {
        todo!()
    }

    fn memory_highlights(&self, _region: &str, _addr: u32, _len: usize) -> Vec<Highlight> {
        todo!()
    }

    fn latest_access(&self) -> Option<(Arc<str>, u32)> {
        todo!()
    }
}

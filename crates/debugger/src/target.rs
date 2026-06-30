use cirsium::Highlight;
use cirsium::components::StatusView;
use cirsium::widgets::{CartridgeView, CpuView, DisassemblyView, MemoryRegion};
use gpui::SharedString;

use crate::breakpoints::BreakpointMark;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    Done,
    Breakpoint,
    Fault,
}

pub trait DebugTarget {
    fn pc(&self) -> u32;
    fn set_pc(&mut self, addr: u32);
    fn step(&mut self) -> bool;
    fn run_frame(&mut self, breakpoints: &[BreakpointMark]) -> RunOutcome;
    fn reset(&mut self);
    fn cpu_view(&self) -> CpuView;
    fn disassemble(&self, base: u32, count: usize) -> DisassemblyView;

    fn has_spc(&self) -> bool {
        false
    }
    fn spc_pc(&self) -> u32 {
        0
    }
    fn spc_set_pc(&mut self, _addr: u32) {}
    fn spc_cpu_view(&self) -> CpuView {
        CpuView::default()
    }
    fn spc_disassemble(&self, _base: u32, _count: usize) -> DisassemblyView {
        DisassemblyView::default()
    }

    fn cartridge(&self) -> &CartridgeView;
    fn status(&self, label: &str) -> StatusView;
    fn caption(&self, label: &str) -> SharedString;
    fn memory_regions(&self) -> Vec<MemoryRegion>;
    fn read_memory(&self, region: &str, addr: u32, len: usize) -> Vec<u8>;
    fn format_address(&self, region: &str, addr: u32) -> SharedString;
    fn memory_highlights(&self, region: &str, addr: u32, len: usize) -> Vec<Highlight>;
    fn latest_access(&self) -> Option<(SharedString, u32)>;
    fn annotate(&self, _addr: u32) -> SharedString {
        SharedString::default()
    }
}

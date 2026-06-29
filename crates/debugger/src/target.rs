use cirsium::components::StatusView;
use cirsium::widgets::{CartridgeView, CpuView, DisassemblyView};
use gpui::SharedString;

pub trait DebugTarget {
    fn pc(&self) -> u32;
    fn set_pc(&mut self, addr: u32);
    fn step(&mut self) -> bool;
    fn run_frame(&mut self) -> bool;
    fn reset(&mut self);
    fn cpu_view(&self) -> CpuView;
    fn disassemble(&self, base: u32, count: usize) -> DisassemblyView;
    fn cartridge(&self) -> &CartridgeView;
    fn status(&self, label: &str) -> StatusView;
    fn caption(&self, label: &str) -> SharedString;
}

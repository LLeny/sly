mod cpu;
mod disasm;
mod mmio;

use std::ops::RangeInclusive;
use std::sync::Arc;
use std::time::Duration;

use core_model::System;
use debugger_model::{
    BreakpointKind, BreakpointMark, CartridgeView, CpuDescriptor, CpuId, CpuView, DisasmLine,
    DisassemblyRequest, Field, Highlight, HookHit, MemoryRegion, MmioView, Row, RunOutcome,
    Section, StructuredView,
};
use workspace_model::{ComponentManifest, DockLocation};

use crate::{Cartridge, Cpu, Gba};

const CPU: &str = "gba.arm7tdmi";
const SPACE: &str = "gba.bus";
const ROM: &str = "gba.rom";
const RUN_BUDGET: usize = 1_000_000;

const REGIONS: [(&str, &str, u32, u32); 10] = [
    (SPACE, "CPU Bus", 0, 0x1000_0000),
    ("gba.bios", "BIOS", 0, 0x4000),
    ("gba.ewram", "EWRAM", 0x0200_0000, 0x4_0000),
    ("gba.iwram", "IWRAM", 0x0300_0000, 0x8000),
    ("gba.io", "I/O", 0x0400_0000, 0x400),
    ("gba.palette", "Palette", 0x0500_0000, 0x400),
    ("gba.vram", "VRAM", 0x0600_0000, 0x1_8000),
    ("gba.oam", "OAM", 0x0700_0000, 0x400),
    (ROM, "ROM", 0x0800_0000, 0x0200_0000),
    ("gba.sram", "SRAM", 0x0E00_0000, 0x1_0000),
];

pub struct GbaDebug {
    core: Gba,
    cartridge: CartridgeView,
    stopped_at: Option<u32>,
    hits: Vec<HookHit>,
}

struct Tick {
    executed: bool,
    frame: bool,
}

impl GbaDebug {
    pub fn new(cartridge: Cartridge, name: String, cover: Option<Arc<str>>) -> Self {
        let details = StructuredView {
            sections: vec![Section {
                rows: vec![
                    Row::Field(Field::new("System", System::Gba.qualified_name())),
                    Row::Field(Field::new(
                        "ROM size",
                        core_model::format_byte_size(cartridge.rom().len()),
                    )),
                    Row::Field(Field::new("Header title", cartridge.title())),
                ],
                ..Default::default()
            }],
            ..Default::default()
        };

        Self {
            core: Gba::new(cartridge),
            cartridge: CartridgeView {
                title: name.into(),
                details,
                cover,
            },
            stopped_at: None,
            hits: Vec::new(),
        }
    }

    fn arm(&self, cpu: &CpuId) -> Option<&Cpu> {
        (cpu.as_str() == CPU).then_some(&self.core.cpu)
    }

    fn execution_address(&self) -> u32 {
        cpu::execution_address(&self.core.cpu)
    }

    fn tick(&mut self) -> Tick {
        let executed = self.core.cpu.pipeline.pending()[0].is_some();
        self.core.step_cpu();

        Tick {
            executed,
            frame: self.core.dispatch_due(),
        }
    }

    fn regions(&self) -> impl Iterator<Item = (&'static str, &'static str, u32, u32)> + '_ {
        let rom_len = self.core.gamepak.cartridge().rom().len();

        REGIONS.into_iter().map(move |(id, name, base, size)| {
            let size = if id == ROM {
                size.min(rom_len as u32)
            } else {
                size
            };

            (id, name, base, size)
        })
    }

    fn clamp_len(&self, region: &str, addr: u32, len: usize) -> Option<usize> {
        let (_, _, base, size) = self.regions().find(|&(id, ..)| id == region)?;
        let end = u64::from(base) + u64::from(size);
        let addr = u64::from(addr);

        (u64::from(base)..end)
            .contains(&addr)
            .then(|| len.min((end - addr) as usize))
    }
}

impl core_model::Core for GbaDebug {
    fn set_button(&mut self, port: usize, button: core_model::Button, pressed: bool) {
        self.core.set_button(port, button, pressed);
    }

    fn controllers(&self) -> Vec<core_model::ControllerDescriptor> {
        self.core.controllers()
    }

    fn software_frame(&self) -> Option<core_model::SoftwareFrame<'_>> {
        self.core.software_frame()
    }

    fn frame_interval(&self) -> Option<Duration> {
        self.core.frame_interval()
    }

    fn cartridge_info(&self) -> Option<core_model::CartridgeInfo> {
        self.core.cartridge_info()
    }

    fn session_info(&self) -> core_model::SessionInfo {
        core_model::SessionInfo {
            system: System::Gba.name().into(),
            session: self.cartridge.title.clone(),
            segments: Vec::new(),
        }
    }

    fn reset(&mut self) {
        self.core.reset();
        self.core.skip_bios();
        self.stopped_at = None;
        self.hits.clear();
    }

    fn step(&mut self) {
        self.core.step();
        self.stopped_at = None;
    }

    fn run_frame(&mut self) {
        self.core.run_frame();
        self.stopped_at = None;
    }
}

impl debugger_model::DebugCore for GbaDebug {
    fn cpus(&self) -> Vec<CpuDescriptor> {
        vec![CpuDescriptor::new(CPU, "ARM7TDMI", SPACE)]
    }

    fn pc(&self, cpu: &CpuId) -> u32 {
        self.arm(cpu).map_or(0, cpu::execution_address)
    }

    fn pipeline_pc(&self, cpu: &CpuId) -> Option<u32> {
        self.arm(cpu).map(Cpu::pc)
    }

    fn set_pc(&mut self, cpu: &CpuId, addr: u32) {
        if self.arm(cpu).is_none() {
            return;
        }

        self.core.cpu.set_pc(addr);
        self.stopped_at = None;
        self.hits.clear();
    }

    fn cpu_view(&self, cpu: &CpuId) -> CpuView {
        self.arm(cpu).map(cpu::view).unwrap_or_default()
    }

    fn disassemble(&self, cpu: &CpuId, request: &DisassemblyRequest) -> Vec<DisasmLine> {
        self.arm(cpu)
            .map(|_| disasm::view(&self.core, request))
            .unwrap_or_default()
    }

    fn debug_step(&mut self) {
        self.stopped_at = None;
        self.hits.clear();
        while !self.tick().executed {}
    }

    fn debug_run_frame(&mut self, breakpoints: &[BreakpointMark]) -> RunOutcome {
        self.hits.clear();

        let cpu = CpuId::from(CPU);
        let execute: Vec<RangeInclusive<u32>> = breakpoints
            .iter()
            .filter(|mark| {
                mark.kind == BreakpointKind::Execute
                    && mark.cpu == cpu
                    && mark.region.as_ref() == SPACE
            })
            .map(|mark| mark.start..=mark.end)
            .collect();

        let resume = self.stopped_at.take();
        let mut skip_stop = resume == Some(self.execution_address());

        for _ in 0..RUN_BUDGET {
            let pc = self.execution_address();
            if !skip_stop && execute.iter().any(|range| range.contains(&pc)) {
                self.stopped_at = Some(pc);
                self.hits.push(HookHit::execute(cpu, SPACE, pc));
                return RunOutcome::Breakpoint;
            }

            let tick = self.tick();
            skip_stop &= !tick.executed;
            if tick.frame {
                return RunOutcome::FrameComplete;
            }
        }

        RunOutcome::BudgetExhausted
    }

    fn take_hits(&mut self) -> Vec<HookHit> {
        std::mem::take(&mut self.hits)
    }

    fn cartridge(&self) -> &CartridgeView {
        &self.cartridge
    }

    fn set_cover_art(&mut self, cover: Arc<str>) {
        self.cartridge.cover = Some(cover);
    }

    fn mmio_view(&self) -> MmioView {
        mmio::view(&self.core)
    }

    fn caption(&self, label: &str) -> String {
        format!("{label} @ {:08X}", self.execution_address())
    }

    fn manifest(&self) -> ComponentManifest {
        let mut manifest = debugger_model::standard_manifest(&self.cpus());
        manifest.system = System::Gba.name().into();
        manifest
            .place_after(
                debugger_model::panel::FRAMEBUFFER,
                debugger_model::CPU_WORKSPACE,
                debugger_model::panel::MMIO,
                DockLocation::Right,
            )
            .expect("shipped panel id");

        manifest
    }

    fn memory_regions(&self) -> Vec<MemoryRegion> {
        self.regions()
            .map(|(id, name, base, size)| MemoryRegion::new(id, name, base, size))
            .collect()
    }

    fn peek_memory(&self, region: &str, addr: u32, len: usize) -> Vec<u8> {
        let Some(len) = self.clamp_len(region, addr, len) else {
            return Vec::new();
        };

        let bus = self.core.bus_view();
        (0..len).map(|i| bus.peek8(addr + i as u32)).collect()
    }

    fn format_address(&self, _region: &str, addr: u32) -> String {
        format!("{addr:08X}")
    }

    fn memory_highlights(&self, region: &str, addr: u32, len: usize) -> Vec<Highlight> {
        let Some(len) = self.clamp_len(region, addr, len) else {
            return Vec::new();
        };

        let width = self.core.cpu.instruction_width();
        let execution = self.execution_address();
        let pipeline = self.core.cpu.pc();
        let marks = |start: u32, address: u32| address.wrapping_sub(start) < width;

        (0..len)
            .map(|i| {
                let address = addr + i as u32;
                if marks(execution, address) {
                    Highlight::Execution
                } else if marks(pipeline, address) {
                    Highlight::Pipeline
                } else {
                    Highlight::None
                }
            })
            .collect()
    }

    fn latest_access(&self) -> Option<(Arc<str>, u32)> {
        None
    }
}

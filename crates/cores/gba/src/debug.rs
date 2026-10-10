mod access;
mod callstack;
mod cpu;
mod disasm;
mod dma;
mod mmio;
mod video;

use std::ops::RangeInclusive;
use std::sync::Arc;
use std::time::Duration;

use core_model::System;
use debugger_model::{
    AccessSource, AccessTracking, AccessWidth, BreakpointKind, BreakpointMark, CallStackView,
    CartridgeView, CpuDescriptor, CpuId, CpuView, DebugMemoryError, DecodeMode, DisasmLine,
    DisassemblyRequest, EventDescriptor, EventHit, Field, Highlight, HookHit, MemoryRegion,
    MmioView, Row, RunOutcome, Section, StepTarget, StructuredView, VideoDebug,
};
use workspace_model::{ComponentManifest, DockLocation};

use crate::cpu::Bus;
use crate::scheduler::EventKind;
use crate::{Cartridge, Cpu, Gba, VIDEO_EVENTS};

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
    label: Arc<str>,
    stopped_at: Option<u32>,
    hits: Vec<HookHit>,
    accesses: access::Capture,
    stack: callstack::Stack,
    watched_events: u8,
    event_hits: Vec<EventHit>,
}

struct Tick {
    executed: bool,
    frame: bool,
    events: u8,
}

impl GbaDebug {
    pub fn new(cartridge: Cartridge, name: String, cover: Option<Arc<str>>) -> Self {
        let title = cartridge.title().into();
        let details = StructuredView {
            sections: vec![Section {
                rows: vec![
                    Row::Field(Field::new("System", System::Gba.qualified_name())),
                    Row::Field(Field::new(
                        "ROM size",
                        core_model::format_byte_size(cartridge.rom().len()),
                    )),
                ],
                ..Default::default()
            }],
            ..Default::default()
        };

        Self {
            core: Gba::new(cartridge),
            cartridge: CartridgeView {
                title,
                details,
                cover,
            },
            label: name.into(),
            stopped_at: None,
            hits: Vec::new(),
            accesses: access::Capture::default(),
            stack: callstack::Stack::default(),
            watched_events: 0,
            event_hits: Vec::new(),
        }
    }

    fn arm(&self, cpu: &CpuId) -> Option<&Cpu> {
        (cpu.as_str() == CPU).then_some(&self.core.cpu)
    }

    fn execution_address(&self) -> u32 {
        cpu::execution_address(&self.core.cpu)
    }

    fn tick(&mut self) -> Tick {
        let before = callstack::Before::new(&self.core.cpu);
        let executed = before.is_some();

        self.accesses.begin_instruction(self.execution_address());
        self.core.step_cpu_with(|cpu, bus| {
            cpu.step(&mut access::TrackedBus {
                bus,
                capture: &mut self.accesses,
            });
        });

        if let Some(before) = before {
            self.stack.observe(before, &self.core.cpu);
        }

        let mut events = 0;
        let frame = self.core.dispatch_due_with(|core, kind| {
            events |= kind.bit();
            if self.watched_events & kind.bit() != 0 {
                let scanline = core.ppu.vcount.scanline();
                self.event_hits.push(EventHit {
                    id: kind.debug_id().into(),
                    value: scanline.into(),
                    detail: format!("scanline {scanline}").into(),
                    pc: cpu::execution_address(&core.cpu),
                });
            }
        });

        Tick {
            executed,
            frame,
            events,
        }
    }

    fn begin_run(&mut self, watch: bool) {
        self.hits.clear();
        self.event_hits.clear();
        self.accesses.watch = watch;
    }

    fn clear_history(&mut self) {
        self.stopped_at = None;
        self.hits.clear();
        self.event_hits.clear();
        self.accesses.clear();
        self.stack.clear();
    }

    fn check_transaction(
        &self,
        region: &str,
        addr: u32,
        width: AccessWidth,
    ) -> Result<(), DebugMemoryError> {
        let len = width.bytes() as usize;

        if self.clamp_len(region, addr, len) != Some(len) {
            return Err(DebugMemoryError::InvalidAddress);
        }

        if !addr.is_multiple_of(width.bytes()) {
            return Err(DebugMemoryError::Misaligned {
                address: addr,
                width,
            });
        }

        Ok(())
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

    fn save_state(&self) -> Result<Vec<u8>, core_model::StateError> {
        self.core.save_state()
    }

    fn load_state(&mut self, bytes: &[u8]) -> Result<(), core_model::StateError> {
        self.core.load_state(bytes)?;
        self.clear_history();
        Ok(())
    }

    fn save_ram(&self) -> Option<&[u8]> {
        self.core.save_ram()
    }

    fn load_save_ram(&mut self, bytes: &[u8]) -> bool {
        self.core.load_save_ram(bytes)
    }

    fn cartridge_info(&self) -> Option<core_model::CartridgeInfo> {
        self.core.cartridge_info()
    }

    fn session_info(&self) -> core_model::SessionInfo {
        core_model::SessionInfo {
            system: System::Gba.name().into(),
            session: self.label.clone(),
            segments: Vec::new(),
        }
    }

    fn reset(&mut self) {
        self.core.reset();
        self.clear_history();
    }

    fn step(&mut self) {
        debugger_model::DebugCore::debug_step(self);
    }

    fn run_frame(&mut self) {
        self.stopped_at = None;
        self.begin_run(false);
        while !self.tick().frame {}
    }
}

impl debugger_model::DebugCore for GbaDebug {
    fn cpus(&self) -> Vec<CpuDescriptor> {
        vec![
            CpuDescriptor::new(CPU, "ARM7TDMI", SPACE).decode_modes(vec![
                DecodeMode::new("arm", "ARM"),
                DecodeMode::new("thumb", "Thumb"),
            ]),
        ]
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
        self.clear_history();
    }

    fn cpu_view(&self, cpu: &CpuId) -> CpuView {
        self.arm(cpu).map(cpu::view).unwrap_or_default()
    }

    fn call_stack(&self, cpu: &CpuId) -> CallStackView {
        if self.arm(cpu).is_none() {
            return CallStackView::default();
        }
        self.stack.view(self.execution_address())
    }

    fn write_register(&mut self, cpu: &CpuId, name: &str, value: u32) -> bool {
        if self.arm(cpu).is_none() || !cpu::write(&mut self.core.cpu, name, value) {
            return false;
        }
        self.clear_history();
        true
    }

    fn track_accesses(&mut self, tracking: AccessTracking) {
        self.accesses.set_tracking(tracking);
    }

    fn patch_memory(&mut self, region: &str, addr: u32, bytes: &[u8]) -> usize {
        let Some(len) = self.clamp_len(region, addr, bytes.len()) else {
            return 0;
        };
        let written = self.core.patch(addr, &bytes[..len]);
        if written > 0 {
            let pc = self.execution_address();
            self.core.cpu.set_pc(pc);
            self.clear_history();
        }
        written
    }

    fn read_bus(
        &mut self,
        region: &str,
        addr: u32,
        width: AccessWidth,
    ) -> Result<u32, DebugMemoryError> {
        self.check_transaction(region, addr, width)?;

        let mut bus = self.core.bus();
        Ok(match width {
            AccessWidth::Byte => u32::from(bus.read8(addr)),
            AccessWidth::Halfword => u32::from(bus.read16(addr)),
            AccessWidth::Word => bus.read32(addr),
        })
    }

    fn write_bus(
        &mut self,
        region: &str,
        addr: u32,
        width: AccessWidth,
        value: u32,
    ) -> Result<(), DebugMemoryError> {
        self.check_transaction(region, addr, width)?;

        let mut bus = self.core.bus();
        match width {
            AccessWidth::Byte => bus.write8(addr, value as u8),
            AccessWidth::Halfword => bus.write16(addr, value as u16),
            AccessWidth::Word => bus.write32(addr, value),
        }

        Ok(())
    }

    fn video(&self) -> Option<&dyn VideoDebug> {
        Some(self)
    }

    fn structured_view(&self, id: &str, _selection: Option<usize>) -> StructuredView {
        match id {
            debugger_model::panel::PPU => video::ppu_view(&self.core),
            debugger_model::panel::DMA => dma::view(&self.core),
            _ => StructuredView::default(),
        }
    }

    fn step_targets(&self) -> Vec<StepTarget> {
        [
            EventKind::HBlankStart,
            EventKind::VBlankStart,
            EventKind::ScanlineStart,
        ]
        .into_iter()
        .map(|kind| StepTarget::new(kind.debug_id(), kind.label()))
        .collect()
    }

    fn run_until(&mut self, target: &str) -> RunOutcome {
        let Some((kind, ..)) = VIDEO_EVENTS
            .into_iter()
            .find(|(kind, ..)| kind.debug_id() == target)
        else {
            return RunOutcome::Unsupported;
        };

        self.stopped_at = None;
        self.begin_run(false);
        for _ in 0..RUN_BUDGET {
            let tick = self.tick();
            if tick.events & kind.bit() != 0 {
                return RunOutcome::TargetReached;
            }
            if !self.event_hits.is_empty() {
                return RunOutcome::Event;
            }
        }

        RunOutcome::BudgetExhausted
    }

    fn events(&self) -> Vec<EventDescriptor> {
        VIDEO_EVENTS
            .into_iter()
            .map(|(kind, ..)| EventDescriptor::new(kind.debug_id(), kind.label()))
            .collect()
    }

    fn watch_events(&mut self, ids: &[Arc<str>]) {
        self.watched_events = VIDEO_EVENTS
            .into_iter()
            .filter(|(kind, ..)| ids.iter().any(|id| id.as_ref() == kind.debug_id()))
            .fold(0, |mask, (kind, ..)| mask | kind.bit());
        self.event_hits.clear();
    }

    fn take_events(&mut self) -> Vec<EventHit> {
        std::mem::take(&mut self.event_hits)
    }

    fn disassemble(&self, cpu: &CpuId, request: &DisassemblyRequest) -> Vec<DisasmLine> {
        self.arm(cpu)
            .map(|_| disasm::view(&self.core, request))
            .unwrap_or_default()
    }

    fn debug_step(&mut self) {
        self.stopped_at = None;
        self.begin_run(false);
        while !self.tick().executed {}
    }

    fn debug_run_frame(&mut self, breakpoints: &[BreakpointMark]) -> RunOutcome {
        self.begin_run(
            breakpoints
                .iter()
                .any(|mark| mark.kind != BreakpointKind::Execute),
        );

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
            self.hits.extend(self.accesses.matching_hits(breakpoints));
            if !self.hits.is_empty() {
                return RunOutcome::Breakpoint;
            }
            if !self.event_hits.is_empty() {
                return RunOutcome::Event;
            }
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
                debugger_model::panel::MEMORY,
                DockLocation::Center,
            )
            .expect("shipped panel id");

        video::extend_manifest(&mut manifest);
        manifest
            .insert_after(
                debugger_model::panel::MMIO,
                workspace_model::PanelDescriptor::new(
                    debugger_model::panel::DMA,
                    "DMA",
                    workspace_model::PanelKind::Structured,
                    DockLocation::Right,
                    workspace_model::RefreshPolicy::OnPause,
                )
                .tag(debugger_model::CPU_WORKSPACE)
                .tag(debugger_model::VIDEO_WORKSPACE),
            )
            .expect("shipped panel id");

        manifest.compose_in(
            debugger_model::CPU_WORKSPACE,
            debugger_model::panel::MEMORY,
            debugger_model::panel::FRAMEBUFFER,
            workspace_model::PanelGroupLayout::CompanionRail,
        );
        manifest
    }

    fn memory_regions(&self) -> Vec<MemoryRegion> {
        self.regions()
            .map(|(id, name, base, size)| {
                MemoryRegion::new(id, name, base, size)
                    .watch_sources(vec![AccessSource::Cpu(CpuId::from(CPU))])
            })
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
                    self.accesses.highlight(address)
                }
            })
            .collect()
    }

    fn latest_access(&self) -> Option<(Arc<str>, u32)> {
        self.accesses.latest.map(|addr| (SPACE.into(), addr))
    }
}

fn field(label: &str, value: String) -> Row {
    Row::Field(Field::new(label, value))
}

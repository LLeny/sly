use std::sync::Arc;
use std::time::Duration;

use core_model::{Core, SessionInfo, SoftwareFrame};
use debugger_model::{
    AccessSource, AccessTracking, AccessWidth, AudioScopesView, BreakpointKind, BreakpointMark,
    CPU_WORKSPACE, CallStackView, CartridgeView, CpuDescriptor, CpuId, CpuView, DebugCore,
    DebugMemoryError, DecodeMode, DisasmLine, DisasmToken, DisassemblyRequest, EventDescriptor,
    EventHit, Field, FlagView, FlowKind, Highlight, HookHit, MemoryAddress, MemoryRegion,
    MmioFieldView, MmioGroup, MmioRegisterView, MmioView, PaletteColor, PaletteView, RasterImage,
    RegisterView, Row, RunOutcome, SOUND_WORKSPACE, Section, SpriteEntry, Sprites, StackFrame,
    StepTarget, StructuredView, SyntaxSpec, TableView, Tilemap, TilemapEntry, TilemapOverlay,
    TilemapOverlayBand, TilemapViewport, Tileset, TilesetTile, TokenKind, VIDEO_WORKSPACE,
    VideoDebug, VideoResource, VideoResourceId, VideoResourceKind, VideoRevision, format_bytes,
    hex, standard_manifest, strip_symbol_sigils, tokenize,
};

use workspace_model::{
    ComponentManifest, DockLocation, PanelDescriptor, PanelGroupLayout, PanelKind, RefreshPolicy,
    WorkspaceDescriptor, WorkspaceIcon,
};

mod cartridge;
mod cpu;
mod dsp;
mod manifest;
mod memory;
mod registers;
mod run;
mod sa1;
mod superfx;
mod video;

use cartridge::*;
use cpu::*;
use registers::*;
use run::{EVENT_IRQ, EVENT_NMI, EVENT_SCANLINE, EventBaseline, EventWatch};
use video::*;

use crate::apu::{Apu, spc700};
use crate::callstack::CallFrame;
use crate::cartridge::{MapMode, Speed};
use crate::cpu::disasm::{Disasm, FlowKind as CoreFlow, InstructionClass};
use crate::mmio::{Address, MemKind, MemoryAccess, TrackMode};
use crate::{Cartridge, Snes};

const WRAM_SIZE: u32 = 0x2_0000;
const PPU_BASE: u32 = 0x2100;
const PPU_SIZE: u32 = 0x0100;
const CPU_BASE: u32 = 0x4200;
const CPU_SIZE: u32 = 0x0200;
const BUS_SIZE: u32 = 0x100_0000;

const BG_LAYERS: usize = 4;

const CPU_MAIN: &str = "cpu.65c816";
const CPU_SPC: &str = "apu.spc700";

pub struct SnesDebug {
    core: Snes,
    cartridge: CartridgeView,
    system: Arc<str>,
    label: Arc<str>,
    segments: Vec<Arc<str>>,
    last_access: MemoryAccess,
    last_hits: Vec<HookHit>,
    watch: EventWatch,
    events: Vec<EventHit>,
    event_baseline: EventBaseline,
    tracking: AccessTracking,
}

impl SnesDebug {
    pub fn new(cart: Cartridge, label: impl Into<Arc<str>>, cover: Option<Arc<str>>) -> Self {
        let mut cartridge = self::cartridge_view(&cart);
        cartridge.cover = cover;

        let segments = vec![
            self::mapper_name(&cart).into(),
            crate::cartridge::tv_system(cart.region())
                .to_string()
                .into(),
            core_model::format_byte_size(cart.rom.len()).into(),
        ];

        let mut debug = Self {
            core: Snes::new(cart),
            cartridge,
            system: "SNES".into(),
            label: label.into(),
            segments,
            last_access: MemoryAccess::default(),
            last_hits: Vec::new(),
            watch: EventWatch::new(),
            events: Vec::new(),
            event_baseline: EventBaseline::default(),
            tracking: AccessTracking::Off,
        };

        debug.reset_access();
        debug
    }

    fn reset_access(&mut self) {
        self.core
            .bus
            .apu
            .access
            .share_order(&self.core.bus.mmu.access);

        self.last_access = MemoryAccess::default();
        self.last_hits.clear();
    }

    fn has_gsu(&self) -> bool {
        self.core.mmu.cartridge.coprocessor() == crate::cartridge::Coprocessor::SuperFx
    }

    fn has_sa1(&self) -> bool {
        self.core.mmu.cartridge.coprocessor() == crate::cartridge::Coprocessor::Sa1
    }

    fn pc_of(&self, cpu: &str) -> u32 {
        match cpu {
            CPU_SPC => self.core.apu.spc700.pc as u32,
            sa1::CPU => {
                Address::new(self.core.sa1.cpu.program_bank, self.core.sa1.cpu.pc).as_24bit()
            }
            superfx::CPU_GSU => {
                Address::new(self.core.superfx.pbr, superfx::pc(&self.core.superfx)).as_24bit()
            }
            _ => Address::new(self.core.cpu.program_bank, self.core.cpu.pc).as_24bit(),
        }
    }
}

impl Core for SnesDebug {
    fn cartridge_info(&self) -> Option<core_model::CartridgeInfo> {
        self.core.cartridge_info()
    }

    fn reset(&mut self) {
        self.core.reset();
        self.reset_access();
    }

    fn step(&mut self) {
        self.core.step();
    }

    fn run_frame(&mut self) {
        self.core.run_frame();
    }

    fn audio_spec(&self) -> Option<core_model::AudioSpec> {
        self.core.audio_spec()
    }

    fn drain_audio(&mut self, out: &mut Vec<i16>) {
        self.core.drain_audio(out);
    }

    fn set_button(&mut self, player: usize, button: core_model::Button, pressed: bool) {
        self.core.joypads.set_button(player, button, pressed);
    }

    fn controllers(&self) -> Vec<core_model::ControllerDescriptor> {
        self.core.controllers()
    }

    fn session_info(&self) -> SessionInfo {
        SessionInfo {
            system: self.system.clone(),
            session: self.label.clone(),
            segments: self.segments.clone(),
        }
    }

    fn software_frame(&self) -> Option<SoftwareFrame<'_>> {
        Some(SoftwareFrame {
            width: crate::ppu::SCREEN_WIDTH,
            height: crate::ppu::SCREEN_HEIGHT,
            pixels: self.core.ppu.frame_buffer(),
        })
    }

    fn frame_interval(&self) -> Option<Duration> {
        Some(self::frame_interval(&self.core))
    }

    fn save_state(&self) -> Result<Vec<u8>, core_model::StateError> {
        self.core.save_state()
    }

    fn load_state(&mut self, bytes: &[u8]) -> Result<(), core_model::StateError> {
        self.core.load_state(bytes)?;
        self.reset_access();
        Ok(())
    }

    fn save_ram(&self) -> Option<&[u8]> {
        self.core.save_ram()
    }

    fn load_save_ram(&mut self, bytes: &[u8]) -> bool {
        self.core.load_save_ram(bytes)
    }
}

impl DebugCore for SnesDebug {
    fn cpus(&self) -> Vec<CpuDescriptor> {
        let mut cpus = vec![
            CpuDescriptor::new(CPU_MAIN, "65C816", "bus").decode_modes(decode_modes()),
            CpuDescriptor::new(CPU_SPC, "SPC700", "spc"),
        ];

        if self.has_gsu() {
            cpus.push(superfx::descriptor());
        }

        if self.has_sa1() {
            cpus.push(sa1::descriptor());
        }
        cpus
    }

    fn pc(&self, cpu: &CpuId) -> u32 {
        self.pc_of(cpu.as_str())
    }

    fn set_pc(&mut self, cpu: &CpuId, addr: u32) {
        match cpu.as_str() {
            CPU_SPC => self.core.apu.spc700.pc = addr as u16,
            sa1::CPU => {
                self.core.sa1.cpu.pc = addr as u16;
                self.core.sa1.cpu.program_bank = (addr >> 16) as u8;
            }
            superfx::CPU_GSU => {
                self.core.superfx.pbr = (addr >> 16) as u8;
                superfx::set_pc(&mut self.core, addr as u16);
            }
            _ => {
                self.core.cpu.pc = addr as u16;
                self.core.cpu.program_bank = (addr >> 16) as u8;
            }
        }
    }

    fn debug_step(&mut self) {
        self.last_access = self.step_captured();
        self.last_hits.clear();
    }

    fn step_targets(&self) -> Vec<StepTarget> {
        let mut targets = vec![
            StepTarget::new("scanline", "Line"),
            StepTarget::new("nmi", "NMI"),
            StepTarget::new("irq", "IRQ"),
            StepTarget::new("spc", "SPC"),
            StepTarget::new("dsp", "DSP"),
        ];
        if self.has_gsu() {
            targets.push(StepTarget::new("gsu", "GSU"));
        }
        if self.has_sa1() {
            targets.push(StepTarget::new("sa1", "SA-1"));
        }
        targets
    }

    fn cpu_view(&self, cpu: &CpuId) -> CpuView {
        match cpu.as_str() {
            CPU_SPC => self::spc_cpu_view(&self.core.apu.spc700),
            sa1::CPU => sa1::cpu_view(&self.core),
            superfx::CPU_GSU => superfx::cpu_view(&self.core.superfx),
            _ => self::cpu_view(&self.core.cpu),
        }
    }

    fn disassemble(&self, cpu: &CpuId, request: &DisassemblyRequest) -> Vec<DisasmLine> {
        match cpu.as_str() {
            CPU_SPC if request.mode.is_none() => {
                spc_disasm_view(&self.core.apu, request.base as u16, request.count)
            }
            sa1::CPU => sa1::disasm_view(&self.core, request),
            superfx::CPU_GSU if request.mode.is_none() => {
                superfx::disasm_view(&self.core, request.base, request.count)
            }
            CPU_MAIN => {
                let Some(decoded) = decode_cpu(&self.core.cpu, request) else {
                    return Vec::new();
                };

                disasm_view_on(&decoded, &self.core.bus, request.base as u16, request.count)
            }
            _ => Vec::new(),
        }
    }

    fn disassemble_one(&self, cpu: &CpuId, request: &DisassemblyRequest) -> Option<DisasmLine> {
        if cpu.as_str() == CPU_MAIN && request.mode.is_none() && request.base == self.pc(cpu) {
            return Some(disasm_line_at(&self.core, request.base as u16, true).0);
        }

        let mut request = request.clone();
        request.count = 1;
        self.disassemble(cpu, &request).into_iter().next()
    }

    fn call_stack(&self, cpu: &CpuId) -> CallStackView {
        match cpu.as_str() {
            sa1::CPU => self::call_stack_view(
                &self.core.sa1.cpu.call_stack,
                Address::new(self.core.sa1.cpu.program_bank, self.core.sa1.cpu.pc).as_24bit(),
                |a| self.format_address("sa1", a),
                |_| String::new(),
            ),
            superfx::CPU_GSU => CallStackView::default(),
            CPU_SPC => self::call_stack_view(
                &self.core.apu.call_stack,
                self.core.apu.spc700.pc as u32,
                |a| self.format_address("spc", a),
                |_| String::new(),
            ),
            _ => self::call_stack_view(
                &self.core.cpu.call_stack,
                Address::new(self.core.cpu.program_bank, self.core.cpu.pc).as_24bit(),
                |a| self.format_address("bus", a),
                |a| self.annotate(a),
            ),
        }
    }

    fn cartridge(&self) -> &CartridgeView {
        &self.cartridge
    }

    fn set_cover_art(&mut self, cover: Arc<str>) {
        self.cartridge.cover = Some(cover);
    }

    fn mmio_view(&self) -> MmioView {
        self::mmio_view(&self.core)
    }

    fn video(&self) -> Option<&dyn VideoDebug> {
        Some(self)
    }

    fn table_view(&self, id: &str) -> TableView {
        match id {
            dsp::VOICES_ID => dsp::voices_view(&self.core.apu.dsp),
            superfx::CACHE_ID => superfx::cache_view(&self.core),
            _ => TableView::default(),
        }
    }

    fn structured_view(&self, id: &str, selection: Option<usize>) -> StructuredView {
        let dsp = &self.core.apu.dsp;
        match id {
            "ppu" => self::ppu_view(&self.core),
            "dma" => self::dma_view(&self.core),
            dsp::REGISTERS_ID => dsp::registers_view(dsp),
            dsp::INSPECTOR_ID => dsp::inspector_view(dsp, selection),
            sa1::STATE_ID => sa1::state_view(&self.core),
            superfx::STATE_ID => superfx::state_view(&self.core),
            _ => StructuredView::default(),
        }
    }

    fn scopes_view(&self, id: &str) -> AudioScopesView {
        match id {
            dsp::SCOPES_ID => dsp::scopes_view(&self.core.apu.dsp),
            _ => AudioScopesView::default(),
        }
    }

    fn manifest(&self) -> ComponentManifest {
        self.build_manifest()
    }

    fn caption(&self, label: &str) -> String {
        self::caption(label, &self.core)
    }

    fn take_hits(&mut self) -> Vec<HookHit> {
        std::mem::take(&mut self.last_hits)
    }

    fn events(&self) -> Vec<EventDescriptor> {
        vec![
            EventDescriptor::new(EVENT_SCANLINE, "Scanline"),
            EventDescriptor::new(EVENT_NMI, "NMI (vblank)"),
            EventDescriptor::new(EVENT_IRQ, "IRQ (H/V timer)"),
        ]
    }

    fn watch_events(&mut self, ids: &[Arc<str>]) {
        if self.watch.ids != ids {
            self.watch.set(ids);
            self.event_baseline = self.event_snapshot();
        }
    }

    fn take_events(&mut self) -> Vec<EventHit> {
        std::mem::take(&mut self.events)
    }

    fn run_until(&mut self, target: &str) -> RunOutcome {
        self.run_to(target)
    }

    fn debug_run_frame(&mut self, breakpoints: &[BreakpointMark]) -> RunOutcome {
        self.run_frame(breakpoints)
    }

    fn memory_regions(&self) -> Vec<MemoryRegion> {
        self.regions()
    }

    fn peek_memory(&self, region: &str, addr: u32, len: usize) -> Vec<u8> {
        self.read(region, addr, len)
    }

    fn track_accesses(&mut self, tracking: AccessTracking) {
        self.tracking = tracking;
    }

    fn read_bus(
        &mut self,
        region: &str,
        addr: u32,
        width: AccessWidth,
    ) -> Result<u32, DebugMemoryError> {
        self.bus_read(region, addr, width)
    }

    fn write_bus(
        &mut self,
        region: &str,
        addr: u32,
        width: AccessWidth,
        value: u32,
    ) -> Result<(), DebugMemoryError> {
        self.bus_write(region, addr, width, value)
    }

    fn patch_memory(&mut self, region: &str, addr: u32, bytes: &[u8]) -> usize {
        self.write(region, addr, bytes)
    }

    fn format_address(&self, region: &str, addr: u32) -> String {
        self.format_addr(region, addr)
    }

    fn memory_highlights(&self, region: &str, addr: u32, len: usize) -> Vec<Highlight> {
        self.highlights(region, addr, len)
    }

    fn annotate(&self, addr: u32) -> String {
        self.annotation(addr)
    }

    fn latest_access(&self) -> Option<(Arc<str>, u32)> {
        self.latest()
    }

    fn disasm_fingerprint(&self, cpu: &CpuId, request: &DisassemblyRequest) -> Option<u64> {
        self.fingerprint(cpu, request)
    }

    fn write_register(&mut self, cpu: &CpuId, name: &str, value: u32) -> bool {
        self.set_register(cpu, name, value)
    }
}

use std::sync::Arc;
use std::time::Duration;

use core_model::{Core, SessionInfo, SoftwareFrame};
use debugger_model::{
    AudioScopesView, BreakpointKind, BreakpointMark, CPU_WORKSPACE, CallStackView, CartridgeField,
    CartridgeView, CpuDescriptor, CpuId, CpuView, DebugCore, DisasmLine, DisasmToken,
    DmaChannelDetail, DmaChannelView, DmaView, EventDescriptor, EventHit, FlagView, FlowKind,
    Highlight, HookHit, MemoryRegion, MmioFieldView, MmioGroup, MmioRegisterView, MmioView,
    PaletteColor, PaletteView, PpuGroup, PpuProperty, PpuView, RegisterView, RunOutcome,
    SOUND_WORKSPACE, SpriteEntry, Sprites, StackFrame, StepTarget, StructuredView, SyntaxSpec,
    TableView, Tilemap, TilemapEntry, TilemapOverlay, TilemapOverlayBand, TilemapViewport, Tileset,
    TilesetLayer, TilesetTile, TokenKind, VIDEO_WORKSPACE, VideoGenerations, hex,
    standard_manifest, strip_symbol_sigils, tokenize,
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

const CAPTURE_FRAMES: u32 = 2;
const TRACK_FRAMES: u32 = 4;

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
    capture_frames: std::sync::atomic::AtomicU32,
    track_frames: std::sync::atomic::AtomicU32,
}

impl SnesDebug {
    pub fn new(cart: Cartridge, label: impl Into<Arc<str>>, cover: Option<Arc<str>>) -> Self {
        let mut cartridge = self::cartridge_view(&cart);
        cartridge.cover = cover;

        let segments = vec![
            self::mapper_name(&cart).into(),
            cart.region().tv_system().to_string().into(),
            self::human_size(cart.rom.len()).into(),
        ];

        Self {
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
            capture_frames: std::sync::atomic::AtomicU32::new(0),
            track_frames: std::sync::atomic::AtomicU32::new(0),
        }
    }

    fn has_gsu(&self) -> bool {
        self.core.mmu.cartridge.coprocessor() == crate::cartridge::Coprocessor::SuperFx
    }
}

impl Core for SnesDebug {
    fn reset(&mut self) {
        self.core.reset();
        self.last_access = MemoryAccess::default();
        self.last_hits.clear();
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
        self.last_access = MemoryAccess::default();
        self.last_hits.clear();
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
            CpuDescriptor::new(CPU_MAIN, "65C816", "bus", 16),
            CpuDescriptor::new(CPU_SPC, "SPC700", "spc", 16),
        ];
        if self.has_gsu() {
            cpus.push(superfx::descriptor());
        }
        cpus
    }

    fn pc(&self, cpu: &CpuId) -> u32 {
        match cpu.as_str() {
            CPU_SPC => self.core.apu.spc700.pc as u32,
            superfx::CPU_GSU => superfx::pc(&self.core.superfx) as u32,
            _ => self.core.cpu.pc as u32,
        }
    }

    fn set_pc(&mut self, cpu: &CpuId, addr: u32) {
        match cpu.as_str() {
            CPU_SPC => self.core.apu.spc700.pc = addr as u16,
            superfx::CPU_GSU => superfx::set_pc(&mut self.core, addr as u16),
            _ => self.core.cpu.pc = addr as u16,
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
        targets
    }

    fn cpu_view(&self, cpu: &CpuId) -> CpuView {
        match cpu.as_str() {
            CPU_SPC => self::spc_cpu_view(&self.core.apu.spc700),
            superfx::CPU_GSU => superfx::cpu_view(&self.core.superfx),
            _ => self::cpu_view(&self.core.cpu),
        }
    }

    fn disassemble(&self, cpu: &CpuId, base: u32, count: usize) -> Vec<DisasmLine> {
        match cpu.as_str() {
            CPU_SPC => self::spc_disasm_view(&self.core.apu, base as u16, count),
            superfx::CPU_GSU => superfx::disasm_view(&self.core, base as u16, count),
            _ => self::disasm_view(&self.core, base as u16, count),
        }
    }

    fn disassemble_one(&self, cpu: &CpuId, addr: u32) -> Option<DisasmLine> {
        match cpu.as_str() {
            superfx::CPU_GSU => superfx::disasm_view(&self.core, addr as u16, 1)
                .into_iter()
                .next(),
            CPU_SPC => self::spc_disasm_view(&self.core.apu, addr as u16, 1)
                .into_iter()
                .next(),
            _ => Some(self::disasm_line_at(&self.core, addr as u16, true).0),
        }
    }

    fn call_stack(&self, cpu: &CpuId) -> CallStackView {
        match cpu.as_str() {
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

    fn dma_view(&self) -> DmaView {
        self::dma_view(&self.core)
    }

    fn ppu_view(&self) -> PpuView {
        self::ppu_view(&self.core)
    }

    fn tileset(&self, layer: usize, color: usize) -> Option<Tileset> {
        if layer >= BG_LAYERS {
            return None;
        }
        self::tileset(&self.core, layer, color)
    }

    fn tilemap(&self, layer: usize) -> Option<Tilemap> {
        if layer >= BG_LAYERS {
            return (self.has_gsu() && layer == BG_LAYERS)
                .then(|| superfx::bitmap_tilemap(&self.core))
                .flatten();
        }
        self::tilemap(&self.core, layer)
    }

    fn tilemap_overlays(&self, layer: usize) -> Vec<TilemapOverlay> {
        if layer >= BG_LAYERS || mode7_bg_layer(&self.core, layer) {
            return Vec::new();
        }
        self::tilemap_window_overlays(&self.core, layer)
    }

    fn sprites(&self) -> Option<Sprites> {
        Some(self::sprites(&self.core))
    }

    fn tilemap_viewport(&self, layer: usize) -> Option<TilemapViewport> {
        if mode7_bg_layer(&self.core, layer) {
            return None;
        }

        let header = self.core.ppu.tilemap_header(layer)?;
        Some(TilemapViewport {
            x: header.scroll_x,
            y: header.scroll_y,
            width: crate::ppu::SCREEN_WIDTH,
            height: crate::ppu::SCREEN_HEIGHT,
        })
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
            dsp::REGISTERS_ID => dsp::registers_view(dsp),
            dsp::INSPECTOR_ID => dsp::inspector_view(dsp, selection),
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

    fn read_memory(&self, region: &str, addr: u32, len: usize) -> Vec<u8> {
        self.read(region, addr, len)
    }

    fn write_memory(&mut self, region: &str, addr: u32, bytes: &[u8]) -> usize {
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

    fn palette_view(&self) -> PaletteView {
        self.palette()
    }

    fn tileset_layers(&self) -> Vec<TilesetLayer> {
        self.layers()
    }

    fn video_generations(&self) -> Option<VideoGenerations> {
        self.generations()
    }

    fn disasm_fingerprint(&self, cpu: &CpuId, base: u32, count: usize) -> Option<u64> {
        self.fingerprint(cpu, base, count)
    }

    fn write_register(&mut self, cpu: &CpuId, name: &str, value: u32) -> bool {
        self.set_register(cpu, name, value)
    }
}

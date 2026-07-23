use std::sync::Arc;
use std::time::Duration;

use core_model::{Core, SessionInfo, SoftwareFrame};
use debugger_model::{
    AudioScopesView, BreakpointKind, BreakpointMark, CPU_WORKSPACE, CallStackView, CartridgeField,
    CartridgeView, CpuDescriptor, CpuId, CpuView, DebugCore, DisasmLine, DisasmToken,
    DmaChannelDetail, DmaChannelView, DmaView, FlagView, FlowKind, Highlight, MemoryRegion,
    MmioFieldView, MmioGroup, MmioRegisterView, MmioView, PaletteColor, PaletteView, PpuGroup,
    PpuProperty, PpuView, RegisterView, RunOutcome, SOUND_WORKSPACE, SpriteEntry, Sprites,
    StackFrame, StructuredView, SyntaxSpec, TableView, Tilemap, TilemapEntry, TilemapOverlay,
    TilemapOverlayBand, TilemapViewport, Tileset, TilesetLayer, TilesetTile, TokenKind,
    VIDEO_WORKSPACE, VideoGenerations, hex, standard_manifest, strip_symbol_sigils, tokenize,
};
use workspace_model::{
    ComponentManifest, DockLocation, PanelDescriptor, PanelGroupLayout, PanelKind, RefreshPolicy,
    WorkspaceDescriptor, WorkspaceIcon,
};

mod dsp;

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
            capture_frames: std::sync::atomic::AtomicU32::new(0),
            track_frames: std::sync::atomic::AtomicU32::new(0),
        }
    }

    fn region_byte(&self, region: &str, addr: u32) -> u8 {
        match region {
            "wram" => self.core.mmu.wram.get(addr as usize).copied().unwrap_or(0),
            "sram" => self
                .core
                .mmu
                .cartridge
                .sram
                .get(addr as usize)
                .copied()
                .unwrap_or(0),
            "rom" => self
                .core
                .mmu
                .cartridge
                .rom
                .get(addr as usize)
                .copied()
                .unwrap_or(0),
            "bus" => self.core.peek8(Address::from_24bit(addr & 0x00FF_FFFF)),
            "spc" => self.core.apu.peek(addr as u16),
            _ => self.core.peek8(Address::new(0x00, addr as u16)),
        }
    }

    fn region_key(&self, region: &str, addr: u32) -> Option<(MemKind, u32)> {
        match region {
            "wram" => Some((MemKind::Wram, addr)),
            "sram" => Some((MemKind::Sram, addr)),
            "rom" => Some((MemKind::Rom, addr)),
            "bus" => self.core.classify(Address::from_24bit(addr & 0x00FF_FFFF)),
            "spc" => Some((MemKind::Spc, addr)),
            _ => Some((MemKind::Io, addr)),
        }
    }

    fn execute_hit(&self, breakpoints: &[BreakpointMark]) -> bool {
        let pcs = [
            (CPU_MAIN, self.core.cpu.pc as u32),
            (CPU_SPC, self.core.apu.spc700.pc as u32),
        ];

        breakpoints
            .iter()
            .filter(|m| m.kind == BreakpointKind::Execute)
            .any(|m| {
                pcs.iter().any(|(id, pc)| {
                    m.cpu.as_str() == *id && (m.start & 0xFFFF..=m.end & 0xFFFF).contains(pc)
                })
            })
    }

    fn captured(&mut self, mode: TrackMode, run: impl FnOnce(&mut Self)) -> MemoryAccess {
        self.core.mmu.access.begin(mode);
        self.core.apu.access.begin(mode);
        self.core.apu.access.resume();

        run(self);

        self.core.mmu.access.stop();
        self.core.apu.access.stop();

        let mut access = self.core.mmu.access.take();
        access.merge(&self.core.apu.access.take());
        access
    }

    fn step_captured(&mut self) -> MemoryAccess {
        self.captured(TrackMode::Full, |debug| debug.core.step())
    }

    fn track_mode(&self) -> TrackMode {
        use std::sync::atomic::Ordering::Relaxed;

        for (frames, mode) in [
            (&self.capture_frames, TrackMode::Full),
            (&self.track_frames, TrackMode::LastOnly),
        ] {
            let remaining = frames.load(Relaxed);
            if remaining > 0 {
                frames.store(remaining - 1, Relaxed);
                return mode;
            }
        }

        TrackMode::Off
    }

    fn watch_hit(&self, breakpoints: &[BreakpointMark], access: &MemoryAccess) -> bool {
        breakpoints.iter().any(|m| {
            let set = match m.kind {
                BreakpointKind::Read => &access.reads,
                BreakpointKind::Write => &access.writes,
                _ => return false,
            };
            match m.region.as_ref() {
                "bus" => (m.start..=m.end).any(|addr| {
                    self.region_key("bus", addr)
                        .is_some_and(|key| set.contains(&key))
                }),
                _ => set.iter().any(|&key| {
                    (m.start..=m.end).contains(&key.1)
                        && self.region_key(&m.region, key.1) == Some(key)
                }),
            }
        })
    }
}

impl Core for SnesDebug {
    fn reset(&mut self) {
        self.core.reset();
        self.last_access = MemoryAccess::default();
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
}

impl DebugCore for SnesDebug {
    fn cpus(&self) -> Vec<CpuDescriptor> {
        vec![
            CpuDescriptor::new(CPU_MAIN, "65C816", "bus"),
            CpuDescriptor::new(CPU_SPC, "SPC700", "spc"),
        ]
    }

    fn pc(&self, cpu: &CpuId) -> u32 {
        match cpu.as_str() {
            CPU_SPC => self.core.apu.spc700.pc as u32,
            _ => self.core.cpu.pc as u32,
        }
    }

    fn set_pc(&mut self, cpu: &CpuId, addr: u32) {
        match cpu.as_str() {
            CPU_SPC => self.core.apu.spc700.pc = addr as u16,
            _ => self.core.cpu.pc = addr as u16,
        }
    }

    fn debug_step(&mut self) {
        self.last_access = self.step_captured();
    }

    fn debug_run_frame(&mut self, breakpoints: &[BreakpointMark]) -> RunOutcome {
        let watch = breakpoints
            .iter()
            .any(|m| matches!(m.kind, BreakpointKind::Read | BreakpointKind::Write));

        let frame_end = self.core.scheduler.frame_deadline();

        if watch {
            let mut access = MemoryAccess::default();
            while self.core.scheduler.cycles() < frame_end {
                let step_access = self.step_captured();
                access.merge(&step_access);

                if self.watch_hit(breakpoints, &step_access) || self.execute_hit(breakpoints) {
                    self.last_access = access;
                    return RunOutcome::Breakpoint;
                }
            }

            self.last_access = access;
            return RunOutcome::Done;
        }

        let mode = self.track_mode();

        let mut outcome = RunOutcome::Done;
        self.last_access = self.captured(mode, |debug| {
            if breakpoints.is_empty() {
                debug.core.run_frame();
            } else {
                while debug.core.scheduler.cycles() < frame_end {
                    debug.core.step();
                    if debug.execute_hit(breakpoints) {
                        outcome = RunOutcome::Breakpoint;
                        break;
                    }
                }
            }
        });

        outcome
    }

    fn cpu_view(&self, cpu: &CpuId) -> CpuView {
        match cpu.as_str() {
            CPU_SPC => self::spc_cpu_view(&self.core.apu.spc700),
            _ => self::cpu_view(&self.core.cpu),
        }
    }

    fn disassemble(&self, cpu: &CpuId, base: u32, count: usize) -> Vec<DisasmLine> {
        match cpu.as_str() {
            CPU_SPC => self::spc_disasm_view(&self.core.apu, base as u16, count),
            _ => self::disasm_view(&self.core, base as u16, count),
        }
    }

    fn disasm_fingerprint(&self, cpu: &CpuId, base: u32, count: usize) -> Option<u64> {
        use std::hash::{DefaultHasher, Hasher};

        let mut hasher = DefaultHasher::new();
        let mut addr = base as u16;

        match cpu.as_str() {
            CPU_SPC => {
                for _ in 0..count * 3 {
                    hasher.write_u8(self.core.apu.peek(addr));
                    addr = addr.wrapping_add(1);
                }
            }
            _ => {
                let bank = self.core.cpu.program_bank;
                hasher.write_u8(bank);
                hasher.write_usize(crate::cpu::mode_selectors(&self.core.cpu));
                for _ in 0..count * 4 {
                    hasher.write_u8(self.core.peek8(Address::new(bank, addr)));
                    addr = addr.wrapping_add(1);
                }
            }
        }

        Some(hasher.finish())
    }

    fn disassemble_one(&self, cpu: &CpuId, addr: u32) -> Option<DisasmLine> {
        match cpu.as_str() {
            CPU_SPC => self::spc_disasm_view(&self.core.apu, addr as u16, 1)
                .into_iter()
                .next(),
            _ => Some(self::disasm_line_at(&self.core, addr as u16, true).0),
        }
    }

    fn call_stack(&self, cpu: &CpuId) -> CallStackView {
        match cpu.as_str() {
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

    fn palette_view(&self) -> PaletteView {
        let palettes = (0..16)
            .map(|p| {
                self.core
                    .ppu
                    .palette(p)
                    .into_iter()
                    .map(|word| PaletteColor {
                        raw: word,
                        rgb: crate::ppu::cgram::rgb(word),
                    })
                    .collect()
            })
            .collect();

        PaletteView {
            palettes,
            meta: format!("CGADD {}", hex(self.core.ppu.cgram.word_addr() as u32, 8)).into(),
        }
    }

    fn ppu_view(&self) -> PpuView {
        self::ppu_view(&self.core)
    }

    fn tileset_layers(&self) -> Vec<TilesetLayer> {
        (0..4)
            .map(|i| TilesetLayer {
                name: bg_layer_name(&self.core, i).into(),
                enabled: self.core.ppu.bg_mode.bpp(i).is_some() || mode7_bg_layer(&self.core, i),
            })
            .collect()
    }

    fn tileset(&self, layer: usize, color: usize) -> Option<Tileset> {
        self::tileset(&self.core, layer, color)
    }

    fn tilemap(&self, layer: usize) -> Option<Tilemap> {
        self::tilemap(&self.core, layer)
    }

    fn tilemap_overlays(&self, layer: usize) -> Vec<TilemapOverlay> {
        if mode7_bg_layer(&self.core, layer) {
            return Vec::new();
        }
        self::tilemap_window_overlays(&self.core, layer)
    }

    fn sprites(&self) -> Option<Sprites> {
        Some(self::sprites(&self.core))
    }

    fn video_generations(&self) -> Option<VideoGenerations> {
        use std::hash::{DefaultHasher, Hasher};

        fn mix(parts: impl IntoIterator<Item = u64>) -> u64 {
            let mut hasher = DefaultHasher::new();
            for part in parts {
                hasher.write_u64(part);
            }
            hasher.finish()
        }

        let ppu = &self.core.ppu;
        let vram = ppu.vram.generation();
        let cgram = ppu.cgram.generation();

        let regs = {
            let mut hasher = DefaultHasher::new();
            hasher.write_u8(ppu.bg_mode as u8);
            hasher.write_u16(ppu.window_trace.background_mode_mask());
            hasher.write_u8(ppu.extbg as u8);
            hasher.write_u8(ppu.color_math.color_source as u8);
            hasher.write_u8(ppu.bg3_priority as u8);
            for layer in &ppu.bg_layers {
                hasher.write_u32(layer.tileset_addr);
                hasher.write_u32(layer.tilemap_addr);
                hasher.write_u8(layer.horizontal_tilemap_count as u8);
                hasher.write_u8(layer.vertical_tilemap_count as u8);
                hasher.write_u8(layer.tile_size as u8);
            }
            hasher.write_u32(ppu.obj.tileset_addr);
            hasher.write_u32(ppu.obj.name_offset);
            hasher.write_u8(ppu.obj.sprite_size as u8);
            hasher.finish()
        };

        let window_regs = {
            use crate::ppu::regs::{W12SEL, W34SEL, WBGLOG, WH0, WOBJLOG, WOBJSEL};

            let mut hasher = DefaultHasher::new();
            for register in [W12SEL, W34SEL, WOBJSEL] {
                hasher.write_u8(ppu.windows.selector(register).raw());
            }
            for register in WH0..=WH0 + 3 {
                hasher.write_u8(ppu.windows.position(register));
            }
            for register in WBGLOG..=WOBJLOG {
                hasher.write_u8(ppu.windows.logic(register));
            }
            hasher.write_u8(ppu.main_screen_window.raw());
            hasher.write_u8(ppu.sub_screen_window.raw());
            hasher.finish()
        };

        let oam = {
            let mut hasher = DefaultHasher::new();
            for i in 0..128 {
                hasher.write_u8(ppu.oam.byte(i * 4 + 2));
                hasher.write_u8(ppu.oam.byte(i * 4 + 3));
            }
            for i in 0..32 {
                hasher.write_u8(ppu.oam.byte(512 + i) & 0b1010_1010);
            }
            hasher.finish()
        };

        Some(VideoGenerations {
            palette: mix([cgram, ppu.cgram.word_addr() as u64]),
            tileset: mix([vram, cgram, regs]),
            tilemap: mix([vram, cgram, regs]),
            tilemap_overlay: mix([window_regs, ppu.window_trace.generation()]),
            sprites: mix([vram, cgram, regs, oam]),
        })
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
            _ => TableView::default(),
        }
    }

    fn structured_view(&self, id: &str, selection: Option<usize>) -> StructuredView {
        let dsp = &self.core.apu.dsp;
        match id {
            dsp::REGISTERS_ID => dsp::registers_view(dsp),
            dsp::INSPECTOR_ID => dsp::inspector_view(dsp, selection),
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
        let mut manifest = standard_manifest(&self.cpus());
        manifest.workspaces.push(WorkspaceDescriptor::new(
            VIDEO_WORKSPACE,
            "PPU",
            WorkspaceIcon::Video,
        ));

        manifest.insert_after(
            PanelKind::Memory,
            PanelDescriptor::new(
                "framebuffer",
                "Screen",
                PanelKind::Framebuffer,
                DockLocation::Center,
                RefreshPolicy::Continuous,
            ),
        );
        manifest.insert_after(
            PanelKind::Breakpoints,
            PanelDescriptor::new(
                "dma",
                "DMA",
                PanelKind::Dma,
                DockLocation::Left,
                RefreshPolicy::OnPause,
            )
            .tag(CPU_WORKSPACE)
            .tag(VIDEO_WORKSPACE),
        );
        manifest.insert_after(
            PanelKind::Dma,
            PanelDescriptor::new(
                "palette",
                "Palettes",
                PanelKind::Palette,
                DockLocation::Left,
                RefreshPolicy::OnPause,
            )
            .tag(VIDEO_WORKSPACE),
        );
        manifest.insert_before(
            PanelKind::Mmio,
            PanelDescriptor::new(
                "ppu",
                "PPU",
                PanelKind::Ppu,
                DockLocation::Right,
                RefreshPolicy::OnPause,
            )
            .tag(VIDEO_WORKSPACE),
        );
        manifest.insert_before(
            PanelKind::Memory,
            PanelDescriptor::new(
                "tileset",
                "Tileset",
                PanelKind::Tileset,
                DockLocation::Center,
                RefreshPolicy::OnPause,
            )
            .tag(VIDEO_WORKSPACE),
        );
        manifest.insert_after(
            PanelKind::Tileset,
            PanelDescriptor::new(
                "tilemap",
                "Tilemap",
                PanelKind::Tilemap,
                DockLocation::Center,
                RefreshPolicy::OnPause,
            )
            .tag(VIDEO_WORKSPACE),
        );
        manifest.insert_after(
            PanelKind::Tilemap,
            PanelDescriptor::new(
                "sprites",
                "Sprites",
                PanelKind::Sprites,
                DockLocation::Center,
                RefreshPolicy::OnPause,
            )
            .tag(VIDEO_WORKSPACE),
        );

        // The sprites panel takes the memory viewer's place in the PPU workspace.
        manifest.restrict(PanelKind::Memory, CPU_WORKSPACE);

        manifest.workspaces.push(WorkspaceDescriptor::new(
            SOUND_WORKSPACE,
            "APU",
            WorkspaceIcon::Sound,
        ));

        manifest.insert_before(
            PanelKind::Memory,
            PanelDescriptor::new(
                "audio_scope",
                "Output",
                PanelKind::AudioScope,
                DockLocation::Center,
                RefreshPolicy::Continuous,
            )
            .tag(SOUND_WORKSPACE),
        );
        manifest.insert_after(
            PanelKind::AudioScope,
            PanelDescriptor::new(
                dsp::INSPECTOR_ID,
                "Audio Inspector",
                PanelKind::Structured,
                DockLocation::Center,
                RefreshPolicy::Continuous,
            )
            .selection_from(dsp::VOICES_ID)
            .tag(SOUND_WORKSPACE),
        );
        manifest.compose_in(
            SOUND_WORKSPACE,
            "audio_scope",
            dsp::INSPECTOR_ID,
            PanelGroupLayout::CompactColumns,
        );
        manifest.insert_after(
            PanelKind::AudioScope,
            PanelDescriptor::new(
                dsp::SCOPES_ID,
                "Voice Scopes",
                PanelKind::ChannelScopes,
                DockLocation::Center,
                RefreshPolicy::Continuous,
            )
            .tag(SOUND_WORKSPACE),
        );

        manifest.insert_before(
            PanelKind::Mmio,
            PanelDescriptor::new(
                dsp::VOICES_ID,
                "DSP Voices",
                PanelKind::Table,
                DockLocation::Right,
                RefreshPolicy::Continuous,
            )
            .tag(SOUND_WORKSPACE),
        );

        manifest.relocate_after("framebuffer", PanelKind::Controls, DockLocation::Left);
        manifest.insert_after(
            PanelKind::Framebuffer,
            PanelDescriptor::new(
                dsp::REGISTERS_ID,
                "DSP Registers",
                PanelKind::Structured,
                DockLocation::Left,
                RefreshPolicy::Continuous,
            )
            .tag(SOUND_WORKSPACE),
        );

        for id in [
            CPU_SPC,
            &format!("{CPU_SPC}.disasm"),
            &format!("{CPU_SPC}.callstack"),
            "breakpoints",
            "log",
        ] {
            manifest.tag(id, SOUND_WORKSPACE);
        }
        manifest.compose_in(
            SOUND_WORKSPACE,
            format!("{CPU_SPC}.disasm"),
            format!("{CPU_SPC}.callstack"),
            PanelGroupLayout::CompanionRail,
        );
        manifest.compose_in(
            CPU_WORKSPACE,
            "memory",
            "framebuffer",
            PanelGroupLayout::CompanionRail,
        );
        manifest.compose_in(
            VIDEO_WORKSPACE,
            "tileset",
            "tilemap",
            PanelGroupLayout::EqualColumns,
        );
        manifest.compose_in(
            VIDEO_WORKSPACE,
            "sprites",
            "framebuffer",
            PanelGroupLayout::EqualColumns,
        );

        manifest
    }

    fn caption(&self, label: &str) -> String {
        self::caption(label, &self.core)
    }

    fn memory_regions(&self) -> Vec<MemoryRegion> {
        let mut regions = vec![
            MemoryRegion::new("wram", "WRAM", 0, WRAM_SIZE),
            MemoryRegion::new("rom", "ROM", 0, self.core.mmu.cartridge.rom.len() as u32),
            MemoryRegion::new("ppu", "PPU/APU", PPU_BASE, PPU_SIZE),
            MemoryRegion::new("cpu", "CPU/DMA", CPU_BASE, CPU_SIZE),
            MemoryRegion::new("bus", "CPU Bus", 0, BUS_SIZE),
            MemoryRegion::new("spc", "SPC RAM", 0, 0x1_0000),
        ];
        if !self.core.mmu.cartridge.sram.is_empty() {
            regions.insert(
                1,
                MemoryRegion::new(
                    "sram",
                    "Cartridge SRAM",
                    0,
                    self.core.mmu.cartridge.sram.len() as u32,
                ),
            );
        }
        regions
    }

    fn read_memory(&self, region: &str, addr: u32, len: usize) -> Vec<u8> {
        fn window(data: &[u8], addr: u32, len: usize) -> Vec<u8> {
            let mut out = vec![0u8; len];

            let start = (addr as usize).min(data.len());
            let end = start.saturating_add(len).min(data.len());
            out[..end - start].copy_from_slice(&data[start..end]);

            out
        }

        match region {
            "wram" => window(&self.core.mmu.wram, addr, len),
            "sram" => window(&self.core.mmu.cartridge.sram, addr, len),
            "rom" => window(&self.core.mmu.cartridge.rom, addr, len),
            _ => (0..len as u32)
                .map(|i| self.region_byte(region, addr.wrapping_add(i)))
                .collect(),
        }
    }

    fn format_address(&self, region: &str, addr: u32) -> String {
        match region {
            "wram" => format!("{:02X}:{:04X}", 0x7E + (addr >> 16), addr & 0xFFFF),
            "sram" => format!("{addr:06X}"),
            "rom" => format!("{addr:06X}"),
            "bus" => format!("{:02X}:{:04X}", (addr >> 16) & 0xFF, addr & 0xFFFF),
            "spc" => format!("{:04X}", addr & 0xFFFF),
            _ => format!("00:{:04X}", addr & 0xFFFF),
        }
    }

    fn memory_highlights(&self, region: &str, addr: u32, len: usize) -> Vec<Highlight> {
        self.capture_frames
            .store(CAPTURE_FRAMES, std::sync::atomic::Ordering::Relaxed);

        (0..len as u32)
            .map(|i| match self.region_key(region, addr.wrapping_add(i)) {
                Some(key) if self.last_access.writes.contains(&key) => Highlight::Written,
                Some(key) if self.last_access.reads.contains(&key) => Highlight::Read,
                _ => Highlight::None,
            })
            .collect()
    }

    fn annotate(&self, addr: u32) -> String {
        crate::mmio::symbol((addr & 0xFFFF) as u16)
            .map(str::to_string)
            .unwrap_or_default()
    }

    fn latest_access(&self) -> Option<(Arc<str>, u32)> {
        self.track_frames
            .store(TRACK_FRAMES, std::sync::atomic::Ordering::Relaxed);

        let (kind, offset) = self.last_access.last?;
        let region = match kind {
            MemKind::Wram => "wram",
            MemKind::Sram => "sram",
            MemKind::Rom => "rom",
            MemKind::Io if (PPU_BASE..PPU_BASE + PPU_SIZE).contains(&offset) => "ppu",
            MemKind::Io if (CPU_BASE..CPU_BASE + CPU_SIZE).contains(&offset) => "cpu",
            MemKind::Io => "bus",
            MemKind::Spc => "spc",
        };
        Some((region.into(), offset))
    }
}

fn caption(label: &str, snes: &Snes) -> String {
    format!(
        "{label} @ {:02X}:{:04X}",
        snes.cpu.program_bank, snes.cpu.pc
    )
}

fn frame_interval(snes: &Snes) -> Duration {
    let clock_hz = snes.scheduler.master_clock_hz();
    let nanos = snes.scheduler.cycles_per_frame() as u128 * 1_000_000_000 / clock_hz as u128;
    Duration::from_nanos(nanos as u64)
}

const MAX_VIEW_FRAMES: usize = 64;

fn call_stack_view(
    frames: &[CallFrame],
    pc: u32,
    fmt: impl Fn(u32) -> String,
    symbol: impl Fn(u32) -> String,
) -> CallStackView {
    let mut rows = vec![StackFrame {
        kind: "PC".into(),
        location: fmt(pc).into(),
        symbol: symbol(pc).into(),
        caller: Arc::from(""),
    }];

    for f in frames.iter().rev().take(MAX_VIEW_FRAMES) {
        rows.push(StackFrame {
            kind: f.kind.tag().into(),
            location: fmt(f.target).into(),
            symbol: symbol(f.target).into(),
            caller: format!("from {}", fmt(f.caller)).into(),
        });
    }

    if frames.len() > MAX_VIEW_FRAMES {
        rows.push(StackFrame {
            kind: "…".into(),
            location: Arc::from(""),
            symbol: format!("{} older frames", frames.len() - MAX_VIEW_FRAMES).into(),
            caller: Arc::from(""),
        });
    }

    CallStackView { frames: rows }
}

fn cpu_view(cpu: &crate::Cpu) -> CpuView {
    let f = &cpu.flags;
    let acc_bits = if f.m() { 8 } else { 16 };
    let idx_bits = if f.x() { 8 } else { 16 };

    let p = f.raw() as u32;

    CpuView {
        isa: "Ricoh 5A22".into(),
        registers: vec![
            RegisterView::new("A", cpu.acc as u32, acc_bits),
            RegisterView::new("X", cpu.index_x as u32, idx_bits),
            RegisterView::new("Y", cpu.index_y as u32, idx_bits),
            RegisterView::new("S", cpu.stack_pointer as u32, 16),
            RegisterView::new("DB", cpu.data_bank as u32, 8),
            RegisterView::new("DP", cpu.direct_page as u32, 16),
            RegisterView::new("PB", cpu.program_bank as u32, 8),
            RegisterView::new("P", p, 8),
            RegisterView::new("PC", cpu.pc as u32, 16),
        ],
        flags: vec![
            FlagView::new("N", f.n()),
            FlagView::new("V", f.v()),
            FlagView::new("M", f.m()),
            FlagView::new("X", f.x()),
            FlagView::new("D", f.d()),
            FlagView::new("I", f.i()),
            FlagView::new("Z", f.z()),
            FlagView::new("C", f.c()),
        ],
    }
}

fn disasm_line_at(snes: &Snes, addr: u16, with_ea: bool) -> (DisasmLine, u8) {
    let bank = snes.cpu.program_bank;
    let d = snes.disassemble_at(addr);
    let len = d.length.max(1);
    let text = strip_symbol_sigils(&d.text, &SNES_SYNTAX);
    let tokens = tokenize(&text, self::mnemonic_kind(d.class), &SNES_SYNTAX);
    let ea = with_ea
        .then(|| crate::cpu::effective_address(snes, addr))
        .flatten();
    let note = self::disasm_note(&tokens, &d, bank, ea);

    let line = DisasmLine::new(
        addr as u32,
        format!("{bank:02X}:{addr:04X}"),
        self::read_bytes(snes, bank, addr, len),
        &*text,
        tokens,
    )
    .target(d.target)
    .flow(self::map_flow(d.flow))
    .note(note);

    (line, len)
}

fn disasm_view(snes: &Snes, base: u16, count: usize) -> Vec<DisasmLine> {
    let mut addr = base;
    let mut lines = Vec::with_capacity(count);

    for _ in 0..count {
        let (line, len) = self::disasm_line_at(snes, addr, false);
        lines.push(line);
        addr = addr.wrapping_add(len as u16);
    }

    lines
}

fn spc_cpu_view(spc: &spc700::Spc700) -> CpuView {
    let psw = &spc.psw;
    CpuView {
        isa: "S-SMP".into(),
        registers: vec![
            RegisterView::new("A", spc.a as u32, 8),
            RegisterView::new("X", spc.x as u32, 8),
            RegisterView::new("Y", spc.y as u32, 8),
            RegisterView::new("SP", spc.sp as u32, 8),
            RegisterView::new("PSW", psw.raw() as u32, 8),
            RegisterView::new("PC", spc.pc as u32, 16),
        ],
        flags: vec![
            FlagView::new("N", psw.negative()),
            FlagView::new("V", psw.overflow()),
            FlagView::new("P", psw.direct_page()),
            FlagView::new("B", psw.brk()),
            FlagView::new("H", psw.half_carry()),
            FlagView::new("I", psw.irq_enable()),
            FlagView::new("Z", psw.zero()),
            FlagView::new("C", psw.carry()),
        ],
    }
}

fn spc_disasm_view(apu: &Apu, base: u16, count: usize) -> Vec<DisasmLine> {
    let mut addr = base;
    let mut lines = Vec::with_capacity(count);

    for _ in 0..count {
        let (raw, len, target, flow) = spc700::disassemble(apu, addr);
        let len = len.max(1);
        let text = strip_symbol_sigils(&raw, &SPC_SYNTAX);
        let tokens = tokenize(&text, self::spc_mnemonic_kind(&text), &SPC_SYNTAX);
        let note = target.map_or(String::new(), |t| format!("→ ${t:04X}"));

        lines.push(
            DisasmLine::new(
                addr as u32,
                format!("{addr:04X}"),
                self::spc_read_bytes(apu, addr, len),
                &*text,
                tokens,
            )
            .target(target.map(|t| t as u32))
            .flow(self::map_flow(flow))
            .note(note),
        );

        addr = addr.wrapping_add(len as u16);
    }

    lines
}

fn spc_read_bytes(apu: &Apu, addr: u16, len: u8) -> String {
    self::format_bytes(len, |offset| apu.peek(addr.wrapping_add(offset)))
}

fn spc_mnemonic_kind(text: &str) -> TokenKind {
    let mnemonic = text.split_ascii_whitespace().next().unwrap_or(text);
    match mnemonic {
        "bra" | "beq" | "bne" | "bcs" | "bcc" | "bvs" | "bvc" | "bmi" | "bpl" | "jmp" | "call"
        | "pcall" | "tcall" | "ret" | "reti" | "cbne" | "dbnz" | "bbs" | "bbc" => {
            TokenKind::MnemonicFlow
        }
        "mov" | "movw" | "push" | "pop" => TokenKind::MnemonicMemory,
        "or" | "and" | "eor" | "cmp" | "adc" | "sbc" | "inc" | "dec" | "asl" | "lsr" | "rol"
        | "ror" | "xcn" | "mul" | "div" | "daa" | "das" | "addw" | "subw" | "incw" | "decw"
        | "cmpw" => TokenKind::MnemonicAlu,
        _ => TokenKind::MnemonicSystem,
    }
}

const SPC_SYNTAX: SyntaxSpec = SyntaxSpec {
    registers: &["A", "X", "Y", "SP", "YA", "PSW", "C"],
    symbols: crate::apu::is_symbol,
};

fn read_bytes(snes: &Snes, bank: u8, addr: u16, len: u8) -> String {
    self::format_bytes(len, |offset| {
        snes.peek8(Address::new(bank, addr.wrapping_add(offset)))
    })
}

fn format_bytes(len: u8, mut byte_at: impl FnMut(u16) -> u8) -> String {
    use std::fmt::Write;

    let mut out = String::with_capacity(len as usize * 3);
    for i in 0..len {
        if i != 0 {
            out.push(' ');
        }
        let _ = write!(out, "{:02X}", byte_at(i as u16));
    }
    out
}

fn disasm_note(tokens: &[DisasmToken], d: &Disasm, bank: u8, ea: Option<Address>) -> String {
    let mut parts: Vec<String> = tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Symbol)
        .filter_map(|t| {
            let addr = crate::mmio::address_of(&t.text)?;
            Some(format!("{} = ${addr:04X}", t.text))
        })
        .collect();

    if let Some(ea) = ea {
        let name = crate::mmio::symbol(ea.address()).map_or(String::new(), |n| format!(" = {n}"));
        parts.push(format!("→ {:02X}:{:04X}{name}", ea.bank(), ea.address()));
    } else if parts.is_empty()
        && d.class == InstructionClass::ControlFlow
        && let Some(target) = d.target
    {
        parts.push(format!("→ {bank:02X}:{target:04X}"));
    }

    parts.join("  ")
}

fn mnemonic_kind(class: InstructionClass) -> TokenKind {
    match class {
        InstructionClass::ControlFlow => TokenKind::MnemonicFlow,
        InstructionClass::Memory => TokenKind::MnemonicMemory,
        InstructionClass::Arithmetic => TokenKind::MnemonicAlu,
        InstructionClass::Transfer => TokenKind::MnemonicTransfer,
        InstructionClass::System => TokenKind::MnemonicSystem,
        InstructionClass::Other => TokenKind::Mnemonic,
    }
}

fn map_flow(flow: CoreFlow) -> FlowKind {
    match flow {
        CoreFlow::None => FlowKind::None,
        CoreFlow::CondBranch => FlowKind::CondBranch,
        CoreFlow::Branch => FlowKind::Branch,
        CoreFlow::Jump => FlowKind::Jump,
        CoreFlow::Call => FlowKind::Call,
        CoreFlow::Return => FlowKind::Return,
    }
}

const SNES_SYNTAX: SyntaxSpec = SyntaxSpec {
    registers: &["A", "X", "Y", "S"],
    symbols: crate::mmio::is_symbol,
};

fn mmio_component(addr: u16) -> (u8, &'static str) {
    match addr {
        0x2100..=0x213F => (0, "PPU"),
        0x2140..=0x2143 => (1, "APU"),
        0x420B..=0x420C | 0x4300..=0x43FF => (2, "DMA"),
        0x4200..=0x42FF => (3, "CPU"),
        _ => (9, "Other"),
    }
}

fn read_mmio(core: &Snes, addr: u16, bits: u8) -> u32 {
    let lo = core.peek_mmio(addr) as u32;
    if bits > 8 {
        lo | (core.peek_mmio(addr.wrapping_add(1)) as u32) << 8
    } else {
        lo
    }
}

fn mmio_field_views(fields: &[crate::mmio::FieldInfo], parent: u32) -> Vec<MmioFieldView> {
    let mut ordered: Vec<_> = fields.iter().collect();
    ordered.sort_by_key(|f| f.offset);
    ordered
        .into_iter()
        .map(|f| self::mmio_field_view(f, parent))
        .collect()
}

fn mmio_field_view(field: &crate::mmio::FieldInfo, parent: u32) -> MmioFieldView {
    use crate::mmio::FieldKind;

    let mask = if field.width >= 32 {
        u32::MAX
    } else {
        (1u32 << field.width) - 1
    };
    let value = (parent >> field.offset) & mask;

    let bit_label = if field.width == 1 {
        format!("[{}]", field.offset)
    } else {
        format!("[{}:{}]", field.offset, field.offset + field.width - 1)
    };

    let fmt_hex = || hex(value, field.width as u8);

    let (display, is_bool, children) = match field.kind {
        FieldKind::Bool => (
            if value != 0 { "on" } else { "off" }.to_string(),
            true,
            Vec::new(),
        ),
        FieldKind::Uint => (fmt_hex(), false, Vec::new()),
        FieldKind::Sint => {
            let sign_shift = 32 - field.width;
            let signed = ((value << sign_shift) as i32) >> sign_shift;
            (signed.to_string(), false, Vec::new())
        }
        FieldKind::Enum(info) => {
            let display = info
                .variants
                .iter()
                .find(|(disc, _)| *disc == value as u128)
                .map(|(_, name)| name.to_string())
                .unwrap_or_else(fmt_hex);
            (display, false, Vec::new())
        }
        FieldKind::Struct(fields) => (fmt_hex(), false, self::mmio_field_views(fields, value)),
    };

    MmioFieldView {
        name: field.name.into(),
        bit_label: bit_label.into(),
        value,
        display: display.into(),
        is_bool,
        children,
    }
}

fn dma_view(core: &Snes) -> DmaView {
    use crate::mmio::dma::transfer_pattern_offsets;
    use crate::mmio::regs::{AddressAdjustMode, Direction};

    let mdmaen = core.dma.mdmaen.raw();
    let hdmaen = core.dma.hdmaen.raw();

    let channels = core
        .dma
        .channels
        .iter()
        .enumerate()
        .map(|(i, ch)| {
            let b_port = crate::mmio::dma::B_BUS_BASE | ch.b_bus_addr as u16;
            let a_side = format!("${:?}", Address::from(ch.a_bus_addr.raw()));
            let b_side = hex(b_port as u32, 16);
            let b_bus = match crate::mmio::symbol(b_port) {
                Some(name) => format!("{b_side} {name}"),
                None => b_side.clone(),
            };
            let (source, dest) = match ch.params.direction() {
                Direction::AToB => (a_side, b_side),
                Direction::BToA => (b_side, a_side),
            };

            let pattern = ch.params.transfer_pattern();
            let offsets = transfer_pattern_offsets(pattern)
                .iter()
                .map(|offset| format!("+{offset}"))
                .collect::<Vec<_>>()
                .join(" ");
            let step = match ch.params.address_adjust_mode() {
                AddressAdjustMode::Increment => "increment",
                AddressAdjustMode::Decrement => "decrement",
                AddressAdjustMode::Fixed => "fixed",
            };

            let line = ch.line_counter;
            let details = vec![
                DmaChannelDetail::new("B-bus", b_bus),
                DmaChannelDetail::new("Pattern", format!("{} ({offsets})", pattern as u8)),
                DmaChannelDetail::new("Step", step),
                DmaChannelDetail::new(
                    "HDMA mode",
                    if ch.params.indirect() {
                        "indirect"
                    } else {
                        "direct"
                    },
                ),
                DmaChannelDetail::new(
                    "HDMA table",
                    format!(
                        "${:?}",
                        Address::new(ch.a_bus_addr.table_bank(), ch.table_addr)
                    ),
                ),
                DmaChannelDetail::new(
                    "HDMA lines",
                    format!(
                        "{}{}",
                        line.line_count(),
                        if line.repeat() { " (repeat)" } else { "" }
                    ),
                ),
                DmaChannelDetail::new(
                    "Indirect address",
                    format!(
                        "${:?}",
                        Address::new(
                            ch.byte_count.indirect_bank(),
                            ch.byte_count.indirect_offset()
                        )
                    ),
                ),
            ];

            DmaChannelView {
                id: format!("dma-{i}").into(),
                name: format!("{i}").into(),
                dma_enabled: mdmaen & (1 << i) != 0,
                hdma_enabled: hdmaen & (1 << i) != 0,
                source: source.into(),
                dest: dest.into(),
                count: hex(ch.byte_count.amount() as u32, 16).into(),
                details,
            }
        })
        .collect();

    DmaView { channels }
}

fn ppu_view(core: &Snes) -> PpuView {
    use crate::ppu::regs::{Priority, TileSize};

    let trace = core.ppu.window_trace.rows();
    let mut mode_names = Vec::new();
    let mut ranges = Vec::new();
    let mut range_start = 0;
    for line in 1..=trace.len() {
        if line == trace.len()
            || trace[line].bg_mode as u8 != trace[range_start].bg_mode as u8
            || trace[line].extbg != trace[range_start].extbg
        {
            let state = trace[range_start];
            let name = background_mode_name(state.bg_mode, state.extbg);
            if !mode_names.contains(&name) {
                mode_names.push(name.clone());
            }

            ranges.push(format!("{name} y{}-{}", range_start + 1, line));
            range_start = line;
        }
    }

    let mode = if mode_names.is_empty() {
        background_mode_name(core.ppu.bg_mode, core.ppu.extbg)
    } else {
        mode_names.join(" + ")
    };
    let bg3_priority = match core.ppu.bg3_priority {
        Priority::Normal => "normal",
        Priority::High => "high",
    };

    let mut properties = vec![
        PpuProperty::new("BG mode", mode).accented(),
        PpuProperty::new("BG3 priority", bg3_priority),
    ];
    if ranges.len() > 1 {
        properties.push(PpuProperty::new("Raster", ranges.join(", ")));
    }

    let groups = core
        .ppu
        .bg_layers
        .iter()
        .enumerate()
        .map(|(i, layer)| {
            let mode7_layer = mode7_bg_layer(core, i);
            let tile_size = if mode7_layer {
                "8x8"
            } else {
                match layer.tile_size {
                    TileSize::Size8x8 => "8x8",
                    TileSize::Size16x16 => "16x16",
                }
            };
            let depth = raster_depth(core, i);
            let tileset: Arc<str> =
                hex(if mode7_layer { 0 } else { layer.tileset_addr }, 16).into();
            let tilemap: Arc<str> =
                hex(if mode7_layer { 0 } else { layer.tilemap_addr }, 16).into();
            let name = bg_layer_name(core, i);

            PpuGroup::new(
                format!("ppu-bg{}", i + 1),
                name,
                vec![
                    PpuProperty::new("Tileset", tileset.clone()),
                    PpuProperty::new("Tilemap", tilemap),
                    PpuProperty::new("Tile size", tile_size),
                    PpuProperty::new("Depth", depth),
                ],
            )
            .summary(tileset)
        })
        .collect();

    PpuView { properties, groups }
}

fn background_mode_name(mode: crate::ppu::regs::BackgroundMode, extbg: bool) -> String {
    use crate::ppu::regs::BackgroundMode;

    match mode {
        BackgroundMode::Invalid => "Invalid".to_string(),
        BackgroundMode::Mode7 if extbg => "M7 EXTBG".to_string(),
        BackgroundMode::Mode7 => "M7".to_string(),
        mode => format!("Mode {}", mode as u8),
    }
}

fn raster_depth(core: &Snes, layer: usize) -> String {
    let mut depths = Vec::new();

    for state in core.ppu.window_trace.rows() {
        if let Some(depth) = state.bg_mode.bpp(layer)
            && !depths.contains(&depth)
        {
            depths.push(depth);
        }
    }

    if depths.is_empty()
        && let Some(depth) = core.ppu.bg_mode.bpp(layer)
    {
        depths.push(depth);
    }

    if depths.is_empty() {
        "unused".to_string()
    } else {
        depths.sort_unstable();
        depths
            .into_iter()
            .map(|depth| format!("{depth}bpp"))
            .collect::<Vec<_>>()
            .join(" / ")
    }
}

fn raster_uses_mode7(core: &Snes) -> bool {
    use crate::ppu::regs::BackgroundMode;

    core.ppu.bg_mode as u8 == BackgroundMode::Mode7 as u8
        || core.ppu.window_trace.uses_mode(BackgroundMode::Mode7)
}

fn mode7_bg_layer(core: &Snes, layer: usize) -> bool {
    layer == 0 && raster_uses_mode7(core)
}

fn bg_layer_name(core: &Snes, layer: usize) -> String {
    if mode7_bg_layer(core, layer) {
        "BG1 / M7".to_string()
    } else {
        format!("BG{}", layer + 1)
    }
}

fn bg_palette_colors(core: &Snes, layer: usize, bpp: u8, palette: usize) -> Vec<(u8, u8, u8)> {
    let (base, count) = core.ppu.bg_palette(layer, bpp, palette);
    (0..count)
        .map(|i| crate::ppu::cgram::rgb(core.ppu.cgram.word((base + i) as u8)))
        .collect()
}

fn tileset(core: &Snes, layer: usize, color: usize) -> Option<Tileset> {
    if mode7_bg_layer(core, layer) {
        return Some(mode7_tileset(&core.ppu));
    }

    let data = core.ppu.tileset(layer)?;

    let palette = core.ppu.bg_palette_index(layer, data.bpp, color);
    let colors = self::bg_palette_colors(core, layer, data.bpp, palette);

    let tiles = data
        .tiles
        .iter()
        .enumerate()
        .map(|(i, tile)| TilesetTile {
            addr: data.tile_addr(i),
            pixels: tile
                .decode(data.bpp)
                .as_flattened()
                .iter()
                .map(|&index| colors[index as usize])
                .collect(),
        })
        .collect();

    Some(Tileset {
        bpp: data.bpp,
        tile_bytes: data.tile_bytes(),
        tiles,
    })
}

fn mode7_colors(ppu: &crate::ppu::Ppu) -> Vec<(u8, u8, u8)> {
    use crate::ppu::regs::ColorSource;

    let direct_color = matches!(ppu.color_math.color_source, ColorSource::Direct);
    (0..=u8::MAX)
        .map(|index| {
            if direct_color {
                crate::ppu::cgram::rgb(crate::ppu::cgram::direct_color(0, index))
            } else {
                crate::ppu::cgram::rgb(ppu.cgram.word(index))
            }
        })
        .collect()
}

fn mode7_tile_pixels(ppu: &crate::ppu::Ppu, tile: u8) -> [[u8; 8]; 8] {
    std::array::from_fn(|y| {
        std::array::from_fn(|x| {
            crate::ppu::mode7::mode7_character_pixel(&ppu.vram, tile, x as u16, y as u16)
        })
    })
}

fn mode7_tileset(ppu: &crate::ppu::Ppu) -> Tileset {
    let colors = mode7_colors(ppu);
    let tiles = (0..=u8::MAX)
        .map(|tile| TilesetTile {
            addr: (tile as u32) << 6,
            pixels: mode7_tile_pixels(ppu, tile)
                .as_flattened()
                .iter()
                .map(|&index| colors[index as usize])
                .collect(),
        })
        .collect();

    Tileset {
        bpp: 8,
        tile_bytes: 64,
        tiles,
    }
}

fn object_pixels(
    width: usize,
    height: usize,
    flip_h: bool,
    flip_v: bool,
    colors: &[(u8, u8, u8)],
    mut tile: impl FnMut(usize, usize) -> [[u8; 8]; 8],
) -> Vec<(u8, u8, u8)> {
    let mut pixels = vec![(0, 0, 0); width * height];
    for ty in 0..height / 8 {
        for tx in 0..width / 8 {
            for (py, line) in tile(tx, ty).iter().enumerate() {
                for (px, &color) in line.iter().enumerate() {
                    let mut x = tx * 8 + px;
                    let mut y = ty * 8 + py;

                    if flip_h {
                        x = width - 1 - x;
                    }

                    if flip_v {
                        y = height - 1 - y;
                    }

                    pixels[y * width + x] = colors[color as usize];
                }
            }
        }
    }
    pixels
}

fn tilemap_window_overlays(core: &Snes, layer: usize) -> Vec<TilemapOverlay> {
    let fallback = crate::ppu::window::WindowDebugState {
        windows: core.ppu.windows,
        main_screen: core.ppu.main_screen_window,
        sub_screen: core.ppu.sub_screen_window,
        bg_mode: core.ppu.bg_mode,
        extbg: core.ppu.extbg,
    };
    let trace = core.ppu.window_trace.rows();
    let mut bands: [Vec<TilemapOverlayBand>; 2] = std::array::from_fn(|_| Vec::new());
    let mut descriptor = [None; 2];
    let mut mixed = [false; 2];

    for y in 0..crate::ppu::SCREEN_HEIGHT as usize {
        let state = trace.get(y).copied().unwrap_or(fallback);
        let (settings, bounds) = state.windows.row_mask(layer as u8).settings();
        let main = state.main_screen.enabled(layer);
        let sub = state.sub_screen.enabled(layer);

        for window in 0..2 {
            let (enabled, inverted) = settings[window];
            if !enabled {
                continue;
            }

            let current_descriptor = (inverted, main, sub);
            if descriptor[window].is_some_and(|value| value != current_descriptor) {
                mixed[window] = true;
            }
            descriptor[window] = Some(current_descriptor);

            let (left, right) = bounds[window];
            let width = if left <= right {
                (right - left) as u32 + 1
            } else {
                0
            };
            let band = TilemapOverlayBand {
                x: left as u32,
                y: y as u32,
                width,
                height: 1,
                inverted,
                active: main || sub,
            };

            if let Some(previous) = bands[window].last_mut()
                && previous.y + previous.height == band.y
                && previous.x == band.x
                && previous.width == band.width
                && previous.inverted == band.inverted
                && previous.active == band.active
            {
                previous.height += 1;
            } else {
                bands[window].push(band);
            }
        }
    }

    bands
        .into_iter()
        .enumerate()
        .filter_map(|(window, bands)| {
            let (inverted, main, sub) = descriptor[window]?;
            let label = if mixed[window] {
                format!("W{} VAR", window + 1)
            } else {
                let screen = match (main, sub) {
                    (true, true) => "M/S",
                    (true, false) => "M",
                    (false, true) => "S",
                    (false, false) => "off",
                };
                let inversion = if inverted { " INV" } else { "" };
                format!("W{} {screen}{inversion}", window + 1)
            };

            Some(TilemapOverlay {
                label: label.into(),
                slot: window,
                bands,
            })
        })
        .collect()
}

fn tilemap(core: &Snes, layer: usize) -> Option<Tilemap> {
    if mode7_bg_layer(core, layer) {
        return Some(mode7_tilemap(&core.ppu));
    }

    let data = core.ppu.tilemap(layer)?;
    let bpp = core.ppu.bg_mode.bpp(layer)?;

    let palettes: Vec<Vec<(u8, u8, u8)>> = (0..8)
        .map(|palette| self::bg_palette_colors(core, layer, bpp, palette))
        .collect();

    let size = data.tile_px as usize;

    let mut decoded: Vec<Option<[[u8; 8]; 8]>> = vec![None; 1024];
    let mut decode = |index: u16| {
        let index = index & 0x3FF;
        *decoded[index as usize].get_or_insert_with(|| {
            core.ppu
                .bg_tile(layer, index)
                .unwrap_or_default()
                .decode(bpp)
        })
    };

    let mut entries = Vec::with_capacity(data.entries.len());
    for (i, entry) in data.entries.iter().enumerate() {
        let (x, y) = (i as u32 % data.width, i as u32 / data.width);
        let colors = &palettes[entry.palette_selection as usize];

        let pixels =
            self::object_pixels(size, size, entry.flip_h, entry.flip_v, colors, |tx, ty| {
                decode(entry.tile_index + tx as u16 + ty as u16 * 16)
            });

        entries.push(TilemapEntry {
            addr: data.entry_addr(x, y),
            raw: entry.raw,
            tile_index: entry.tile_index,
            palette: entry.palette_selection,
            priority: entry.priority == crate::ppu::regs::Priority::High,
            flip_h: entry.flip_h,
            flip_v: entry.flip_v,
            pixels,
        });
    }

    Some(Tilemap {
        width: data.width as usize,
        height: data.height as usize,
        tile_px: data.tile_px as u8,
        entries,
        viewport: Some(TilemapViewport {
            x: data.scroll_x,
            y: data.scroll_y,
            width: crate::ppu::SCREEN_WIDTH,
            height: crate::ppu::SCREEN_HEIGHT,
        }),
    })
}

fn mode7_tilemap(ppu: &crate::ppu::Ppu) -> Tilemap {
    const MAP_TILES: usize = 128;

    let colors = mode7_colors(ppu);
    let decoded: Vec<_> = (0..=u8::MAX)
        .map(|tile| mode7_tile_pixels(ppu, tile))
        .collect();
    let mut entries = Vec::with_capacity(MAP_TILES * MAP_TILES);

    for address in 0..MAP_TILES * MAP_TILES {
        let tile = ppu.vram.word(address as u16) as u8;
        let pixels = decoded[tile as usize]
            .as_flattened()
            .iter()
            .map(|&raw| colors[raw as usize])
            .collect();
        entries.push(TilemapEntry {
            addr: address as u32,
            raw: tile as u16,
            tile_index: tile as u16,
            palette: 0,
            priority: false,
            flip_h: false,
            flip_v: false,
            pixels,
        });
    }

    Tilemap {
        width: MAP_TILES,
        height: MAP_TILES,
        tile_px: 8,
        entries,
        viewport: None,
    }
}

fn sprites(core: &Snes) -> Sprites {
    let palettes: Vec<Vec<(u8, u8, u8)>> = (0..8)
        .map(|palette| {
            let mut colors: Vec<_> = core
                .ppu
                .obj_palette(palette)
                .into_iter()
                .map(crate::ppu::cgram::rgb)
                .collect();
            // Color 0 is transparent; render it black.
            colors[0] = (0, 0, 0);
            colors
        })
        .collect();

    let entries = core
        .ppu
        .sprites()
        .into_iter()
        .map(|sprite| {
            let pixels = self::object_pixels(
                sprite.width as usize,
                sprite.height as usize,
                sprite.flip_h,
                sprite.flip_v,
                &palettes[sprite.palette as usize],
                |tx, ty| {
                    core.ppu
                        .obj_tile(sprite.sub_tile(tx as u32, ty as u32))
                        .decode(4)
                },
            );

            SpriteEntry {
                x: sprite.x,
                y: sprite.y,
                width: sprite.width,
                height: sprite.height,
                tile: sprite.tile,
                tile_addr: sprite.tile_addr,
                palette: sprite.palette,
                priority: sprite.priority,
                flip_h: sprite.flip_h,
                flip_v: sprite.flip_v,
                pixels,
            }
        })
        .collect();

    let size = core.ppu.obj.sprite_size;

    Sprites {
        small: size.small(),
        large: size.large(),
        entries,
    }
}

fn mmio_view(core: &Snes) -> MmioView {
    let mut symbols: Vec<_> = crate::mmio::symbols().collect();
    symbols.sort_by_key(|s| (self::mmio_component(s.addr).0, s.addr));

    let mut groups: Vec<MmioGroup> = Vec::new();
    for sym in symbols {
        let (_, component) = self::mmio_component(sym.addr);
        let value = self::read_mmio(core, sym.addr, sym.bits);
        let reg = self::mmio_register("mmio", 4, sym, value);

        match groups.last_mut() {
            Some(group) if &*group.name == component => group.registers.push(reg),
            _ => groups.push(MmioGroup {
                name: component.into(),
                registers: vec![reg],
            }),
        }
    }

    let spc = MmioGroup {
        name: "SPC".into(),
        registers: crate::apu::symbols()
            .map(|sym| self::mmio_register("spc", 2, sym, core.apu.peek(sym.addr) as u32))
            .collect(),
    };
    let after_apu = groups
        .iter()
        .position(|g| &*g.name == "APU")
        .map_or(groups.len(), |ix| ix + 1);
    groups.insert(after_apu, spc);

    MmioView { groups }
}

fn mmio_register(
    id_prefix: &str,
    addr_width: usize,
    sym: &crate::mmio::MmioSymbol,
    value: u32,
) -> MmioRegisterView {
    MmioRegisterView {
        id: format!("{id_prefix}-{:0width$X}", sym.addr, width = addr_width).into(),
        name: sym.name.into(),
        addr: hex(sym.addr as u32, (addr_width * 4) as u8).into(),
        value,
        bits: sym.bits,
        highlight: Highlight::None,
        fields: self::mmio_field_views(sym.fields, value),
    }
}

fn cartridge_view(cart: &Cartridge) -> CartridgeView {
    let speed = match cart.memory_map() {
        Ok(mm) => match mm.speed() {
            Speed::Fast => "Fast (3.58 MHz)",
            Speed::Slow => "Slow (2.68 MHz)",
        },
        Err(_) => "unknown",
    };

    CartridgeView {
        title: cart.title().into(),
        fields: vec![
            CartridgeField::new("Mapper", self::mapper_name(cart)).accented(),
            CartridgeField::new("Speed", speed),
            CartridgeField::new("Region", cart.region().to_string()),
            CartridgeField::new("ROM size", self::human_size(cart.rom.len())),
            CartridgeField::new("SRAM size", self::human_size(cart.sram.len())),
            CartridgeField::new("Header", format!("${:06X}", cart.header_offset())),
        ],
        cover: None,
    }
}

fn mapper_name(cart: &Cartridge) -> &'static str {
    match cart.memory_map() {
        Ok(mm) => match mm.map_mode() {
            MapMode::LoRom => "LoROM",
            MapMode::HiRom => "HiROM",
            MapMode::Sdd1 => "S-DD1",
            MapMode::Sa1 => "SA-1",
            MapMode::ExHiRom => "ExHiROM",
            MapMode::Spc7110 => "SPC7110",
        },
        Err(_) => "unknown",
    }
}

fn human_size(bytes: usize) -> String {
    const MB: usize = 1 << 20;
    const KB: usize = 1 << 10;

    if bytes >= MB && bytes.is_multiple_of(MB) {
        format!("{} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{} KB", bytes / KB)
    } else {
        format!("{bytes} B")
    }
}

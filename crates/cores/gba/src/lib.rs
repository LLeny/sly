pub mod bus;
pub mod cartridge;
pub mod cpu;
pub mod dma;
pub mod error;
pub mod gamepak;
mod input;
pub mod irq;
pub mod mmio;
pub mod ppu;
mod scheduler;
mod state;

#[cfg(feature = "debug")]
pub mod debug;

pub use cartridge::Cartridge;
pub use cpu::Cpu;

use std::time::Duration;

use crate::bus::{BusView, CpuBus};
use crate::cpu::{Bus, Mode, Psr};
use crate::dma::Dma;
use crate::gamepak::GamePak;
use crate::input::Input;
use crate::irq::Irq;
use crate::mmio::{Handler, Mmio, MmioMap, RegisterInfo};
use crate::ppu::{
    FRAME_CYCLES, HBLANK_START_CYCLES, Ppu, SCANLINE_CYCLES, VBLANK_END_CYCLES, VBLANK_START_CYCLES,
};
use crate::scheduler::{Event, EventKind, Scheduler};

const CLOCK_HZ: f64 = 16_777_216.0;

const SCREEN_WIDTH: u32 = 240;
const SCREEN_HEIGHT: u32 = 160;

const VIDEO_EVENTS: [(EventKind, u64, u64); 4] = [
    (EventKind::HBlankStart, HBLANK_START_CYCLES, SCANLINE_CYCLES),
    (EventKind::ScanlineStart, SCANLINE_CYCLES, SCANLINE_CYCLES),
    (EventKind::VBlankStart, VBLANK_START_CYCLES, FRAME_CYCLES),
    (EventKind::VBlankEnd, VBLANK_END_CYCLES, FRAME_CYCLES),
];

pub struct Gba {
    dma: Dma,
    cpu: Cpu,
    ppu: Ppu,
    gamepak: GamePak,
    input: Input,
    mmio: Mmio,
    scheduler: Scheduler,
    irq: Irq,
}

impl Gba {
    pub fn new(cartridge: Cartridge) -> Self {
        let mut map = MmioMap::new(Handler::MEMORY);
        ppu::io::register(&mut map);
        gamepak::io::register(&mut map);
        input::register(&mut map);
        irq::io::register(&mut map);
        dma::io::register(&mut map);

        let mut gba = Self {
            dma: Dma::new(),
            cpu: Cpu::new(),
            ppu: Ppu::new(),
            gamepak: GamePak::new(cartridge),
            input: Input::new(),
            mmio: Mmio::new(map),
            scheduler: Scheduler::new(),
            irq: Irq::new(),
        };

        gba.skip_bios();
        gba.schedule_video_events();

        gba
    }

    pub fn skip_bios(&mut self) {
        self.cpu = Cpu::new();
        self.cpu.cpsr = Psr::from_raw(0x1F);

        self.cpu.banks[Mode::Irq.index().unwrap()][13] = 0x0300_7FA0;
        self.cpu.banks[Mode::Supervisor.index().unwrap()][13] = 0x0300_7FE0;

        self.cpu.set_sp(0x0300_7F00);
        self.cpu.set_lr(0x0800_0000);
        self.cpu.set_pc(0x0800_0000);

        let mut bus = self.bus();
        bus.write8(0x0400_0300, 1); // POSTFLG
        bus.write16(0x0400_0204, 0); // WAITCNT
    }

    pub fn bus(&mut self) -> CpuBus<'_> {
        self.split().1
    }

    pub fn bus_view(&self) -> BusView<'_> {
        BusView {
            dma: &self.dma,
            ppu: &self.ppu,
            gamepak: &self.gamepak,
            input: &self.input,
            mmio: &self.mmio,
            irq: &self.irq,
        }
    }

    pub fn mmio_registers(&self) -> impl Iterator<Item = RegisterInfo> {
        self.mmio.map.registers().map(|register| register.info())
    }

    fn schedule_video_events(&mut self) {
        for (kind, deadline, period) in VIDEO_EVENTS {
            self.scheduler.schedule_periodic_at(deadline, period, kind);
        }
    }

    fn step_cpu(&mut self) {
        self.step_cpu_with(|cpu, mut bus| cpu.step(&mut bus));
    }

    fn step_cpu_with(&mut self, step: impl FnOnce(&mut Cpu, CpuBus<'_>)) {
        if self.irq.pending() && !self.cpu.cpsr.irq_disable() {
            self.cpu.take_irq();
        } else {
            let (cpu, bus) = self.split();
            step(cpu, bus);
        }

        // TODO: account for CPU internal cycles and bus wait states.
        self.scheduler.advance_by(1);
    }

    fn dispatch_due(&mut self) -> bool {
        self.dispatch_due_with(|_, _| {})
    }

    fn dispatch_due_with(&mut self, mut on_event: impl FnMut(&Self, EventKind)) -> bool {
        let mut vblank = false;
        while let Some(event) = self.scheduler.pop_due() {
            vblank |= event.kind == EventKind::VBlankStart;
            self.dispatch_event(event);
            on_event(self, event.kind);
        }

        vblank
    }

    fn dispatch_event(&mut self, event: Event) {
        match event.kind {
            EventKind::HBlankStart => {
                self.ppu.start_hblank();

                if self.ppu.dispstat.hblank_irq() {
                    self.irq.interrupt_request.set_hblank(true);
                }
            }
            EventKind::ScanlineStart => {
                self.ppu.start_scanline();

                if self.ppu.dispstat.vcounter() && self.ppu.dispstat.vcounter_irq() {
                    self.irq.interrupt_request.set_vcounter(true);
                }
            }
            EventKind::VBlankStart => {
                self.ppu.start_vblank();

                if self.ppu.dispstat.vblank_irq() {
                    self.irq.interrupt_request.set_vblank(true);
                }
            }
            EventKind::VBlankEnd => self.ppu.end_vblank(),
        }
    }

    #[cfg(feature = "debug")]
    fn patch(&mut self, address: u32, bytes: &[u8]) -> usize {
        use crate::gamepak::{SRAM_BASE, SRAM_END};
        use crate::mmio::{EWRAM_BASE, EWRAM_END, EWRAM_SIZE, IWRAM_BASE, IWRAM_END, IWRAM_SIZE};
        use crate::ppu::{
            OAM_BASE, OAM_END, OAM_SIZE, PALETTE_BASE, PALETTE_END, PALETTE_SIZE, VRAM_BASE,
            VRAM_END,
        };

        let (memory, offset): (&mut [u8], usize) = match address {
            EWRAM_BASE..=EWRAM_END => (&mut self.mmio.ewram, (address & (EWRAM_SIZE - 1)) as usize),
            IWRAM_BASE..=IWRAM_END => (&mut self.mmio.iwram, (address & (IWRAM_SIZE - 1)) as usize),
            PALETTE_BASE..=PALETTE_END => (
                &mut self.ppu.palette,
                (address & (PALETTE_SIZE - 1)) as usize,
            ),
            VRAM_BASE..=VRAM_END => (&mut self.ppu.vram, ppu::vram_offset(address)),
            OAM_BASE..=OAM_END => (&mut self.ppu.oam, (address & (OAM_SIZE - 1)) as usize),
            SRAM_BASE..=SRAM_END => (&mut self.gamepak.sram, (address - SRAM_BASE) as usize),
            _ => return 0,
        };

        let len = bytes.len().min(memory.len() - offset);
        memory[offset..offset + len].copy_from_slice(&bytes[..len]);
        if len > 0 {
            self.ppu.revisions.bump();
        }

        len
    }

    fn split(&mut self) -> (&mut Cpu, CpuBus<'_>) {
        let bus = CpuBus {
            dma: &mut self.dma,
            ppu: &mut self.ppu,
            gamepak: &mut self.gamepak,
            input: &mut self.input,
            mmio: &mut self.mmio,
            irq: &mut self.irq,
        };

        (&mut self.cpu, bus)
    }
}

impl core_model::Core for Gba {
    fn set_button(&mut self, port: usize, button: core_model::Button, pressed: bool) {
        self.input.set_button(port, button, pressed);
    }

    fn controllers(&self) -> Vec<core_model::ControllerDescriptor> {
        self.input.controllers()
    }

    fn reset(&mut self) {
        self.dma = Dma::new();
        self.ppu.reset();
        self.input = Input::new();
        self.scheduler = Scheduler::new();

        self.mmio.ewram.fill(0);
        self.mmio.iwram.fill(0);
        self.mmio.io.fill(0);
        self.skip_bios();
        self.schedule_video_events();
    }

    fn step(&mut self) {
        self.step_cpu();
        self.dispatch_due();
    }

    fn run_frame(&mut self) {
        loop {
            // TODO: Probably should do smth like scheduler.next_deadline()
            match self.scheduler.pop_due() {
                Some(event) => {
                    self.dispatch_event(event);

                    if event.kind == EventKind::VBlankStart {
                        break;
                    }
                }
                None => self.step_cpu(),
            }
        }
    }

    fn software_frame(&self) -> Option<core_model::SoftwareFrame<'_>> {
        Some(core_model::SoftwareFrame {
            width: SCREEN_WIDTH,
            height: SCREEN_HEIGHT,
            pixels: self.ppu.frame(),
        })
    }

    fn frame_interval(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(FRAME_CYCLES as f64 / CLOCK_HZ))
    }

    fn save_state(&self) -> Result<Vec<u8>, core_model::StateError> {
        self.encode_state()
    }

    fn load_state(&mut self, bytes: &[u8]) -> Result<(), core_model::StateError> {
        self.restore_state(bytes)
    }

    fn save_ram(&self) -> Option<&[u8]> {
        Some(&self.gamepak.sram)
    }

    fn load_save_ram(&mut self, bytes: &[u8]) -> bool {
        if bytes.len() != self.gamepak.sram.len() {
            return false;
        }
        self.gamepak.sram.copy_from_slice(bytes);
        true
    }

    fn cartridge_info(&self) -> Option<core_model::CartridgeInfo> {
        Some(self.gamepak.cartridge().info())
    }
}

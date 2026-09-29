pub mod bus;
pub mod cartridge;
pub mod cpu;
pub mod error;
pub mod gamepak;
pub mod mmio;
pub mod ppu;
mod scheduler;

#[cfg(feature = "debug")]
pub mod debug;

pub use cartridge::Cartridge;
pub use cpu::Cpu;

use std::time::Duration;

use crate::bus::{BusView, CpuBus};
use crate::cpu::{Bus, Mode, Psr};
use crate::gamepak::GamePak;
use crate::mmio::{Handler, Mmio, MmioMap, RegisterInfo};
use crate::ppu::{
    FRAME_CYCLES, HBLANK_START_CYCLES, Ppu, SCANLINE_CYCLES, VBLANK_END_CYCLES, VBLANK_START_CYCLES,
};
use crate::scheduler::{Event, EventKind, Scheduler};

const CLOCK_HZ: f64 = 16_777_216.0;

const SCREEN_WIDTH: u32 = 240;
const SCREEN_HEIGHT: u32 = 160;

pub struct Gba {
    cpu: Cpu,
    ppu: Ppu,
    gamepak: GamePak,
    mmio: Mmio,
    scheduler: Scheduler,
}

impl Gba {
    pub fn new(cartridge: Cartridge) -> Self {
        let mut map = MmioMap::new(Handler::MEMORY);
        ppu::io::register(&mut map);
        gamepak::io::register(&mut map);

        let mut gba = Self {
            cpu: Cpu::new(),
            ppu: Ppu::new(),
            gamepak: GamePak::new(cartridge),
            mmio: Mmio::new(map),
            scheduler: Scheduler::new(),
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
            ppu: &self.ppu,
            gamepak: &self.gamepak,
            mmio: &self.mmio,
        }
    }

    pub fn mmio_registers(&self) -> impl Iterator<Item = RegisterInfo> {
        self.mmio.map.registers().map(|register| register.info())
    }

    fn schedule_video_events(&mut self) {
        let events = [
            (HBLANK_START_CYCLES, SCANLINE_CYCLES, EventKind::HBlankStart),
            (SCANLINE_CYCLES, SCANLINE_CYCLES, EventKind::ScanlineStart),
            (VBLANK_START_CYCLES, FRAME_CYCLES, EventKind::VBlankStart),
            (VBLANK_END_CYCLES, FRAME_CYCLES, EventKind::VBlankEnd),
        ];

        for (deadline, period, kind) in events {
            self.scheduler.schedule_periodic_at(deadline, period, kind);
        }
    }

    fn step_cpu(&mut self) {
        let (cpu, mut bus) = self.split();
        cpu.step(&mut bus);

        // TODO: account for CPU internal cycles and bus wait states.
        self.scheduler.advance_by(1);
    }

    fn dispatch_event(&mut self, event: Event) {
        match event.kind {
            EventKind::HBlankStart => self.ppu.start_hblank(),
            EventKind::ScanlineStart => self.ppu.start_scanline(),
            EventKind::VBlankStart => self.ppu.start_vblank(),
            EventKind::VBlankEnd => self.ppu.end_vblank(),
        }
    }

    fn split(&mut self) -> (&mut Cpu, CpuBus<'_>) {
        let bus = CpuBus {
            ppu: &mut self.ppu,
            gamepak: &mut self.gamepak,
            mmio: &mut self.mmio,
        };

        (&mut self.cpu, bus)
    }
}

impl core_model::Core for Gba {
    fn reset(&mut self) {
        self.cpu = Cpu::new();
        self.ppu = Ppu::new();
        self.scheduler = Scheduler::new();

        self.schedule_video_events();
    }

    fn step(&mut self) {
        self.step_cpu();

        while let Some(event) = self.scheduler.pop_due() {
            self.dispatch_event(event);
        }
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

    fn cartridge_info(&self) -> Option<core_model::CartridgeInfo> {
        Some(self.gamepak.cartridge().info())
    }
}

mod alu;
pub mod apu;
pub mod callstack;
pub mod cartridge;
pub mod cpu;
#[cfg(feature = "debug")]
pub mod debug;
pub mod error;
pub mod interrupt;
pub mod joypad;
mod macros;
mod math;
pub mod mmio;
pub mod ppu;
pub mod scheduler;
pub mod state;

pub use cartridge::Cartridge;
pub use cpu::Cpu;
pub use error::SnesError;
pub use mmio::{CartridgeMap, MemoryMap, Mmio};

use crate::apu::Apu;
use crate::cpu::disasm::Disasm;
use crate::interrupt::Irq;
use crate::joypad::Joypads;
use crate::math::Math;
use crate::mmio::Address;
use crate::mmio::dma::{self, Dma};
use crate::ppu::Ppu;
use crate::scheduler::{EventKind, Scheduler};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Snes<M: MemoryMap = CartridgeMap> {
    pub cpu: Cpu,
    pub mmu: Mmio<M>,
    pub irq: Irq,
    pub dma: Dma,
    pub apu: Apu,
    pub ppu: Ppu,
    pub math: Math,
    pub joypads: Joypads,
    pub scheduler: Scheduler,
}

impl Snes {
    pub fn new(cartridge: Cartridge) -> Self {
        Self::with_mmu(Mmio::new(cartridge))
    }
}

impl<M: MemoryMap + 'static> Snes<M> {
    pub fn with_mmu(mmu: Mmio<M>) -> Self {
        let tv_system = mmu.cartridge.region().tv_system();
        let mut snes = Self {
            cpu: Cpu::new(0),
            mmu,
            irq: Irq::default(),
            dma: Dma::default(),
            apu: Apu::new(),
            ppu: Ppu::new(),
            math: Math::new(),
            joypads: Joypads::default(),
            scheduler: Scheduler::new(tv_system),
        };
        snes.reset();
        snes
    }

    pub fn reset(&mut self) {
        const RESET_VECTOR_ADDR: u16 = 0xFFFC;
        const BANK_ZERO: u8 = 0x00;

        let tv_system = self.mmu.cartridge.region().tv_system();

        self.cpu = Cpu::new(0);
        self.irq = Irq::default();
        self.dma = Dma::default();
        self.apu = Apu::new();
        self.ppu = Ppu::new();
        self.joypads.reset();

        self.scheduler = Scheduler::new(tv_system);
        let spc_delay = self.scheduler.spc_master_clocks(1);
        self.scheduler
            .schedule_event(spc_delay, EventKind::Spc700Tick);
        self.scheduler.schedule_periodic_at(
            scheduler::VBLANK_START_SCANLINE * scheduler::MASTER_CLOCKS_PER_SCANLINE,
            self.scheduler.cycles_per_frame(),
            EventKind::VblankStart,
        );
        self.scheduler
            .schedule_periodic(self.scheduler.cycles_per_frame(), EventKind::VblankEnd);
        self.scheduler.schedule_periodic(
            scheduler::MASTER_CLOCKS_PER_SCANLINE,
            EventKind::DrawScanline,
        );
        self.scheduler.schedule_periodic_at(
            dma::HDMA_INIT_CLOCK,
            self.scheduler.cycles_per_frame(),
            EventKind::InitHdma,
        );
        self.scheduler.schedule_periodic_at(
            dma::HDMA_RUN_CLOCK,
            scheduler::MASTER_CLOCKS_PER_SCANLINE,
            EventKind::RunHdma,
        );
        self.scheduler.schedule_periodic(
            scheduler::MASTER_CLOCKS_PER_SCANLINE,
            EventKind::HvTimerLine,
        );
        self.scheduler.schedule_periodic_at(
            scheduler::REFRESH_START_CLOCK,
            scheduler::MASTER_CLOCKS_PER_SCANLINE,
            EventKind::DramRefresh,
        );

        self.scheduler
            .schedule_periodic_spc(apu::SPC_TIMER01_DIVIDER, EventKind::Timer01Tick);
        self.scheduler
            .schedule_periodic_spc(apu::SPC_TIMER2_DIVIDER, EventKind::Timer2Tick);
        self.scheduler
            .schedule_periodic_spc(apu::DSP_SAMPLE_PERIOD_SPC, EventKind::DspTick);

        self.mmu.reset();
        self.cpu.pc = self.read16(Address::new(BANK_ZERO, RESET_VECTOR_ADDR));
    }

    pub fn disassemble_at(&self, pc: u16) -> Disasm {
        cpu::disassemble_at(self, pc)
    }

    pub fn disassemble_next(&self) -> Disasm {
        self.disassemble_at(self.cpu.pc)
    }

    pub fn step(&mut self) {
        let clocks_before = self.mmu.clocks;

        // TODO: STORES OCCUR ON FINAL BUS CYCLES!!!
        // Hold PPU stores their visible effects until scheduler
        // events crossed by the instruction, such as HDMA, have run!
        // THIS IS A HACK FOR MODE 7 IN SECRET OF MANA FOR NOW
        self.begin_cpu_instruction();
        cpu::step(self);
        self.end_cpu_instruction();

        self.scheduler.advance_by(self.mmu.clocks - clocks_before);
        while let Some((deadline, kind)) = self.scheduler.pop_current_event() {
            kind.dispatch(self, deadline);
        }
        self.commit_cpu_ppu_writes();
    }

    pub fn run_frame(&mut self) {
        let end = self.scheduler.frame_deadline();
        while self.scheduler.cycles() < end {
            self.step();
        }
    }
}

impl<M: MemoryMap + 'static> core_model::Core for Snes<M> {
    fn reset(&mut self) {
        Snes::reset(self);
    }

    fn step(&mut self) {
        Snes::step(self);
    }

    fn run_frame(&mut self) {
        Snes::run_frame(self);
    }

    fn audio_spec(&self) -> Option<core_model::AudioSpec> {
        Some(core_model::AudioSpec {
            sample_rate: (apu::SPC700_CLOCK / apu::DSP_SAMPLE_PERIOD_SPC) as u32,
            channels: 2,
        })
    }

    fn drain_audio(&mut self, out: &mut Vec<i16>) {
        self.apu.drain_audio(out);
    }

    fn set_button(&mut self, player: usize, button: core_model::Button, pressed: bool) {
        self.joypads.set_button(player, button, pressed);
    }

    fn session_info(&self) -> core_model::SessionInfo {
        core_model::SessionInfo {
            system: "SNES".into(),
            session: self.mmu.cartridge.title().into(),
            segments: vec![
                self.mmu.cartridge.region().to_string().into(),
                format!("{} KB", self.mmu.cartridge.rom.len() / 1024).into(),
            ],
        }
    }

    fn software_frame(&self) -> Option<core_model::SoftwareFrame<'_>> {
        Some(core_model::SoftwareFrame {
            width: ppu::SCREEN_WIDTH,
            height: ppu::SCREEN_HEIGHT,
            pixels: self.ppu.frame_buffer(),
        })
    }

    fn save_state(&self) -> Result<Vec<u8>, core_model::StateError> {
        Snes::save_state(self)
    }

    fn load_state(&mut self, bytes: &[u8]) -> Result<(), core_model::StateError> {
        Snes::load_state(self, bytes)
    }

    fn save_ram(&self) -> Option<&[u8]> {
        Snes::save_ram(self)
    }

    fn load_save_ram(&mut self, bytes: &[u8]) -> bool {
        Snes::load_save_ram(self, bytes)
    }

    fn frame_interval(&self) -> Option<std::time::Duration> {
        let nanos = self.scheduler.cycles_per_frame() as u128 * 1_000_000_000
            / self.scheduler.master_clock_hz() as u128;
        Some(std::time::Duration::from_nanos(nanos as u64))
    }
}

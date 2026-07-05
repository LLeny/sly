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
#[cfg(feature = "debug")]
use crate::mmio::MemoryAccess;
use crate::mmio::dma::Dma;
use crate::ppu::Ppu;
use crate::scheduler::Scheduler;

pub const CYCLES_PER_STEP: u64 = 30; // TODO: CYCLE ACCURACY

#[cfg(feature = "debug")]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Component {
    Cpu,
    Spc,
}

#[cfg(feature = "debug")]
pub struct StepTrace {
    pub access: MemoryAccess,
    pub fault: Option<Component>,
}

pub struct Snes<M: MemoryMap = CartridgeMap> {
    pub cpu: Cpu,
    pub mmu: Mmio<M>,
    pub irq: Irq,
    pub dma: Dma,
    pub apu: Apu,
    pub ppu: Ppu,
    pub math: Math,
    pub joypads: Joypads,
    pub scheduler: Scheduler<M>,
    #[cfg(feature = "debug")]
    pub executing: Component,
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
            #[cfg(feature = "debug")]
            executing: Component::Cpu,
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
        self.scheduler
            .schedule_periodic(apu::SNES_CYCLES_PER_SPC700_CYCLE, scheduler::spc700_tick);
        self.scheduler.schedule_periodic_at(
            scheduler::VBLANK_START_SCANLINE * scheduler::MASTER_CLOCKS_PER_SCANLINE,
            self.scheduler.cycles_per_frame(),
            ppu::vblank_start,
        );
        self.scheduler
            .schedule_periodic(self.scheduler.cycles_per_frame(), ppu::vblank_end);
        self.scheduler
            .schedule_periodic(scheduler::MASTER_CLOCKS_PER_SCANLINE, ppu::draw_scanline);

        // TODO: Could schedule these when enabled?
        self.scheduler
            .schedule_periodic(apu::TIMER01_PERIOD, apu::timer01_tick);
        self.scheduler
            .schedule_periodic(apu::TIMER2_PERIOD, apu::timer2_tick);

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
        #[cfg(feature = "debug")]
        {
            self.executing = Component::Cpu;
        }

        cpu::step(self);

        self.scheduler.advance_by(CYCLES_PER_STEP);
        while let Some((deadline, callback)) = self.scheduler.pop_current_event() {
            callback(self, deadline);
        }
    }

    pub fn run_frame(&mut self) {
        let end = self.scheduler.frame_deadline();
        while self.scheduler.cycles() < end {
            self.step();
        }
    }

    #[cfg(feature = "debug")]
    pub fn step_traced(&mut self) -> StepTrace {
        self.mmu.access.start();
        self.apu.access.start();
        self.apu.access.resume();

        let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.step())).is_ok();

        self.mmu.access.stop();
        self.apu.access.stop();

        let mut access = self.mmu.access.take();
        access.merge(&self.apu.access.take());

        StepTrace {
            access,
            fault: (!ok).then_some(self.executing),
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
}

mod alu;
pub mod apu;
pub mod cartridge;
pub mod cpu;
pub mod error;
pub mod interrupt;
mod macros;
pub mod mmio;
pub mod scheduler;

pub use cartridge::Cartridge;
pub use cpu::Cpu;
pub use error::SnesError;
pub use mmio::{CartridgeMap, MemoryMap, Mmio};

use crate::apu::Apu;
use crate::cpu::disasm::Disasm;
use crate::interrupt::Irq;
use crate::mmio::Address;
#[cfg(feature = "debug")]
use crate::mmio::MemoryAccess;
use crate::mmio::dma::Dma;
use crate::scheduler::Scheduler;

pub const CYCLES_PER_STEP: u64 = 30; // TODO: CYCLE ACCURACY

pub struct Snes<M: MemoryMap = CartridgeMap> {
    pub cpu: Cpu,
    pub mmu: Mmio<M>,
    pub irq: Irq,
    pub dma: Dma,
    pub apu: Apu,
    pub scheduler: Scheduler<M>,
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
        self.scheduler = Scheduler::new(tv_system);
        self.scheduler
            .schedule_event(apu::SNES_CYCLES_PER_SPC700_CYCLE, scheduler::spc700_tick);
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
        cpu::step(self);

        self.scheduler.advance_by(CYCLES_PER_STEP);
        while let Some((deadline, mut callback)) = self.scheduler.pop_current_event() {
            callback(self, deadline);
        }
    }

    #[cfg(feature = "debug")]
    pub fn trace(&mut self, body: impl FnOnce(&mut Self)) -> MemoryAccess {
        self.mmu.access.start();
        body(self);
        self.mmu.access.stop();
        self.mmu.access.snapshot()
    }
}

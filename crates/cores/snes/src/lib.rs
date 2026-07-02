pub mod apu;
pub mod cartridge;
pub mod cpu;
pub mod error;
pub mod interrupt;
mod macros;
pub mod mmio;

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

// TODO: lol
pub const INSTRUCTIONS_PER_FRAME: usize = 10_000;

pub struct Snes<M: MemoryMap = CartridgeMap> {
    pub cpu: Cpu,
    pub mmu: Mmio<M>,
    pub irq: Irq,
    pub dma: Dma,
    pub apu: Apu,
}

impl Snes {
    pub fn new(cartridge: Cartridge) -> Self {
        Self::with_mmu(Mmio::new(cartridge))
    }
}

impl<M: MemoryMap> Snes<M> {
    pub fn with_mmu(mmu: Mmio<M>) -> Self {
        let mut snes = Self {
            cpu: Cpu::new(0),
            mmu,
            irq: Irq::default(),
            dma: Dma::default(),
            apu: Apu::new(),
        };
        snes.reset();
        snes
    }

    pub fn reset(&mut self) {
        const RESET_VECTOR_ADDR: u16 = 0xFFFC;
        const BANK_ZERO: u8 = 0x00;

        self.cpu = Cpu::new(0);
        self.irq = Irq::default();
        self.dma = Dma::default();
        self.apu = Apu::new();
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
        apu::spc700::step(&mut self.apu);
    }

    pub fn run_frame(&mut self) {
        for _ in 0..INSTRUCTIONS_PER_FRAME {
            self.step();
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

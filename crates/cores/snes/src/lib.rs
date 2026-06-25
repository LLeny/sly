pub mod cartridge;
pub mod cpu;
pub mod error;
pub mod interrupt;
pub mod mmu;

pub use cartridge::Cartridge;
pub use cpu::{Cpu, Disasm};
pub use error::SnesError;
pub use mmu::Mmu;

use crate::interrupt::Irq;

pub struct Snes {
    pub cpu: Cpu,
    pub mmu: Mmu,
    pub irq: Irq,
}

impl Snes {
    pub fn new(cartridge: Cartridge) -> Self {
        const RESET_VECTOR_ADDR: u16 = 0xFFFC;
        const BANK_ZERO: u8 = 0x00;

        let mut snes = Self {
            cpu: Cpu::new(0),
            mmu: Mmu::new(cartridge),
            irq: Irq::default(),
        };
        snes.cpu.pc = snes.read16(BANK_ZERO, RESET_VECTOR_ADDR);
        snes
    }

    pub fn disassemble_at(&self, pc: u16) -> Disasm {
        cpu::disassemble_at(self, pc)
    }

    pub fn disassemble_next(&self) -> Disasm {
        self.disassemble_at(self.cpu.pc)
    }

    pub fn step(&mut self) {
        cpu::step(self);
    }
}

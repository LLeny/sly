pub mod cartridge;
pub mod cpu;
pub mod error;
pub mod interrupt;
mod macros;
pub mod mmio;

pub use cartridge::Cartridge;
pub use cpu::Cpu;
pub use error::SnesError;
pub use mmio::Mmio;

use crate::cpu::disasm::Disasm;
use crate::interrupt::Irq;
use crate::mmio::Address;
use crate::mmio::dma::Dma;

pub struct Snes {
    pub cpu: Cpu,
    pub mmu: Mmio,
    pub irq: Irq,
    pub dma: Dma,
}

impl Snes {
    pub fn new(cartridge: Cartridge) -> Self {
        const RESET_VECTOR_ADDR: u16 = 0xFFFC;
        const BANK_ZERO: u8 = 0x00;

        let mut snes = Self {
            cpu: Cpu::new(0),
            mmu: Mmio::new(cartridge),
            irq: Irq::default(),
            dma: Dma::default(),
        };
        snes.cpu.pc = snes.read16(Address::new(BANK_ZERO, RESET_VECTOR_ADDR));
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

    pub fn run_frame(&mut self) {
        // TODO: lol
        const INSTRUCTIONS_PER_FRAME: usize = 10_000;

        for _ in 0..INSTRUCTIONS_PER_FRAME {
            self.step();
        }
    }
}

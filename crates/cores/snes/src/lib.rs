pub mod cartridge;
pub mod error;
pub mod mmu;

pub use cartridge::Cartridge;
pub use error::SnesError;
pub use mmu::Mmu;
pub use wdc_65c816::{Cpu, Disasm};

pub struct Snes {
    pub cpu: Cpu,
    pub mmu: Mmu,
}

impl Snes {
    pub fn new(cartridge: Cartridge) -> Self {
        const RESET_VECTOR_ADDR: u16 = 0xFFFC;
        const BANK_ZERO: u8 = 0x00;

        let mmu = Mmu::new(cartridge);
        let cpu = Cpu::new(mmu.read16(BANK_ZERO, RESET_VECTOR_ADDR));

        Self { cpu, mmu }
    }

    pub fn disassemble_at(&self, pc: u16) -> Disasm {
        wdc_65c816::disassemble_at(&self.cpu, &self.mmu, pc)
    }

    pub fn disassemble_next(&self) -> Disasm {
        self.disassemble_at(self.cpu.pc)
    }

    pub fn step(&mut self) {
        self.cpu.step(&mut self.mmu);
    }
}

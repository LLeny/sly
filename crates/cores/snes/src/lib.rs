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
    pub cartridge: Cartridge,
}

impl Snes {
    pub fn new(cartridge: Cartridge) -> Self {
        let mut mmu = Mmu::default();

        const HEADER_ADDR: u16 = 0xFFC0;
        mmu.load(HEADER_ADDR, cartridge.raw_header());

        const RESET_VECTOR_ADDR: u16 = 0xFFFC;
        let cpu = Cpu::new(mmu.read16(RESET_VECTOR_ADDR));

        Self {
            cpu,
            mmu,
            cartridge,
        }
    }

    pub fn disassemble_at(&self, pc: u16) -> Disasm {
        wdc_65c816::disassemble_at(&self.cpu, &self.mmu, pc)
    }

    pub fn disassemble_next(&self) -> Disasm {
        self.disassemble_at(self.cpu.pc)
    }
}

pub mod cartridge;
pub mod error;

pub use cartridge::Cartridge;
pub use error::SnesError;
pub use wdc_65c816::{self, Cpu, Disasm, combo, disassemble_at};

#[derive(Default)]
pub struct Snes {
    pub cpu: Cpu,
}

impl Snes {
    pub fn new() -> Self {
        Self { cpu: Cpu::new() }
    }

    pub fn load(&mut self, addr: u16, bytes: &[u8]) {
        self.cpu.load(addr, bytes);
    }

    pub fn disassemble_at(&self, pc: u16) -> Disasm {
        disassemble_at(&self.cpu, pc)
    }

    pub fn disassemble_next(&self) -> Disasm {
        self.disassemble_at(self.cpu.pc)
    }
}

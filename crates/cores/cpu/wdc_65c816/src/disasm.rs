use crate::cpu::{Cpu, FLAG_M, FLAG_X};
use crate::decoder::{self, DisasmCtx};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disasm {
    pub text: String,
    pub length: u8,
}

impl std::fmt::Display for Disasm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.text)
    }
}

#[inline]
pub fn disassemble_at(cpu: &Cpu, pc: u16) -> Disasm {
    let (text, length) = decoder::disasm_ctx(pc as u64, cpu);
    Disasm { text, length }
}

impl DisasmCtx for Cpu {
    fn read_u8(&self, addr: u64) -> u8 {
        self.mem[(addr as usize) & 0xffff]
    }

    fn mode(&self, name: &str) -> u64 {
        match name {
            "m" => (self.flags & FLAG_M != 0) as u64,
            "x" => (self.flags & FLAG_X != 0) as u64,
            _ => 0,
        }
    }
}

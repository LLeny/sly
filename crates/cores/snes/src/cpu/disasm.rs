use super::decoder::{self, DisasmCtx};
use crate::Snes;

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

struct DisasmView<'a> {
    snes: &'a Snes,
}

impl DisasmCtx for DisasmView<'_> {
    fn read_u8(&self, addr: u64) -> u8 {
        self.snes.read(addr as u32)
    }

    fn mode(&self, name: &str) -> u64 {
        match name {
            "m" => self.snes.cpu.flags.m() as u64,
            "x" => self.snes.cpu.flags.x() as u64,
            _ => 0,
        }
    }
}

#[inline]
pub fn disassemble_at(snes: &Snes, pc: u16) -> Disasm {
    let view = DisasmView { snes };
    let (text, length) = decoder::disasm_ctx(pc as u64, &view);
    Disasm { text, length }
}

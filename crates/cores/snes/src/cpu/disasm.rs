use super::decoder;
use crate::Snes;
use crate::cpu::CodeReader;

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
pub fn disassemble_at(snes: &Snes, pc: u16) -> Disasm {
    let reader = CodeReader { snes };
    let addr = pc as u64;
    let modes = decoder::pack_modes(snes.cpu.flags.m() as u64, snes.cpu.flags.x() as u64);

    let (inst, length) = decoder::decode(modes, addr, &reader);
    let text = inst.render(addr, &reader);

    Disasm { text, length }
}

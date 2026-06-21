use crate::bus::Bus;
use crate::cpu::Cpu;
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

pub struct DisasmView<'a, B: Bus + ?Sized> {
    pub cpu: &'a Cpu,
    pub bus: &'a B,
}

impl<B: Bus + ?Sized> DisasmCtx for DisasmView<'_, B> {
    fn read_u8(&self, addr: u64) -> u8 {
        self.bus.read(addr as u32)
    }

    fn mode(&self, name: &str) -> u64 {
        match name {
            "m" => self.cpu.flags.m() as u64,
            "x" => self.cpu.flags.x() as u64,
            _ => 0,
        }
    }
}

#[inline]
pub fn disassemble_at<B: Bus + ?Sized>(cpu: &Cpu, bus: &B, pc: u16) -> Disasm {
    let view = DisasmView { cpu, bus };
    let (text, length) = decoder::disasm_ctx(pc as u64, &view);
    Disasm { text, length }
}

#[allow(non_snake_case)]
mod generated {
    chipi_macros::isa!("../../../../submodules/chipi-spec2/snes/ricoh_5a22.chipi");
}

pub use generated::Ricoh5A22 as decoder;

pub mod cpu;
pub mod disasm;

pub use cpu::Cpu;
pub use disasm::{Disasm, disassemble_at};

#[inline]
pub fn combo(m8: bool, x8: bool) -> usize {
    decoder::pack_modes(m8 as u64, x8 as u64)
}

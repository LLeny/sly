#[allow(non_snake_case)]
mod generated {
    chipi_macros::isa!("../../../submodules/chipi-spec/snes/superfx.chipi");
}

pub use generated::SuperFx::*;

impl Instruction {
    pub(crate) fn low_nibble(self) -> u8 {
        self.0 & 0x0F
    }
}

pub(crate) fn mode_combo(sfr: &crate::superfx::regs::Sfr) -> usize {
    pack_modes(sfr.alt1() as u64, sfr.alt2() as u64, sfr.with() as u64)
}

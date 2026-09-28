#[allow(non_snake_case)]
mod generated {
    chipi_macros::isa!("../../../submodules/chipi-spec/gba/arm.chipi");
    chipi_macros::isa!("../../../submodules/chipi-spec/gba/thumb.chipi");
}

pub use generated::{ARM as arm, Thumb as thumb};

impl thumb::Instruction {
    pub fn rlist(&self) -> u16 {
        let mut rlist = self.list() as u16;
        if self.extra() {
            match self.mnemonic() {
                thumb::Mnemonic::Push => rlist |= 1 << 14,
                thumb::Mnemonic::Pop => rlist |= 1 << 15,
                _ => {}
            }
        }
        rlist
    }
}

pub fn registers(mut mask: u16) -> impl Iterator<Item = usize> {
    std::iter::from_fn(move || {
        let reg = (mask != 0).then(|| mask.trailing_zeros() as usize)?;
        mask &= mask - 1;
        Some(reg)
    })
}

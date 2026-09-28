#[allow(non_snake_case)]
mod generated {
    chipi_macros::isa!("../../../submodules/chipi-spec/gba/arm.chipi");
    chipi_macros::isa!("../../../submodules/chipi-spec/gba/thumb.chipi");
}

pub use generated::{ARM as arm, Thumb as thumb};

impl thumb::Instruction {
    pub fn rlist(&self) -> impl Iterator<Item = usize> + Clone {
        let mut rlist = self.list() as u16;
        if self.extra() {
            match self.mnemonic() {
                thumb::Mnemonic::Push => rlist |= 1 << 14,
                thumb::Mnemonic::Pop => rlist |= 1 << 15,
                _ => {}
            }
        }
        (0..16).filter(move |&i| (rlist >> i) & 1 != 0)
    }
}

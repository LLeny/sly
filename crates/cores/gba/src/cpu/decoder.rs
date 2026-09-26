#[allow(non_snake_case)]
mod generated {
    chipi_macros::isa!("../../../submodules/chipi-spec/gba/arm.chipi");
    chipi_macros::isa!("../../../submodules/chipi-spec/gba/thumb.chipi");
}

pub use generated::{ARM as arm, Thumb as thumb};

#[allow(non_snake_case)]
mod generated {
    chipi_macros::isa!("../../../submodules/chipi-spec/snes/spc700.chipi", style = enum);
}

pub use generated::Spc700::*;

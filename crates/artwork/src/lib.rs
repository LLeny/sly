pub mod layle;
pub mod libretro;

pub use core_model::System;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Header {
    None,
    SnesCopier,
}

impl Header {
    pub fn strip(self, rom: &[u8]) -> &[u8] {
        match self {
            Self::SnesCopier if rom.len() % 1024 == 512 => &rom[512..],
            _ => rom,
        }
    }
}

#[derive(Clone, Copy)]
pub struct SystemInfo {
    pub libretro_name: &'static str,
    pub header: Header,
}

pub const fn info(system: System) -> SystemInfo {
    match system {
        System::Snes => SystemInfo {
            libretro_name: "Nintendo - Super Nintendo Entertainment System",
            header: Header::SnesCopier,
        },
    }
}

pub fn rom_sha1(system: System, rom: &[u8]) -> String {
    use std::fmt::Write;

    use sha1::{Digest, Sha1};

    Sha1::digest(self::info(system).header.strip(rom))
        .iter()
        .fold(String::with_capacity(40), |mut hex, byte| {
            write!(hex, "{byte:02x}").expect("writing to a String cannot fail");
            hex
        })
}

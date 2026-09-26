use std::fmt::Write;

use core_model::{CartridgeInfo, RomIdentity};
use sha1::{Digest, Sha1};

use crate::error::GbaError;

pub struct Cartridge {
    rom: Vec<u8>,
}

impl Cartridge {
    pub fn new(rom: Vec<u8>) -> Result<Self, GbaError> {
        Ok(Self { rom })
    }

    pub fn entry_point(&self) -> u32 {
        u32::from_le_bytes(self.rom[0x34..0x38].try_into().unwrap())
    }

    pub fn title(&self) -> String {
        String::from_utf8_lossy(&self.rom[0xA0..0xAC])
            .trim_end_matches('\0')
            .to_string()
    }

    pub fn info(&self) -> CartridgeInfo {
        CartridgeInfo {
            system: core_model::System::Gba,
            title: self.title(),
            region: None,
            tv_system: None,
            rom: self::identity(&self.rom),
            backup: None,
        }
    }
}

pub fn identity(rom: &[u8]) -> RomIdentity {
    let sha1 = Sha1::digest(rom)
        .iter()
        .fold(String::with_capacity(40), |mut hex, byte| {
            write!(hex, "{byte:02x}").expect("writing to a String cannot fail");
            hex
        });

    RomIdentity {
        sha1,
        crc32: crc32fast::hash(rom),
        len: rom.len(),
    }
}

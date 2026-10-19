use std::fmt::Write;

use core_model::{CartridgeInfo, Region, RomIdentity};
use sha1::{Digest, Sha1};

#[derive(Clone, Debug, Default)]
pub struct CartridgeHeader {
    title: String,
    region: Option<Region>,
    entry_point: u32,
}

impl CartridgeHeader {
    pub fn parse(rom: &[u8]) -> Self {
        Self {
            title: String::from_utf8_lossy(rom.get(0xA0..0xAC).unwrap_or(&[]))
                .trim_end_matches('\0')
                .to_string(),
            region: match rom.get(0xAF).copied().unwrap_or(0) {
                b'J' => Some(Region::Japan),
                b'E' => Some(Region::NorthAmerica),
                b'P' => Some(Region::Europe),
                b'D' => Some(Region::Germany),
                b'F' => Some(Region::France),
                b'I' => Some(Region::Italy),
                b'S' => Some(Region::Spain),
                _ => None,
            },
            entry_point: rom
                .get(0x34..0x38)
                .and_then(|bytes| bytes.try_into().ok())
                .map(u32::from_le_bytes)
                .unwrap_or(0),
        }
    }

    pub fn title(&self) -> String {
        self.title.clone()
    }

    pub fn region(&self) -> Option<Region> {
        self.region
    }

    pub fn entry_point(&self) -> u32 {
        self.entry_point
    }
}

pub fn title(rom: &[u8]) -> String {
    CartridgeHeader::parse(rom).title()
}

pub fn entry_point(rom: &[u8]) -> u32 {
    CartridgeHeader::parse(rom).entry_point()
}

pub fn info(cartridge: &core_model::RomFile) -> CartridgeInfo {
    let header = CartridgeHeader::parse(&cartridge.data);
    CartridgeInfo {
        system: core_model::System::Gba,
        title: header.title(),
        region: header.region(),
        tv_system: None,
        rom: identity(&cartridge.data),
        backup: None,
        display_rotation: core_model::DisplayRotation::None,
    }
}

pub fn region(rom: &[u8]) -> Option<Region> {
    CartridgeHeader::parse(rom).region()
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

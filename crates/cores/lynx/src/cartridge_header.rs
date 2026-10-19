use std::fmt::Write;

use core_model::{CartridgeError, CartridgeInfo, DisplayRotation, RomFile, RomIdentity};
use holani::cartridge::lnx_header::LNXRotation;
use sha1::{Digest, Sha1};

#[derive(Clone, Debug, Default)]
pub struct CartridgeHeader {
    title: String,
    eeprom: bool,
}

impl CartridgeHeader {
    pub fn parse(rom: &[u8]) -> Option<Self> {
        if rom.get(0..4) != Some(b"LYNX") {
            return None;
        }

        let title = String::from_utf8_lossy(rom.get(10..42).unwrap_or(&[]))
            .trim_end_matches('\0')
            .to_string();

        let eeprom = rom
            .get(59)
            .copied()
            .map(|byte| (0x01..=0x05).contains(&byte) || (0x81..=0x85).contains(&byte))
            .unwrap_or(false);

        Some(Self { title, eeprom })
    }

    pub fn title(&self) -> String {
        self.title.clone()
    }

    pub fn eeprom(&self) -> bool {
        self.eeprom
    }
}

pub fn rotation(rotation: LNXRotation) -> DisplayRotation {
    match rotation {
        LNXRotation::_90 => DisplayRotation::Rotate90,
        LNXRotation::_270 => DisplayRotation::Rotate270,
        _ => DisplayRotation::None,
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

pub fn info(cartridge: &RomFile, display_rotation: DisplayRotation) -> CartridgeInfo {
    let header = CartridgeHeader::parse(&cartridge.data).unwrap_or_default();
    CartridgeInfo {
        system: core_model::System::Lynx,
        title: header.title(),
        region: None,
        tv_system: None,
        rom: identity(&cartridge.data),
        backup: header.eeprom().then_some(core_model::BackupMemory {
            kind: core_model::BackupKind::Eeprom,
            capacity: 0,
            battery_backed: true,
        }),
        display_rotation,
    }
}

pub fn inspect(rom: &[u8]) -> Result<CartridgeInfo, CartridgeError> {
    let mut machine = holani::lynx::Lynx::new();
    machine
        .load_cart_from_slice(rom)
        .map_err(|err| CartridgeError::InvalidRom(err.to_string()))?;

    let cartridge = RomFile::new().data(rom.to_vec());
    Ok(info(&cartridge, rotation(machine.rotation())))
}

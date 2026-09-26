use std::path::Path;

use core_model::{CartridgeInfo, Core, RomIdentity, System};
use thiserror::Error;

pub type PlayerCore = Box<dyn Core + Send + Sync>;

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("No supported console matches ROM file {0}")]
    UnknownFormat(String),
    #[error("Invalid SNES cartridge: {0}")]
    Snes(#[from] snes::SnesError),
    #[error("Cannot load GBA cartridge: {0}")]
    Gba(#[from] gba::error::GbaError),
}

pub fn system_for_path(path: impl AsRef<Path>) -> Result<System, LoadError> {
    let path = path.as_ref();
    let extension = path.extension().and_then(|extension| extension.to_str());
    System::ALL
        .into_iter()
        .find(|system| {
            extension.is_some_and(|extension| {
                system
                    .rom_extensions()
                    .iter()
                    .any(|supported| extension.eq_ignore_ascii_case(supported))
            })
        })
        .ok_or_else(|| LoadError::UnknownFormat(path.display().to_string()))
}

pub fn identity(system: System, rom: &[u8]) -> RomIdentity {
    match system {
        System::Snes => snes::cartridge::identity(rom),
        System::Gba => gba::cartridge::identity(rom),
    }
}

pub fn inspect(system: System, rom: Vec<u8>) -> Result<CartridgeInfo, LoadError> {
    match system {
        System::Snes => Ok(snes::Cartridge::new(rom)?.info()),
        System::Gba => Ok(gba::Cartridge::new(rom)?.info()),
    }
}

pub fn load(system: System, rom: Vec<u8>) -> Result<PlayerCore, LoadError> {
    match system {
        System::Snes => Ok(Box::new(snes::Snes::new(snes::Cartridge::new(rom)?))),
        System::Gba => Ok(Box::new(gba::Gba::new(gba::Cartridge::new(rom)?))),
    }
}

#[cfg(feature = "debug")]
pub fn load_debug(
    system: System,
    rom: Vec<u8>,
    name: String,
) -> Result<Box<dyn debugger_model::DebugCore>, LoadError> {
    match system {
        System::Snes => Ok(Box::new(snes::debug::SnesDebug::new(
            snes::Cartridge::new(rom)?,
            name,
            None,
        ))),
        System::Gba => Ok(Box::new(gba::debug::GbaDebug::new(
            gba::Cartridge::new(rom)?,
            name,
            None,
        ))),
    }
}

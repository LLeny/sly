use std::path::Path;

use core_model::{CartridgeInfo, Core, RomFile, RomIdentity, System};
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
    #[error("Cannot load cartridge: {0}")]
    Core(#[from] core_model::CoreError),
    #[error("The debugger does not support {0:?} yet")]
    DebuggerUnsupported(System),
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
        System::Snes => snes::cartridge_header::identity(rom),
        System::Gba => gba::cartridge_header::identity(rom),
        System::Lynx => lynx::cartridge_header::identity(rom),
    }
}

pub fn inspect(system: System, rom: Vec<u8>) -> Result<CartridgeInfo, LoadError> {
    match system {
        System::Snes => Ok(snes::cartridge_header::info(
            &RomFile::new().data(rom),
            &[],
        )?),
        System::Gba => Ok(gba::cartridge_header::info(&RomFile::new().data(rom))),
        System::Lynx => {
            Ok(lynx::cartridge_header::inspect(&rom).map_err(core_model::CoreError::from)?)
        }
    }
}

pub fn load(system: System, cartridge: RomFile) -> Result<PlayerCore, LoadError> {
    match system {
        System::Snes => {
            let mut core = snes::Snes::new();
            core.load_cartridge(cartridge)?;
            Ok(Box::new(core))
        }
        System::Gba => {
            let mut core = gba::Gba::new();
            core.load_cartridge(cartridge)?;
            Ok(Box::new(core))
        }
        System::Lynx => {
            let mut core = lynx::Lynx::new();
            core.load_cartridge(cartridge)?;
            Ok(Box::new(core))
        }
    }
}

#[cfg(feature = "debug")]
pub fn load_debug(
    system: System,
    rom: Vec<u8>,
    name: String,
) -> Result<Box<dyn debugger_model::DebugCore>, LoadError> {
    match system {
        System::Snes => {
            let cart = RomFile::new().data(rom);
            snes::cartridge_header::info(&cart, &[])?;
            Ok(Box::new(snes::debug::SnesDebug::new(cart, name, None)))
        }
        System::Gba => Ok(Box::new(gba::debug::GbaDebug::new(
            RomFile::new().data(rom),
            name,
            None,
        ))),
        System::Lynx => Err(LoadError::DebuggerUnsupported(System::Lynx)),
    }
}

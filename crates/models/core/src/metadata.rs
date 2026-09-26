use crate::{Region, System, TvSystem};

/// Computed without any copier header.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GameId {
    pub system: System,
    pub sha1: String,
}

impl GameId {
    pub fn new(system: System, sha1: impl Into<String>) -> Self {
        Self {
            system,
            sha1: sha1.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RomIdentity {
    pub sha1: String,
    pub crc32: u32,
    pub len: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackupKind {
    Sram,
    Fram,
    Eeprom,
    Flash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackupMemory {
    pub kind: BackupKind,
    pub capacity: usize,
    pub battery_backed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CartridgeInfo {
    pub system: System,
    pub title: String,
    pub region: Option<Region>,
    pub tv_system: Option<TvSystem>,
    pub rom: RomIdentity,
    pub backup: Option<BackupMemory>,
}

impl CartridgeInfo {
    pub fn game_id(&self) -> GameId {
        GameId::new(self.system, self.rom.sha1.clone())
    }
}

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DisplayRotation {
    #[default]
    None,
    Rotate90,
    Rotate270,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CartridgeInfo {
    pub system: System,
    pub title: String,
    pub region: Option<Region>,
    pub tv_system: Option<TvSystem>,
    pub rom: RomIdentity,
    pub backup: Option<BackupMemory>,
    pub display_rotation: DisplayRotation,
}

impl CartridgeInfo {
    pub fn game_id(&self) -> GameId {
        GameId::new(self.system, self.rom.sha1.clone())
    }
}

pub fn format_byte_size(bytes: usize) -> String {
    const MB: usize = 1 << 20;
    const KB: usize = 1 << 10;

    if bytes >= MB && bytes.is_multiple_of(MB) {
        format!("{} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{} KB", bytes / KB)
    } else {
        format!("{bytes} B")
    }
}

use std::future::Future;

use core_model::{Core, System};
use thiserror::Error;

#[cfg(not(target_family = "wasm"))]
mod fs;
#[cfg(not(target_family = "wasm"))]
pub use fs::FsStore;

#[cfg(target_family = "wasm")]
mod idb;
#[cfg(target_family = "wasm")]
pub use idb::IdbStore;

#[cfg(not(target_family = "wasm"))]
pub type PlatformStore = FsStore;
#[cfg(target_family = "wasm")]
pub type PlatformStore = IdbStore;

pub const STATE_EXTENSION: &str = "sav";
pub const SAVE_RAM_FILE: &str = "sram.bin";

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("Save directory is unavailable: {0}")]
    Unavailable(String),
    #[error("I/O error on {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Slot {
    Quick,
    Numbered(u8),
}

impl Slot {
    pub const ALL: [Slot; 11] = [
        Slot::Quick,
        Slot::Numbered(0),
        Slot::Numbered(1),
        Slot::Numbered(2),
        Slot::Numbered(3),
        Slot::Numbered(4),
        Slot::Numbered(5),
        Slot::Numbered(6),
        Slot::Numbered(7),
        Slot::Numbered(8),
        Slot::Numbered(9),
    ];

    pub fn file_name(self) -> String {
        match self {
            Slot::Quick => format!("quick.{STATE_EXTENSION}"),
            Slot::Numbered(n) => format!("slot-{n}.{STATE_EXTENSION}"),
        }
    }

    pub fn id(self) -> String {
        match self {
            Slot::Quick => "quick".to_string(),
            Slot::Numbered(n) => n.to_string(),
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id.as_bytes() {
            b"quick" => Some(Slot::Quick),
            [digit @ b'0'..=b'9'] => Some(Slot::Numbered(digit - b'0')),
            _ => None,
        }
    }

    pub fn label(self) -> String {
        match self {
            Slot::Quick => "Quick".to_string(),
            Slot::Numbered(n) => format!("Slot {n}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Entry {
    State(Slot),
    SaveRam,
}

#[derive(Debug, Clone)]
pub struct SlotInfo {
    pub slot: Slot,
    pub len: u64,
}

pub fn entry_path(game: &GameId, entry: Entry) -> String {
    let system = game.system.slug();
    let sha1 = &game.sha1;
    match entry {
        Entry::SaveRam => format!("{system}/saves/{sha1}/{SAVE_RAM_FILE}"),
        Entry::State(slot) => format!("{system}/states/{sha1}/{}", slot.file_name()),
    }
}

pub fn game_prefixes(game: &GameId) -> [String; 2] {
    let system = game.system.slug();
    let sha1 = &game.sha1;
    [
        format!("{system}/saves/{sha1}"),
        format!("{system}/states/{sha1}"),
    ]
}

pub trait SaveStore {
    fn read(&self, game: &GameId, entry: Entry) -> Result<Option<Vec<u8>>, StoreError>;
    fn write(&self, game: &GameId, entry: Entry, bytes: &[u8]) -> Result<(), StoreError>;
    fn remove(&self, game: &GameId, entry: Entry) -> Result<(), StoreError>;
    fn slots(&self, game: &GameId) -> Result<Vec<SlotInfo>, StoreError>;

    fn hydrate(&self, game: &GameId) -> impl Future<Output = Result<(), StoreError>> + 'static {
        let _ = game;
        std::future::ready(Ok(()))
    }
}

pub struct Session<S: SaveStore> {
    store: S,
    game: GameId,
}

impl<S: SaveStore> Session<S> {
    pub fn new(store: S, game: GameId) -> Self {
        Self { store, game }
    }

    pub fn game(&self) -> &GameId {
        &self.game
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn slots(&self) -> Result<Vec<SlotInfo>, StoreError> {
        self.store.slots(&self.game)
    }

    pub fn hydrate(&self) -> impl Future<Output = Result<(), StoreError>> + 'static {
        self.store.hydrate(&self.game)
    }

    pub fn read_state(&self, slot: Slot) -> Result<Option<Vec<u8>>, StoreError> {
        self.store.read(&self.game, Entry::State(slot))
    }

    pub fn remove_state(&self, slot: Slot) -> Result<(), StoreError> {
        self.store.remove(&self.game, Entry::State(slot))
    }

    pub fn save_state<C: Core + ?Sized>(
        &self,
        core: &C,
        slot: Slot,
    ) -> Result<Vec<u8>, SessionError> {
        let bytes = core.save_state()?;
        self.store.write(&self.game, Entry::State(slot), &bytes)?;
        Ok(bytes)
    }

    pub fn load_state<C: Core + ?Sized>(
        &self,
        core: &mut C,
        slot: Slot,
    ) -> Result<bool, SessionError> {
        let Some(bytes) = self.read_state(slot)? else {
            return Ok(false);
        };

        core.load_state(&bytes)?;
        Ok(true)
    }

    pub fn restore_save_ram<C: Core + ?Sized>(&self, core: &mut C) -> Result<bool, SessionError> {
        let Some(bytes) = self.store.read(&self.game, Entry::SaveRam)? else {
            return Ok(false);
        };

        if !core.load_save_ram(&bytes) {
            return Err(SessionError::SaveRamMismatch { len: bytes.len() });
        }

        Ok(true)
    }

    pub fn persist_save_ram<C: Core + ?Sized>(&self, core: &C) -> Result<bool, SessionError> {
        let Some(bytes) = core.save_ram() else {
            return Ok(false);
        };

        self.store.write(&self.game, Entry::SaveRam, bytes)?;
        Ok(true)
    }
}

#[derive(Debug, Error)]
pub enum SessionError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    State(#[from] core_model::StateError),
    #[error("Stored save RAM ({len} bytes) does not fit this cartridge")]
    SaveRamMismatch { len: usize },
}

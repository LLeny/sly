use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use core_model::Core;
pub use core_model::GameId;
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
pub const SCRIPT_FILE: &str = "script.lua";

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

#[derive(Debug)]
pub struct BackupPolicy {
    allowed: AtomicBool,
}

impl Default for BackupPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl BackupPolicy {
    pub const fn new() -> Self {
        Self {
            allowed: AtomicBool::new(true),
        }
    }

    pub fn allow_persist(&self) -> bool {
        self.allowed.load(Ordering::Relaxed)
    }

    pub fn inhibit(&self) {
        self.allowed.store(false, Ordering::Relaxed);
    }

    pub fn restored(&self, success: bool) {
        if !success {
            self.inhibit();
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
    Script,
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
        Entry::Script => format!("{system}/scripts/{sha1}/{SCRIPT_FILE}"),
    }
}

pub fn game_prefixes(game: &GameId) -> [String; 3] {
    let system = game.system.slug();
    let sha1 = &game.sha1;
    [
        format!("{system}/saves/{sha1}"),
        format!("{system}/states/{sha1}"),
        format!("{system}/scripts/{sha1}"),
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
    backup_policy: Arc<BackupPolicy>,
}

impl<S: SaveStore> Session<S> {
    pub fn new(store: S, game: GameId) -> Self {
        Self {
            store,
            game,
            backup_policy: Arc::new(BackupPolicy::new()),
        }
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
        let hydrate = self.store.hydrate(&self.game);
        let policy = self.backup_policy.clone();
        async move {
            let result = hydrate.await;
            if result.is_err() {
                policy.inhibit();
            }

            result
        }
    }

    pub fn inhibit_save_ram_persistence(&self) {
        self.backup_policy.inhibit();
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

    pub fn read_script(&self) -> Result<Option<String>, StoreError> {
        Ok(self
            .store
            .read(&self.game, Entry::Script)?
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned()))
    }

    pub fn write_script(&self, source: &str) -> Result<(), StoreError> {
        self.store
            .write(&self.game, Entry::Script, source.as_bytes())
    }

    pub fn restore_save_ram<C: Core + ?Sized>(&self, core: &mut C) -> Result<bool, SessionError> {
        let stored = self
            .store
            .read(&self.game, Entry::SaveRam)
            .inspect_err(|_| self.backup_policy.inhibit())?;

        let Some(bytes) = stored else {
            return Ok(false);
        };

        if !core.load_save_ram(&bytes) {
            self.backup_policy.inhibit();
            return Err(SessionError::SaveRamMismatch { len: bytes.len() });
        }

        Ok(true)
    }

    pub fn persist_save_ram<C: Core + ?Sized>(&self, core: &C) -> Result<bool, SessionError> {
        if !self.backup_policy.allow_persist() {
            return Err(SessionError::SaveRamPersistenceInhibited);
        }

        let Some(bytes) = core.save_ram() else {
            return Ok(false);
        };

        self.store.write(&self.game, Entry::SaveRam, bytes)?;
        Ok(true)
    }
}

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("Save RAM persistence is disabled because restoring the existing backup failed")]
    SaveRamPersistenceInhibited,
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    State(#[from] core_model::StateError),
    #[error("Stored save RAM ({len} bytes) does not fit this cartridge")]
    SaveRamMismatch { len: usize },
}

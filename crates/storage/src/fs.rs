use std::path::{Path, PathBuf};

use crate::{Entry, GameId, SaveStore, Slot, SlotInfo, StoreError, entry_path};

pub const DATA_DIR_ENV: &str = "SLY_DATA_DIR";

pub struct FsStore {
    root: PathBuf,
}

impl FsStore {
    pub fn new() -> Self {
        let root = std::env::var_os(DATA_DIR_ENV)
            .map(PathBuf::from)
            .or_else(|| dirs::data_dir().map(|dir| dir.join("sly")))
            .unwrap_or_else(|| PathBuf::from("sly-data"));

        Self { root }
    }

    pub fn with_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn path(&self, game: &GameId, entry: Entry) -> PathBuf {
        self.root.join(entry_path(game, entry))
    }
}

impl Default for FsStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SaveStore for FsStore {
    fn read(&self, game: &GameId, entry: Entry) -> Result<Option<Vec<u8>>, StoreError> {
        let path = self.path(game, entry);
        match std::fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(io(&path, source)),
        }
    }

    fn write(&self, game: &GameId, entry: Entry, bytes: &[u8]) -> Result<(), StoreError> {
        let path = self.path(game, entry);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| io(parent, source))?;
        }

        let temp = path.with_extension("tmp");
        std::fs::write(&temp, bytes).map_err(|source| io(&temp, source))?;
        std::fs::rename(&temp, &path).map_err(|source| {
            let _ = std::fs::remove_file(&temp);
            io(&path, source)
        })
    }

    fn remove(&self, game: &GameId, entry: Entry) -> Result<(), StoreError> {
        let path = self.path(game, entry);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(io(&path, source)),
        }
    }

    fn slots(&self, game: &GameId) -> Result<Vec<SlotInfo>, StoreError> {
        let mut found = Vec::new();
        for slot in Slot::ALL {
            let path = self.path(game, Entry::State(slot));
            let metadata = match std::fs::metadata(&path) {
                Ok(metadata) => metadata,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
                Err(source) => return Err(io(&path, source)),
            };

            found.push(SlotInfo {
                slot,
                len: metadata.len(),
            });
        }

        Ok(found)
    }
}

fn io(path: impl AsRef<Path>, source: std::io::Error) -> StoreError {
    StoreError::Io {
        path: path.as_ref().display().to_string(),
        source,
    }
}

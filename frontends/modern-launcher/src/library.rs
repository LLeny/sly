use std::collections::HashSet;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};

use core_model::{GameId, Region, System, TvSystem};
use game_database::GameMeta;
use walkdir::WalkDir;

const MAX_ROM_BYTES: u64 = 64 * 1024 * 1024;
const MAX_WARNINGS: usize = 32;

#[derive(Clone, Debug)]
pub struct LibraryEntry {
    pub path: PathBuf,
    pub system: System,
    pub sha1: String,
    pub title: String,
    pub region: Option<Region>,
    pub tv_system: Option<TvSystem>,
    pub meta: GameMeta,
}

impl LibraryEntry {
    pub fn game_id(&self) -> GameId {
        GameId::new(self.system, self.sha1.clone())
    }

    pub fn initial(&self) -> String {
        title_initial(&self.title)
    }
}

pub fn title_initial(title: &str) -> String {
    title
        .chars()
        .find(|ch| ch.is_alphanumeric())
        .map(|ch| ch.to_uppercase().collect())
        .unwrap_or_else(|| "#".into())
}

#[derive(Default)]
pub struct DiscoveryResult {
    pub entries: Vec<LibraryEntry>,
    pub warnings: Vec<String>,
    pub skipped: usize,
}

impl DiscoveryResult {
    fn warn(&mut self, warning: String) {
        self.skipped += 1;

        if self.warnings.len() < MAX_WARNINGS {
            self.warnings.push(warning);
        }
    }
}

pub struct Discovery {
    receiver: Receiver<DiscoveryResult>,
    cancelled: Arc<AtomicBool>,
    finished: bool,
}

impl Discovery {
    pub fn start(roots: Vec<PathBuf>) -> io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();

        std::thread::Builder::new()
            .name("launcher-library".into())
            .spawn(move || {
                let result = discover(&roots, &worker_cancelled);

                if !worker_cancelled.load(Ordering::Relaxed) {
                    let _ = sender.send(result);
                }
            })?;

        Ok(Self {
            receiver,
            cancelled,
            finished: false,
        })
    }

    pub fn poll(&mut self) -> Option<DiscoveryResult> {
        match self.receiver.try_recv() {
            Ok(result) => {
                self.finished = true;
                Some(result)
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) if !self.finished => {
                self.finished = true;

                Some(DiscoveryResult {
                    warnings: vec!["Library scanner stopped unexpectedly".into()],
                    ..DiscoveryResult::default()
                })
            }
            Err(_) => None,
        }
    }

    pub fn finished(&self) -> bool {
        self.finished
    }
}

impl Drop for Discovery {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

fn discover(roots: &[PathBuf], cancelled: &AtomicBool) -> DiscoveryResult {
    let mut result = DiscoveryResult::default();
    let mut visited = HashSet::new();

    for root in roots {
        for item in WalkDir::new(root).follow_links(false) {
            if cancelled.load(Ordering::Relaxed) {
                return result;
            }

            let item = match item {
                Ok(item) => item,
                Err(error) => {
                    result.warn(error.to_string());
                    continue;
                }
            };

            if !item.file_type().is_file() || core_loader::system_for_path(item.path()).is_err() {
                continue;
            }

            let path = match item.path().canonicalize() {
                Ok(path) => path,
                Err(error) => {
                    result.warn(format!("{}: {error}", item.path().display()));
                    continue;
                }
            };

            if !visited.insert(path.clone()) {
                continue;
            }

            match read_entry(&path) {
                Ok(entry) => result.entries.push(entry),
                Err(error) => result.warn(format!("{}: {error}", path.display())),
            }
        }
    }

    result
        .entries
        .sort_by_cached_key(|entry| (entry.title.to_lowercase(), entry.path.clone()));

    result
}

pub(crate) fn read_rom(path: &Path) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let length = file.metadata().map_err(|error| error.to_string())?.len();

    if length > MAX_ROM_BYTES {
        return Err(format!(
            "ROM exceeds the {} MiB limit",
            MAX_ROM_BYTES / 1024 / 1024
        ));
    }

    let mut bytes = Vec::with_capacity(length as usize);

    file.take(MAX_ROM_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;

    if bytes.len() as u64 > MAX_ROM_BYTES {
        return Err("ROM grew beyond the size limit while reading".into());
    }

    Ok(bytes)
}

pub fn read_entry(path: &Path) -> Result<LibraryEntry, String> {
    let system = core_loader::system_for_path(path).map_err(|error| error.to_string())?;
    let cartridge =
        core_loader::inspect(system, read_rom(path)?).map_err(|error| error.to_string())?;
    let meta = game_database::lookup(system, &cartridge.rom);

    let title = meta.display_title().unwrap_or_else(|| {
        let title = cartridge.title.trim();

        if title.is_empty() {
            path.file_stem()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned()
        } else {
            title.to_owned()
        }
    });

    Ok(LibraryEntry {
        path: path.to_owned(),
        system,
        sha1: cartridge.rom.sha1,
        title,
        region: cartridge.region,
        tv_system: cartridge.tv_system,
        meta,
    })
}

use std::path::{Path, PathBuf};

use core_model::{Region, System, TvSystem};

use walkdir::WalkDir;

use crate::config;
use game_database::{self, GameMeta};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CartShell {
    Us,
    JpPal,
}

impl CartShell {
    pub fn from_region(region: Option<Region>, tv_system: Option<TvSystem>) -> Self {
        match (region, tv_system) {
            (Some(Region::Japan), _) | (_, Some(TvSystem::Pal)) => Self::JpPal,
            _ => Self::Us,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxStyle {
    Western,
    Japanese,
}

impl BoxStyle {
    pub fn from_region(region: Option<Region>) -> Self {
        match region {
            Some(Region::Japan) => Self::Japanese,
            _ => Self::Western,
        }
    }
}

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
    pub fn cart_shell(&self) -> CartShell {
        CartShell::from_region(self.region, self.tv_system)
    }

    pub fn box_style(&self) -> BoxStyle {
        BoxStyle::from_region(self.region)
    }
}

pub fn discover(roots: &[PathBuf]) -> Result<Vec<LibraryEntry>, String> {
    if roots.is_empty() {
        return Err(format!("Set [library] roms in {}", config::CONFIG_FILE));
    }

    let mut paths = Vec::new();
    for root in roots {
        match root.exists() {
            true => paths.extend(scan(root)),
            false => eprintln!("skipping {}: path does not exist", root.display()),
        }
    }
    paths.sort_by_cached_key(|path| path.to_string_lossy().to_ascii_lowercase());
    paths.dedup();

    let mut entries = Vec::new();
    for path in paths {
        match read_entry(&path) {
            Ok(entry) => entries.push(entry),
            Err(err) => eprintln!("skipping {}: {err}", path.display()),
        }
    }
    Ok(entries)
}

fn scan(root: &Path) -> impl Iterator<Item = PathBuf> {
    WalkDir::new(root)
        .follow_links(true)
        .into_iter()
        .filter_map(|entry| match entry {
            Ok(entry) => Some(entry),
            Err(err) => {
                eprintln!("skipping unreadable entry: {err}");
                None
            }
        })
        .filter(|entry| {
            entry.file_type().is_file() && core_loader::system_for_path(entry.path()).is_ok()
        })
        .map(walkdir::DirEntry::into_path)
}

pub fn load_core(system: System, path: &Path) -> Result<core_loader::PlayerCore, String> {
    let bytes = std::fs::read(path).map_err(|err| format!("failed to read ROM: {err}"))?;
    core_loader::load(system, bytes).map_err(|err| err.to_string())
}

fn read_entry(path: &Path) -> Result<LibraryEntry, String> {
    let system = core_loader::system_for_path(path).map_err(|err| err.to_string())?;
    let bytes = std::fs::read(path).map_err(|err| err.to_string())?;
    let cartridge = core_loader::inspect(system, bytes).map_err(|err| err.to_string())?;
    let sha1 = cartridge.rom.sha1.clone();
    let meta = game_database::lookup(system, &cartridge.rom);
    let normalized_title = meta.display_title();
    let header_title = cartridge.title.trim().to_string();
    let title = normalized_title.unwrap_or_else(|| {
        if header_title.is_empty() {
            path.file_stem()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned()
        } else {
            header_title
        }
    });

    let region = cartridge.region;

    Ok(LibraryEntry {
        path: path.to_owned(),
        system,
        tv_system: cartridge.tv_system,
        sha1,
        title,
        region,
        meta,
    })
}

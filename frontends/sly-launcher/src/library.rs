use std::path::{Path, PathBuf};

use core_model::{Region, System, TvSystem};
use snes::Cartridge;
use walkdir::WalkDir;

use crate::config;
use crate::gamedb::{self, GameMeta};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CartShell {
    Us,
    JpPal,
}

impl CartShell {
    pub fn from_region(region: Region) -> Self {
        match (region, region.tv_system()) {
            (Region::Japan, _) | (_, TvSystem::Pal) => Self::JpPal,
            (_, TvSystem::Ntsc) => Self::Us,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxStyle {
    Western,
    Japanese,
}

impl BoxStyle {
    pub fn from_region(region: Region) -> Self {
        match region {
            Region::Japan => Self::Japanese,
            _ => Self::Western,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LibraryEntry {
    pub path: PathBuf,
    pub sha1: String,
    pub title: String,
    pub region: Region,
    pub meta: GameMeta,
}

impl LibraryEntry {
    pub fn cart_shell(&self) -> CartShell {
        CartShell::from_region(self.region)
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
        .filter(|entry| entry.file_type().is_file() && is_snes_rom(entry.path()))
        .map(walkdir::DirEntry::into_path)
}

pub fn read_rom(path: &Path) -> Result<Vec<u8>, String> {
    let mut bytes = std::fs::read(path).map_err(|err| format!("failed to read ROM: {err}"))?;
    if bytes.len() % 1024 == 512 {
        bytes.drain(..512);
    }
    Ok(bytes)
}

pub fn load_cartridge(path: &Path) -> Result<Cartridge, String> {
    let bytes = read_rom(path)?;
    Cartridge::new(bytes).map_err(|err| format!("invalid SNES ROM: {err}"))
}

fn read_entry(path: &Path) -> Result<LibraryEntry, String> {
    let bytes = read_rom(path)?;
    let sha1 = artwork::rom_sha1(artwork::System::Snes, &bytes);
    let cartridge = Cartridge::new(bytes).map_err(|err| format!("invalid SNES ROM: {err}"))?;
    let crc = cartridge.rom_crc32();
    let meta = gamedb::lookup(&sha1, crc);
    let normalized_title = meta.display_title();
    let header_title = cartridge.title().trim().to_string();
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
    let region = cartridge.region();

    Ok(LibraryEntry {
        path: path.to_owned(),
        sha1,
        title,
        region,
        meta,
    })
}

fn is_snes_rom(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            System::Snes
                .rom_extensions()
                .iter()
                .any(|supported| extension.eq_ignore_ascii_case(supported))
        })
}

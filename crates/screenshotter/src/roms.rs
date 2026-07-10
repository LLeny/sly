use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

pub struct RomFile {
    pub path: PathBuf,
    pub file_name: String,
    pub stem: String,
}

pub fn scan(dir: &Path, extensions: &[&str]) -> Result<Vec<RomFile>> {
    let mut roms = Vec::new();
    let mut stack = vec![dir.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries =
            std::fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))?;
        for entry in entries {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }

            let Some(ext) = self::extension(&path) else {
                continue;
            };
            if ext != "zip" && !extensions.contains(&ext.as_str()) {
                continue;
            }

            let Some(file_name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };

            roms.push(RomFile {
                file_name: file_name.to_string(),
                stem: stem.to_string(),
                path,
            });
        }
    }

    roms.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    Ok(roms)
}

pub fn load(path: &Path, extensions: &[&str]) -> Result<Vec<u8>> {
    if self::extension(path).as_deref() == Some("zip") {
        self::load_zip(path, extensions)
    } else {
        std::fs::read(path).with_context(|| format!("reading {}", path.display()))
    }
}

fn load_zip(path: &Path, extensions: &[&str]) -> Result<Vec<u8>> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file))
        .with_context(|| format!("reading {}", path.display()))?;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let matches = Path::new(entry.name())
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|ext| extensions.contains(&ext.to_ascii_lowercase().as_str()));
        if !matches {
            continue;
        }

        let mut rom = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut rom)
            .with_context(|| format!("extracting {} from {}", entry.name(), path.display()))?;
        return Ok(rom);
    }

    bail!(
        "no .{} file inside {}",
        extensions.join("/."),
        path.display()
    );
}

fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
}

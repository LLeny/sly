use std::path::PathBuf;

use bevy::prelude::*;
use bevy::window::{WindowMode, WindowResolution};
use clap::Parser;
use serde::Deserialize;
use shellexpand::path::tilde;

pub const CONFIG_FILE: &str = "sly-launcher.toml";

const DEFAULT_CONFIG: &str = include_str!("../../../configs/sly-launcher.toml");

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about,
    after_help = "Settings are read from sly-launcher.toml next to the executable."
)]
struct Args {
    #[arg(value_name = "ROMS_DIR")]
    roms: Vec<PathBuf>,
}

#[derive(Resource, Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub library: LibraryConfig,
    pub storage: StorageConfig,
    pub window: WindowConfig,

    #[serde(skip)]
    pub source: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LibraryConfig {
    pub roms: Vec<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageConfig {
    #[serde(default)]
    pub data_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowConfig {
    pub title: String,
    pub fullscreen: bool,

    #[serde(default)]
    pub resolution: Option<[u32; 2]>,
}

impl WindowConfig {
    pub fn window(&self) -> Window {
        let mode = if self.fullscreen {
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        } else {
            WindowMode::Windowed
        };

        Window {
            title: self.title.clone(),
            mode,
            resolution: self
                .resolution
                .map_or_else(WindowResolution::default, |[width, height]| {
                    WindowResolution::new(width, height)
                }),
            ..default()
        }
    }
}

impl Config {
    pub fn save_store(&self) -> storage::FsStore {
        match &self.storage.data_dir {
            Some(dir) => storage::FsStore::with_root(dir),
            None => storage::FsStore::new(),
        }
    }
}

pub fn load() -> Result<Config, String> {
    let args = Args::parse();

    let mut config = match config_path() {
        Some(path) if path.is_file() => {
            let text = std::fs::read_to_string(&path)
                .map_err(|err| format!("failed to read config {}: {err}", path.display()))?;
            let mut config =
                parse(&text).map_err(|err| format!("invalid config {}: {err}", path.display()))?;
            config.source = path.display().to_string();
            config
        }
        Some(path) => {
            let mut config = defaults();
            config.source = match std::fs::write(&path, DEFAULT_CONFIG) {
                Ok(()) => format!("{} (written from the shipped defaults)", path.display()),
                Err(err) => format!(
                    "built-in defaults ({} is not writable: {err})",
                    path.display()
                ),
            };
            config
        }
        None => defaults(),
    };

    if !args.roms.is_empty() {
        config.library.roms = args.roms;
    }
    for root in &mut config.library.roms {
        *root = tilde(root).into_owned();
    }
    config.storage.data_dir = config
        .storage
        .data_dir
        .as_ref()
        .map(|dir| tilde(dir).into_owned());

    Ok(config)
}

fn config_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.with_file_name(CONFIG_FILE))
}

fn defaults() -> Config {
    toml::from_str(DEFAULT_CONFIG).expect("the shipped default config is a valid Config")
}

fn parse(text: &str) -> Result<Config, toml::de::Error> {
    let mut table: toml::Table =
        toml::from_str(DEFAULT_CONFIG).expect("the shipped default config is valid TOML");
    merge(&mut table, toml::from_str(text)?);
    toml::Value::Table(table).try_into()
}

fn merge(base: &mut toml::Table, overrides: toml::Table) {
    for (key, value) in overrides {
        match (base.get_mut(&key), value) {
            (Some(toml::Value::Table(base_table)), toml::Value::Table(table)) => {
                merge(base_table, table);
            }
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

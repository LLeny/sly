mod gba;
mod snes;

use std::time::Duration;

use crate::capture::{CaptureConfig, ScreenshotCore};
use anyhow::Result;
use core_model::System as CoreSystem;

pub struct System {
    pub core: CoreSystem,
    pub config: CaptureConfig,
    pub timeout: Duration,
    build: fn(Vec<u8>) -> Result<Box<dyn ScreenshotCore>>,
}

impl System {
    pub fn slug(&self) -> &'static str {
        self.core.slug()
    }

    pub fn rom_extensions(&self) -> &'static [&'static str] {
        self.core.rom_extensions()
    }

    pub fn build(&self, rom: Vec<u8>) -> Result<Box<dyn ScreenshotCore>> {
        (self.build)(rom)
    }
}

static SYSTEMS: &[System] = &[gba::SYSTEM, snes::SYSTEM];

pub fn select(name: &str) -> Option<&'static System> {
    SYSTEMS.iter().find(|system| system.slug() == name)
}

pub fn available() -> Vec<&'static str> {
    SYSTEMS.iter().map(System::slug).collect()
}

fn game_id(title: &str, crc: u32) -> String {
    let mut normalized = String::new();
    for chunk in title.split(|c: char| !c.is_ascii_alphanumeric()) {
        if chunk.is_empty() {
            continue;
        }

        if !normalized.is_empty() {
            normalized.push('-');
        }

        normalized.push_str(&chunk.to_ascii_uppercase());
    }

    if normalized.is_empty() {
        normalized.push_str("UNKNOWN");
    }

    format!("{normalized}-{crc:08X}")
}

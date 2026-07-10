mod snes;

use crate::capture::{CaptureConfig, ScreenshotCore};
use anyhow::Result;

pub struct System {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
    pub config: CaptureConfig,
    build: fn(Vec<u8>) -> Result<Box<dyn ScreenshotCore>>,
}

impl System {
    pub fn build(&self, rom: Vec<u8>) -> Result<Box<dyn ScreenshotCore>> {
        (self.build)(rom)
    }
}

static SYSTEMS: &[System] = &[snes::SYSTEM];

pub fn select(name: &str) -> Option<&'static System> {
    SYSTEMS.iter().find(|s| s.name == name)
}

pub fn available() -> Vec<&'static str> {
    SYSTEMS.iter().map(|s| s.name).collect()
}

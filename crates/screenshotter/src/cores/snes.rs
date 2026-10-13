use std::time::Duration;

use anyhow::{Context, Result};
use core_model::{Core, System as CoreSystem};
use snes::{Cartridge, Snes};

use crate::capture::{CaptureConfig, Frame, ScreenshotCore};
use crate::cores::System;

pub const SYSTEM: System = System {
    core: CoreSystem::Snes,
    config: CaptureConfig {
        warmup_frames: 60,
        every_frames: 60,
        max_frames: 2400,
    },
    timeout: Duration::from_secs(120),
    build: |rom| Ok(Box::new(SnesScreenshotter::new(rom)?)),
};

struct SnesScreenshotter {
    snes: Snes,
    game_id: String,
    frame_rate: f64,
}

impl SnesScreenshotter {
    fn new(rom: Vec<u8>) -> Result<Self> {
        let cartridge = Cartridge::new(rom).context("parsing cartridge header")?;
        let game_id = crate::cores::game_id(&cartridge.title(), crc32fast::hash(&cartridge.rom));
        let frame_rate =
            snes::cartridge::frame_rate(snes::cartridge::tv_system(cartridge.region()));

        Ok(Self {
            snes: Snes::new(cartridge),
            game_id,
            frame_rate,
        })
    }
}

impl ScreenshotCore for SnesScreenshotter {
    fn game_id(&self) -> String {
        self.game_id.clone()
    }

    fn frame_rate(&self) -> f64 {
        self.frame_rate
    }

    fn run_frame(&mut self) {
        self.snes.run_frame();
    }

    fn frame(&self) -> Frame {
        let frame = self.snes.software_frame().expect("SNES must expose video");
        let mut pixels = Vec::with_capacity(frame.pixels.len() * 3);
        for &[r, g, b, _] in frame.pixels {
            pixels.extend_from_slice(&[r, g, b]);
        }

        Frame {
            width: frame.width,
            height: frame.height,
            pixels,
        }
    }
}

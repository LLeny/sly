use std::time::Duration;

use anyhow::{Context, Result};
use core_model::{Core, System as CoreSystem, TvSystem};
use snes::Snes;

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
        let cartridge = core_model::RomFile::new().data(rom);
        let info =
            snes::cartridge_header::info(&cartridge, &[]).context("parsing cartridge header")?;
        let game_id = crate::cores::game_id(&info.title, info.rom.crc32);
        let frame_rate =
            snes::cartridge_header::frame_rate(info.tv_system.unwrap_or(TvSystem::Ntsc));

        let mut snes = Snes::new();
        snes.load_cartridge(cartridge)
            .expect("fresh core accepts cartridge");

        Ok(Self {
            snes,
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

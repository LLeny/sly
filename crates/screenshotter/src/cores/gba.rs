use std::time::Duration;

use anyhow::{Result, ensure};
use core_model::{Core, System as CoreSystem};
use gba::Gba;

use crate::capture::{CaptureConfig, Frame, ScreenshotCore};
use crate::cores::System;

pub const SYSTEM: System = System {
    core: CoreSystem::Gba,
    config: CaptureConfig {
        warmup_frames: 60,
        every_frames: 60,
        max_frames: 2400,
    },
    timeout: Duration::from_secs(120),
    build: |rom| Ok(Box::new(GbaScreenshotter::new(rom)?)),
};

struct GbaScreenshotter {
    gba: Gba,
    game_id: String,
}

impl GbaScreenshotter {
    fn new(rom: Vec<u8>) -> Result<Self> {
        ensure!(
            rom.len() >= 0xC0,
            "ROM is too short for a GBA cartridge header"
        );
        let crc = crc32fast::hash(&rom);
        let cartridge = core_model::RomFile::new().data(rom);
        let info = gba::cartridge_header::info(&cartridge);
        let game_id = crate::cores::game_id(&info.title, crc);

        let mut gba = Gba::new();
        gba.load_cartridge(cartridge)
            .expect("fresh core accepts cartridge");

        Ok(Self { gba, game_id })
    }
}

impl ScreenshotCore for GbaScreenshotter {
    fn game_id(&self) -> String {
        self.game_id.clone()
    }

    fn frame_rate(&self) -> f64 {
        self.gba
            .frame_interval()
            .expect("GBA must expose frame timing")
            .as_secs_f64()
            .recip()
    }

    fn run_frame(&mut self) {
        self.gba.run_frame();
    }

    fn frame(&self) -> Frame {
        let frame = self.gba.software_frame().expect("GBA must expose video");
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

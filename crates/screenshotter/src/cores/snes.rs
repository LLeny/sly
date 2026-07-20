use std::time::Duration;

use anyhow::{Context, Result};
use core_model::Core;
use snes::{Cartridge, Snes};

use crate::capture::{CaptureConfig, Frame, ScreenshotCore};
use crate::cores::System;

pub const SYSTEM: System = System {
    name: "snes",
    extensions: &["sfc", "smc"],
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
    fn new(mut rom: Vec<u8>) -> Result<Self> {
        if rom.len() % 1024 == 512 {
            rom.drain(..512);
        }

        let cartridge = Cartridge::new(rom).context("parsing cartridge header")?;
        let game_id = Self::game_id(&cartridge);
        let frame_rate = snes::cartridge::frame_rate(cartridge.region().tv_system());

        Ok(Self {
            snes: Snes::new(cartridge),
            game_id,
            frame_rate,
        })
    }

    fn game_id(cartridge: &Cartridge) -> String {
        let crc = crc32fast::hash(&cartridge.rom);

        let mut title = String::new();
        for chunk in cartridge
            .title()
            .split(|c: char| !c.is_ascii_alphanumeric())
        {
            if chunk.is_empty() {
                continue;
            }

            if !title.is_empty() {
                title.push('-');
            }

            title.push_str(&chunk.to_ascii_uppercase());
        }

        if title.is_empty() {
            title.push_str("UNKNOWN");
        }

        format!("{title}-{crc:08X}")
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
        for &(r, g, b) in frame.pixels {
            pixels.extend_from_slice(&[r, g, b]);
        }

        Frame {
            width: frame.width,
            height: frame.height,
            pixels,
        }
    }
}

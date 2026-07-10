use anyhow::{Context, Result};
use snes::ppu::{SCREEN_HEIGHT, SCREEN_WIDTH};
use snes::{Cartridge, Snes};

use crate::capture::{CaptureConfig, Frame, ScreenshotCore};
use crate::cores::System;

pub const SYSTEM: System = System {
    name: "snes",
    extensions: &["sfc", "smc"],
    config: CaptureConfig {
        warmup_frames: 120,
        every_frames: 60,
        max_frames: 1800,
    },
    build: |rom| Ok(Box::new(SnesScreenshotter::new(rom)?)),
};

struct SnesScreenshotter {
    snes: Snes,
    game_id: String,
}

impl SnesScreenshotter {
    fn new(mut rom: Vec<u8>) -> Result<Self> {
        if rom.len() % 1024 == 512 {
            rom.drain(..512);
        }

        let cartridge = Cartridge::new(rom).context("parsing cartridge header")?;
        let game_id = Self::game_id(&cartridge);

        Ok(Self {
            snes: Snes::new(cartridge),
            game_id,
        })
    }

    fn game_id(cartridge: &Cartridge) -> String {
        const CHECKSUM_OFFSET: usize = 0x1E;

        let offset = cartridge.header_offset() + CHECKSUM_OFFSET;
        let checksum = u16::from_le_bytes([
            cartridge.rom.get(offset).copied().unwrap_or(0),
            cartridge.rom.get(offset + 1).copied().unwrap_or(0),
        ]);

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

        format!("{title}-{checksum:04X}")
    }
}

impl ScreenshotCore for SnesScreenshotter {
    fn game_id(&self) -> String {
        self.game_id.clone()
    }

    fn run_frame(&mut self) {
        self.snes.run_frame();
    }

    fn frame(&self) -> Frame {
        let frame_buffer = self.snes.ppu.frame_buffer();
        let mut pixels = Vec::with_capacity(frame_buffer.len() * 3);
        for &(r, g, b) in frame_buffer {
            pixels.extend_from_slice(&[r, g, b]);
        }

        Frame {
            width: SCREEN_WIDTH,
            height: SCREEN_HEIGHT,
            pixels,
        }
    }
}

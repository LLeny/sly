use std::collections::HashSet;
use std::fs::File;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::BufWriter;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use anyhow::{Context, Result};

pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

pub trait ScreenshotCore {
    fn game_id(&self) -> String;
    fn run_frame(&mut self);
    fn frame(&self) -> Frame;
}

#[derive(Clone, Copy)]
pub struct CaptureConfig {
    pub warmup_frames: u32,
    pub every_frames: u32,
    pub max_frames: u32,
}

#[derive(Default)]
pub struct Capture {
    pub screenshots: usize,
    pub skipped: usize,
    pub frames_run: u32,
    pub panic: Option<String>,
}

pub fn run(
    core: &mut dyn ScreenshotCore,
    config: &CaptureConfig,
    out_dir: &Path,
) -> Result<Capture> {
    let mut capture = Capture::default();
    let mut seen = HashSet::new();

    std::fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))?;

    while capture.frames_run < config.max_frames {
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| core.run_frame())) {
            capture.panic = Some(self::panic_message(payload.as_ref()));
            break;
        }
        capture.frames_run += 1;

        if capture.frames_run < config.warmup_frames
            || !(capture.frames_run - config.warmup_frames).is_multiple_of(config.every_frames)
        {
            continue;
        }

        let frame = core.frame();
        if self::monochrome(&frame) || !seen.insert(self::hash_pixels(&frame.pixels)) {
            capture.skipped += 1;
            continue;
        }

        let path = out_dir.join(format!("{:06}.png", capture.frames_run));
        self::write_png(&path, &frame)?;

        capture.screenshots += 1;
    }

    Ok(capture)
}

fn monochrome(frame: &Frame) -> bool {
    let Some(first) = frame.pixels.first_chunk::<3>() else {
        return true;
    };
    frame
        .pixels
        .as_chunks::<3>()
        .0
        .iter()
        .all(|pixel| pixel == first)
}

fn hash_pixels(pixels: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    pixels.hash(&mut hasher);
    hasher.finish()
}

fn write_png(path: &Path, frame: &Frame) -> Result<()> {
    let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;

    let mut encoder = png::Encoder::new(BufWriter::new(file), frame.width, frame.height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()?
        .write_image_data(&frame.pixels)
        .with_context(|| format!("writing {}", path.display()))?;

    Ok(())
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic".to_string()
    }
}

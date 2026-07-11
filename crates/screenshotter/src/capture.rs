use std::fs::File;
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
    fn frame_rate(&self) -> f64;
}

#[derive(Clone, Copy)]
pub struct CaptureConfig {
    pub warmup_frames: u32,
    pub every_frames: u32,
    pub max_frames: u32,
}

const GIF_FRAME_STEP: u32 = 4;
const SIGNATURE_SIZE: usize = 16;
const SIMILARITY_THRESHOLD: u32 = 5;

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
    record_gif: bool,
) -> Result<Capture> {
    let mut capture = Capture::default();
    let mut kept: Vec<[u8; SIGNATURE_SIZE * SIGNATURE_SIZE]> = Vec::new();

    std::fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))?;

    let mut gif = record_gif.then(|| GifRecorder::new(out_dir.join("demo.gif"), core.frame_rate()));

    while capture.frames_run < config.max_frames {
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| core.run_frame())) {
            capture.panic = Some(self::panic_message(payload.as_ref()));
            break;
        }
        capture.frames_run += 1;

        let after_warmup = capture.frames_run.checked_sub(config.warmup_frames);
        let png_due = after_warmup.is_some_and(|n| n.is_multiple_of(config.every_frames));
        let gif_due =
            gif.is_some() && after_warmup.is_some_and(|n| n.is_multiple_of(GIF_FRAME_STEP));
        if !png_due && !gif_due {
            continue;
        }

        let frame = core.frame();

        if gif_due {
            gif.as_mut().unwrap().push(&frame)?;
        }

        if !png_due {
            continue;
        }
        if self::monochrome(&frame) {
            capture.skipped += 1;
            continue;
        }
        let signature = self::signature(&frame);
        if kept.iter().any(|kept| self::similar(kept, &signature)) {
            capture.skipped += 1;
            continue;
        }
        kept.push(signature);

        let path = out_dir.join(format!("{:06}.png", capture.frames_run));
        self::write_png(&path, &frame)?;

        capture.screenshots += 1;
    }

    if let Some(recorder) = gif {
        recorder.finish();
    }

    Ok(capture)
}

struct GifRecorder {
    path: std::path::PathBuf,
    encoder: Option<gif::Encoder<BufWriter<File>>>,
    delay: u16,
    has_content: bool,
}

impl GifRecorder {
    fn new(path: std::path::PathBuf, fps: f64) -> Self {
        let delay = (100.0 * f64::from(GIF_FRAME_STEP) / fps).round().max(1.0) as u16;
        Self {
            path,
            encoder: None,
            delay,
            has_content: false,
        }
    }

    fn push(&mut self, frame: &Frame) -> Result<()> {
        if !self.has_content && !self::monochrome(frame) {
            self.has_content = true;
        }

        let encoder = match &mut self.encoder {
            Some(encoder) => encoder,
            None => {
                let file = File::create(&self.path)
                    .with_context(|| format!("creating {}", self.path.display()))?;
                let mut encoder = gif::Encoder::new(
                    BufWriter::new(file),
                    frame.width as u16,
                    frame.height as u16,
                    &[],
                )
                .with_context(|| format!("encoding {}", self.path.display()))?;
                encoder.set_repeat(gif::Repeat::Infinite)?;
                self.encoder.insert(encoder)
            }
        };

        let mut gif_frame =
            gif::Frame::from_rgb_speed(frame.width as u16, frame.height as u16, &frame.pixels, 30);
        gif_frame.delay = self.delay;
        encoder.write_frame(&gif_frame).context("writing gif frame")
    }

    fn finish(self) {
        drop(self.encoder);
        if !self.has_content {
            let _ = std::fs::remove_file(&self.path);
        }
    }
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

fn signature(frame: &Frame) -> [u8; SIGNATURE_SIZE * SIGNATURE_SIZE] {
    let mut sig = [0u8; SIGNATURE_SIZE * SIGNATURE_SIZE];
    let (w, h) = (frame.width as usize, frame.height as usize);

    for cy in 0..SIGNATURE_SIZE {
        let y0 = cy * h / SIGNATURE_SIZE;
        let y1 = ((cy + 1) * h / SIGNATURE_SIZE).max(y0 + 1);
        for cx in 0..SIGNATURE_SIZE {
            let x0 = cx * w / SIGNATURE_SIZE;
            let x1 = ((cx + 1) * w / SIGNATURE_SIZE).max(x0 + 1);

            let mut sum = 0u32;
            let mut count = 0u32;
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = (y * w + x) * 3;
                    let (r, g, b) = (
                        u32::from(frame.pixels[i]),
                        u32::from(frame.pixels[i + 1]),
                        u32::from(frame.pixels[i + 2]),
                    );
                    sum += (r * 77 + g * 150 + b * 29) >> 8;
                    count += 1;
                }
            }

            sig[cy * SIGNATURE_SIZE + cx] = (sum / count.max(1)) as u8;
        }
    }

    sig
}

fn similar(a: &[u8], b: &[u8]) -> bool {
    let total: u32 = a
        .iter()
        .zip(b)
        .map(|(&x, &y)| u32::from(x.abs_diff(y)))
        .sum();
    total / a.len() as u32 <= SIMILARITY_THRESHOLD
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

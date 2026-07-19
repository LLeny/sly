use std::fs::File;
use std::io::BufWriter;
use std::time::{Duration, Instant};

use sdl3::event::Event;
use sdl3::keyboard::Keycode;
use sdl3::pixels::PixelFormat;
use sdl3::render::ScaleMode;
use snes::{Cartridge, Snes};

const SCALE: u32 = 2;

struct Recorder {
    encoder: gif::Encoder<BufWriter<File>>,
    path: String,
    width: u16,
    height: u16,
    hundredths_per_frame: f64,
    delay_accum: f64,
}

impl Recorder {
    fn new(path: String, target_fps: f64, width: u32, height: u32) -> Self {
        let file = File::create(&path).expect("Failed to create gif file");

        let mut encoder = gif::Encoder::new(BufWriter::new(file), width as u16, height as u16, &[])
            .expect("Failed to create gif encoder");
        encoder
            .set_repeat(gif::Repeat::Infinite)
            .expect("Failed to set gif repeat");

        Self {
            encoder,
            path,
            width: width as u16,
            height: height as u16,
            hundredths_per_frame: 100.0 / target_fps,
            delay_accum: 0.0,
        }
    }

    fn push(&mut self, rgb: &[u8]) {
        self.delay_accum += self.hundredths_per_frame;
        let delay = self.delay_accum.round().max(1.0);
        self.delay_accum -= delay;

        let mut frame = gif::Frame::from_rgb_speed(self.width, self.height, rgb, 30);
        frame.delay = delay as u16;

        self.encoder
            .write_frame(&frame)
            .expect("Failed to write gif frame");
    }
}

fn main() {
    use core_model::Core;

    let mut rom_path = None;
    let mut uncapped = false;
    let mut record = false;
    let mut mute = false;

    for arg in std::env::args().skip(1) {
        if arg == "--uncapped" {
            uncapped = true;
        } else if arg == "--record" {
            record = true;
        } else if arg == "--mute" {
            mute = true;
        } else {
            rom_path = Some(arg);
        }
    }

    let path = rom_path.expect("Usage: runner <path-to-rom> [--uncapped] [--record] [--mute]");

    let mut rom = std::fs::read(&path).expect("Failed to read ROM");
    if rom.len() % 1024 == 512 {
        rom.drain(..512);
    }

    let cartridge = Cartridge::new(rom).expect("Failed to parse cartridge");
    let title = cartridge.title().trim().to_string();
    let tv_system = cartridge.region().tv_system();
    let target_fps = tv_system.frame_rate();
    let mut snes = Snes::new(cartridge);
    let initial_frame = snes.software_frame().expect("SNES must expose video");
    let (frame_width, frame_height) = (initial_frame.width, initial_frame.height);
    let frame_duration = snes
        .frame_interval()
        .unwrap_or_else(|| Duration::from_secs_f64(1.0 / target_fps));

    let mut recorder = record.then(|| {
        let name: String = title
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let name = name.trim_matches('_');
        let path = format!("{}.gif", if name.is_empty() { "recording" } else { name });
        println!("Recording to {path}");
        Recorder::new(path, target_fps, frame_width, frame_height)
    });

    let sdl = sdl3::init().expect("Failed to init SDL");
    let video = sdl.video().expect("Failed to init SDL video");
    let window = video
        .window(&title, frame_width * SCALE, frame_height * SCALE)
        .position_centered()
        .build()
        .expect("Failed to create window");

    let mut canvas = window.into_canvas();
    let texture_creator = canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormat::RGB24, frame_width, frame_height)
        .expect("Failed to create texture");
    texture.set_scale_mode(ScaleMode::Nearest);

    let mut event_pump = sdl.event_pump().expect("Failed to get event pump");

    let mut player = match (mute, snes.audio_spec()) {
        (false, Some(spec)) => audio::AudioPlayer::new(spec)
            .inspect_err(|err| eprintln!("audio disabled: {err}"))
            .ok(),
        _ => None,
    };
    let mut audio_buffer: Vec<i16> = Vec::new();

    let mut buffer = vec![0u8; frame_width as usize * frame_height as usize * 3];
    let mut frames = 0u32;
    let mut core_time = Duration::ZERO;
    let mut stats_since = Instant::now();
    let mut next_frame = Instant::now();

    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                _ => {}
            }
        }

        let start = Instant::now();
        snes.run_frame();
        core_time += start.elapsed();
        frames += 1;

        if let Some(player) = player.as_mut() {
            snes.drain_audio(&mut audio_buffer);
            player.queue(&audio_buffer);
            audio_buffer.clear();
        }

        let frame = snes.software_frame().expect("SNES video disappeared");
        for (dst, &(r, g, b)) in buffer.chunks_exact_mut(3).zip(frame.pixels) {
            dst[0] = r;
            dst[1] = g;
            dst[2] = b;
        }
        if let Some(rec) = recorder.as_mut() {
            rec.push(&buffer);
        }

        texture
            .update(None, &buffer, frame_width as usize * 3)
            .expect("Failed to update texture");
        canvas.clear();
        canvas
            .copy(&texture, None, None)
            .expect("Failed to copy texture");
        canvas.present();

        if !uncapped {
            next_frame += frame_duration;
            let now = Instant::now();
            if next_frame > now {
                std::thread::sleep(next_frame - now);
            } else {
                next_frame = now;
            }
        }

        if stats_since.elapsed() >= Duration::from_secs(1) {
            let frame_ms = core_time.as_secs_f64() * 1000.0 / f64::from(frames);
            let core_fps = f64::from(frames) / core_time.as_secs_f64();
            let stats = format!(
                "{title} [{tv_system}{}] — core: {core_fps:.0} fps, {frame_ms:.2} ms/frame",
                if uncapped { ", uncapped" } else { "" },
            );
            canvas
                .window_mut()
                .set_title(&stats)
                .expect("Failed to set title");
            println!("{stats}");

            frames = 0;
            core_time = Duration::ZERO;
            stats_since = Instant::now();
        }
    }

    if let Some(rec) = recorder {
        let path = rec.path.clone();
        drop(rec);
        println!("Saved recording to {path}");
    }
}

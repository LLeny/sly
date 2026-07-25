use std::fs::File;
use std::io::{BufWriter, Write};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use sdl3::event::Event;
use sdl3::keyboard::{Keycode, Scancode};
use sdl3::pixels::PixelFormat;
use sdl3::render::ScaleMode;
use snes::{Cartridge, Snes};

const SCALE: u32 = 2;
const SAVE_RAM_FLUSH: Duration = Duration::from_secs(60);

fn game_button(key: Scancode) -> Option<core_model::Button> {
    use core_model::Button;

    Some(match key {
        Scancode::Up => Button::Up,
        Scancode::Down => Button::Down,
        Scancode::Left => Button::Left,
        Scancode::Right => Button::Right,
        Scancode::Z => Button::B,
        Scancode::X => Button::A,
        Scancode::A => Button::Y,
        Scancode::S => Button::X,
        Scancode::Q => Button::L,
        Scancode::W => Button::R,
        Scancode::Return => Button::Start,
        Scancode::Backspace => Button::Select,
        _ => return None,
    })
}

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

struct VideoRecorder {
    ffmpeg: Child,
    out_path: String,
    video_tmp: String,
    audio: Option<AudioSink>,
}

struct AudioSink {
    file: BufWriter<File>,
    path: String,
    sample_rate: u32,
    channels: u8,
    scratch: Vec<u8>,
}

impl VideoRecorder {
    fn new(
        out_path: String,
        target_fps: f64,
        width: u32,
        height: u32,
        audio_spec: Option<core_model::AudioSpec>,
    ) -> Option<Self> {
        let video_tmp = format!("{out_path}.video.tmp.mp4");

        let ffmpeg = Command::new("ffmpeg")
            .args(["-y", "-loglevel", "error"])
            .args(["-f", "rawvideo", "-pixel_format", "rgb24"])
            .args(["-video_size", &format!("{width}x{height}")])
            .args(["-framerate", &format!("{target_fps}")])
            .args(["-i", "pipe:0"])
            .args([
                "-c:v", "libx264", "-pix_fmt", "yuv420p", "-preset", "veryfast",
            ])
            .arg(&video_tmp)
            .stdin(Stdio::piped())
            .spawn();

        let ffmpeg = match ffmpeg {
            Ok(child) => child,
            Err(err) => {
                eprintln!("video recording disabled: failed to launch ffmpeg: {err}");
                return None;
            }
        };

        let audio = audio_spec.and_then(|spec| {
            let path = format!("{out_path}.audio.tmp.raw");
            match File::create(&path) {
                Ok(file) => Some(AudioSink {
                    file: BufWriter::new(file),
                    path,
                    sample_rate: spec.sample_rate,
                    channels: spec.channels,
                    scratch: Vec::new(),
                }),
                Err(err) => {
                    eprintln!("video audio disabled: failed to create temp file: {err}");
                    None
                }
            }
        });

        Some(Self {
            ffmpeg,
            out_path,
            video_tmp,
            audio,
        })
    }

    fn push_video(&mut self, rgb: &[u8]) {
        if let Some(stdin) = self.ffmpeg.stdin.as_mut() {
            if let Err(err) = stdin.write_all(rgb) {
                eprintln!("video recording: failed to write frame: {err}");
            }
        }
    }

    fn push_audio(&mut self, samples: &[i16]) {
        if let Some(sink) = self.audio.as_mut() {
            sink.scratch.clear();
            sink.scratch
                .extend(samples.iter().flat_map(|sample| sample.to_le_bytes()));
            let _ = sink.file.write_all(&sink.scratch);
        }
    }

    fn finish(mut self) {
        drop(self.ffmpeg.stdin.take());
        if let Err(err) = self.ffmpeg.wait() {
            eprintln!("video recording: ffmpeg encode failed: {err}");
            return;
        }

        let mut cmd = Command::new("ffmpeg");
        cmd.args(["-y", "-loglevel", "error"])
            .args(["-i", &self.video_tmp]);

        let audio_tmp = if let Some(mut sink) = self.audio.take() {
            let _ = sink.file.flush();
            drop(sink.file);
            cmd.args(["-f", "s16le"])
                .args(["-ar", &sink.sample_rate.to_string()])
                .args(["-ac", &sink.channels.to_string()])
                .args(["-i", &sink.path])
                .args(["-c:v", "copy", "-c:a", "aac", "-b:a", "192k", "-shortest"]);
            Some(sink.path)
        } else {
            cmd.args(["-c:v", "copy"]);
            None
        };

        match cmd.arg(&self.out_path).status() {
            Ok(status) if status.success() => println!("Saved video to {}", self.out_path),
            Ok(status) => eprintln!("video recording: ffmpeg mux exited with {status}"),
            Err(err) => eprintln!("video recording: failed to run ffmpeg mux: {err}"),
        }

        let _ = std::fs::remove_file(&self.video_tmp);
        if let Some(path) = audio_tmp {
            let _ = std::fs::remove_file(&path);
        }
    }
}

fn upscale(src: &[u8], dst: &mut [u8], width: usize, height: usize) {
    let scale = SCALE as usize;
    let src_stride = width * 3;
    let dst_stride = src_stride * scale;

    for y in 0..height {
        let src_row = &src[y * src_stride..][..src_stride];
        let (first, rest) =
            dst[y * scale * dst_stride..][..scale * dst_stride].split_at_mut(dst_stride);

        for (x, pixel) in src_row.chunks_exact(3).enumerate() {
            for out in first[x * scale * 3..][..scale * 3].chunks_exact_mut(3) {
                out.copy_from_slice(pixel);
            }
        }

        for row in rest.chunks_exact_mut(dst_stride) {
            row.copy_from_slice(first);
        }
    }
}

fn sanitize_name(title: &str) -> String {
    let name: String = title
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let name = name.trim_matches('_');
    if name.is_empty() {
        "recording".to_string()
    } else {
        name.to_string()
    }
}

fn save_state(session: &storage::Session<storage::FsStore>, snes: &Snes) {
    match session.save_state(snes, storage::Slot::Quick) {
        Ok(bytes) => println!("Saved state ({} KiB)", bytes.len() / 1024),
        Err(err) => eprintln!("Failed to save state: {err}"),
    }
}

fn load_state(session: &storage::Session<storage::FsStore>, snes: &mut Snes) {
    match session.load_state(snes, storage::Slot::Quick) {
        Ok(true) => println!("Loaded state"),
        Ok(false) => println!("No quick save for this game"),
        Err(err) => eprintln!("Failed to load state: {err}"),
    }
}

fn main() {
    use core_model::Core;

    let mut rom_path = None;
    let mut uncapped = false;
    let mut record = false;
    let mut record_video = false;
    let mut mute = false;

    for arg in std::env::args().skip(1) {
        if arg == "--uncapped" {
            uncapped = true;
        } else if arg == "--record-gif" {
            record = true;
        } else if arg == "--record-mp4" {
            record_video = true;
        } else if arg == "--mute" {
            mute = true;
        } else {
            rom_path = Some(arg);
        }
    }

    let path = rom_path
        .expect("Usage: runner <path-to-rom> [--uncapped] [--record-gif] [--record-mp4] [--mute]");

    let rom = std::fs::read(&path).expect("Failed to read ROM");
    let session = storage::Session::new(
        storage::FsStore::new(),
        storage::GameId::new(
            core_model::System::Snes,
            artwork::rom_sha1(artwork::System::Snes, &rom),
        ),
    );

    let cartridge = Cartridge::new(rom).expect("Failed to parse cartridge");
    let title = cartridge.title().trim().to_string();
    let tv_system = cartridge.region().tv_system();
    let target_fps = snes::cartridge::frame_rate(tv_system);
    let mut snes = Snes::new(cartridge);

    if let Err(err) = session.restore_save_ram(&mut snes) {
        eprintln!("Failed to restore save RAM: {err}");
    }

    let initial_frame = snes.software_frame().expect("SNES must expose video");
    let (frame_width, frame_height) = (initial_frame.width, initial_frame.height);
    let frame_duration = snes
        .frame_interval()
        .unwrap_or_else(|| Duration::from_secs_f64(1.0 / target_fps));

    let (record_width, record_height) = (frame_width * SCALE, frame_height * SCALE);

    let mut recorder = record.then(|| {
        let path = format!("{}.gif", sanitize_name(&title));
        println!("Recording GIF to {path} at {record_width}x{record_height}");
        Recorder::new(path, target_fps, record_width, record_height)
    });

    let mut video_recorder = record_video
        .then(|| {
            let path = format!("{}.mp4", sanitize_name(&title));
            println!("Recording video to {path} at {record_width}x{record_height}");
            VideoRecorder::new(
                path,
                target_fps,
                record_width,
                record_height,
                snes.audio_spec(),
            )
        })
        .flatten();

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
    let mut record_buffer = if recorder.is_some() || video_recorder.is_some() {
        vec![0u8; record_width as usize * record_height as usize * 3]
    } else {
        Vec::new()
    };
    let mut frames = 0u32;
    let mut core_time = Duration::ZERO;
    let mut stats_since = Instant::now();
    let mut next_frame = Instant::now();
    let mut sram_flushed = Instant::now();

    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                Event::KeyDown {
                    scancode: Some(key),
                    repeat: false,
                    ..
                } => match key {
                    Scancode::F5 => save_state(&session, &snes),
                    Scancode::F8 => load_state(&session, &mut snes),
                    _ => {
                        if let Some(button) = game_button(key) {
                            snes.set_button(0, button, true);
                        }
                    }
                },
                Event::KeyUp {
                    scancode: Some(key),
                    ..
                } => {
                    if let Some(button) = game_button(key) {
                        snes.set_button(0, button, false);
                    }
                }
                _ => {}
            }
        }

        let start = Instant::now();
        snes.run_frame();
        core_time += start.elapsed();
        frames += 1;

        if sram_flushed.elapsed() >= SAVE_RAM_FLUSH {
            sram_flushed = Instant::now();
            if let Err(err) = session.persist_save_ram(&snes) {
                eprintln!("Failed to write save RAM: {err}");
            }
        }

        if player.is_some() || video_recorder.is_some() {
            snes.drain_audio(&mut audio_buffer);
            if let Some(player) = player.as_mut() {
                player.queue(&audio_buffer);
            }
            if let Some(rec) = video_recorder.as_mut() {
                rec.push_audio(&audio_buffer);
            }
            audio_buffer.clear();
        }

        let frame = snes.software_frame().expect("SNES video disappeared");
        for (dst, &(r, g, b)) in buffer.chunks_exact_mut(3).zip(frame.pixels) {
            dst[0] = r;
            dst[1] = g;
            dst[2] = b;
        }

        if !record_buffer.is_empty() {
            upscale(
                &buffer,
                &mut record_buffer,
                frame_width as usize,
                frame_height as usize,
            );

            if let Some(rec) = recorder.as_mut() {
                rec.push(&record_buffer);
            }
            if let Some(rec) = video_recorder.as_mut() {
                rec.push_video(&record_buffer);
            }
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

    if let Err(err) = session.persist_save_ram(&snes) {
        eprintln!("Failed to write save RAM: {err}");
    }

    if let Some(rec) = recorder {
        let path = rec.path.clone();
        drop(rec);
        println!("Saved GIF to {path}");
    }

    if let Some(rec) = video_recorder {
        println!("Encoding video, please wait...");
        rec.finish();
    }
}

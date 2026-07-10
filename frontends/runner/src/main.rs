use std::time::{Duration, Instant};

use minifb::{Key, Scale, Window, WindowOptions};
use snes::cartridge::TvSystem;
use snes::ppu::{SCREEN_HEIGHT, SCREEN_WIDTH};
use snes::{Cartridge, Snes};

const WIDTH: usize = SCREEN_WIDTH as usize;
const HEIGHT: usize = SCREEN_HEIGHT as usize;

const NTSC_FPS: f64 = 60.0988;
const PAL_FPS: f64 = 50.007;

fn main() {
    let mut rom_path = None;
    let mut uncapped = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--uncapped" => uncapped = true,
            _ => rom_path = Some(arg),
        }
    }
    let path = rom_path.expect("Usage: runner <path-to-rom> [--uncapped]");

    let mut rom = std::fs::read(&path).expect("Failed to read ROM");
    if rom.len() % 1024 == 512 {
        rom.drain(..512);
    }

    let cartridge = Cartridge::new(rom).expect("Failed to parse cartridge");
    let title = cartridge.title().trim().to_string();
    let tv_system = cartridge.region().tv_system();
    let target_fps = match tv_system {
        TvSystem::Ntsc => NTSC_FPS,
        TvSystem::Pal => PAL_FPS,
    };
    let frame_duration = Duration::from_secs_f64(1.0 / target_fps);
    let mut snes = Snes::new(cartridge);

    let mut window = Window::new(
        &title,
        WIDTH,
        HEIGHT,
        WindowOptions {
            scale: Scale::X2,
            ..WindowOptions::default()
        },
    )
    .expect("Failed to create window");
    window.set_target_fps(0);

    let mut buffer = vec![0u32; WIDTH * HEIGHT];
    let mut frames = 0u32;
    let mut core_time = Duration::ZERO;
    let mut stats_since = Instant::now();
    let mut next_frame = Instant::now();

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let start = Instant::now();
        snes.run_frame();
        core_time += start.elapsed();
        frames += 1;

        for (dst, &(r, g, b)) in buffer.iter_mut().zip(snes.ppu.frame_buffer()) {
            *dst = u32::from_be_bytes([0, r, g, b]);
        }
        window
            .update_with_buffer(&buffer, WIDTH, HEIGHT)
            .expect("Failed to update window");

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
            window.set_title(&stats);
            println!("{stats}");

            frames = 0;
            core_time = Duration::ZERO;
            stats_since = Instant::now();
        }
    }
}

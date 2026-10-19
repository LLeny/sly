mod cartridge;
pub mod cartridge_header;

pub use cartridge::Cartridge;

use std::sync::Mutex;
use std::time::Duration;

use core_model::state::{Envelope, StateError, Thumbnail};
use core_model::{
    AudioSpec, Button, CartridgeError, CartridgeInfo, ControllerDescriptor, CoreError,
    DisplayRotation, RomFile, SessionInfo, SoftwareFrame,
};
use holani::consts::{LYNX_SCREEN_HEIGHT, LYNX_SCREEN_WIDTH};

const PAYLOAD_VERSION: u16 = 1;
const TICKS_PER_AUDIO_SAMPLE: usize = 16_000_000 / 22_050;

pub struct Lynx {
    machine: Mutex<holani::lynx::Lynx>,
    cartridge: Cartridge,
    pixels: Vec<[u8; 4]>,
    pending_audio: Vec<i16>,
    joystick: u8,
    switches: u8,
    audio_ticks: usize,
    frame_ticks: usize,
    last_interval: Duration,
    rotation: DisplayRotation,
    loaded: bool,
}

impl Lynx {
    pub fn new() -> Self {
        Self {
            machine: Mutex::new(holani::lynx::Lynx::new()),
            cartridge: Cartridge::default(),
            pixels: vec![[0; 4]; LYNX_SCREEN_WIDTH * LYNX_SCREEN_HEIGHT],
            pending_audio: Vec::new(),
            joystick: 0,
            switches: 0,
            audio_ticks: 0,
            frame_ticks: 0,
            last_interval: Duration::from_secs_f64(1.0 / 75.0),
            rotation: DisplayRotation::None,
            loaded: false,
        }
    }

    fn tick_with_audio(&mut self) {
        let machine = self.machine.get_mut().unwrap();
        machine.tick();
        self.audio_ticks += 1;
        if self.audio_ticks >= TICKS_PER_AUDIO_SAMPLE {
            self.audio_ticks = 0;
            let (left, right) = machine.audio_sample();
            self.pending_audio.extend([left, right]);
        }
    }
}

impl Default for Lynx {
    fn default() -> Self {
        Self::new()
    }
}

impl core_model::Core for Lynx {
    fn load_cartridge(&mut self, rom: RomFile) -> Result<(), CoreError> {
        let cartridge = Cartridge::load(rom);
        let machine = self.machine.get_mut().unwrap();
        machine
            .load_cart_from_slice(cartridge.data())
            .map_err(|_| CartridgeError::InvalidRom("unidentified cartridge format".into()))?;
        self.rotation = cartridge_header::rotation(machine.rotation());
        self.cartridge = cartridge;
        self.pixels = vec![[0; 4]; LYNX_SCREEN_WIDTH * LYNX_SCREEN_HEIGHT];
        self.pending_audio.clear();
        self.audio_ticks = 0;
        self.frame_ticks = 0;
        self.last_interval = Duration::from_secs_f64(1.0 / 75.0);
        self.loaded = true;
        self.reset();
        Ok(())
    }

    fn reset(&mut self) {
        if self.loaded {
            self.machine.get_mut().unwrap().reset();
        }
    }

    fn step(&mut self) {
        if !self.loaded {
            return;
        }
        self.tick_with_audio();
    }

    fn run_frame(&mut self) {
        if !self.loaded {
            return;
        }
        let machine = self.machine.get_mut().unwrap();
        self.frame_ticks = 0;
        while !machine.redraw_requested() {
            machine.tick();
            self.audio_ticks += 1;
            self.frame_ticks += 1;
            if self.audio_ticks >= TICKS_PER_AUDIO_SAMPLE {
                self.audio_ticks = 0;
                let (left, right) = machine.audio_sample();
                self.pending_audio.extend([left, right]);
            }
        }
        let seconds = self.frame_ticks as f64 / 16_000_000.0;
        if (1.0 / 240.0..=1.0 / 25.0).contains(&seconds) {
            self.last_interval = Duration::from_secs_f64(seconds);
        }
        for (pixel, &argb) in self.pixels.iter_mut().zip(machine.screen_argb()) {
            *pixel = [
                (argb >> 16) as u8,
                (argb >> 8) as u8,
                argb as u8,
                (argb >> 24) as u8,
            ];
        }
    }

    fn audio_spec(&self) -> Option<AudioSpec> {
        self.loaded.then_some(AudioSpec {
            sample_rate: 22_050,
            channels: 2,
        })
    }

    fn drain_audio(&mut self, out: &mut Vec<i16>) {
        out.append(&mut self.pending_audio);
    }

    fn set_button(&mut self, player: usize, button: Button, pressed: bool) {
        if player != 0 {
            return;
        }

        let button = rotate_button(button, self.rotation);

        const UP: u8 = 0b0100_0000;
        const DOWN: u8 = 0b1000_0000;
        const LEFT: u8 = 0b0001_0000;
        const RIGHT: u8 = 0b0010_0000;
        const OPTION_1: u8 = 0b0000_1000;
        const OPTION_2: u8 = 0b0000_0100;
        const INSIDE: u8 = 0b0000_0010;
        const OUTSIDE: u8 = 0b0000_0001;
        const PAUSE: u8 = 0b0000_0001;

        let bit = match button {
            Button::Up => UP,
            Button::Down => DOWN,
            Button::Left => LEFT,
            Button::Right => RIGHT,
            Button::A => INSIDE,
            Button::B => OUTSIDE,
            Button::L => OPTION_1,
            Button::R => OPTION_2,
            Button::Start => PAUSE,
            Button::X | Button::Y | Button::Select => return,
        };

        if button == Button::Start {
            set_bit(&mut self.switches, bit, pressed);
            self.machine
                .get_mut()
                .unwrap()
                .set_switches_u8(self.switches);
        } else {
            set_bit(&mut self.joystick, bit, pressed);
            self.machine
                .get_mut()
                .unwrap()
                .set_joystick_u8(self.joystick);
        }
    }

    fn controllers(&self) -> Vec<ControllerDescriptor> {
        vec![ControllerDescriptor {
            port: 0,
            name: "Lynx Pad".into(),
            buttons: Button::CLASSIC.to_vec(),
        }]
    }

    fn cartridge_info(&self) -> Option<CartridgeInfo> {
        Some(cartridge_header::info(self.cartridge.rom(), self.rotation))
    }

    fn session_info(&self) -> SessionInfo {
        let data = self.cartridge.data();
        SessionInfo {
            system: "Atari Lynx".into(),
            session: self.cartridge.title().into(),
            segments: vec![format!("{} KB", data.len() / 1024).into()],
        }
    }

    fn software_frame(&self) -> Option<SoftwareFrame<'_>> {
        Some(SoftwareFrame {
            width: LYNX_SCREEN_WIDTH as u32,
            height: LYNX_SCREEN_HEIGHT as u32,
            pixels: &self.pixels,
        })
    }

    fn display_aspect(&self) -> Option<f32> {
        let aspect = LYNX_SCREEN_WIDTH as f32 / LYNX_SCREEN_HEIGHT as f32;
        Some(if self.rotation == DisplayRotation::None {
            aspect
        } else {
            1.0 / aspect
        })
    }

    fn frame_interval(&self) -> Option<Duration> {
        self.loaded.then_some(self.last_interval)
    }

    fn save_state(&self) -> Result<Vec<u8>, StateError> {
        if !self.loaded {
            return Err(StateError::Unsupported);
        }

        let machine = self.machine.lock().unwrap();
        let mut payload = vec![0u8; machine.serialize_size()];
        holani::serialize(&machine, &mut payload)
            .map_err(|err| StateError::Corrupt(format!("encoding state: {err}")))?;

        let data = self.cartridge.data();

        Envelope {
            system: core_model::System::Lynx,
            rom_crc32: cartridge_header::identity(data).crc32,
            rom_len: data.len() as u32,
            payload_version: PAYLOAD_VERSION,
            thumbnail: Some(Thumbnail {
                width: LYNX_SCREEN_WIDTH as u32,
                height: LYNX_SCREEN_HEIGHT as u32,
                pixels: self.pixels.as_flattened(),
            }),
            payload: &payload,
        }
        .encode()
    }

    fn load_state(&mut self, bytes: &[u8]) -> Result<(), StateError> {
        let envelope = Envelope::decode(bytes)?;
        if envelope.system != core_model::System::Lynx {
            return Err(StateError::SystemMismatch);
        }

        envelope.check_payload_version(PAYLOAD_VERSION)?;

        let data = self.cartridge.data();
        envelope.check_rom(cartridge_header::identity(data).crc32, data.len())?;

        let restored = {
            let machine = self.machine.get_mut().unwrap();
            holani::deserialize(envelope.payload, machine)
                .map_err(|err| StateError::Corrupt(format!("decoding state: {err}")))?
        };
        self.machine = Mutex::new(restored);
        self.loaded = true;

        Ok(())
    }
}

fn rotate_button(button: Button, rotation: DisplayRotation) -> Button {
    match (rotation, button) {
        (DisplayRotation::Rotate90, Button::Left) => Button::Right,
        (DisplayRotation::Rotate90, Button::Right) => Button::Left,
        (DisplayRotation::Rotate270, Button::Up) => Button::Down,
        (DisplayRotation::Rotate270, Button::Down) => Button::Up,
        (DisplayRotation::Rotate270, Button::Left) => Button::Right,
        (DisplayRotation::Rotate270, Button::Right) => Button::Left,
        _ => button,
    }
}

fn set_bit(byte: &mut u8, bit: u8, pressed: bool) {
    if pressed {
        *byte |= bit;
    } else {
        *byte &= !bit;
    }
}

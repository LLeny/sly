mod echo;
pub(crate) mod registers;
mod tables;
mod timing;
mod voice;

use echo::{EchoUnit, StereoMix};
use registers::*;
use serde::{Deserialize, Serialize};
use timing::{KeyOnLatch, NoiseGenerator, RateCounter};
use voice::Voice;
#[cfg(feature = "debug")]
pub use voice::VoiceDebug;

const APU_ADDRESS_MASK: usize = 0xFFFF;
const SOURCE_DIRECTORY_PAGE_BYTES: usize = 0x100;
const SOURCE_DIRECTORY_ENTRY_BYTES: usize = 4;
const STEREO_CHANNELS: usize = 2;
const NOISE_RATE_MASK: u8 = 0x1F;
const SOFT_RESET: u8 = 0x80;
const PITCH_MODULATION_SAMPLE_SHIFT: u32 = 5;
const PITCH_MODULATION_SCALE_SHIFT: u32 = 10;
const VOICE_VOLUME_SCALE_SHIFT: u32 = 7;

#[cfg(feature = "debug")]
const VOICE_SCOPE: usize = 256;

#[cfg(feature = "debug")]
const DEBUG_CAPTURE_TICKS: u32 = 32_000;

#[cfg(feature = "debug")]
fn empty_voice_hist() -> Box<[[i16; VOICE_SCOPE]; VOICE_COUNT]> {
    Box::new([[0; VOICE_SCOPE]; VOICE_COUNT])
}

#[derive(Serialize, Deserialize)]
pub struct Dsp {
    regs: RegisterFile,
    voices: [Voice; VOICE_COUNT],
    noise: NoiseGenerator,
    counter: RateCounter,
    key_on: KeyOnLatch,
    echo: EchoUnit,

    #[cfg(feature = "debug")]
    #[serde(skip, default = "empty_voice_hist")]
    voice_hist: Box<[[i16; VOICE_SCOPE]; VOICE_COUNT]>,
    #[cfg(feature = "debug")]
    #[serde(skip)]
    voice_hist_pos: usize,
    #[cfg(feature = "debug")]
    #[serde(skip)]
    debug_capture: std::sync::atomic::AtomicU32,
}

impl Default for Dsp {
    fn default() -> Self {
        Self::new()
    }
}

impl Dsp {
    pub fn new() -> Self {
        let voices = std::array::from_fn(|_| Voice::new());
        Self {
            regs: RegisterFile::power_on(),
            voices,
            noise: NoiseGenerator::new(),
            counter: RateCounter::new(),
            key_on: KeyOnLatch::new(),
            echo: EchoUnit::new(),
            #[cfg(feature = "debug")]
            voice_hist: Box::new([[0; VOICE_SCOPE]; VOICE_COUNT]),
            #[cfg(feature = "debug")]
            voice_hist_pos: 0,
            #[cfg(feature = "debug")]
            debug_capture: std::sync::atomic::AtomicU32::new(0),
        }
    }

    #[cfg(feature = "debug")]
    fn arm_debug_capture(&self) {
        self.debug_capture
            .store(DEBUG_CAPTURE_TICKS, std::sync::atomic::Ordering::Relaxed);
    }

    #[cfg(feature = "debug")]
    pub fn voice_waveform(&self, i: usize) -> std::sync::Arc<[f32]> {
        const POINTS: usize = 64;

        self.arm_debug_capture();
        let hist = &self.voice_hist[i];
        let start = self.voice_hist_pos;
        (0..POINTS)
            .map(|k| hist[(start + k * VOICE_SCOPE / POINTS) % VOICE_SCOPE] as f32 / 32768.0)
            .collect()
    }

    pub fn read(&self, addr: u8) -> u8 {
        self.regs.read_masked(addr as usize)
    }

    #[cfg(feature = "debug")]
    pub fn reg(&self, addr: usize) -> u8 {
        self.read(addr as u8)
    }

    #[cfg(feature = "debug")]
    pub fn voice_pitch(&self, i: usize) -> i32 {
        VoiceRegisters::new(&self.regs, VoiceId::new(i)).pitch()
    }

    #[cfg(feature = "debug")]
    pub fn voice_debug(&self, i: usize) -> VoiceDebug {
        self.arm_debug_capture();
        self.voices[i].debug_snapshot()
    }

    pub fn write(&mut self, addr: u8, value: u8) {
        let addr = (addr & 0x7F) as usize;
        self.regs.write_raw(addr, value);

        match addr {
            R_KEY_ON => self.key_on.write(value),
            R_VOICE_END => self.regs[R_VOICE_END] = 0,
            _ => {}
        }
    }

    pub fn tick(&mut self, ram: &mut [u8; 0x10000]) -> (i16, i16) {
        #[cfg(feature = "debug")]
        let debug_capture = {
            use std::sync::atomic::Ordering::Relaxed;
            let remaining = self.debug_capture.load(Relaxed);
            if remaining > 0 {
                self.debug_capture.store(remaining - 1, Relaxed);
            }
            remaining > 0
        };

        let settings = SampleSettings::capture(&self.regs);

        let (poll_keys, key_on) = self.key_on.begin_sample();

        self.counter.advance();
        self.noise.clock(
            self.counter
                .fires((settings.flags & NOISE_RATE_MASK) as usize),
        );

        let mut main_out: StereoMix = [0; STEREO_CHANNELS];
        let mut echo_out: StereoMix = [0; STEREO_CHANNELS];
        let mut prev_output = 0i32;

        for i in 0..VOICE_COUNT {
            let voice_id = VoiceId::new(i);
            let base = voice_id.register_base();
            let vbit = voice_id.mask();
            let voice_registers = VoiceRegisters::new(&self.regs, voice_id);

            let source_number = voice_registers.source_number();
            let dir_addr = (settings.source_directory_page * SOURCE_DIRECTORY_PAGE_BYTES
                + source_number * SOURCE_DIRECTORY_ENTRY_BYTES)
                & APU_ADDRESS_MASK;
            let entry = (dir_addr + self.voices[i].source_directory_offset()) & APU_ADDRESS_MASK;
            let brr_next_addr =
                u16::from_le_bytes([ram[entry], ram[(entry + 1) & APU_ADDRESS_MASK]]);
            let adsr_control = voice_registers.adsr_control();
            let adsr_sustain = voice_registers.adsr_sustain();
            let gain = voice_registers.gain();
            let volumes = [voice_registers.volume(0), voice_registers.volume(1)];
            let mut pitch = voice_registers.pitch();
            let (mut brr_header, brr_byte) = self.voices[i].current_brr_bytes(ram);

            // pitch modulation
            if SampleSettings::enabled(settings.pitch_modulation, voice_id) {
                pitch += ((prev_output >> PITCH_MODULATION_SAMPLE_SHIFT) * pitch)
                    >> PITCH_MODULATION_SCALE_SHIFT;
            }

            // KEYON startup delay
            let keyed_on =
                self.voices[i].advance_key_on_delay(brr_next_addr, &mut brr_header, &mut pitch);

            // gauss interpolation
            let mut output = self.voices[i].interpolated_sample();
            if SampleSettings::enabled(settings.noise_voices, voice_id) {
                output = (self.noise.sample() as i16 as i32) * 2;
            }
            let (voice_output, envx_out) = self.voices[i].apply_envelope(output);

            #[cfg(feature = "debug")]
            if debug_capture {
                self.voices[i].record_signal(brr_header, pitch, output, voice_output);
                self.voice_hist[i][self.voice_hist_pos] = voice_output as i16;
            }

            if settings.flags & SOFT_RESET != 0 || self::brr_ends_without_loop(brr_header) {
                self.voices[i].silence_and_release();
            }

            if poll_keys {
                if SampleSettings::enabled(settings.key_off_voices, voice_id) {
                    self.voices[i].release();
                }

                if key_on & vbit != 0 {
                    self.voices[i].key_on();
                }
            }

            if self.voices[i].key_on_delay == 0 {
                self.voices[i].advance_envelope(adsr_control, adsr_sustain, gain, &self.counter);
            }

            // decode next BRR group
            let mut t_looped = false;
            if self.voices[i].needs_brr_decode() {
                self.voices[i].decode_brr_group(brr_byte, brr_header, ram);
                t_looped = self.voices[i].advance_brr_cursor(brr_header, brr_next_addr);
            }
            self.voices[i].advance_pitch(pitch);

            // MIX LETS GO
            for ch in 0..STEREO_CHANNELS {
                let amp = (voice_output * volumes[ch]) >> VOICE_VOLUME_SCALE_SHIFT;

                #[cfg(feature = "debug")]
                if debug_capture {
                    self.voices[i].record_mixed_sample(ch, amp);
                }

                main_out[ch] = (main_out[ch] + amp).clamp(-32768, 32767);
                if SampleSettings::enabled(settings.echo_voices, voice_id) {
                    echo_out[ch] = (echo_out[ch] + amp).clamp(-32768, 32767);
                }
            }

            if t_looped {
                self.regs[R_VOICE_END] |= vbit;
            }
            if keyed_on {
                self.regs[R_VOICE_END] &= !vbit;
            }
            self.regs[base + V_SAMPLE_OUTPUT] = (voice_output >> 8) as u8;
            self.regs[base + V_ENVELOPE_OUTPUT] = envx_out;

            prev_output = voice_output;
        }

        #[cfg(feature = "debug")]
        if debug_capture {
            self.voice_hist_pos = (self.voice_hist_pos + 1) % VOICE_SCOPE;
        }

        self.echo
            .process(&self.regs, ram, settings.flags, main_out, echo_out)
    }
}

fn brr_ends_without_loop(header: u8) -> bool {
    let header = BrrHeader::from_raw(header);
    header.end() && !header.looping()
}

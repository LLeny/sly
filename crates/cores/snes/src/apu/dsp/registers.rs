use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use std::ops::{Index, IndexMut};

pub(crate) const REGISTER_COUNT: usize = 128;
pub(crate) const VOICE_COUNT: usize = 8;
pub(crate) const VOICE_REGISTER_STRIDE: usize = 0x10;

pub(crate) const V_VOLUME_LEFT: usize = 0x00;
#[cfg_attr(not(feature = "debug"), expect(dead_code))] // only the debug views read it by name
pub(crate) const V_VOLUME_RIGHT: usize = 0x01;
pub(crate) const V_PITCH_LOW: usize = 0x02;
pub(crate) const V_PITCH_HIGH: usize = 0x03;
pub(crate) const V_SOURCE_NUMBER: usize = 0x04;
pub(crate) const V_ADSR_CONTROL: usize = 0x05;
pub(crate) const V_ADSR_SUSTAIN: usize = 0x06;
pub(crate) const V_GAIN: usize = 0x07;
pub(crate) const V_ENVELOPE_OUTPUT: usize = 0x08;
pub(crate) const V_SAMPLE_OUTPUT: usize = 0x09;

pub(crate) const R_MASTER_VOLUME_LEFT: usize = 0x0C;
pub(crate) const R_MASTER_VOLUME_RIGHT: usize = 0x1C;
pub(crate) const R_ECHO_VOLUME_LEFT: usize = 0x2C;
pub(crate) const R_ECHO_VOLUME_RIGHT: usize = 0x3C;
pub(crate) const R_KEY_ON: usize = 0x4C;
pub(crate) const R_KEY_OFF: usize = 0x5C;
pub(crate) const R_FLAGS: usize = 0x6C;
pub(crate) const R_VOICE_END: usize = 0x7C;
pub(crate) const R_ECHO_FEEDBACK: usize = 0x0D;
pub(crate) const R_PITCH_MODULATION: usize = 0x2D;
pub(crate) const R_NOISE_ENABLE: usize = 0x3D;
pub(crate) const R_ECHO_ENABLE: usize = 0x4D;
pub(crate) const R_SOURCE_DIRECTORY: usize = 0x5D;
pub(crate) const R_ECHO_START_ADDRESS: usize = 0x6D;
pub(crate) const R_ECHO_DELAY: usize = 0x7D;
pub(crate) const R_FIR_FIRST: usize = 0x0F;

const POWER_ON_FLAGS: u8 = 0xE0;

#[chapa::bitfield(u8, order = lsb0)]
pub(crate) struct AdsrControl {
    #[bits(0..=3)]
    pub attack: u8,

    #[bits(4..=6)]
    pub decay: u8,

    #[bits(7)]
    pub use_adsr: bool,
}

#[chapa::bitfield(u8, order = lsb0)]
pub(crate) struct AdsrSustain {
    #[bits(0..=4)]
    pub rate: u8,

    #[bits(5..=7)]
    pub level: u8,
}

#[chapa::bitfield(u8, order = lsb0)]
pub(crate) struct Gain {
    #[bits(0..=4)]
    pub rate: u8,

    #[bits(5..=7)]
    pub mode: u8,
}

#[chapa::bitfield(u8, order = lsb0)]
pub(crate) struct BrrHeader {
    #[bits(0)]
    pub end: bool,

    #[bits(1)]
    pub looping: bool,

    #[bits(2..=3)]
    pub filter: u8,

    #[bits(4..=7)]
    pub shift: u8,
}

#[derive(Serialize, Deserialize)]
pub(super) struct RegisterFile(#[serde(with = "BigArray")] [u8; REGISTER_COUNT]);

impl RegisterFile {
    pub(super) fn power_on() -> Self {
        let mut bytes = [0; REGISTER_COUNT];
        bytes[R_FLAGS] = POWER_ON_FLAGS;
        Self(bytes)
    }

    pub(super) fn read_masked(&self, address: usize) -> u8 {
        self.0[address & (REGISTER_COUNT - 1)]
    }

    pub(super) fn write_raw(&mut self, address: usize, value: u8) {
        self.0[address] = value;
    }
}

impl Index<usize> for RegisterFile {
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl IndexMut<usize> for RegisterFile {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

#[derive(Clone, Copy)]
pub(super) struct VoiceId(usize);

impl VoiceId {
    pub(super) const fn new(index: usize) -> Self {
        debug_assert!(index < VOICE_COUNT);
        Self(index)
    }

    pub(super) const fn register_base(self) -> usize {
        self.0 * VOICE_REGISTER_STRIDE
    }

    pub(super) const fn mask(self) -> u8 {
        1 << self.0
    }
}

#[derive(Clone, Copy)]
pub(super) struct SampleSettings {
    pub(super) flags: u8,
    pub(super) pitch_modulation: u8,
    pub(super) noise_voices: u8,
    pub(super) echo_voices: u8,
    pub(super) source_directory_page: usize,
    pub(super) key_off_voices: u8,
}

impl SampleSettings {
    pub(super) fn capture(registers: &RegisterFile) -> Self {
        Self {
            flags: registers[R_FLAGS],
            pitch_modulation: registers[R_PITCH_MODULATION] & 0xFE,
            noise_voices: registers[R_NOISE_ENABLE],
            echo_voices: registers[R_ECHO_ENABLE],
            source_directory_page: registers[R_SOURCE_DIRECTORY] as usize,
            key_off_voices: registers[R_KEY_OFF],
        }
    }

    pub(super) const fn enabled(mask: u8, voice: VoiceId) -> bool {
        mask & voice.mask() != 0
    }
}

pub(super) struct VoiceRegisters<'a> {
    registers: &'a RegisterFile,
    base: usize,
}

impl<'a> VoiceRegisters<'a> {
    pub(super) fn new(registers: &'a RegisterFile, voice: VoiceId) -> Self {
        Self {
            registers,
            base: voice.register_base(),
        }
    }

    pub(super) fn source_number(&self) -> usize {
        self.registers[self.base + V_SOURCE_NUMBER] as usize
    }

    pub(super) fn pitch(&self) -> i32 {
        self.registers[self.base + V_PITCH_LOW] as i32
            | ((self.registers[self.base + V_PITCH_HIGH] as i32 & 0x3F) << 8)
    }

    pub(super) fn adsr_control(&self) -> u8 {
        self.registers[self.base + V_ADSR_CONTROL]
    }

    pub(super) fn adsr_sustain(&self) -> u8 {
        self.registers[self.base + V_ADSR_SUSTAIN]
    }

    pub(super) fn gain(&self) -> u8 {
        self.registers[self.base + V_GAIN]
    }

    pub(super) fn volume(&self, channel: usize) -> i32 {
        self.registers[self.base + V_VOLUME_LEFT + channel] as i8 as i32
    }
}

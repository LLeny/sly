use super::APU_ADDRESS_MASK;
use super::tables::GAUSS;
use super::timing::RateCounter;

const BRR_RING_SAMPLES: usize = 12;
pub(super) const BRR_BLOCK_BYTES: u16 = 9;
pub(super) const BRR_DATA_START: u16 = 1;
pub(super) const BRR_DECODE_THRESHOLD: i32 = 0x4000;
pub(super) const MAX_INTERPOLATION_POSITION: i32 = 0x7FFF;
const MAX_ENVELOPE: i32 = 0x7FF;
const SOURCE_LOOP_ADDRESS_OFFSET: usize = 2;
const ENVELOPE_SCALE_SHIFT: u32 = 11;
const GAUSSIAN_SCALE_SHIFT: u32 = 11;
const INTERPOLATION_SAMPLE_SHIFT: u32 = 12;
const GAUSSIAN_FRACTION_SHIFT: u32 = 4;
const GAUSSIAN_FRACTION_MASK: i32 = 0xFF;

#[cfg(feature = "debug")]
pub struct VoiceDebug {
    pub env: i32,
    pub envx: u8,
    pub env_mode: &'static str,
    pub hidden_env: i32,
    pub brr_addr: u16,
    pub brr_offset: u16,
    pub brr_header: u8,
    pub interpolation_position: i32,
    pub key_on_delay: u8,
    pub effective_pitch: i32,
    pub source_sample: i32,
    pub output_sample: i32,
    pub mixed_sample: [i32; 2],
}

#[cfg(feature = "debug")]
#[derive(Clone, Copy, Default)]
struct SignalDebug {
    brr_header: u8,
    effective_pitch: i32,
    source_sample: i32,
    output_sample: i32,
    mixed_sample: [i32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum EnvelopeMode {
    Release,
    Attack,
    Decay,
    Sustain,
}

#[derive(Clone)]
pub(super) struct Voice {
    samples: [i32; BRR_RING_SAMPLES * 2],
    pub(super) sample_cursor: usize,
    pub(super) interpolation_position: i32,
    pub(super) brr_address: u16,
    pub(super) brr_offset: u16,
    pub(super) key_on_delay: u8,
    pub(super) envelope_mode: EnvelopeMode,
    pub(super) envelope: i32,
    hidden_envelope: i32,

    #[cfg(feature = "debug")]
    signal_debug: SignalDebug,
}

impl Voice {
    pub(super) const fn new() -> Self {
        Self {
            samples: [0; BRR_RING_SAMPLES * 2],
            sample_cursor: 0,
            interpolation_position: 0,
            brr_address: 0,
            brr_offset: BRR_DATA_START,
            key_on_delay: 0,
            envelope_mode: EnvelopeMode::Release,
            envelope: 0,
            hidden_envelope: 0,

            #[cfg(feature = "debug")]
            signal_debug: SignalDebug {
                brr_header: 0,
                effective_pitch: 0,
                source_sample: 0,
                output_sample: 0,
                mixed_sample: [0; 2],
            },
        }
    }

    #[cfg(feature = "debug")]
    pub(super) fn debug_snapshot(&self) -> VoiceDebug {
        VoiceDebug {
            env: self.envelope,
            envx: (self.envelope >> 4) as u8,
            env_mode: match self.envelope_mode {
                EnvelopeMode::Release => "rel",
                EnvelopeMode::Attack => "atk",
                EnvelopeMode::Decay => "dec",
                EnvelopeMode::Sustain => "sus",
            },
            hidden_env: self.hidden_envelope,
            brr_addr: self.brr_address,
            brr_offset: self.brr_offset,
            brr_header: self.signal_debug.brr_header,
            interpolation_position: self.interpolation_position,
            key_on_delay: self.key_on_delay,
            effective_pitch: self.signal_debug.effective_pitch,
            source_sample: self.signal_debug.source_sample,
            output_sample: self.signal_debug.output_sample,
            mixed_sample: self.signal_debug.mixed_sample,
        }
    }

    #[cfg(feature = "debug")]
    pub(super) fn record_signal(
        &mut self,
        brr_header: u8,
        effective_pitch: i32,
        source_sample: i32,
        output_sample: i32,
    ) {
        self.signal_debug = SignalDebug {
            brr_header,
            effective_pitch,
            source_sample,
            output_sample,
            mixed_sample: [0; 2],
        };
    }

    #[cfg(feature = "debug")]
    pub(super) fn record_mixed_sample(&mut self, channel: usize, sample: i32) {
        self.signal_debug.mixed_sample[channel] = sample;
    }

    pub(super) const fn source_directory_offset(&self) -> usize {
        if self.key_on_delay == 0 {
            SOURCE_LOOP_ADDRESS_OFFSET
        } else {
            0
        }
    }

    pub(super) fn current_brr_bytes(&self, ram: &[u8; 0x10000]) -> (u8, u8) {
        (
            ram[self.brr_address as usize],
            ram[(self.brr_address as usize + self.brr_offset as usize) & APU_ADDRESS_MASK],
        )
    }

    pub(super) fn advance_key_on_delay(
        &mut self,
        source_start: u16,
        brr_header: &mut u8,
        pitch: &mut i32,
    ) -> bool {
        if self.key_on_delay == 0 {
            return false;
        }

        let starts_source = self.key_on_delay == 5;
        if starts_source {
            self.brr_address = source_start;
            self.brr_offset = BRR_DATA_START;
            self.sample_cursor = 0;
            *brr_header = 0;
        }

        self.envelope = 0;
        self.hidden_envelope = 0;
        self.key_on_delay -= 1;
        self.interpolation_position = if self.key_on_delay & 3 != 0 {
            BRR_DECODE_THRESHOLD
        } else {
            0
        };

        *pitch = 0;

        starts_source
    }

    pub(super) fn key_on(&mut self) {
        self.key_on_delay = 5;
        self.envelope_mode = EnvelopeMode::Attack;
    }

    pub(super) fn release(&mut self) {
        self.envelope_mode = EnvelopeMode::Release;
    }

    pub(super) fn silence_and_release(&mut self) {
        self.envelope_mode = EnvelopeMode::Release;
        self.envelope = 0;
    }

    pub(super) fn apply_envelope(&mut self, sample: i32) -> (i32, u8) {
        let output = (sample * self.envelope) >> ENVELOPE_SCALE_SHIFT & !1;
        (output, (self.envelope >> 4) as u8)
    }

    pub(super) const fn needs_brr_decode(&self) -> bool {
        self.interpolation_position >= BRR_DECODE_THRESHOLD
    }

    pub(super) fn advance_pitch(&mut self, pitch: i32) {
        self.interpolation_position =
            (self.interpolation_position & (BRR_DECODE_THRESHOLD - 1)) + pitch;
        self.interpolation_position = self.interpolation_position.min(MAX_INTERPOLATION_POSITION);
    }

    #[inline]
    pub(super) fn interpolated_sample(&self) -> i32 {
        let base = ((self.interpolation_position >> INTERPOLATION_SAMPLE_SHIFT) as usize)
            + self.sample_cursor;
        let sample = |offset: usize| self.samples[base + offset];
        let fraction = ((self.interpolation_position >> GAUSSIAN_FRACTION_SHIFT)
            & GAUSSIAN_FRACTION_MASK) as usize;

        let mut output = (GAUSS[255 - fraction] as i32 * sample(0)) >> GAUSSIAN_SCALE_SHIFT;
        output += (GAUSS[511 - fraction] as i32 * sample(1)) >> GAUSSIAN_SCALE_SHIFT;
        output += (GAUSS[256 + fraction] as i32 * sample(2)) >> GAUSSIAN_SCALE_SHIFT;
        output = output as i16 as i32;
        output += (GAUSS[fraction] as i32 * sample(3)) >> GAUSSIAN_SCALE_SHIFT;

        output.clamp(-32768, 32767) & !1
    }

    pub(super) fn decode_brr_group(&mut self, first_byte: u8, header: u8, ram: &[u8; 0x10000]) {
        let second_byte =
            ram[(self.brr_address as usize + self.brr_offset as usize + 1) & APU_ADDRESS_MASK];
        let mut nibbles = (first_byte as i32) * 0x100 + second_byte as i32;

        let group_start = self.sample_cursor;
        self.sample_cursor = (group_start + 4) % BRR_RING_SAMPLES;

        let shift = (header >> 4) as i32;
        let filter = (header & 0x0C) as i32;

        for sample_index in 0..4 {
            let mut sample = (nibbles as i16 >> 12) as i32;
            if shift <= 12 {
                sample = (sample << shift) >> 1;
            } else {
                sample &= !0x7FF;
            }

            let index = group_start + sample_index;
            let previous = self.samples[index + BRR_RING_SAMPLES - 1];
            let previous_two = self.samples[index + BRR_RING_SAMPLES - 2] >> 1;

            if filter >= 8 {
                sample += previous;
                sample -= previous_two;

                if filter == 8 {
                    sample += previous_two >> 4;
                    sample += (previous * -3) >> 6;
                } else {
                    sample += (previous * -13) >> 7;
                    sample += (previous_two * 3) >> 4;
                }
            } else if filter != 0 {
                sample += previous >> 1;
                sample += (-previous) >> 5;
            }

            let sample = (sample.clamp(-32768, 32767) as i16).wrapping_mul(2) as i32;
            self.samples[index] = sample;
            self.samples[index + BRR_RING_SAMPLES] = sample;

            nibbles <<= 4;
        }
    }

    pub(super) fn advance_brr_cursor(&mut self, header: u8, loop_address: u16) -> bool {
        self.brr_offset += 2;
        if self.brr_offset < BRR_BLOCK_BYTES {
            return false;
        }

        self.brr_address = self.brr_address.wrapping_add(BRR_BLOCK_BYTES);

        let looped = header & 1 != 0;
        if looped {
            self.brr_address = loop_address;
        }

        self.brr_offset = BRR_DATA_START;

        looped
    }

    pub(super) fn advance_envelope(
        &mut self,
        adsr_control: u8,
        adsr_sustain: u8,
        gain: u8,
        counter: &RateCounter,
    ) {
        let mut envelope = self.envelope;
        if self.envelope_mode == EnvelopeMode::Release {
            self.envelope = (envelope - 0x8).max(0);
            return;
        }

        let rate;
        let mut envelope_data = adsr_sustain as i32;
        if adsr_control & 0x80 != 0 {
            if self.envelope_mode >= EnvelopeMode::Decay {
                envelope -= 1;
                envelope -= envelope >> 8;

                rate = if self.envelope_mode == EnvelopeMode::Decay {
                    ((adsr_control as i32) >> 3 & 0x0E) + 0x10
                } else {
                    envelope_data & 0x1F
                };
            } else {
                rate = (adsr_control as i32 & 0x0F) * 2 + 1;
                envelope += if rate < 31 { 0x20 } else { 0x400 };
            }
        } else {
            envelope_data = gain as i32;

            let mode = envelope_data >> 5;
            if mode < 4 {
                envelope = envelope_data * 0x10;
                rate = 31;
            } else {
                rate = envelope_data & 0x1F;

                if mode == 4 {
                    envelope -= 0x20;
                } else if mode < 6 {
                    envelope -= 1;
                    envelope -= envelope >> 8;
                } else {
                    envelope += 0x20;

                    if mode > 6 && self.hidden_envelope as u32 >= 0x600 {
                        envelope += 0x8 - 0x20;
                    }
                }
            }
        }

        if (envelope >> 8) == (envelope_data >> 5) && self.envelope_mode == EnvelopeMode::Decay {
            self.envelope_mode = EnvelopeMode::Sustain;
        }

        self.hidden_envelope = envelope;
        if envelope as u32 > MAX_ENVELOPE as u32 {
            envelope = if envelope < 0 { 0 } else { MAX_ENVELOPE };
            if self.envelope_mode == EnvelopeMode::Attack {
                self.envelope_mode = EnvelopeMode::Decay;
            }
        }

        if counter.fires(rate as usize) {
            self.envelope = envelope;
        }
    }
}

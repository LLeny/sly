use super::registers::{
    R_ECHO_DELAY, R_ECHO_FEEDBACK, R_ECHO_START_ADDRESS, R_ECHO_VOLUME_LEFT, R_ECHO_VOLUME_RIGHT,
    R_FIR_FIRST, R_MASTER_VOLUME_LEFT, R_MASTER_VOLUME_RIGHT, RegisterFile, VOICE_REGISTER_STRIDE,
};
use super::{APU_ADDRESS_MASK as ADDRESS_MASK, STEREO_CHANNELS as CHANNELS};

pub(super) type StereoMix = [i32; CHANNELS];

const FIR_TAPS: usize = 8;
const BYTES_PER_ECHO_SAMPLE: u32 = 4;
const ECHO_DELAY_BYTES: u32 = 0x800;
const ECHO_BASE_PAGE_BYTES: usize = 0x100;
const ECHO_WRITE_DISABLE: u8 = 0x20;
const GLOBAL_MUTE: u8 = 0x40;
const FIR_COEFFICIENT_SCALE_SHIFT: u32 = 6;
const VOLUME_SCALE_SHIFT: u32 = 7;

pub(super) struct EchoUnit {
    history: [[i32; CHANNELS]; FIR_TAPS],
    history_position: usize,
    byte_offset: u32,
    byte_length: u32,
}

impl EchoUnit {
    pub(super) const fn new() -> Self {
        Self {
            history: [[0; CHANNELS]; FIR_TAPS],
            history_position: 0,
            byte_offset: 0,
            byte_length: 0,
        }
    }

    pub(super) fn process(
        &mut self,
        registers: &RegisterFile,
        ram: &mut [u8; 0x10000],
        flags: u8,
        main_mix: StereoMix,
        echo_mix: StereoMix,
    ) -> (i16, i16) {
        self.history_position = (self.history_position + 1) & (FIR_TAPS - 1);

        let echo_base = registers[R_ECHO_START_ADDRESS] as usize * ECHO_BASE_PAGE_BYTES;
        let echo_address = (echo_base + self.byte_offset as usize) & ADDRESS_MASK;

        let read_sample = |address: usize| {
            u16::from_le_bytes([ram[address], ram[(address + 1) & ADDRESS_MASK]]) as i16 as i32
        };
        self.history[self.history_position] = [
            read_sample(echo_address) >> 1,
            read_sample((echo_address + 2) & ADDRESS_MASK) >> 1,
        ];

        let filtered_echo = self.filter(registers);
        let output = mix_for_dac(registers, main_mix, filtered_echo);

        if flags & ECHO_WRITE_DISABLE == 0 {
            self.write_feedback(registers, ram, echo_address, echo_mix, filtered_echo);
        }

        self.advance_buffer(registers);

        if flags & GLOBAL_MUTE != 0 {
            (0, 0)
        } else {
            (output[0], output[1])
        }
    }

    fn filter(&self, registers: &RegisterFile) -> StereoMix {
        let tap = |channel: usize, index: usize| {
            let history = (self.history_position + 1 + index) & (FIR_TAPS - 1);
            let coefficient = registers[R_FIR_FIRST + index * VOICE_REGISTER_STRIDE] as i8 as i32;
            (self.history[history][channel] * coefficient) >> FIR_COEFFICIENT_SCALE_SHIFT
        };

        let mut output = [0; CHANNELS];
        for (channel, slot) in output.iter_mut().enumerate() {
            let mut sum = tap(channel, 0) + tap(channel, 1) + tap(channel, 2);
            sum += tap(channel, 3) + tap(channel, 4) + tap(channel, 5);
            sum = (sum + tap(channel, 6)) as i16 as i32;
            sum += tap(channel, 7) as i16 as i32;
            *slot = sum.clamp(-32768, 32767) & !1;
        }

        output
    }

    fn write_feedback(
        &self,
        registers: &RegisterFile,
        ram: &mut [u8; 0x10000],
        echo_address: usize,
        voice_mix: StereoMix,
        filtered_echo: StereoMix,
    ) {
        let feedback_volume = registers[R_ECHO_FEEDBACK] as i8 as i32;

        for channel in 0..CHANNELS {
            let feedback =
                ((filtered_echo[channel] * feedback_volume) >> VOLUME_SCALE_SHIFT) as i16 as i32;
            let sample = (voice_mix[channel] + feedback).clamp(-32768, 32767) & !1;

            let address = echo_address + channel * 2;
            let [low, high] = (sample as u16).to_le_bytes();
            ram[address & ADDRESS_MASK] = low;
            ram[(address + 1) & ADDRESS_MASK] = high;
        }
    }

    fn advance_buffer(&mut self, registers: &RegisterFile) {
        if self.byte_offset == 0 {
            self.byte_length = (registers[R_ECHO_DELAY] as u32 & 0x0F) * ECHO_DELAY_BYTES;
        }

        self.byte_offset += BYTES_PER_ECHO_SAMPLE;

        if self.byte_offset >= self.byte_length {
            self.byte_offset = 0;
        }
    }
}

fn mix_for_dac(
    registers: &RegisterFile,
    main_mix: StereoMix,
    filtered_echo: StereoMix,
) -> [i16; CHANNELS] {
    let master_volume = [
        registers[R_MASTER_VOLUME_LEFT] as i8 as i32,
        registers[R_MASTER_VOLUME_RIGHT] as i8 as i32,
    ];
    let echo_volume = [
        registers[R_ECHO_VOLUME_LEFT] as i8 as i32,
        registers[R_ECHO_VOLUME_RIGHT] as i8 as i32,
    ];

    std::array::from_fn(|channel| {
        let main =
            ((main_mix[channel] * master_volume[channel]) >> VOLUME_SCALE_SHIFT) as i16 as i32;
        let echo =
            ((filtered_echo[channel] * echo_volume[channel]) >> VOLUME_SCALE_SHIFT) as i16 as i32;
        (main + echo).clamp(-32768, 32767) as i16
    })
}

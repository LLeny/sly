use crate::apu::regs::Control;
#[cfg(feature = "debug")]
use crate::mmio::{AccessTracker, MemKind};
use crate::{Bus, MemoryMap};
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

pub mod dsp;
pub mod io;
mod map;
mod regs;
pub mod spc700;
mod symbol;

pub use dsp::Dsp;
pub use map::{ApuMap, HardwareMap, Region};
pub use symbol::{is_symbol, symbol, symbols};

pub const APU_CLOCK: u64 = 24_576_000;
pub const SDSP_CLOCK: u64 = APU_CLOCK / 8;
pub const SPC700_CLOCK: u64 = APU_CLOCK / 24;

pub const SPC_TIMER01_DIVIDER: u64 = 128;
pub const SPC_TIMER2_DIVIDER: u64 = 16;

pub const DSP_SAMPLE_PERIOD_SPC: u64 = 32;
const AUDIO_BUFFER_CAP: usize = 0x10000;

const IPL_ROM: &[u8] = include_bytes!("../../../../bins/snes/spc700.rom");

#[derive(Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Apu<M: ApuMap = HardwareMap> {
    #[serde(skip)]
    map: M,

    // Registers ($00F0-$00FF except $F8-$F9)
    unknown: u8,      // $F0
    control: Control, // $F1 (W)
    dsp_addr: u8,     // $F2 (DSP register select)
    pub dsp: Dsp,     // $F3 reads/writes the selected DSP register
    #[serde(skip)]
    audio_out: Vec<i16>, // interleaved stereo samples produced by the DSP
    #[cfg(feature = "debug")]
    #[serde(skip)]
    samples_generated: u64,
    cpu_to_apu: [u8; 4], // written by main CPU $2140-$2143, read by SPC $F4-$F7
    apu_to_cpu: [u8; 4], // written by SPC $F4-$F7, read by main CPU $2140-$2143
    timer_settings: [u8; 3], // $FA-$FC on write
    timer_counters: [u8; 3], // $FD-$FF on read

    internal_timers: [u16; 3], // internal counters for timers 0, 1 and 2

    // 64 KB RAM
    #[serde(with = "BigArray")]
    ram: [u8; 0x10000],

    // SPC-700 core registers
    pub spc700: spc700::Spc700,

    /// Active SPC700 subroutine frames
    #[cfg(feature = "debug")]
    #[serde(skip)]
    pub call_stack: Vec<crate::callstack::CallFrame>,

    #[cfg(feature = "debug")]
    #[serde(skip)]
    pub access: AccessTracker,
}

impl Apu<HardwareMap> {
    pub fn new() -> Self {
        Self::with_map(HardwareMap)
    }
}

impl<M: ApuMap> Apu<M> {
    #[cfg(feature = "debug")]
    pub(crate) fn samples_generated(&self) -> u64 {
        self.samples_generated
    }

    pub fn with_map(map: M) -> Self {
        let reset_vector = u16::from_le_bytes([IPL_ROM[0x3E], IPL_ROM[0x3F]]);

        Self {
            map,
            unknown: 0,
            control: Control::default(),
            dsp_addr: 0,
            dsp: Dsp::new(),
            audio_out: Vec::new(),
            #[cfg(feature = "debug")]
            samples_generated: 0,
            cpu_to_apu: [0; 4],
            apu_to_cpu: [0; 4],
            timer_settings: [0; 3],
            timer_counters: [0; 3],
            internal_timers: [0; 3],
            ram: [0; 0x10000],
            spc700: spc700::Spc700::new(reset_vector),
            #[cfg(feature = "debug")]
            call_stack: Vec::new(),
            #[cfg(feature = "debug")]
            access: AccessTracker::default(),
        }
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        #[cfg(feature = "debug")]
        self.access.record((MemKind::Spc, addr as u32), false);

        match self.map.region(addr) {
            Region::Register => self.read_io(addr as u8),
            Region::Ipl if self.control.ipl_rom_enable() => IPL_ROM[(addr - 0xFFC0) as usize],
            _ => self.ram[addr as usize],
        }
    }

    pub fn cpu_read_port(&self, index: usize) -> u8 {
        self.apu_to_cpu[index]
    }

    pub fn cpu_write_port(&mut self, index: usize, value: u8) {
        self.cpu_to_apu[index] = value;
    }

    pub fn dsp_tick(&mut self) {
        let (left, right) = self.dsp.tick(&mut self.ram);
        #[cfg(feature = "debug")]
        {
            self.samples_generated += 1;
        }
        if self.audio_out.len() < AUDIO_BUFFER_CAP {
            self.audio_out.push(left);
            self.audio_out.push(right);
        }
    }

    pub fn drain_audio(&mut self, out: &mut Vec<i16>) {
        out.append(&mut self.audio_out);
    }

    pub fn peek(&self, addr: u16) -> u8 {
        match self.map.region(addr) {
            Region::Register => self.peek_io(addr as u8),
            Region::Ipl if self.control.ipl_rom_enable() => IPL_ROM[(addr - 0xFFC0) as usize],
            _ => self.ram[addr as usize],
        }
    }

    pub fn write(&mut self, addr: u16, value: u8) {
        #[cfg(feature = "debug")]
        self.access.record((MemKind::Spc, addr as u32), true);

        match self.map.region(addr) {
            Region::Register => self.write_io(addr as u8, value),
            _ => self.ram[addr as usize] = value,
        }
    }

    /// Sideeffect free & allows reading write only registers.
    fn peek_io(&self, reg: u8) -> u8 {
        match reg {
            0xF0 => self.unknown,
            0xF1 => self.control.raw(),
            0xF2 => self.dsp_addr,
            0xF3 => self.dsp.read(self.dsp_addr),
            0xF4..=0xF7 => self.cpu_to_apu[(reg - 0xF4) as usize],
            0xFA..=0xFC => self.timer_settings[(reg - 0xFA) as usize],
            0xFD..=0xFF => self.timer_counters[(reg - 0xFD) as usize],
            _ => 0,
        }
    }

    fn read_io(&mut self, reg: u8) -> u8 {
        match reg {
            0xFA..=0xFC => {
                tracing::error!(
                    reg = format!("{:02X}", reg),
                    "APU read from write-only I/O register"
                );
                0
            }
            0xFD..=0xFF => {
                let value = self.peek_io(reg);
                self.timer_counters[(reg - 0xFD) as usize] = 0;
                value
            }
            _ => self.peek_io(reg),
        }
    }

    fn write_io(&mut self, reg: u8, value: u8) {
        match reg {
            0xF0 => self.unknown = value,
            0xF1 => {
                let ctrl = Control::from_raw(value);

                if ctrl.clear_cpuio0() {
                    self.cpu_to_apu[0] = 0;
                    self.cpu_to_apu[1] = 0;
                }

                if ctrl.clear_cpuio1() {
                    self.cpu_to_apu[2] = 0;
                    self.cpu_to_apu[3] = 0;
                }

                if !self.control.enable_timer0() && ctrl.enable_timer0() {
                    self.timer_counters[0] = 0;
                    self.internal_timers[0] = 0;
                }

                if !self.control.enable_timer1() && ctrl.enable_timer1() {
                    self.timer_counters[1] = 0;
                    self.internal_timers[1] = 0;
                }

                if !self.control.enable_timer2() && ctrl.enable_timer2() {
                    self.timer_counters[2] = 0;
                    self.internal_timers[2] = 0;
                }

                self.control = ctrl;
            }
            0xF2 => self.dsp_addr = value,
            0xF3 => self.dsp.write(self.dsp_addr, value),
            0xF4..=0xF7 => self.apu_to_cpu[(reg - 0xF4) as usize] = value,
            0xFA..=0xFC => self.timer_settings[(reg - 0xFA) as usize] = value,
            _ => {
                tracing::error!(
                    reg = format!("{:02X}", reg),
                    value = format!("{:02X}", value),
                    "APU write to read-only/unmapped I/O register"
                );
            }
        }
    }
}

fn tick_timer(apu: &mut Apu<impl ApuMap>, index: usize) {
    apu.internal_timers[index] += 1;

    let divider = match apu.timer_settings[index] {
        0 => 256,
        value => value as u16,
    };

    if apu.internal_timers[index] >= divider {
        apu.internal_timers[index] = 0;
        apu.timer_counters[index] = apu.timer_counters[index].wrapping_add(1) & 0x0F;
    }
}

pub(crate) fn timer01_tick<M: MemoryMap>(snes: &mut Bus<M>, _deadline: u64) {
    if snes.apu.control.enable_timer0() {
        tick_timer(&mut snes.apu, 0);
    }

    if snes.apu.control.enable_timer1() {
        tick_timer(&mut snes.apu, 1);
    }
}

pub(crate) fn timer2_tick<M: MemoryMap>(snes: &mut Bus<M>, _deadline: u64) {
    if snes.apu.control.enable_timer2() {
        tick_timer(&mut snes.apu, 2);
    }
}

pub(crate) fn dsp_tick<M: MemoryMap>(snes: &mut Bus<M>, _deadline: u64) {
    snes.apu.dsp_tick();
}

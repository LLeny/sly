use crate::apu::regs::Control;
#[cfg(feature = "debug")]
use crate::mmio::{AccessTracker, MemKind, MemoryAccess};

mod map;
mod regs;
pub mod spc700;
mod symbol;

pub use map::{ApuMap, HardwareMap, Region};
pub use symbol::{is_symbol, symbol, symbols};

pub const APU_CLOCK: u64 = 24_576_000;
pub const SDSP_CLOCK: u64 = APU_CLOCK / 8;
pub const SPC700_CLOCK: u64 = APU_CLOCK / 24;

pub const SPC_TIMER01_DIVIDER: u64 = 128;
pub const SPC_TIMER2_DIVIDER: u64 = 16;

pub const SNES_CYCLES_PER_SPC700_CYCLE: u64 = 21;

const IPL_ROM: &[u8] = include_bytes!("../assets/spc700.rom");

pub struct Apu<M: ApuMap = HardwareMap> {
    map: M,

    // Registers ($00F0-$00FF except $F8-$F9)
    unknown: u8,             // $F0
    control: Control,        // $F1 (W)
    dsp_addr: u8,            // $F2
    dsp_data: u8,            // $F3
    cpu_to_apu: [u8; 4],     // written by main CPU $2140-$2143, read by SPC $F4-$F7
    apu_to_cpu: [u8; 4],     // written by SPC $F4-$F7, read by main CPU $2140-$2143
    timer_settings: [u8; 3], // $FA-$FC on write
    timer_counters: [u8; 3], // $FD-$FF on read

    // 64 KB RAM
    ram: [u8; 0x10000],

    // SPC-700 core registers
    pub spc700: spc700::Spc700,

    #[cfg(feature = "debug")]
    pub access: AccessTracker,
}

impl Apu<HardwareMap> {
    pub fn new() -> Self {
        Self::with_map(HardwareMap)
    }
}

impl<M: ApuMap> Apu<M> {
    pub fn with_map(map: M) -> Self {
        let reset_vector = u16::from_le_bytes([IPL_ROM[0x3E], IPL_ROM[0x3F]]);

        Self {
            map,
            unknown: 0,
            control: Control::default(),
            dsp_addr: 0,
            dsp_data: 0,
            cpu_to_apu: [0; 4],
            apu_to_cpu: [0; 4],
            timer_settings: [0; 3],
            timer_counters: [0; 3],
            ram: [0; 0x10000],
            spc700: spc700::Spc700::new(reset_vector),

            #[cfg(feature = "debug")]
            access: AccessTracker::default(),
        }
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        #[cfg(feature = "debug")]
        self.access.record((MemKind::Spc, addr as u32), false);

        tracing::trace!(addr = format!("{:04X}", addr), "APU read");

        match self.map.region(addr) {
            Region::Register => self.read_io(addr as u8),
            Region::Ipl if self.control.ipl_rom_enable() => IPL_ROM[(addr - 0xFFC0) as usize],
            _ => self.ram[addr as usize],
        }
    }

    #[cfg(feature = "debug")]
    pub fn trace(&mut self, body: impl FnOnce(&mut Self)) -> MemoryAccess {
        self.access.start();
        self.access.resume();
        body(self);
        self.access.stop();
        self.access.snapshot()
    }

    pub fn cpu_read_port(&self, index: usize) -> u8 {
        self.apu_to_cpu[index]
    }

    pub fn cpu_write_port(&mut self, index: usize, value: u8) {
        self.cpu_to_apu[index] = value;
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

        tracing::trace!(
            addr = format!("{:04X}", addr),
            value = format!("{:02X}", value),
            "APU write"
        );

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
            0xF3 => self.dsp_data,
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
            _ => self.peek_io(reg),
        }
    }

    fn write_io(&mut self, reg: u8, value: u8) {
        match reg {
            0xF0 => self.unknown = value,
            0xF1 => {
                self.control = Control::from_raw(value);
                if self.control.clear_cpuio0() {
                    self.cpu_to_apu[0] = 0;
                    self.cpu_to_apu[1] = 0;
                }
                if self.control.clear_cpuio1() {
                    self.cpu_to_apu[2] = 0;
                    self.cpu_to_apu[3] = 0;
                }
            }
            0xF2 => self.dsp_addr = value,
            0xF3 => self.dsp_data = value,
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

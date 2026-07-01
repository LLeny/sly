use crate::apu::regs::Control;

mod regs;
pub mod spc700;
mod symbol;

pub use symbol::{is_symbol, symbol};

const IPL_ROM: &[u8] = include_bytes!("../assets/spc700.rom");

pub struct Apu {
    // Registers ($00F0-$00FF except $F8-$F9)
    unknown: u8,             // $F0
    control: Control,        // $F1 (W)
    dsp_addr: u8,            // $F2
    dsp_data: u8,            // $F3
    pub ports: [u8; 4],      // $F4-$F7
    timer_settings: [u8; 3], // $FA-$FC on write
    timer_counters: [u8; 3], // $FD-$FF on read

    // 64 KB RAM
    ram: [u8; 0x10000],

    // SPC-700 core registers
    pub spc700: spc700::Spc700,
}

impl Apu {
    pub fn new() -> Self {
        let reset_vector = u16::from_le_bytes([IPL_ROM[0x3E], IPL_ROM[0x3F]]);

        Self {
            unknown: 0,
            control: Control::default(),
            dsp_addr: 0,
            dsp_data: 0,
            ports: [0; 4],
            timer_settings: [0; 3],
            timer_counters: [0; 3],
            ram: [0; 0x10000],
            spc700: spc700::Spc700::new(reset_vector),
        }
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        self.peek(addr)
    }

    pub fn peek(&self, addr: u16) -> u8 {
        match addr {
            0x00F0..=0x00F7 | 0x00FA..=0x00FF => self.read_io(addr as u8),
            0xFFC0..=0xFFFF if self.control.ipl_rom_enable() => IPL_ROM[(addr - 0xFFC0) as usize],
            _ => self.ram[addr as usize],
        }
    }

    pub fn write(&mut self, addr: u16, value: u8) {
        match addr {
            0x00F0..=0x00F7 | 0x00FA..=0x00FF => self.write_io(addr as u8, value),
            _ => self.ram[addr as usize] = value,
        }
    }

    fn read_io(&self, reg: u8) -> u8 {
        match reg {
            0xF0 => self.unknown,
            0xF1 => self.control.raw(),
            0xF2 => self.dsp_addr,
            0xF3 => self.dsp_data,
            0xF4..=0xF7 => self.ports[(reg - 0xF4) as usize],
            0xFD..=0xFF => self.timer_counters[(reg - 0xFD) as usize],
            _ => {
                tracing::error!(
                    reg = format!("{:02X}", reg),
                    "APU read from write-only/unmapped I/O register"
                );
                0
            }
        }
    }

    fn write_io(&mut self, reg: u8, value: u8) {
        match reg {
            0xF0 => self.unknown = value,
            0xF1 => self.control = Control::from_raw(value),
            0xF2 => self.dsp_addr = value,
            0xF3 => self.dsp_data = value,
            0xF4..=0xF7 => self.ports[(reg - 0xF4) as usize] = value,
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

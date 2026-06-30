const IPL_ROM: &[u8] = include_bytes!("../assets/spc700.rom");

pub struct Apu {
    // Registers ($00F0-$00FC)
    unknown: u8,             // $F0
    control: u8,             // $F1 (W)
    dsp_addr: u8,            // $F2
    dsp_data: u8,            // $F3
    pub ports: [u8; 4],      // $F4-$F7
    aux: [u8; 2],            // $F8-$F9
    timer_settings: [u8; 3], // $FA-$FC on write
    timer_counters: [u8; 3], // $FA-$FC on read

    // Memory
    page0: [u8; 0xF0],  // $0000-$00EF
    page1: [u8; 0x100], // $0100-$01FF
}

impl Apu {
    pub fn new() -> Self {
        Self {
            unknown: 0,
            control: 0,
            dsp_addr: 0,
            dsp_data: 0,
            ports: [0; 4],
            aux: [0; 2],
            timer_settings: [0; 3],
            timer_counters: [0; 3],
            page0: [0; 0xF0],
            page1: [0; 0x100],
        }
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x00EF => self.page0[addr as usize],
            0x00F0..=0x00FF => self.read_io(addr as u8),
            0x0100..=0x01FF => self.page1[(addr - 0x0100) as usize],
            0xFFC0..=0xFFFF => IPL_ROM[(addr - 0xFFC0) as usize],
            _ => {
                tracing::error!(
                    addr = format!("{:04X}", addr),
                    "APU read from invalid address"
                );
                0
            }
        }
    }

    pub fn write(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x00EF => self.page0[addr as usize] = value,
            0x00F0..=0x00FF => self.write_io(addr as u8, value),
            0x0100..=0x01FF => self.page1[(addr - 0x0100) as usize] = value,
            0xFFC0..=0xFFFF => {
                tracing::warn!(
                    addr = format!("{:04X}", addr),
                    value = format!("{:02X}", value),
                    "APU write to IPL ROM"
                );
            }
            _ => {
                tracing::error!(
                    addr = format!("{:04X}", addr),
                    value = format!("{:02X}", value),
                    "APU write to invalid address"
                );
            }
        }
    }

    fn read_io(&self, reg: u8) -> u8 {
        match reg {
            0xF0 => self.unknown,
            0xF1 => self.control,
            0xF2 => self.dsp_addr,
            0xF3 => self.dsp_data,
            0xF4..=0xF7 => self.ports[(reg - 0xF4) as usize],
            0xF8..=0xF9 => self.aux[(reg - 0xF8) as usize],
            0xFA..=0xFC => self.timer_counters[(reg - 0xFA) as usize],
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
            0xF1 => self.control = value,
            0xF2 => self.dsp_addr = value,
            0xF3 => self.dsp_data = value,
            0xF4..=0xF7 => self.ports[(reg - 0xF4) as usize] = value,
            0xF8..=0xF9 => self.aux[(reg - 0xF8) as usize] = value,
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

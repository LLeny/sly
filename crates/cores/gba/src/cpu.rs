pub mod decoder;
pub mod interpreter;

use crate::bus::Bus;

pub struct Cpu {
    pub registers: [u32; 16],
    pub cpsr: Psr,
    pipeline: [Option<u32>; 3],
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            pipeline: [None; 3],
            registers: [0; 16],
            cpsr: Psr::default(),
        }
    }

    pub fn step(&mut self, bus: &mut Bus) {
        self.pipeline[0] = self.pipeline[1];
        self.pipeline[1] = self.pipeline[2];
        self.pipeline[2] = Some(self.fetch_instruction(bus));

        *self.pc_mut() += if self.is_thumb() { 2 } else { 4 };

        if let Some(instruction) = self.pipeline[0] {
            self.execute(bus, instruction);
        }
    }

    pub fn fetch_instruction(&self, bus: &mut Bus) -> u32 {
        if self.is_thumb() {
            u32::from(bus.fetch16(self.pc() & !0b1))
        } else {
            bus.fetch32(self.pc() & !0b11)
        }
    }

    pub fn pc(&self) -> u32 {
        self.registers[15]
    }

    pub fn pc_mut(&mut self) -> &mut u32 {
        &mut self.registers[15]
    }

    pub fn sp(&self) -> u32 {
        self.registers[13]
    }

    pub fn sp_mut(&mut self) -> &mut u32 {
        &mut self.registers[13]
    }

    pub fn lr(&self) -> u32 {
        self.registers[14]
    }

    pub fn lr_mut(&mut self) -> &mut u32 {
        &mut self.registers[14]
    }

    pub fn is_thumb(&self) -> bool {
        self.cpsr.t()
    }
}

#[chapa::bitenum]
pub enum Mode {
    #[fallback]
    User = 0b10000,
    Fiq = 0b10001,
    Irq = 0b10010,
    Supervisor = 0b10011,
    Abort = 0b10111,
    Undefined = 0b11011,
    System = 0b11111,
}

#[chapa::bitfield(u32, order = lsb0)]
pub struct Psr {
    #[bits(31, alias = "n")]
    pub negative: bool,
    #[bits(30, alias = "z")]
    pub zero: bool,
    #[bits(29, alias = "c")]
    pub carry: bool,
    #[bits(28, alias = "v")]
    pub overflow: bool,

    #[bits(7, alias = "i")]
    pub irq_disable: bool,
    #[bits(6, alias = "f")]
    pub fiq_disable: bool,
    #[bits(5, alias = "t")]
    pub state: bool,
    #[bits(4, alias = "m", default = Mode::Supervisor)]
    pub mode: Mode,
}

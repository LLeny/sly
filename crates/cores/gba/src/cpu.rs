pub mod decoder;
pub mod interpreter;
mod pipeline;

use crate::mmio::Mmio;
use pipeline::Pipeline;

pub struct Cpu {
    pub registers: [u32; 16],
    pub cpsr: Psr,
    pipeline: Pipeline,
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            pipeline: Pipeline::default(),
            registers: [0; 16],
            cpsr: Psr::default(),
        }
    }

    pub fn step(&mut self, mmio: &mut Mmio) {
        let instruction = self.fetch_instruction(mmio);
        if let Some((addr, instruction)) = self.pipeline.advance(self.pc(), instruction) {
            self.execute(mmio, addr, instruction);
        }

        if !self.pipeline.is_empty() {
            let increment = if self.is_thumb() { 2 } else { 4 };
            self.registers[15] = self.registers[15].wrapping_add(increment);
        }
    }

    pub(crate) fn flush_pipeline(&mut self) {
        self.pipeline.flush();
    }

    pub(crate) fn fetch_instruction(&self, mmio: &mut Mmio) -> u32 {
        if self.is_thumb() {
            u32::from(mmio.fetch16(self.pc() & !0b1))
        } else {
            mmio.fetch32(self.pc() & !0b11)
        }
    }

    pub(crate) fn pc(&self) -> u32 {
        self.registers[15]
    }

    pub(crate) fn set_pc(&mut self, value: u32) {
        self.flush_pipeline();
        self.registers[15] = value;
    }

    pub(crate) fn sp(&self) -> u32 {
        self.registers[13]
    }

    pub(crate) fn set_sp(&mut self, value: u32) {
        self.registers[13] = value;
    }

    pub(crate) fn lr(&self) -> u32 {
        self.registers[14]
    }

    pub(crate) fn set_lr(&mut self, value: u32) {
        self.registers[14] = value;
    }

    pub(crate) fn is_thumb(&self) -> bool {
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

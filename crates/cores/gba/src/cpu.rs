mod bus;
pub mod decoder;
pub mod interpreter;
mod multiply;
mod operand;
mod pipeline;
mod shift;

use std::ops::RangeInclusive;

pub use bus::Bus;
pub use pipeline::Pipeline;

pub struct Cpu {
    pub registers: [u32; 16],
    pub cpsr: Psr,
    pub spsr: [Psr; 5],
    pub banks: [[u32; 16]; 5],
    pub user_bank: [u32; 16],
    pub pipeline: Pipeline,
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            pipeline: Pipeline::default(),
            registers: [0; 16],
            cpsr: Psr::default(),
            spsr: [Psr::zeroed(); 5],
            banks: [[0; 16]; 5],
            user_bank: [0; 16],
        }
    }

    pub fn step(&mut self, mmio: &mut impl Bus) {
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

    pub(crate) fn fetch_instruction(&self, mmio: &mut impl Bus) -> u32 {
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
        self.registers[15] = value & if self.is_thumb() { !0b1 } else { !0b11 };
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

    pub(crate) fn set_register(&mut self, index: usize, value: u32) {
        if index == 15 {
            self.set_pc(value);
        } else {
            self.registers[index] = value;
        }
    }

    pub(crate) fn is_thumb(&self) -> bool {
        self.cpsr.t()
    }

    pub(crate) fn get_current_spsr(&self) -> Psr {
        self.cpsr
            .mode()
            .index()
            .map(|index| self.spsr[index])
            .expect("something is fucked")
    }

    pub(crate) fn branch_exchange(&mut self, target: u32) {
        self.cpsr.set_t(target & 1 != 0);
        self.set_pc(target);
    }

    pub(crate) fn enter_exception(&mut self, mode: Mode, vector: u32, return_address: u32) {
        let saved_cpsr = self.cpsr;
        self.switch_mode(mode);
        self.spsr[mode.index().expect("exception modes have an SPSR")] = saved_cpsr;
        self.set_lr(return_address);

        self.cpsr.set_mode(mode);
        self.cpsr.set_t(false);
        self.cpsr.set_i(true);
        self.set_pc(vector);
    }

    // SPSR_<current mode> -> CPSR
    pub(crate) fn restore_cpsr(&mut self) {
        let spsr = self.get_current_spsr();
        self.switch_mode(spsr.mode());
        self.cpsr = spsr;
    }

    fn switch_mode(&mut self, new: Mode) {
        let old = self.cpsr.mode();

        // registers -> banks[old], user_bank -> registers
        if let Some(index) = old.index()
            && let Some(range) = old.bank_register_range()
        {
            for i in range {
                self.banks[index][i] = self.registers[i];
                self.registers[i] = self.user_bank[i];
            }
        }

        // registers -> user_bank, banks[new] -> registers
        if let Some(index) = new.index()
            && let Some(range) = new.bank_register_range()
        {
            for i in range {
                self.user_bank[i] = self.registers[i];
                self.registers[i] = self.banks[index][i];
            }
        }
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

impl Mode {
    pub fn index(&self) -> Option<usize> {
        match self {
            Mode::Fiq => Some(0),
            Mode::Irq => Some(1),
            Mode::Supervisor => Some(2),
            Mode::Abort => Some(3),
            Mode::Undefined => Some(4),
            _ => None,
        }
    }

    pub fn bank_register_range(&self) -> Option<RangeInclusive<usize>> {
        match self {
            Mode::Fiq => Some(8..=14),
            Mode::Irq | Mode::Supervisor | Mode::Abort | Mode::Undefined => Some(13..=14),
            _ => None,
        }
    }

    pub fn is_banked(&self, index: usize) -> bool {
        self.bank_register_range()
            .is_some_and(|range| range.contains(&index))
    }
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
    #[bits(0..=4, alias = "m", default = Mode::Supervisor)]
    pub mode: Mode,
}

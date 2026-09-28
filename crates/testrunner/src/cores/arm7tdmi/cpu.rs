use gba::Cpu;
use gba::cpu::{Bus, Psr};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct State {
    pub registers: [u32; 16],
    pub banks: [[u32; 16]; 5],
    pub spsr: [u32; 5],
    pub cpsr: u32,
    pub pipeline: [u32; 2],
}

pub(super) trait TestCpu {
    fn load_test_state(&mut self, state: &State);
    fn step_test_instruction(&mut self, bus: &mut impl Bus);
    fn test_state(&self) -> State;
}

impl TestCpu for Cpu {
    fn load_test_state(&mut self, state: &State) {
        self.registers = state.registers;
        self.user_bank = state.registers;
        self.banks = state.banks;
        self.cpsr = Psr::from_raw(state.cpsr);
        self.spsr = state.spsr.map(Psr::from_raw);

        let mode = self.cpsr.mode();
        if let Some(index) = mode.index()
            && let Some(range) = mode.bank_register_range()
        {
            for reg in range {
                self.registers[reg] = self.banks[index][reg];
            }
        }

        self.pipeline.flush();
        let width = if self.cpsr.t() { 2 } else { 4 };
        let pc = self.registers[15];
        self.pipeline
            .advance(pc.wrapping_sub(2 * width), state.pipeline[0]);
        self.pipeline
            .advance(pc.wrapping_sub(width), state.pipeline[1]);
    }

    fn step_test_instruction(&mut self, bus: &mut impl Bus) {
        self.step(bus);
        if self.pipeline.is_empty() {
            self.step(bus);
            self.step(bus);
        }
    }

    fn test_state(&self) -> State {
        let mut state = State {
            registers: self.registers,
            banks: self.banks,
            spsr: self.spsr.map(|psr| psr.raw()),
            cpsr: self.cpsr.raw(),
            pipeline: self
                .pipeline
                .pending()
                .map(|stage| stage.expect("primed pipeline").1),
        };

        let mode = self.cpsr.mode();
        if let Some(index) = mode.index()
            && let Some(range) = mode.bank_register_range()
        {
            for reg in range {
                state.banks[index][reg] = self.registers[reg];
                state.registers[reg] = self.user_bank[reg];
            }
        }

        state
    }
}

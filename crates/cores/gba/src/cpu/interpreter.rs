use super::Cpu;
use super::decoder::{arm, thumb};
use crate::mmio::Mmio;

impl Cpu {
    pub(super) fn execute(&mut self, mmio: &mut Mmio, addr: u32, instruction: u32) {
        let mut interpreter = Interpreter { cpu: self, mmio };
        interpreter.execute(addr, instruction);
    }
}

pub(super) struct Interpreter<'a> {
    cpu: &'a mut Cpu,
    mmio: &'a mut Mmio,
}

impl<'a> Interpreter<'a> {
    fn execute(&mut self, addr: u32, instruction: u32) {
        if self.cpu.is_thumb() {
            let instr = thumb::Instruction(instruction as u16);

            #[cfg(feature = "disasm")]
            println!("[{:04X}] {}", addr, instr);

            thumb::dispatch_ops(self, instr);
        } else {
            let instr = arm::Instruction(instruction);

            #[cfg(feature = "disasm")]
            println!("[{:04X}] {}", addr, instr);

            if self.check_condition(instr.cond()) {
                arm::dispatch_ops(self, instr);
            }
        }
    }

    fn check_condition(&self, cond: u8) -> bool {
        let psr = self.cpu.cpsr;
        let passed = match cond >> 1 {
            0 => psr.z(),                        // EQ / NE
            1 => psr.c(),                        // CS / CC
            2 => psr.n(),                        // MI / PL
            3 => psr.v(),                        // VS / VC
            4 => psr.c() && !psr.z(),            // HI / LS
            5 => psr.n() == psr.v(),             // GE / LT
            6 => !psr.z() && psr.n() == psr.v(), // GT / LE
            7 => true,                           // AL / NV
            _ => return false,
        };
        passed ^ (cond & 1 != 0)
    }
}

impl arm::Ops for Interpreter<'_> {
    fn data_processing(&mut self, _op: arm::DataProcessingKind, _inst: arm::Instruction) {
        todo!()
    }

    fn psr_transfer(&mut self, _op: arm::PsrTransferKind, _inst: arm::Instruction) {
        todo!()
    }

    fn multiply_long(&mut self, _op: arm::MultiplyLongKind, _inst: arm::Instruction) {
        todo!()
    }

    fn single_transfer(&mut self, _op: arm::SingleTransferKind, _inst: arm::Instruction) {
        todo!()
    }

    fn halfword_transfer(&mut self, _op: arm::HalfwordTransferKind, _inst: arm::Instruction) {
        todo!()
    }

    fn block_transfer(&mut self, _op: arm::BlockTransferKind, _inst: arm::Instruction) {
        todo!()
    }

    fn b(&mut self, inst: arm::Instruction) {
        if !self.check_condition(inst.cond()) {
            return;
        }

        if inst.link() {
            self.cpu.set_lr(self.cpu.pc().wrapping_sub(4));
        }

        let dst = self.cpu.pc().wrapping_add_signed(inst.offset() as i32);
        self.cpu.set_pc(dst);
    }

    fn bx(&mut self, _inst: arm::Instruction) {
        todo!()
    }

    fn multiply(&mut self, _inst: arm::Instruction) {
        todo!()
    }

    fn swi(&mut self, _inst: arm::Instruction) {
        todo!()
    }

    fn swp(&mut self, _inst: arm::Instruction) {
        todo!()
    }

    fn on_invalid(&mut self, _inst: arm::Instruction) {
        todo!()
    }
}

impl thumb::Ops for Interpreter<'_> {
    fn shift_immediate(&mut self, _op: thumb::ShiftImmediateKind, _inst: thumb::Instruction) {
        todo!()
    }

    fn add_subtract(&mut self, _op: thumb::AddSubtractKind, _inst: thumb::Instruction) {
        todo!()
    }

    fn immediate(&mut self, _op: thumb::ImmediateKind, _inst: thumb::Instruction) {
        todo!()
    }

    fn alu(&mut self, _op: thumb::AluKind, _inst: thumb::Instruction) {
        todo!()
    }

    fn high_register(&mut self, _op: thumb::HighRegisterKind, _inst: thumb::Instruction) {
        todo!()
    }

    fn register_transfer(&mut self, _op: thumb::RegisterTransferKind, _inst: thumb::Instruction) {
        todo!()
    }

    fn immediate_transfer(&mut self, _op: thumb::ImmediateTransferKind, _inst: thumb::Instruction) {
        todo!()
    }

    fn address(&mut self, _op: thumb::AddressKind, _inst: thumb::Instruction) {
        todo!()
    }

    fn multiple_transfer(&mut self, _op: thumb::MultipleTransferKind, _inst: thumb::Instruction) {
        todo!()
    }

    fn branch(&mut self, _op: thumb::BranchKind, _inst: thumb::Instruction) {
        todo!()
    }

    fn swi(&mut self, _inst: thumb::Instruction) {
        todo!()
    }

    fn on_invalid(&mut self, _inst: thumb::Instruction) {
        todo!()
    }
}

use super::Cpu;
use super::decoder::{arm, thumb};
use crate::mmio::Mmio;

impl Cpu {
    pub(super) fn execute(&mut self, mmio: &mut Mmio, instruction: u32) {
        let mut interpreter = Interpreter { cpu: self, mmio };
        interpreter.execute(instruction);
    }
}

pub(super) struct Interpreter<'a> {
    cpu: &'a mut Cpu,
    mmio: &'a mut Mmio,
}

impl Interpreter<'_> {
    fn execute(&mut self, instruction: u32) {
        if self.cpu.is_thumb() {
            let instr = thumb::Instruction(instruction as u16);
            println!("{}", instr);
            thumb::dispatch_ops(self, instr);
        } else {
            let instr = arm::Instruction(instruction);
            println!("{}", instr);
            arm::dispatch_ops(self, instr);
        }
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

    fn b(&mut self, _inst: arm::Instruction) {
        todo!()
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

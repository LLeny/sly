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

    fn resolve_operand(&self, inst: arm::Instruction) -> u32 {
        match inst.form() {
            arm::Form::Imm => inst.imm(),
            arm::Form::RegImm => {
                let value = self.cpu.registers[inst.rm() as usize];
                let amount = u32::from(inst.amount());
                if inst.shift() == 3 && amount == 0 {
                    // RRX: shift the old carry into bit 31.
                    (u32::from(self.cpu.cpsr.c()) << 31) | (value >> 1)
                } else {
                    Self::shift_operand(value, inst.shift(), amount)
                }
            }
            arm::Form::RegReg => {
                let mut value = self.cpu.registers[inst.rm() as usize];
                if inst.rm() == 15 {
                    value = value.wrapping_add(4); // PC + 12 for register shifts.
                }
                let amount = self.cpu.registers[inst.rs() as usize] & 0xff;
                Self::shift_operand(value, inst.shift(), amount)
            }
            _ => unreachable!(),
        }
    }

    fn shift_operand(value: u32, shift: u8, amount: u32) -> u32 {
        match shift {
            0 => value.checked_shl(amount).unwrap_or(0),
            1 => value.checked_shr(amount).unwrap_or(0),
            2 => ((value as i32) >> amount.min(31)) as u32,
            3 => value.rotate_right(amount),
            _ => unreachable!("decoded shift kind is two bits"),
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
    fn data_processing(&mut self, op: arm::DataProcessingKind, inst: arm::Instruction) {
        use arm::Mnemonic::*;

        let op1 = self.cpu.registers[inst.rn() as usize];
        let op2 = self.resolve_operand(inst);

        let result = match inst.mnemonic() {
            And | Tst => op1 & op2,
            Eor | Teq => op1 ^ op2,
            Sub | Cmp => op1.wrapping_sub(op2),
            Add | Cmn => op1.wrapping_add(op2),
            Rsb => op2.wrapping_sub(op1),
            Adc => op1
                .wrapping_add(op2)
                .wrapping_add(if self.cpu.cpsr.c() { 1 } else { 0 }),
            Sbc => op1
                .wrapping_sub(op2)
                .wrapping_sub(if self.cpu.cpsr.c() { 0 } else { 1 }),
            Rsc => op2
                .wrapping_sub(op1)
                .wrapping_sub(if self.cpu.cpsr.c() { 0 } else { 1 }),
            Orr => op1 | op2,
            Mov => op2,
            Bic => op1 & !op2,
            Mvn => !op2,
            _ => unreachable!(),
        };

        // These should always have the S bit set.
        debug_assert!(!matches!(inst.mnemonic(), Tst | Teq | Cmp | Cmn) || inst.s());

        if inst.s() {
            if inst.rd() == 15 {
                self.cpu.restore_cpsr();
            } else {
                // Flags are only updated if we don't update PC.
                self.cpu.cpsr.set_n(result >> 31 != 0);
                self.cpu.cpsr.set_z(result == 0);
                self.cpu.cpsr.set_c(result >> 31 != 0);
                self.cpu.cpsr.set_v(result >> 31 != 0);
            }
        }

        self.cpu.set_register(inst.rd() as usize, result);
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

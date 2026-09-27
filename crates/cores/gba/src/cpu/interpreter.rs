use super::Cpu;
use super::decoder::{arm, thumb};
use super::shift::Shift;
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

    fn operand_register(&self, index: u8, form: arm::Form) -> u32 {
        match (index, form) {
            // Stored PC is already instruction address + 8 but register shifts read + 12.
            (15, arm::Form::RegReg) => self.cpu.pc().wrapping_add(4),
            _ => self.cpu.registers[index as usize],
        }
    }

    fn resolve_operand(&self, inst: arm::Instruction) -> (u32, bool) {
        let (value, shift) = match inst.form() {
            arm::Form::Imm => (
                u32::from(inst.imm8()),
                Shift::Ror(u32::from(inst.rotate()) * 2),
            ),
            arm::Form::RegImm => {
                let shift = match Shift::decode(inst.shift(), u32::from(inst.amount())) {
                    Shift::Ror(0) => Shift::Rrx,
                    shift => shift,
                };
                (self.cpu.registers[inst.rm() as usize], shift)
            }
            arm::Form::RegReg => {
                let value = self.operand_register(inst.rm(), arm::Form::RegReg);
                let amount = self.cpu.registers[inst.rs() as usize] & 0xff;
                (value, Shift::decode(inst.shift(), amount))
            }
            _ => unreachable!(),
        };
        shift.apply(value, self.cpu.cpsr.c())
    }

    fn add_with_carry(a: u32, b: u32, carry: bool) -> (u32, bool, bool) {
        let sum = a as u64 + b as u64 + carry as u64;
        let result = sum as u32;
        let overflow = (!(a ^ b) & (a ^ result)) >> 31 != 0;
        (result, sum >> 32 != 0, overflow)
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

        let c_in = self.cpu.cpsr.c();
        let v_in = self.cpu.cpsr.v();
        let op1 = self.operand_register(inst.rn(), inst.form());
        let (op2, shifter_carry) = self.resolve_operand(inst);

        let mnemonic = inst.mnemonic();
        let (result, carry, overflow) = match mnemonic {
            And | Tst => (op1 & op2, shifter_carry, v_in),
            Eor | Teq => (op1 ^ op2, shifter_carry, v_in),
            Sub | Cmp => Self::add_with_carry(op1, !op2, true),
            Add | Cmn => Self::add_with_carry(op1, op2, false),
            Rsb => Self::add_with_carry(op2, !op1, true),
            Adc => Self::add_with_carry(op1, op2, c_in),
            Sbc => Self::add_with_carry(op1, !op2, c_in),
            Rsc => Self::add_with_carry(op2, !op1, c_in),
            Orr => (op1 | op2, shifter_carry, v_in),
            Mov => (op2, shifter_carry, v_in),
            Bic => (op1 & !op2, shifter_carry, v_in),
            Mvn => (!op2, shifter_carry, v_in),
            _ => unreachable!(),
        };

        if inst.s() {
            if inst.rd() == 15 {
                self.cpu.restore_cpsr();
            } else {
                // Flags are only updated if we don't update PC.
                self.cpu.cpsr.set_n(result >> 31 != 0);
                self.cpu.cpsr.set_z(result == 0);
                self.cpu.cpsr.set_c(carry);
                self.cpu.cpsr.set_v(overflow);
            }
        }

        let is_test_instr = matches!(mnemonic, Tst | Teq | Cmp | Cmn);
        debug_assert!(!is_test_instr || inst.s());

        if !is_test_instr {
            self.cpu.set_register(inst.rd() as usize, result);
        }
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

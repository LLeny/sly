use super::decoder::{arm, thumb};
use super::shift::Shift;
use super::{Cpu, Mode};
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
            println!("[{:08X}][T] {}", addr, instr);

            thumb::dispatch_ops(self, instr);
        } else {
            let instr = arm::Instruction(instruction);

            #[cfg(feature = "disasm")]
            println!("[{:08X}][A] {}", addr, instr);

            if self.check_condition(instr.cond()) {
                arm::dispatch_ops(self, instr);
            }
        }
    }

    fn load_word_rotated(&mut self, addr: u32) -> u32 {
        self.mmio
            .read32(addr & !0b11)
            .rotate_right((addr & 0b11) * 8)
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
    fn data_processing(&mut self, _op: arm::DataProcessingKind, inst: arm::Instruction) {
        use arm::Mnemonic::*;

        let c_in = self.cpu.cpsr.c();
        let v_in = self.cpu.cpsr.v();
        let op1 = self.cpu.operand_register(inst.rn(), inst.form());
        let (op2, shifter_carry) = self.cpu.read_arm_operand(inst);

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

    fn psr_transfer(&mut self, op: arm::PsrTransferKind, inst: arm::Instruction) {
        match op {
            arm::PsrTransferKind::Mrs => {
                let psr = if inst.psr() == 0 {
                    self.cpu.cpsr
                } else {
                    self.cpu.get_current_spsr()
                };
                self.cpu.set_register(inst.rd() as usize, psr.raw());
            }
            arm::PsrTransferKind::MsrPsrReg | arm::PsrTransferKind::MsrPsrImm => {
                let (value, _) = self.cpu.read_arm_operand(inst);
                let saved = inst.psr() != 0;
                let mut psr = if saved {
                    self.cpu.get_current_spsr()
                } else {
                    self.cpu.cpsr
                };

                if inst.flags() {
                    psr = chapa::insert_bits!(psr; 28..=31; value);
                }
                if inst.control() && (saved || !matches!(self.cpu.cpsr.mode(), Mode::User)) {
                    psr = if saved {
                        chapa::insert_bits!(psr; 0..=7; value)
                    } else {
                        chapa::insert_bits!(psr; 0..=4, 6..=7; value)
                    };
                }

                if saved {
                    let mode = self.cpu.cpsr.mode();
                    let index = mode
                        .index()
                        .expect("SPSR access requires an exception mode");
                    self.cpu.spsr[index] = psr;
                } else {
                    self.cpu.switch_mode(psr.mode());
                    self.cpu.cpsr = psr;
                }
            }
        }
    }

    fn multiply_long(&mut self, _op: arm::MultiplyLongKind, _inst: arm::Instruction) {
        todo!()
    }

    fn single_transfer(&mut self, _op: arm::SingleTransferKind, inst: arm::Instruction) {
        let rd = inst.rd() as usize;
        let rn = inst.rn() as usize;
        let base = self.cpu.registers[rn];
        let (offset, _) = self.cpu.read_arm_operand(inst);

        let updated_base = if inst.up() {
            base.wrapping_add(offset)
        } else {
            base.wrapping_sub(offset)
        };
        let address = if inst.pre() { updated_base } else { base };
        let value = if rd == 15 {
            self.cpu.pc().wrapping_add(4)
        } else {
            self.cpu.registers[rd]
        };

        let loaded = match inst.mnemonic() {
            arm::Mnemonic::Ldr if inst.byte() => Some(self.mmio.read8(address) as u32),
            arm::Mnemonic::Ldr if !inst.byte() => Some(self.load_word_rotated(address)),
            arm::Mnemonic::Str if inst.byte() => {
                self.mmio.write8(address, value as u8);
                None
            }
            arm::Mnemonic::Str if !inst.byte() => {
                self.mmio.write32(address & !0b11, value);
                None
            }
            _ => unreachable!(),
        };

        if !inst.pre() || inst.write() {
            self.cpu.set_register(rn, updated_base);
        }
        if let Some(value) = loaded {
            self.cpu.set_register(rd, value);
        }
    }

    fn halfword_transfer(&mut self, _op: arm::HalfwordTransferKind, inst: arm::Instruction) {
        let rd = inst.rd() as usize;
        let rn = inst.rn() as usize;
        let base = self.cpu.registers[rn];
        let (offset, _) = self.cpu.read_arm_operand(inst);

        let updated_base = if inst.up() {
            base.wrapping_add(offset)
        } else {
            base.wrapping_sub(offset)
        };
        let address = if inst.pre() { updated_base } else { base };

        let loaded = match inst.mnemonic() {
            arm::Mnemonic::Ldrh => {
                Some((self.mmio.read16(address & !0b1) as u32).rotate_right((address & 0b1) * 8))
            }
            arm::Mnemonic::Ldrsh if address & 0b1 != 0 => {
                Some(self.mmio.read8(address) as i8 as u32)
            }
            arm::Mnemonic::Ldrsh => Some(self.mmio.read16(address & !0b1) as i16 as u32),
            arm::Mnemonic::Ldrsb => Some(self.mmio.read8(address) as i8 as u32),
            arm::Mnemonic::Strh => {
                let value = if rd == 15 {
                    self.cpu.pc().wrapping_add(4)
                } else {
                    self.cpu.registers[rd]
                };
                self.mmio.write16(address & !0b1, value as u16);
                None
            }
            _ => unreachable!(),
        };

        if !inst.pre() || inst.write() {
            self.cpu.set_register(rn, updated_base);
        }
        if let Some(value) = loaded {
            self.cpu.set_register(rd, value);
        }
    }

    fn block_transfer(&mut self, _op: arm::BlockTransferKind, _inst: arm::Instruction) {
        todo!()
    }

    fn b(&mut self, inst: arm::Instruction) {
        if inst.link() {
            self.cpu.set_lr(self.cpu.pc().wrapping_sub(4));
        }

        let dst = self.cpu.pc().wrapping_add_signed(inst.offset() as i32);
        self.cpu.set_pc(dst);
    }

    fn bx(&mut self, inst: arm::Instruction) {
        let target = self.cpu.registers[inst.rm() as usize];
        self.cpu.cpsr.set_t(target & 1 != 0);
        self.cpu.set_pc(target);
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
    fn shift_immediate(&mut self, op: thumb::ShiftImmediateKind, inst: thumb::Instruction) {
        let rd = inst.rd() as usize;
        let rs = inst.rs() as usize;
        let offset = inst.shift_imm() as u32;

        let shift = match op {
            thumb::ShiftImmediateKind::LslShiftImm => Shift::Lsl(offset),
            thumb::ShiftImmediateKind::LsrShiftImm => {
                Shift::Lsr(if offset == 0 { 32 } else { offset })
            }
            thumb::ShiftImmediateKind::AsrShiftImm => {
                Shift::Asr(if offset == 0 { 32 } else { offset })
            }
        };

        let (result, carry) = shift.apply(self.cpu.registers[rs], self.cpu.cpsr.c());
        self.cpu.set_register(rd, result);

        self.cpu.cpsr.set_n(result >> 31 != 0);
        self.cpu.cpsr.set_z(result == 0);
        self.cpu.cpsr.set_c(carry);
    }

    fn add_subtract(&mut self, op: thumb::AddSubtractKind, inst: thumb::Instruction) {
        let rd = inst.rd() as usize;
        let rs = self.cpu.registers[inst.rs() as usize];
        let rn = inst.rn() as usize;

        let (result, carry, overflow) = match op {
            thumb::AddSubtractKind::AddReg3 => {
                Self::add_with_carry(rs, self.cpu.registers[rn], false)
            }
            thumb::AddSubtractKind::AddImm3 => Self::add_with_carry(rs, inst.imm3() as u32, false),
            thumb::AddSubtractKind::SubReg3 => {
                Self::add_with_carry(rs, !self.cpu.registers[rn], true)
            }
            thumb::AddSubtractKind::SubImm3 => {
                Self::add_with_carry(rs, !(inst.imm3() as u32), true)
            }
        };
        self.cpu.set_register(rd, result);

        self.cpu.cpsr.set_n(result >> 31 != 0);
        self.cpu.cpsr.set_z(result == 0);
        self.cpu.cpsr.set_c(carry);
        self.cpu.cpsr.set_v(overflow);
    }

    fn immediate(&mut self, op: thumb::ImmediateKind, inst: thumb::Instruction) {
        match op {
            thumb::ImmediateKind::MovImm8 => {
                let result = inst.imm8() as u32;
                self.cpu.set_register(inst.dest8() as usize, result);
                self.cpu.cpsr.set_n(false);
                self.cpu.cpsr.set_z(result == 0);
            }
            thumb::ImmediateKind::AddImm8 => {
                let value = self.cpu.registers[inst.dest8() as usize];
                let (result, carry, overflow) =
                    Self::add_with_carry(value, inst.imm8() as u32, false);
                self.cpu.set_register(inst.dest8() as usize, result);
                self.cpu.cpsr.set_n(result >> 31 != 0);
                self.cpu.cpsr.set_z(result == 0);
                self.cpu.cpsr.set_c(carry);
                self.cpu.cpsr.set_v(overflow);
            }
            thumb::ImmediateKind::SubImm8 => {
                let value = self.cpu.registers[inst.dest8() as usize];
                let (result, carry, overflow) =
                    Self::add_with_carry(value, !(inst.imm8() as u32), true);
                self.cpu.set_register(inst.dest8() as usize, result);
                self.cpu.cpsr.set_n(result >> 31 != 0);
                self.cpu.cpsr.set_z(result == 0);
                self.cpu.cpsr.set_c(carry);
                self.cpu.cpsr.set_v(overflow);
            }
            thumb::ImmediateKind::CmpImm8 => {
                let value = self.cpu.registers[inst.dest8() as usize];
                let (result, carry, overflow) =
                    Self::add_with_carry(value, !(inst.imm8() as u32), true);
                self.cpu.cpsr.set_n(result >> 31 != 0);
                self.cpu.cpsr.set_z(result == 0);
                self.cpu.cpsr.set_c(carry);
                self.cpu.cpsr.set_v(overflow);
            }
        }
    }

    fn alu(&mut self, op: thumb::AluKind, inst: thumb::Instruction) {
        use thumb::AluKind::*;

        let rd = inst.rd() as usize;
        let lhs = self.cpu.registers[rd];
        let rs = self.cpu.registers[inst.rs() as usize];
        let c_in = self.cpu.cpsr.c();
        let v_in = self.cpu.cpsr.v();

        let (result, carry, overflow) = match op {
            AndReg | TstReg => (lhs & rs, c_in, v_in),
            EorReg => (lhs ^ rs, c_in, v_in),
            LslReg | LsrReg | AsrReg | RorReg => {
                let amount = rs & 0xFF;
                let shift = match op {
                    LslReg => Shift::Lsl(amount),
                    LsrReg => Shift::Lsr(amount),
                    AsrReg => Shift::Asr(amount),
                    RorReg => Shift::Ror(amount),
                    _ => unreachable!(),
                };
                let (result, carry) = shift.apply(lhs, c_in);
                (result, carry, v_in)
            }
            AdcReg => Self::add_with_carry(lhs, rs, c_in),
            SbcReg => Self::add_with_carry(lhs, !rs, c_in),
            NegReg => Self::add_with_carry(0, !rs, true),
            CmpReg => Self::add_with_carry(lhs, !rs, true),
            CmnReg => Self::add_with_carry(lhs, rs, false),
            OrrReg => (lhs | rs, c_in, v_in),
            MulReg => (lhs.wrapping_mul(rs), false, v_in),
            BicReg => (lhs & !rs, c_in, v_in),
            MvnReg => (!rs, c_in, v_in),
        };

        self.cpu.cpsr.set_n(result >> 31 != 0);
        self.cpu.cpsr.set_z(result == 0);
        self.cpu.cpsr.set_c(carry);
        self.cpu.cpsr.set_v(overflow);

        if !matches!(op, TstReg | CmpReg | CmnReg) {
            self.cpu.set_register(rd, result);
        }
    }

    fn high_register(&mut self, op: thumb::HighRegisterKind, inst: thumb::Instruction) {
        let rd = inst.high_dest() as usize;
        let rs = self.cpu.registers[inst.high_src() as usize];

        match op {
            thumb::HighRegisterKind::AddHigh => {
                let result = self.cpu.registers[rd].wrapping_add(rs);
                self.cpu.set_register(rd, result);
            }
            thumb::HighRegisterKind::MovHigh => {
                self.cpu.set_register(rd, rs);
            }
            thumb::HighRegisterKind::CmpHigh => {
                let (result, carry, overflow) =
                    Self::add_with_carry(self.cpu.registers[rd], !rs, true);
                self.cpu.cpsr.set_n(result >> 31 != 0);
                self.cpu.cpsr.set_z(result == 0);
                self.cpu.cpsr.set_c(carry);
                self.cpu.cpsr.set_v(overflow);
            }
        }
    }

    fn register_transfer(&mut self, _op: thumb::RegisterTransferKind, _inst: thumb::Instruction) {
        todo!()
    }

    fn immediate_transfer(&mut self, op: thumb::ImmediateTransferKind, inst: thumb::Instruction) {
        match op {
            thumb::ImmediateTransferKind::LdrWordImm => {
                let addr = self.cpu.registers[inst.rb() as usize].wrapping_add(inst.word_offset());
                let value = self.load_word_rotated(addr);
                self.cpu.set_register(inst.rd() as usize, value);
            }
            thumb::ImmediateTransferKind::StrWordImm => {
                let addr = self.cpu.registers[inst.rb() as usize].wrapping_add(inst.word_offset());
                let value = self.cpu.registers[inst.rd() as usize];
                self.mmio.write32(addr & !0b11, value);
            }
            thumb::ImmediateTransferKind::LdrLiteral => {
                let addr = (self.cpu.pc() & !0b11).wrapping_add(inst.scaled_offset() as u32);
                let value = self.mmio.read32(addr);
                self.cpu.set_register(inst.dest8() as usize, value);
            }
            _ => todo!(),
        }
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

use super::decoder::{arm, registers, thumb};
use super::shift::Shift;
use super::{Bus, Cpu, Mode, multiply};

impl Cpu {
    pub(super) fn execute(&mut self, mmio: &mut impl Bus, addr: u32, instruction: u32) {
        let mut interpreter = Interpreter { cpu: self, mmio };
        interpreter.execute(addr, instruction);
    }
}

pub(super) struct Interpreter<'a, B> {
    cpu: &'a mut Cpu,
    mmio: &'a mut B,
}

impl<B: Bus> Interpreter<'_, B> {
    fn execute(&mut self, _addr: u32, instruction: u32) {
        if self.cpu.is_thumb() {
            let instr = thumb::Instruction(instruction as u16);
            thumb::dispatch_ops(self, instr);
        } else {
            let instr = arm::Instruction(instruction);
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

    fn load_halfword_rotated(&mut self, addr: u32) -> u32 {
        (self.mmio.read16(addr & !0b1) as u32).rotate_right((addr & 0b1) * 8)
    }

    fn load_signed_halfword(&mut self, addr: u32) -> u32 {
        if addr & 0b1 != 0 {
            self.mmio.read8(addr) as i8 as u32
        } else {
            self.mmio.read16(addr) as i16 as u32
        }
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

impl<B: Bus> arm::Ops for Interpreter<'_, B> {
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

        let is_test_instr = matches!(mnemonic, Tst | Teq | Cmp | Cmn);
        debug_assert!(!is_test_instr || inst.s());

        if inst.s() {
            if inst.rd() == 15 && self.cpu.cpsr.mode().index().is_some() {
                self.cpu.restore_cpsr();
            } else {
                // User and System modes have no SPSR to restore.
                self.cpu.cpsr.set_n(result >> 31 != 0);
                self.cpu.cpsr.set_z(result == 0);
                self.cpu.cpsr.set_c(carry);
                self.cpu.cpsr.set_v(overflow);
            }
        }

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

    fn multiply_long(&mut self, op: arm::MultiplyLongKind, inst: arm::Instruction) {
        use arm::MultiplyLongKind::*;

        let rm = self.cpu.registers[inst.rm() as usize];
        let rs = self.cpu.registers[inst.rs() as usize];
        let signed = matches!(op, Smull | Smlal);
        let accumulator = match op {
            Umlal | Smlal => {
                ((self.cpu.registers[inst.hi() as usize] as u64) << 32)
                    | self.cpu.registers[inst.lo() as usize] as u64
            }
            Umull | Smull => 0,
        };
        let product = match op {
            Umull | Umlal => (rm as u64) * (rs as u64),
            Smull | Smlal => ((rm as i32 as i64) * (rs as i32 as i64)) as u64,
        };
        let result = product.wrapping_add(accumulator);

        self.cpu.set_register(inst.lo() as usize, result as u32);
        self.cpu
            .set_register(inst.hi() as usize, (result >> 32) as u32);
        if inst.s() {
            self.cpu.cpsr.set_n(result >> 63 != 0);
            self.cpu.cpsr.set_z(result == 0);
            self.cpu
                .cpsr
                .set_c(multiply::long_carry(rm, rs, accumulator, signed));
        }
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
            arm::Mnemonic::Ldrh => Some(self.load_halfword_rotated(address)),
            arm::Mnemonic::Ldrsh => Some(self.load_signed_halfword(address)),
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

    fn block_transfer(&mut self, op: arm::BlockTransferKind, inst: arm::Instruction) {
        // Empty lists transfer PC but write back as tho 16 registers moved.
        let (list, size) = match inst.list() {
            0 => (1 << 15, 0x40),
            list => (list, list.count_ones() * 4),
        };
        let rn = inst.rn() as usize;
        let base = self.cpu.registers[rn];
        let updated_base = if inst.up() {
            base.wrapping_add(size)
        } else {
            base.wrapping_sub(size)
        };
        let start = match (inst.up(), inst.pre()) {
            (true, false) => base,
            (true, true) => base.wrapping_add(4),
            (false, false) => updated_base.wrapping_add(4),
            (false, true) => updated_base,
        };
        let restore_psr = inst.user() && op == arm::BlockTransferKind::Ldm && list & (1 << 15) != 0;
        let user_transfer = inst.user() && !restore_psr;
        let mode = self.cpu.cpsr.mode();
        let user_banked = |reg| user_transfer && mode.is_banked(reg);

        match op {
            arm::BlockTransferKind::Stm => {
                for (index, reg) in registers(list).enumerate() {
                    let addr = start.wrapping_add(index as u32 * 4);
                    let value = match reg {
                        15 => self.cpu.pc().wrapping_add(4),
                        _ if user_banked(reg) => self.cpu.user_bank[reg],
                        _ if reg == rn && inst.write() && index != 0 => updated_base,
                        _ => self.cpu.registers[reg],
                    };
                    self.mmio.write32(addr & !0b11, value);
                }
                if inst.write() {
                    self.cpu.set_register(rn, updated_base);
                }
            }
            arm::BlockTransferKind::Ldm => {
                let mut pc = None;
                for (index, reg) in registers(list).enumerate() {
                    let addr = start.wrapping_add(index as u32 * 4);
                    let value = self.mmio.read32(addr & !0b11);
                    match reg {
                        15 => pc = Some(value),
                        _ if user_banked(reg) => self.cpu.user_bank[reg] = value,
                        _ => self.cpu.registers[reg] = value,
                    }
                }

                if inst.write() && list & (1 << rn) == 0 {
                    self.cpu.set_register(rn, updated_base);
                }

                // Finish writes to the current bank before restoring the return state.
                if restore_psr {
                    self.cpu.restore_cpsr();
                }

                if let Some(pc) = pc {
                    self.cpu.set_pc(pc);
                }
            }
        }
    }

    fn b(&mut self, inst: arm::Instruction) {
        if inst.link() {
            self.cpu.set_lr(self.cpu.pc().wrapping_sub(4));
        }

        let dst = self.cpu.pc().wrapping_add_signed(inst.offset() as i32);
        self.cpu.set_pc(dst);
    }

    fn bx(&mut self, inst: arm::Instruction) {
        self.cpu
            .branch_exchange(self.cpu.registers[inst.rm() as usize]);
    }

    fn multiply(&mut self, inst: arm::Instruction) {
        let rm = self.cpu.registers[inst.rm() as usize];
        let rs = self.cpu.registers[inst.rs() as usize];
        let accumulator = if inst.a() {
            self.cpu.registers[inst.acc() as usize]
        } else {
            0
        };
        let result = rm.wrapping_mul(rs).wrapping_add(accumulator);

        self.cpu.set_register(inst.dest() as usize, result);
        if inst.s() {
            self.cpu.cpsr.set_n(result >> 31 != 0);
            self.cpu.cpsr.set_z(result == 0);
            let carry = if inst.a() {
                multiply::accumulate_carry(rm, rs, accumulator)
            } else {
                multiply::carry(rm, rs)
            };
            self.cpu.cpsr.set_c(carry);
        }
    }

    fn swi(&mut self, _inst: arm::Instruction) {
        let return_address = self.cpu.pc().wrapping_sub(4);
        self.cpu
            .enter_exception(Mode::Supervisor, 0x08, return_address);
    }

    fn swp(&mut self, inst: arm::Instruction) {
        let rm = self.cpu.registers[inst.rm() as usize];
        let rn = self.cpu.registers[inst.rn() as usize];

        if inst.byte() {
            self.cpu.registers[inst.rd() as usize] = self.mmio.read8(rn) as u32;
            self.mmio.write8(rn, rm as u8);
        } else {
            self.cpu.registers[inst.rd() as usize] = self.load_word_rotated(rn);
            self.mmio.write32(rn & !0b11, rm);
        }
    }

    fn on_invalid(&mut self, _inst: arm::Instruction) {
        let return_address = self.cpu.pc().wrapping_sub(4);
        self.cpu
            .enter_exception(Mode::Undefined, 0x04, return_address);
    }
}

impl<B: Bus> thumb::Ops for Interpreter<'_, B> {
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
            MulReg => (lhs.wrapping_mul(rs), multiply::carry(rs, lhs), v_in),
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

    fn register_transfer(&mut self, op: thumb::RegisterTransferKind, inst: thumb::Instruction) {
        use thumb::RegisterTransferKind::*;

        let rd = inst.rd() as usize;
        let addr = self.cpu.registers[inst.rb() as usize]
            .wrapping_add(self.cpu.registers[inst.ro() as usize]);
        let value = self.cpu.registers[rd];

        let loaded = match op {
            StrRegOffset => {
                self.mmio.write32(addr & !0b11, value);
                return;
            }
            StrbRegOffset => {
                self.mmio.write8(addr, value as u8);
                return;
            }
            StrhRegOffset => {
                self.mmio.write16(addr & !0b1, value as u16);
                return;
            }
            LdrRegOffset => self.load_word_rotated(addr),
            LdrbRegOffset => self.mmio.read8(addr) as u32,
            LdrsbRegOffset => self.mmio.read8(addr) as i8 as u32,
            LdrhRegOffset => self.load_halfword_rotated(addr),
            LdrshRegOffset => self.load_signed_halfword(addr),
        };
        self.cpu.set_register(rd, loaded);
    }

    fn immediate_transfer(&mut self, op: thumb::ImmediateTransferKind, inst: thumb::Instruction) {
        use thumb::ImmediateTransferKind::*;

        let (rd, base, offset) = match op {
            LdrWordImm | StrWordImm => (
                inst.rd(),
                self.cpu.registers[inst.rb() as usize],
                inst.word_offset(),
            ),
            LdrbByteImm | StrbByteImm => (
                inst.rd(),
                self.cpu.registers[inst.rb() as usize],
                inst.off5() as u32,
            ),
            LdrhHalfImm | StrhHalfImm => (
                inst.rd(),
                self.cpu.registers[inst.rb() as usize],
                inst.half_offset(),
            ),
            LdrSp | StrSp => (inst.dest8(), self.cpu.sp(), inst.scaled_offset() as u32),
            LdrLiteral => (
                inst.dest8(),
                self.cpu.pc() & !0b11,
                inst.scaled_offset() as u32,
            ),
        };
        let rd = rd as usize;
        let addr = base.wrapping_add(offset);
        let value = self.cpu.registers[rd];

        let loaded = match op {
            StrWordImm | StrSp => {
                self.mmio.write32(addr & !0b11, value);
                return;
            }
            StrbByteImm => {
                self.mmio.write8(addr, value as u8);
                return;
            }
            StrhHalfImm => {
                self.mmio.write16(addr & !0b1, value as u16);
                return;
            }
            LdrWordImm | LdrSp => self.load_word_rotated(addr),
            LdrbByteImm => self.mmio.read8(addr) as u32,
            LdrhHalfImm => self.load_halfword_rotated(addr),
            LdrLiteral => self.mmio.read32(addr),
        };
        self.cpu.set_register(rd, loaded);
    }

    fn address(&mut self, op: thumb::AddressKind, inst: thumb::Instruction) {
        let rd = inst.dest8() as usize;

        match op {
            thumb::AddressKind::AddPc => self.cpu.set_register(
                rd,
                (self.cpu.pc() & !0b11).wrapping_add(inst.scaled_offset() as u32),
            ),
            thumb::AddressKind::AddSp => self
                .cpu
                .set_register(rd, self.cpu.sp().wrapping_add(inst.scaled_offset() as u32)),
            thumb::AddressKind::AddSpAdjust => self
                .cpu
                .set_sp(self.cpu.sp().wrapping_add(inst.stack_offset() as u32)),
            thumb::AddressKind::SubSpAdjust => self
                .cpu
                .set_sp(self.cpu.sp().wrapping_sub(inst.stack_offset() as u32)),
        }
    }

    fn multiple_transfer(&mut self, op: thumb::MultipleTransferKind, inst: thumb::Instruction) {
        use thumb::MultipleTransferKind::*;

        let rb = match op {
            Push | Pop => 13,
            Stmia | Ldmia => inst.base() as usize,
        };
        let base = self.cpu.registers[rb];
        // Empty lists transfer PC but write back as tho 16 registers moved.
        let (list, size) = match inst.rlist() {
            0 => (1 << 15, 0x40),
            list => (list, list.count_ones() * 4),
        };
        let (start, updated_base) = match op {
            Push => {
                let start = base.wrapping_sub(size);
                (start, start)
            }
            Stmia | Ldmia | Pop => (base, base.wrapping_add(size)),
        };

        match op {
            Ldmia | Pop => {
                // A loaded base register takes precedence over writeback.
                self.cpu.set_register(rb, updated_base);
                for (index, reg) in registers(list).enumerate() {
                    let addr = start.wrapping_add(index as u32 * 4);
                    let value = self.mmio.read32(addr & !0b11);
                    self.cpu.set_register(reg, value);
                }
            }
            Stmia | Push => {
                for (index, reg) in registers(list).enumerate() {
                    let addr = start.wrapping_add(index as u32 * 4);
                    let value = match reg {
                        15 => self.cpu.pc().wrapping_add(2),
                        _ if reg == rb && index != 0 => updated_base,
                        _ => self.cpu.registers[reg],
                    };
                    self.mmio.write32(addr & !0b11, value);
                }
                self.cpu.set_register(rb, updated_base);
            }
        }
    }

    fn branch(&mut self, op: thumb::BranchKind, inst: thumb::Instruction) {
        match op {
            thumb::BranchKind::BCond => {
                if self.check_condition(inst.cond()) {
                    let offset = inst.cond_offset() as i32;
                    self.cpu.set_pc(self.cpu.pc().wrapping_add_signed(offset));
                }
            }
            thumb::BranchKind::BUncond => {
                let offset = inst.offset() as i32;
                self.cpu.set_pc(self.cpu.pc().wrapping_add_signed(offset));
            }
            thumb::BranchKind::Bx => self
                .cpu
                .branch_exchange(self.cpu.registers[inst.high_src() as usize]),
            thumb::BranchKind::Blh => {
                let lr = self.cpu.pc().wrapping_add_signed(inst.prefix() as i32);
                self.cpu.set_lr(lr);
            }
            thumb::BranchKind::Bll => {
                let target = self.cpu.lr().wrapping_add(inst.suffix() as u32);
                let return_address = self.cpu.pc().wrapping_sub(2) | 1;
                self.cpu.set_lr(return_address);
                self.cpu.set_pc(target);
            }
        }
    }

    fn swi(&mut self, _inst: thumb::Instruction) {
        let return_address = self.cpu.pc().wrapping_sub(2);
        self.cpu
            .enter_exception(Mode::Supervisor, 0x08, return_address);
    }

    fn on_invalid(&mut self, _inst: thumb::Instruction) {
        let return_address = self.cpu.pc().wrapping_sub(2);
        self.cpu
            .enter_exception(Mode::Undefined, 0x04, return_address);
    }
}

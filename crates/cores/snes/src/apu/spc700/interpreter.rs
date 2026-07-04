use super::decoder::{
    Alu, BitBranch, BitOps, Branches, Call, Decimal, FlagOps, Instruction, Load, MemStep, MoveMem,
    MoveReg, Muldiv, RegStep, Returns, Shift, Store, Word,
};
use super::regs::Flags;
use crate::alu::{ShiftOp, add_binary};
use crate::apu::Apu;
use crate::apu::map::ApuMap;
#[cfg(feature = "debug")]
use crate::callstack::{CallFrame, CallKind};

#[derive(Clone, Copy)]
enum AluOp {
    Or,
    And,
    Eor,
    Cmp,
    Adc,
    Sbc,
}

#[derive(Clone, Copy)]
enum BitOp {
    Or,
    OrN,
    And,
    AndN,
    Eor,
    Mov,
}

#[derive(Clone, Copy)]
enum Mode {
    Dp(u8),
    Dpx(u8),
    Dpy(u8),
    Abs(u16),
    Absx(u16),
    Absy(u16),
    Idx(u8),
    Idy(u8),
    Xind,
}

pub(super) struct Interpreter<'a, M: ApuMap> {
    apu: &'a mut Apu<M>,
    next_pc: u16,
}

impl<'a, M: ApuMap> Interpreter<'a, M> {
    pub(super) fn new(apu: &'a mut Apu<M>, len: u8) -> Self {
        let next_pc = apu.spc700.pc.wrapping_add(len as u16);
        Self { apu, next_pc }
    }

    pub(super) fn next_pc(&self) -> u16 {
        self.next_pc
    }

    fn set_a(&mut self, value: u8) {
        self.apu.spc700.a = value;
        self.set_nz8(value);
    }

    fn set_x(&mut self, value: u8) {
        self.apu.spc700.x = value;
        self.set_nz8(value);
    }

    fn set_y(&mut self, value: u8) {
        self.apu.spc700.y = value;
        self.set_nz8(value);
    }

    fn set_nz8(&mut self, value: u8) {
        let psw = &mut self.apu.spc700.psw;
        psw.set_negative(value & 0x80 != 0);
        psw.set_zero(value == 0);
    }

    fn set_nz16(&mut self, value: u16) {
        let psw = &mut self.apu.spc700.psw;
        psw.set_negative(value & 0x8000 != 0);
        psw.set_zero(value == 0);
    }

    fn dp_addr(&self, offset: u8) -> u16 {
        let page = if self.apu.spc700.psw.direct_page() {
            0x100
        } else {
            0
        };

        page | offset as u16
    }

    fn effective_address(&mut self, mode: Mode) -> u16 {
        match mode {
            Mode::Dp(dp) => self.dp_addr(dp),
            Mode::Dpx(dp) => self.dp_addr(dp.wrapping_add(self.apu.spc700.x)),
            Mode::Dpy(dp) => self.dp_addr(dp.wrapping_add(self.apu.spc700.y)),
            Mode::Abs(addr) => addr,
            Mode::Absx(addr) => addr.wrapping_add(self.apu.spc700.x as u16),
            Mode::Absy(addr) => addr.wrapping_add(self.apu.spc700.y as u16),
            Mode::Idx(dp) => self.read_dp_word(dp.wrapping_add(self.apu.spc700.x)),
            Mode::Idy(dp) => self.read_dp_word(dp).wrapping_add(self.apu.spc700.y as u16),
            Mode::Xind => self.dp_addr(self.apu.spc700.x),
        }
    }

    fn read_ea(&mut self, mode: Mode) -> u8 {
        let addr = self.effective_address(mode);
        self.apu.read(addr)
    }

    fn write_ea(&mut self, mode: Mode, value: u8) {
        let addr = self.effective_address(mode);
        self.apu.write(addr, value);
    }

    fn read16(&mut self, addr: u16) -> u16 {
        let lo = self.apu.read(addr) as u16;
        let hi = self.apu.read(addr.wrapping_add(1)) as u16;
        lo | (hi << 8)
    }

    /// Read a 16bit word from the direct page, wrapping the high byte within the page.
    fn read_dp_word(&mut self, dp: u8) -> u16 {
        let lo = self.apu.read(self.dp_addr(dp)) as u16;
        let hi = self.apu.read(self.dp_addr(dp.wrapping_add(1))) as u16;
        lo | (hi << 8)
    }

    /// Write a 16bit word to the direct page, wrapping the high byte within the page.
    fn write_dp_word(&mut self, dp: u8, value: u16) {
        self.apu.write(self.dp_addr(dp), value as u8);
        self.apu
            .write(self.dp_addr(dp.wrapping_add(1)), (value >> 8) as u8);
    }

    fn push8(&mut self, value: u8) {
        let addr = 0x0100 | self.apu.spc700.sp as u16;
        self.apu.write(addr, value);
        self.apu.spc700.sp = self.apu.spc700.sp.wrapping_sub(1);
    }

    fn pop8(&mut self) -> u8 {
        self.apu.spc700.sp = self.apu.spc700.sp.wrapping_add(1);
        let addr = 0x0100 | self.apu.spc700.sp as u16;
        self.apu.read(addr)
    }

    fn push16(&mut self, value: u16) {
        self.push8((value >> 8) as u8);
        self.push8(value as u8);
    }

    fn pop16(&mut self) -> u16 {
        let lo = self.pop8() as u16;
        let hi = self.pop8() as u16;
        lo | (hi << 8)
    }

    /// Record a call frame opened by the current instruction.
    #[cfg(feature = "debug")]
    fn enter_call(&mut self, kind: CallKind, caller: u16, target: u16) {
        crate::callstack::push_frame(
            &mut self.apu.call_stack,
            CallFrame {
                kind,
                caller: caller as u32,
                target: target as u32,
                sp: self.apu.spc700.sp as u16,
            },
        );
    }

    /// Unwind call frames the stack pointer has moved past on return.
    #[cfg(feature = "debug")]
    fn leave_call(&mut self) {
        crate::callstack::unwind(&mut self.apu.call_stack, self.apu.spc700.sp as u16);
    }

    fn branch(&mut self, disp: i8) {
        self.next_pc = self.next_pc.wrapping_add(disp as u16);
    }

    pub(super) fn exec(&mut self, inst: Instruction) {
        match inst {
            Instruction::Load(op) => self.load(op),
            Instruction::Store(op) => self.store(op),
            Instruction::MoveReg(op) => self.move_reg(op),
            Instruction::MoveMem(op) => self.move_mem(op),
            Instruction::Word(op) => self.word(op),
            Instruction::Alu(op) => self.alu(op),
            Instruction::Muldiv(op) => self.muldiv(op),
            Instruction::Decimal(op) => self.decimal(op),
            Instruction::Shift(op) => self.shift(op),
            Instruction::RegStep(op) => self.reg_step(op),
            Instruction::MemStep(op) => self.mem_step(op),
            Instruction::Branches(op) => self.branches(op),
            Instruction::BitBranch(op) => self.bit_branch(op),
            Instruction::Call(op) => self.call(op),
            Instruction::Returns(op) => self.returns(op),
            Instruction::FlagOps(op) => self.flag_ops(op),
            Instruction::BitOps(op) => self.bit_ops(op),
            // TODO: actually halt the core
            Instruction::Halt(_) => {}
            Instruction::JmpAbs { addr } => self.next_pc = addr,
            Instruction::JmpIax { addr } => {
                self.next_pc = self.read16(addr.wrapping_add(self.apu.spc700.x as u16));
            }
            Instruction::Nop | Instruction::Invalid { .. } => {}
            Instruction::Brk => self.brk(),
            Instruction::PushPsw => self.push8(self.apu.spc700.psw.raw()),
            Instruction::PushA => self.push8(self.apu.spc700.a),
            Instruction::PushX => self.push8(self.apu.spc700.x),
            Instruction::PushY => self.push8(self.apu.spc700.y),
            Instruction::PopPsw => self.apu.spc700.psw = Flags::from_raw(self.pop8()),
            Instruction::PopA => self.apu.spc700.a = self.pop8(),
            Instruction::PopX => self.apu.spc700.x = self.pop8(),
            Instruction::PopY => self.apu.spc700.y = self.pop8(),
        }
    }

    fn ld(&mut self, mode: Mode, set: fn(&mut Self, u8)) {
        let value = self.read_ea(mode);
        set(self, value);
    }

    fn load(&mut self, op: Load) {
        match op {
            Load::MovAImm { imm } => self.set_a(imm),
            Load::MovAXind => self.ld(Mode::Xind, Self::set_a),
            Load::MovAXinc => {
                self.ld(Mode::Xind, Self::set_a);
                self.apu.spc700.x = self.apu.spc700.x.wrapping_add(1);
            }
            Load::MovADp { dp } => self.ld(Mode::Dp(dp), Self::set_a),
            Load::MovADpx { dp } => self.ld(Mode::Dpx(dp), Self::set_a),
            Load::MovAAbs { addr } => self.ld(Mode::Abs(addr), Self::set_a),
            Load::MovAAbsx { addr } => self.ld(Mode::Absx(addr), Self::set_a),
            Load::MovAAbsy { addr } => self.ld(Mode::Absy(addr), Self::set_a),
            Load::MovAIdx { dp } => self.ld(Mode::Idx(dp), Self::set_a),
            Load::MovAIdy { dp } => self.ld(Mode::Idy(dp), Self::set_a),
            Load::MovXImm { imm } => self.set_x(imm),
            Load::MovXDp { dp } => self.ld(Mode::Dp(dp), Self::set_x),
            Load::MovXDpy { dp } => self.ld(Mode::Dpy(dp), Self::set_x),
            Load::MovXAbs { addr } => self.ld(Mode::Abs(addr), Self::set_x),
            Load::MovYImm { imm } => self.set_y(imm),
            Load::MovYDp { dp } => self.ld(Mode::Dp(dp), Self::set_y),
            Load::MovYDpx { dp } => self.ld(Mode::Dpx(dp), Self::set_y),
            Load::MovYAbs { addr } => self.ld(Mode::Abs(addr), Self::set_y),
        }
    }

    fn store(&mut self, op: Store) {
        match op {
            Store::MovXindA => self.write_ea(Mode::Xind, self.apu.spc700.a),
            Store::MovXincA => {
                self.write_ea(Mode::Xind, self.apu.spc700.a);
                self.apu.spc700.x = self.apu.spc700.x.wrapping_add(1);
            }
            Store::MovDpA { dp } => self.write_ea(Mode::Dp(dp), self.apu.spc700.a),
            Store::MovDpxA { dp } => self.write_ea(Mode::Dpx(dp), self.apu.spc700.a),
            Store::MovAbsA { addr } => self.write_ea(Mode::Abs(addr), self.apu.spc700.a),
            Store::MovAbsxA { addr } => self.write_ea(Mode::Absx(addr), self.apu.spc700.a),
            Store::MovAbsyA { addr } => self.write_ea(Mode::Absy(addr), self.apu.spc700.a),
            Store::MovIdxA { dp } => self.write_ea(Mode::Idx(dp), self.apu.spc700.a),
            Store::MovIdyA { dp } => self.write_ea(Mode::Idy(dp), self.apu.spc700.a),
            Store::MovDpX { dp } => self.write_ea(Mode::Dp(dp), self.apu.spc700.x),
            Store::MovDpyX { dp } => self.write_ea(Mode::Dpy(dp), self.apu.spc700.x),
            Store::MovAbsX { addr } => self.write_ea(Mode::Abs(addr), self.apu.spc700.x),
            Store::MovDpY { dp } => self.write_ea(Mode::Dp(dp), self.apu.spc700.y),
            Store::MovDpxY { dp } => self.write_ea(Mode::Dpx(dp), self.apu.spc700.y),
            Store::MovAbsY { addr } => self.write_ea(Mode::Abs(addr), self.apu.spc700.y),
        }
    }

    fn move_reg(&mut self, op: MoveReg) {
        match op {
            MoveReg::MovAX => self.set_a(self.apu.spc700.x),
            MoveReg::MovXA => self.set_x(self.apu.spc700.a),
            MoveReg::MovAY => self.set_a(self.apu.spc700.y),
            MoveReg::MovYA => self.set_y(self.apu.spc700.a),
            MoveReg::MovXSp => self.set_x(self.apu.spc700.sp),
            MoveReg::MovSpX => self.apu.spc700.sp = self.apu.spc700.x,
        }
    }

    fn move_mem(&mut self, op: MoveMem) {
        match op {
            MoveMem::MovDpDp { src, dst } => {
                let value = self.read_ea(Mode::Dp(src));
                self.write_ea(Mode::Dp(dst), value);
            }
            MoveMem::MovDpImm { imm, dp } => self.write_ea(Mode::Dp(dp), imm),
        }
    }

    fn word(&mut self, op: Word) {
        match op {
            Word::MovwYaDp { dp } => {
                let value = self.read_dp_word(dp);
                self.apu.spc700.set_ya(value);
                self.set_nz16(value);
            }
            Word::MovwDpYa { dp } => {
                let value = self.apu.spc700.ya();
                self.write_dp_word(dp, value);
            }
            Word::IncwDp { dp } => {
                let value = self.read_dp_word(dp).wrapping_add(1);
                self.write_dp_word(dp, value);
                self.set_nz16(value);
            }
            Word::DecwDp { dp } => {
                let value = self.read_dp_word(dp).wrapping_sub(1);
                self.write_dp_word(dp, value);
                self.set_nz16(value);
            }
            Word::AddwYaDp { dp } => {
                let m = self.read_dp_word(dp);
                let ya = self.apu.spc700.ya();
                self.add16(ya, m, false);
            }
            Word::SubwYaDp { dp } => {
                let m = self.read_dp_word(dp);
                let ya = self.apu.spc700.ya();
                self.add16(ya, !m, true);
            }
            Word::CmpwYaDp { dp } => {
                let m = self.read_dp_word(dp);
                let ya = self.apu.spc700.ya();
                let result = ya.wrapping_sub(m);
                let psw = &mut self.apu.spc700.psw;
                psw.set_carry(ya >= m);
                psw.set_zero(result == 0);
                psw.set_negative(result & 0x8000 != 0);
            }
        }
    }

    fn add16(&mut self, a: u16, b: u16, carry_in: bool) {
        let alu = add_binary(a, b, carry_in, true);

        let mut psw = self.apu.spc700.psw;
        psw.set_carry(alu.carry);
        psw.set_half_carry(((a ^ b ^ alu.result) >> 8) & 0x10 != 0);
        psw.set_overflow(alu.overflow);
        psw.set_negative(alu.negative);
        psw.set_zero(alu.zero);
        self.apu.spc700.psw = psw;

        self.apu.spc700.set_ya(alu.result);
    }

    fn alu(&mut self, op: Alu) {
        use AluOp::*;

        match op {
            Alu::OrAImm { imm } => self.alu_a(Or, imm),
            Alu::OrAXind => self.alu_a_mem(Or, Mode::Xind),
            Alu::OrADp { dp } => self.alu_a_mem(Or, Mode::Dp(dp)),
            Alu::OrADpx { dp } => self.alu_a_mem(Or, Mode::Dpx(dp)),
            Alu::OrAAbs { addr } => self.alu_a_mem(Or, Mode::Abs(addr)),
            Alu::OrAAbsx { addr } => self.alu_a_mem(Or, Mode::Absx(addr)),
            Alu::OrAAbsy { addr } => self.alu_a_mem(Or, Mode::Absy(addr)),
            Alu::OrAIdx { dp } => self.alu_a_mem(Or, Mode::Idx(dp)),
            Alu::OrAIdy { dp } => self.alu_a_mem(Or, Mode::Idy(dp)),
            Alu::OrXy => self.alu_xy(Or),
            Alu::OrDpDp { src, dst } => self.alu_dp_dp(Or, src, dst),
            Alu::OrDpImm { imm, dp } => self.alu_dp_imm(Or, imm, dp),

            Alu::AndAImm { imm } => self.alu_a(And, imm),
            Alu::AndAXind => self.alu_a_mem(And, Mode::Xind),
            Alu::AndADp { dp } => self.alu_a_mem(And, Mode::Dp(dp)),
            Alu::AndADpx { dp } => self.alu_a_mem(And, Mode::Dpx(dp)),
            Alu::AndAAbs { addr } => self.alu_a_mem(And, Mode::Abs(addr)),
            Alu::AndAAbsx { addr } => self.alu_a_mem(And, Mode::Absx(addr)),
            Alu::AndAAbsy { addr } => self.alu_a_mem(And, Mode::Absy(addr)),
            Alu::AndAIdx { dp } => self.alu_a_mem(And, Mode::Idx(dp)),
            Alu::AndAIdy { dp } => self.alu_a_mem(And, Mode::Idy(dp)),
            Alu::AndXy => self.alu_xy(And),
            Alu::AndDpDp { src, dst } => self.alu_dp_dp(And, src, dst),
            Alu::AndDpImm { imm, dp } => self.alu_dp_imm(And, imm, dp),

            Alu::EorAImm { imm } => self.alu_a(Eor, imm),
            Alu::EorAXind => self.alu_a_mem(Eor, Mode::Xind),
            Alu::EorADp { dp } => self.alu_a_mem(Eor, Mode::Dp(dp)),
            Alu::EorADpx { dp } => self.alu_a_mem(Eor, Mode::Dpx(dp)),
            Alu::EorAAbs { addr } => self.alu_a_mem(Eor, Mode::Abs(addr)),
            Alu::EorAAbsx { addr } => self.alu_a_mem(Eor, Mode::Absx(addr)),
            Alu::EorAAbsy { addr } => self.alu_a_mem(Eor, Mode::Absy(addr)),
            Alu::EorAIdx { dp } => self.alu_a_mem(Eor, Mode::Idx(dp)),
            Alu::EorAIdy { dp } => self.alu_a_mem(Eor, Mode::Idy(dp)),
            Alu::EorXy => self.alu_xy(Eor),
            Alu::EorDpDp { src, dst } => self.alu_dp_dp(Eor, src, dst),
            Alu::EorDpImm { imm, dp } => self.alu_dp_imm(Eor, imm, dp),

            Alu::CmpAImm { imm } => self.alu_a(Cmp, imm),
            Alu::CmpAXind => self.alu_a_mem(Cmp, Mode::Xind),
            Alu::CmpADp { dp } => self.alu_a_mem(Cmp, Mode::Dp(dp)),
            Alu::CmpADpx { dp } => self.alu_a_mem(Cmp, Mode::Dpx(dp)),
            Alu::CmpAAbs { addr } => self.alu_a_mem(Cmp, Mode::Abs(addr)),
            Alu::CmpAAbsx { addr } => self.alu_a_mem(Cmp, Mode::Absx(addr)),
            Alu::CmpAAbsy { addr } => self.alu_a_mem(Cmp, Mode::Absy(addr)),
            Alu::CmpAIdx { dp } => self.alu_a_mem(Cmp, Mode::Idx(dp)),
            Alu::CmpAIdy { dp } => self.alu_a_mem(Cmp, Mode::Idy(dp)),
            Alu::CmpXy => self.alu_xy(Cmp),
            Alu::CmpDpDp { src, dst } => self.alu_dp_dp(Cmp, src, dst),
            Alu::CmpDpImm { imm, dp } => self.alu_dp_imm(Cmp, imm, dp),
            Alu::CmpXImm { imm } => self.cmp(self.apu.spc700.x, imm),
            Alu::CmpXDp { dp } => self.cmp_mem(self.apu.spc700.x, Mode::Dp(dp)),
            Alu::CmpXAbs { addr } => self.cmp_mem(self.apu.spc700.x, Mode::Abs(addr)),
            Alu::CmpYImm { imm } => self.cmp(self.apu.spc700.y, imm),
            Alu::CmpYDp { dp } => self.cmp_mem(self.apu.spc700.y, Mode::Dp(dp)),
            Alu::CmpYAbs { addr } => self.cmp_mem(self.apu.spc700.y, Mode::Abs(addr)),

            Alu::AdcAImm { imm } => self.alu_a(Adc, imm),
            Alu::AdcAXind => self.alu_a_mem(Adc, Mode::Xind),
            Alu::AdcADp { dp } => self.alu_a_mem(Adc, Mode::Dp(dp)),
            Alu::AdcADpx { dp } => self.alu_a_mem(Adc, Mode::Dpx(dp)),
            Alu::AdcAAbs { addr } => self.alu_a_mem(Adc, Mode::Abs(addr)),
            Alu::AdcAAbsx { addr } => self.alu_a_mem(Adc, Mode::Absx(addr)),
            Alu::AdcAAbsy { addr } => self.alu_a_mem(Adc, Mode::Absy(addr)),
            Alu::AdcAIdx { dp } => self.alu_a_mem(Adc, Mode::Idx(dp)),
            Alu::AdcAIdy { dp } => self.alu_a_mem(Adc, Mode::Idy(dp)),
            Alu::AdcXy => self.alu_xy(Adc),
            Alu::AdcDpDp { src, dst } => self.alu_dp_dp(Adc, src, dst),
            Alu::AdcDpImm { imm, dp } => self.alu_dp_imm(Adc, imm, dp),

            Alu::SbcAImm { imm } => self.alu_a(Sbc, imm),
            Alu::SbcAXind => self.alu_a_mem(Sbc, Mode::Xind),
            Alu::SbcADp { dp } => self.alu_a_mem(Sbc, Mode::Dp(dp)),
            Alu::SbcADpx { dp } => self.alu_a_mem(Sbc, Mode::Dpx(dp)),
            Alu::SbcAAbs { addr } => self.alu_a_mem(Sbc, Mode::Abs(addr)),
            Alu::SbcAAbsx { addr } => self.alu_a_mem(Sbc, Mode::Absx(addr)),
            Alu::SbcAAbsy { addr } => self.alu_a_mem(Sbc, Mode::Absy(addr)),
            Alu::SbcAIdx { dp } => self.alu_a_mem(Sbc, Mode::Idx(dp)),
            Alu::SbcAIdy { dp } => self.alu_a_mem(Sbc, Mode::Idy(dp)),
            Alu::SbcXy => self.alu_xy(Sbc),
            Alu::SbcDpDp { src, dst } => self.alu_dp_dp(Sbc, src, dst),
            Alu::SbcDpImm { imm, dp } => self.alu_dp_imm(Sbc, imm, dp),
        }
    }

    fn alu8(&mut self, op: AluOp, a: u8, b: u8) -> Option<u8> {
        let (result, write) = match op {
            AluOp::Or => (a | b, true),
            AluOp::And => (a & b, true),
            AluOp::Eor => (a ^ b, true),
            AluOp::Cmp => {
                self.apu.spc700.psw.set_carry(a >= b);
                (a.wrapping_sub(b), false)
            }
            AluOp::Adc => return Some(self.adc(a, b)),
            AluOp::Sbc => return Some(self.adc(a, !b)),
        };

        self.set_nz8(result);
        write.then_some(result)
    }

    fn adc(&mut self, a: u8, b: u8) -> u8 {
        let alu = add_binary(a as u16, b as u16, self.apu.spc700.psw.carry(), false);
        let result = alu.result as u8;

        let mut psw = self.apu.spc700.psw;
        psw.set_carry(alu.carry);
        psw.set_half_carry((a ^ b ^ result) & 0x10 != 0);
        psw.set_overflow(alu.overflow);
        psw.set_negative(alu.negative);
        psw.set_zero(alu.zero);
        self.apu.spc700.psw = psw;

        result
    }

    fn cmp(&mut self, lhs: u8, rhs: u8) {
        self.alu8(AluOp::Cmp, lhs, rhs);
    }

    fn cmp_mem(&mut self, lhs: u8, mode: Mode) {
        let rhs = self.read_ea(mode);
        self.cmp(lhs, rhs);
    }

    fn alu_a(&mut self, op: AluOp, rhs: u8) {
        let a = self.apu.spc700.a;
        if let Some(result) = self.alu8(op, a, rhs) {
            self.apu.spc700.a = result;
        }
    }

    fn alu_a_mem(&mut self, op: AluOp, mode: Mode) {
        let rhs = self.read_ea(mode);
        self.alu_a(op, rhs);
    }

    fn alu_xy(&mut self, op: AluOp) {
        let x_addr = self.effective_address(Mode::Xind);
        let y_addr = self.dp_addr(self.apu.spc700.y);
        let lhs = self.apu.read(x_addr);
        let rhs = self.apu.read(y_addr);
        if let Some(result) = self.alu8(op, lhs, rhs) {
            self.apu.write(x_addr, result);
        }
    }

    fn alu_dp_dp(&mut self, op: AluOp, src: u8, dst: u8) {
        let rhs = self.read_ea(Mode::Dp(src));
        let dst_addr = self.dp_addr(dst);
        let lhs = self.apu.read(dst_addr);
        if let Some(result) = self.alu8(op, lhs, rhs) {
            self.apu.write(dst_addr, result);
        }
    }

    fn alu_dp_imm(&mut self, op: AluOp, imm: u8, dp: u8) {
        let dst_addr = self.dp_addr(dp);
        let lhs = self.apu.read(dst_addr);
        if let Some(result) = self.alu8(op, lhs, imm) {
            self.apu.write(dst_addr, result);
        }
    }

    fn muldiv(&mut self, op: Muldiv) {
        match op {
            Muldiv::MulYa => {
                let result = self.apu.spc700.y as u16 * self.apu.spc700.a as u16;
                self.apu.spc700.set_ya(result);
                self.set_nz8((result >> 8) as u8);
            }
            Muldiv::DivYaX => self.div(),
        }
    }

    fn div(&mut self) {
        let x = self.apu.spc700.x;
        let y = self.apu.spc700.y;

        let mut yva = self.apu.spc700.ya() as u32;
        let divisor = (x as u32) << 9;
        for _ in 0..9 {
            yva = ((yva << 1) | (yva >> 16)) & 0x1ffff;
            if yva >= divisor {
                yva ^= 1;
            }
            if yva & 1 != 0 {
                yva = yva.wrapping_sub(divisor) & 0x1ffff;
            }
        }

        let a = (yva & 0xff) as u8;
        let quotient_high = ((yva >> 9) & 0xff) as u8;

        let mut psw = self.apu.spc700.psw;
        psw.set_half_carry((x & 0x0f) <= (y & 0x0f));
        psw.set_overflow(yva & 0x100 != 0);
        psw.set_negative(a & 0x80 != 0);
        psw.set_zero(a == 0);
        self.apu.spc700.psw = psw;

        self.apu.spc700.a = a;
        self.apu.spc700.y = quotient_high;
    }

    fn decimal(&mut self, op: Decimal) {
        let mut a = self.apu.spc700.a;
        let mut psw = self.apu.spc700.psw;

        match op {
            Decimal::DaaA => {
                if psw.carry() || a > 0x99 {
                    a = a.wrapping_add(0x60);
                    psw.set_carry(true);
                }
                if psw.half_carry() || (a & 0x0f) > 0x09 {
                    a = a.wrapping_add(0x06);
                }
            }
            Decimal::DasA => {
                if !psw.carry() || a > 0x99 {
                    a = a.wrapping_sub(0x60);
                    psw.set_carry(false);
                }
                if !psw.half_carry() || (a & 0x0f) > 0x09 {
                    a = a.wrapping_sub(0x06);
                }
            }
        }

        psw.set_negative(a & 0x80 != 0);
        psw.set_zero(a == 0);
        self.apu.spc700.psw = psw;
        self.apu.spc700.a = a;
    }

    fn shift(&mut self, op: Shift) {
        match op {
            Shift::AslA => self.shift_a(ShiftOp::Asl),
            Shift::AslDp { dp } => self.shift_mem(ShiftOp::Asl, Mode::Dp(dp)),
            Shift::AslDpx { dp } => self.shift_mem(ShiftOp::Asl, Mode::Dpx(dp)),
            Shift::AslAbs { addr } => self.shift_mem(ShiftOp::Asl, Mode::Abs(addr)),
            Shift::LsrA => self.shift_a(ShiftOp::Lsr),
            Shift::LsrDp { dp } => self.shift_mem(ShiftOp::Lsr, Mode::Dp(dp)),
            Shift::LsrDpx { dp } => self.shift_mem(ShiftOp::Lsr, Mode::Dpx(dp)),
            Shift::LsrAbs { addr } => self.shift_mem(ShiftOp::Lsr, Mode::Abs(addr)),
            Shift::RolA => self.shift_a(ShiftOp::Rol),
            Shift::RolDp { dp } => self.shift_mem(ShiftOp::Rol, Mode::Dp(dp)),
            Shift::RolDpx { dp } => self.shift_mem(ShiftOp::Rol, Mode::Dpx(dp)),
            Shift::RolAbs { addr } => self.shift_mem(ShiftOp::Rol, Mode::Abs(addr)),
            Shift::RorA => self.shift_a(ShiftOp::Ror),
            Shift::RorDp { dp } => self.shift_mem(ShiftOp::Ror, Mode::Dp(dp)),
            Shift::RorDpx { dp } => self.shift_mem(ShiftOp::Ror, Mode::Dpx(dp)),
            Shift::RorAbs { addr } => self.shift_mem(ShiftOp::Ror, Mode::Abs(addr)),
            Shift::XcnA => {
                let a = self.apu.spc700.a;
                self.set_a((a >> 4) | (a << 4));
            }
        }
    }

    fn shift_a(&mut self, op: ShiftOp) {
        let value = self.apu.spc700.a;
        let result = self.shift_value(op, value);
        self.apu.spc700.a = result;
    }

    fn shift_mem(&mut self, op: ShiftOp, mode: Mode) {
        let addr = self.effective_address(mode);
        let value = self.apu.read(addr);
        let result = self.shift_value(op, value);
        self.apu.write(addr, result);
    }

    fn shift_value(&mut self, op: ShiftOp, value: u8) -> u8 {
        let (result, carry_out) =
            crate::alu::shift(op, value as u16, self.apu.spc700.psw.carry(), false);
        let result = result as u8;

        self.apu.spc700.psw.set_carry(carry_out);
        self.set_nz8(result);
        result
    }

    fn reg_step(&mut self, op: RegStep) {
        match op {
            RegStep::IncA => self.set_a(self.apu.spc700.a.wrapping_add(1)),
            RegStep::DecA => self.set_a(self.apu.spc700.a.wrapping_sub(1)),
            RegStep::IncX => self.set_x(self.apu.spc700.x.wrapping_add(1)),
            RegStep::DecX => self.set_x(self.apu.spc700.x.wrapping_sub(1)),
            RegStep::IncY => self.set_y(self.apu.spc700.y.wrapping_add(1)),
            RegStep::DecY => self.set_y(self.apu.spc700.y.wrapping_sub(1)),
        }
    }

    fn mem_step(&mut self, op: MemStep) {
        let (mode, delta) = match op {
            MemStep::IncDp { dp } => (Mode::Dp(dp), 1),
            MemStep::IncDpx { dp } => (Mode::Dpx(dp), 1),
            MemStep::IncAbs { addr } => (Mode::Abs(addr), 1),
            MemStep::DecDp { dp } => (Mode::Dp(dp), -1),
            MemStep::DecDpx { dp } => (Mode::Dpx(dp), -1),
            MemStep::DecAbs { addr } => (Mode::Abs(addr), -1),
        };

        let addr = self.effective_address(mode);
        let value = self.apu.read(addr).wrapping_add_signed(delta);
        self.apu.write(addr, value);
        self.set_nz8(value);
    }

    fn branches(&mut self, op: Branches) {
        let flags = self.apu.spc700.psw;
        let (taken, disp) = match op {
            Branches::BraRel { disp } => (true, disp),
            Branches::BeqRel { disp } => (flags.zero(), disp),
            Branches::BneRel { disp } => (!flags.zero(), disp),
            Branches::BcsRel { disp } => (flags.carry(), disp),
            Branches::BccRel { disp } => (!flags.carry(), disp),
            Branches::BmiRel { disp } => (flags.negative(), disp),
            Branches::BplRel { disp } => (!flags.negative(), disp),
            Branches::BvsRel { disp } => (flags.overflow(), disp),
            Branches::BvcRel { disp } => (!flags.overflow(), disp),
        };

        if taken {
            self.branch(disp);
        }
    }

    fn bit_branch(&mut self, op: BitBranch) {
        match op {
            BitBranch::BbsB0 { dp, disp } => self.branch_bit(dp, 0, disp, true),
            BitBranch::BbsB1 { dp, disp } => self.branch_bit(dp, 1, disp, true),
            BitBranch::BbsB2 { dp, disp } => self.branch_bit(dp, 2, disp, true),
            BitBranch::BbsB3 { dp, disp } => self.branch_bit(dp, 3, disp, true),
            BitBranch::BbsB4 { dp, disp } => self.branch_bit(dp, 4, disp, true),
            BitBranch::BbsB5 { dp, disp } => self.branch_bit(dp, 5, disp, true),
            BitBranch::BbsB6 { dp, disp } => self.branch_bit(dp, 6, disp, true),
            BitBranch::BbsB7 { dp, disp } => self.branch_bit(dp, 7, disp, true),
            BitBranch::BbcB0 { dp, disp } => self.branch_bit(dp, 0, disp, false),
            BitBranch::BbcB1 { dp, disp } => self.branch_bit(dp, 1, disp, false),
            BitBranch::BbcB2 { dp, disp } => self.branch_bit(dp, 2, disp, false),
            BitBranch::BbcB3 { dp, disp } => self.branch_bit(dp, 3, disp, false),
            BitBranch::BbcB4 { dp, disp } => self.branch_bit(dp, 4, disp, false),
            BitBranch::BbcB5 { dp, disp } => self.branch_bit(dp, 5, disp, false),
            BitBranch::BbcB6 { dp, disp } => self.branch_bit(dp, 6, disp, false),
            BitBranch::BbcB7 { dp, disp } => self.branch_bit(dp, 7, disp, false),
            BitBranch::CbneDp { dp, disp } => self.cbne(Mode::Dp(dp), disp),
            BitBranch::CbneDpx { dp, disp } => self.cbne(Mode::Dpx(dp), disp),
            BitBranch::DbnzDp { dp, disp } => {
                let addr = self.dp_addr(dp);
                let value = self.apu.read(addr).wrapping_sub(1);
                self.apu.write(addr, value);
                if value != 0 {
                    self.branch(disp);
                }
            }
            BitBranch::DbnzY { disp } => {
                let value = self.apu.spc700.y.wrapping_sub(1);
                self.apu.spc700.y = value;
                if value != 0 {
                    self.branch(disp);
                }
            }
        }
    }

    fn branch_bit(&mut self, dp: u8, bit: u8, disp: i8, want_set: bool) {
        let value = self.read_ea(Mode::Dp(dp));
        if ((value >> bit) & 1 != 0) == want_set {
            self.branch(disp);
        }
    }

    fn cbne(&mut self, mode: Mode, disp: i8) {
        let value = self.read_ea(mode);
        if self.apu.spc700.a != value {
            self.branch(disp);
        }
    }

    fn call(&mut self, op: Call) {
        let target = match op {
            Call::CallAbs { addr } => addr,
            Call::Pcall { u } => 0xFF00 | u as u16,
            Call::Tcall0 => self.read16(0xFFDE),
            Call::Tcall1 => self.read16(0xFFDC),
            Call::Tcall2 => self.read16(0xFFDA),
            Call::Tcall3 => self.read16(0xFFD8),
            Call::Tcall4 => self.read16(0xFFD6),
            Call::Tcall5 => self.read16(0xFFD4),
            Call::Tcall6 => self.read16(0xFFD2),
            Call::Tcall7 => self.read16(0xFFD0),
            Call::Tcall8 => self.read16(0xFFCE),
            Call::Tcall9 => self.read16(0xFFCC),
            Call::Tcall10 => self.read16(0xFFCA),
            Call::Tcall11 => self.read16(0xFFC8),
            Call::Tcall12 => self.read16(0xFFC6),
            Call::Tcall13 => self.read16(0xFFC4),
            Call::Tcall14 => self.read16(0xFFC2),
            Call::Tcall15 => self.read16(0xFFC0),
        };

        self.push16(self.next_pc);
        self.next_pc = target;

        #[cfg(feature = "debug")]
        self.enter_call(CallKind::Call, self.apu.spc700.pc, target);
    }

    fn returns(&mut self, op: Returns) {
        match op {
            Returns::Ret => self.next_pc = self.pop16(),
            Returns::Reti => {
                let flags = self.pop8();
                self.apu.spc700.psw = Flags::from_raw(flags);
                self.next_pc = self.pop16();
            }
        }
        #[cfg(feature = "debug")]
        self.leave_call();
    }

    fn brk(&mut self) {
        self.push16(self.next_pc);
        let flags = self.apu.spc700.psw.raw();
        self.push8(flags);

        let mut psw = self.apu.spc700.psw;
        psw.set_brk(true);
        psw.set_irq_enable(false);
        self.apu.spc700.psw = psw;

        self.next_pc = self.read16(0xFFDE);

        #[cfg(feature = "debug")]
        self.enter_call(CallKind::Interrupt, self.apu.spc700.pc, self.next_pc);
    }

    fn flag_ops(&mut self, op: FlagOps) {
        let psw = &mut self.apu.spc700.psw;
        match op {
            FlagOps::Clrc => psw.set_carry(false),
            FlagOps::Clrv => {
                psw.set_overflow(false);
                psw.set_half_carry(false);
            }
            FlagOps::Clrp => psw.set_direct_page(false),
            FlagOps::Setc => psw.set_carry(true),
            FlagOps::Setp => psw.set_direct_page(true),
            FlagOps::Notc => psw.set_carry(!psw.carry()),
            FlagOps::Di => psw.set_irq_enable(false),
            FlagOps::Ei => psw.set_irq_enable(true),
        }
    }

    fn bit_ops(&mut self, op: BitOps) {
        match op {
            BitOps::Set1B0 { dp } => self.set_dp_bit(dp, 0, true),
            BitOps::Set1B1 { dp } => self.set_dp_bit(dp, 1, true),
            BitOps::Set1B2 { dp } => self.set_dp_bit(dp, 2, true),
            BitOps::Set1B3 { dp } => self.set_dp_bit(dp, 3, true),
            BitOps::Set1B4 { dp } => self.set_dp_bit(dp, 4, true),
            BitOps::Set1B5 { dp } => self.set_dp_bit(dp, 5, true),
            BitOps::Set1B6 { dp } => self.set_dp_bit(dp, 6, true),
            BitOps::Set1B7 { dp } => self.set_dp_bit(dp, 7, true),
            BitOps::Clr1B0 { dp } => self.set_dp_bit(dp, 0, false),
            BitOps::Clr1B1 { dp } => self.set_dp_bit(dp, 1, false),
            BitOps::Clr1B2 { dp } => self.set_dp_bit(dp, 2, false),
            BitOps::Clr1B3 { dp } => self.set_dp_bit(dp, 3, false),
            BitOps::Clr1B4 { dp } => self.set_dp_bit(dp, 4, false),
            BitOps::Clr1B5 { dp } => self.set_dp_bit(dp, 5, false),
            BitOps::Clr1B6 { dp } => self.set_dp_bit(dp, 6, false),
            BitOps::Clr1B7 { dp } => self.set_dp_bit(dp, 7, false),
            BitOps::Tset1Abs { addr } => self.test_bits(addr, true),
            BitOps::Tclr1Abs { addr } => self.test_bits(addr, false),
            BitOps::Or1Mb { mb } => self.carry_bit(mb, BitOp::Or),
            BitOps::Or1nMb { mb } => self.carry_bit(mb, BitOp::OrN),
            BitOps::And1Mb { mb } => self.carry_bit(mb, BitOp::And),
            BitOps::And1nMb { mb } => self.carry_bit(mb, BitOp::AndN),
            BitOps::Eor1Mb { mb } => self.carry_bit(mb, BitOp::Eor),
            BitOps::Mov1CMb { mb } => self.carry_bit(mb, BitOp::Mov),
            BitOps::Mov1MbC { mb } => {
                let (addr, bit) = Self::split_mb(mb);
                self.write_bit(addr, bit, self.apu.spc700.psw.carry());
            }
            BitOps::Not1Mb { mb } => {
                let (addr, bit) = Self::split_mb(mb);
                let value = self.apu.read(addr) ^ (1 << bit);
                self.apu.write(addr, value);
            }
        }
    }

    fn set_dp_bit(&mut self, dp: u8, bit: u8, set: bool) {
        self.write_bit(self.dp_addr(dp), bit, set);
    }

    fn write_bit(&mut self, addr: u16, bit: u8, set: bool) {
        let mut value = self.apu.read(addr);
        if set {
            value |= 1 << bit;
        } else {
            value &= !(1 << bit);
        }
        self.apu.write(addr, value);
    }

    fn test_bits(&mut self, addr: u16, set: bool) {
        let value = self.apu.read(addr);
        let a = self.apu.spc700.a;
        self.set_nz8(a.wrapping_sub(value));
        let result = if set { value | a } else { value & !a };
        self.apu.write(addr, result);
    }

    fn split_mb(mb: u16) -> (u16, u8) {
        (mb & 0x1fff, (mb >> 13) as u8 & 0x07)
    }

    fn carry_bit(&mut self, mb: u16, op: BitOp) {
        let (addr, bit) = Self::split_mb(mb);
        let mem = (self.apu.read(addr) >> bit) & 1 != 0;
        let carry = self.apu.spc700.psw.carry();
        let result = match op {
            BitOp::Or => carry || mem,
            BitOp::OrN => carry || !mem,
            BitOp::And => carry && mem,
            BitOp::AndN => carry && !mem,
            BitOp::Eor => carry ^ mem,
            BitOp::Mov => mem,
        };
        self.apu.spc700.psw.set_carry(result);
    }
}

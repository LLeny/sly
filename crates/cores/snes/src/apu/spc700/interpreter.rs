use super::decoder::{Instruction, Load};
use crate::apu::Apu;
use crate::apu::spc700::decoder::{
    Alu, Branches, FlagOps, MemStep, MoveMem, MoveReg, RegStep, Store, Word,
};

pub(super) struct Interpreter<'a> {
    apu: &'a mut Apu,
    next_pc: u16,
}

impl<'a> Interpreter<'a> {
    pub(super) fn new(apu: &'a mut Apu, len: u8) -> Self {
        let next_pc = apu.spc700.pc.wrapping_add(len as u16);
        Self { apu, next_pc }
    }

    pub(super) fn next_pc(&self) -> u16 {
        self.next_pc
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

    pub(super) fn exec(&mut self, inst: Instruction) {
        match inst {
            Instruction::Load(op) => self.load(op),
            Instruction::Store(op) => self.store(op),
            Instruction::MoveReg(op) => self.move_reg(op),
            Instruction::MoveMem(op) => self.move_mem(op),
            Instruction::RegStep(op) => self.reg_step(op),
            Instruction::Alu(op) => self.alu(op),
            Instruction::Branches(op) => self.branches(op),
            Instruction::JmpAbs { addr } => self.next_pc = addr,
            Instruction::JmpIax { addr } => self.jmp_iax(addr),
            Instruction::Word(op) => self.word(op),
            Instruction::MemStep(op) => self.mem_step(op),
            Instruction::FlagOps(op) => self.flag_ops(op),
            other => todo!("Instruction not implemented: {other:?}"),
        }
    }

    fn load(&mut self, op: Load) {
        match op {
            Load::MovXImm { imm } => {
                self.apu.spc700.x = imm;
                self.set_nz8(imm);
            }
            Load::MovAImm { imm } => {
                self.apu.spc700.a = imm;
                self.set_nz8(imm);
            }
            Load::MovYDp { dp } => {
                let value = self.apu.read(self.dp_addr(dp));
                self.apu.spc700.y = value;
                self.set_nz8(value);
            }
            Load::MovADp { dp } => {
                let value = self.apu.read(self.dp_addr(dp));
                self.apu.spc700.a = value;
                self.set_nz8(value);
            }
            other => todo!("Load instruction not implemented: {other:?}"),
        }
    }

    fn store(&mut self, op: Store) {
        match op {
            Store::MovXindA => {
                let addr = self.dp_addr(self.apu.spc700.x);
                self.apu.write(addr, self.apu.spc700.a);
            }
            Store::MovDpA { dp } => self.apu.write(self.dp_addr(dp), self.apu.spc700.a),
            Store::MovDpY { dp } => self.apu.write(self.dp_addr(dp), self.apu.spc700.y),
            Store::MovIdyA { dp } => {
                let addr = self.read_dp_word(dp).wrapping_add(self.apu.spc700.y as u16);
                self.apu.write(addr, self.apu.spc700.a);
            }
            other => todo!("Store instruction not implemented: {other:?}"),
        }
    }

    fn move_reg(&mut self, op: MoveReg) {
        match op {
            MoveReg::MovAX => {
                self.apu.spc700.a = self.apu.spc700.x;
                self.set_nz8(self.apu.spc700.a);
            }
            MoveReg::MovXA => {
                self.apu.spc700.x = self.apu.spc700.a;
                self.set_nz8(self.apu.spc700.x);
            }
            MoveReg::MovAY => {
                self.apu.spc700.a = self.apu.spc700.y;
                self.set_nz8(self.apu.spc700.a);
            }
            MoveReg::MovYA => {
                self.apu.spc700.y = self.apu.spc700.a;
                self.set_nz8(self.apu.spc700.y);
            }
            MoveReg::MovSpX => self.apu.spc700.sp = self.apu.spc700.x,
            MoveReg::MovXSp => {
                self.apu.spc700.x = self.apu.spc700.sp;
                self.set_nz8(self.apu.spc700.x);
            }
        }
    }

    fn move_mem(&mut self, op: MoveMem) {
        match op {
            MoveMem::MovDpImm { imm, dp } => {
                self.apu.write(self.dp_addr(dp), imm);
            }
            other => todo!("MoveMem instruction not implemented: {other:?}"),
        }
    }

    fn reg_step(&mut self, op: RegStep) {
        match op {
            RegStep::IncA => {
                self.apu.spc700.a = self.apu.spc700.a.wrapping_add(1);
                self.set_nz8(self.apu.spc700.a);
            }
            RegStep::DecA => {
                self.apu.spc700.a = self.apu.spc700.a.wrapping_sub(1);
                self.set_nz8(self.apu.spc700.a);
            }
            RegStep::IncX => {
                self.apu.spc700.x = self.apu.spc700.x.wrapping_add(1);
                self.set_nz8(self.apu.spc700.x);
            }
            RegStep::DecX => {
                self.apu.spc700.x = self.apu.spc700.x.wrapping_sub(1);
                self.set_nz8(self.apu.spc700.x);
            }
            RegStep::IncY => {
                self.apu.spc700.y = self.apu.spc700.y.wrapping_add(1);
                self.set_nz8(self.apu.spc700.y);
            }
            RegStep::DecY => {
                self.apu.spc700.y = self.apu.spc700.y.wrapping_sub(1);
                self.set_nz8(self.apu.spc700.y);
            }
        }
    }

    fn alu(&mut self, op: Alu) {
        match op {
            Alu::CmpDpImm { imm, dp } => {
                let value = self.apu.read(self.dp_addr(dp));
                let result = value.wrapping_sub(imm);
                self.set_nz8(result);
                self.apu.spc700.psw.set_carry(value >= imm);
            }
            Alu::CmpYDp { dp } => {
                let value = self.apu.read(self.dp_addr(dp));
                let y = self.apu.spc700.y;
                self.set_nz8(y.wrapping_sub(value));
                self.apu.spc700.psw.set_carry(y >= value);
            }
            other => todo!("ALU instruction not implemented: {other:?}"),
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
                self.write_dp_word(dp, self.apu.spc700.ya());
            }
            other => todo!("Word instruction not implemented: {other:?}"),
        }
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
            self.next_pc = self.next_pc.wrapping_add(disp as u16);
        }
    }

    fn jmp_iax(&mut self, addr: u16) {
        let ptr = addr.wrapping_add(self.apu.spc700.x as u16);
        let target = self.apu.read(ptr) as u16 | ((self.apu.read(ptr.wrapping_add(1)) as u16) << 8);
        self.next_pc = target;
    }

    fn mem_step(&mut self, op: MemStep) {
        let (addr, delta) = match op {
            MemStep::IncDp { dp } => (self.dp_addr(dp), 1),
            MemStep::IncDpx { dp } => (self.dp_addr(dp.wrapping_add(self.apu.spc700.x)), 1),
            MemStep::IncAbs { addr } => (addr, 1),
            MemStep::DecDp { dp } => (self.dp_addr(dp), -1),
            MemStep::DecDpx { dp } => (self.dp_addr(dp.wrapping_add(self.apu.spc700.x)), -1),
            MemStep::DecAbs { addr } => (addr, -1),
        };

        let value = self.apu.read(addr).wrapping_add_signed(delta);
        self.apu.write(addr, value);
        self.set_nz8(value);
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
}

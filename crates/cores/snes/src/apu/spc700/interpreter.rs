use super::decoder::{Instruction, Load};
use crate::apu::Apu;
use crate::apu::spc700::decoder::{Alu, Branches, MoveMem, MoveReg, RegStep, Store};

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

    fn set_nz(&mut self, value: u8) {
        let psw = &mut self.apu.spc700.psw;
        psw.set_negative(value & 0x80 != 0);
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

    pub(super) fn exec(&mut self, inst: Instruction) {
        match inst {
            Instruction::Load(op) => self.load(op),
            Instruction::Store(op) => self.store(op),
            Instruction::MoveReg(op) => self.move_reg(op),
            Instruction::MoveMem(op) => self.move_mem(op),
            Instruction::RegStep(op) => self.reg_step(op),
            Instruction::Alu(op) => self.alu(op),
            Instruction::Branches(op) => self.branches(op),
            other => todo!("Instruction not implemented: {other:?}"),
        }
    }

    fn load(&mut self, op: Load) {
        match op {
            Load::MovXImm { imm } => {
                self.apu.spc700.x = imm;
                self.set_nz(imm);
            }
            Load::MovAImm { imm } => {
                self.apu.spc700.a = imm;
                self.set_nz(imm);
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
            other => todo!("Store instruction not implemented: {other:?}"),
        }
    }

    fn move_reg(&mut self, op: MoveReg) {
        match op {
            MoveReg::MovAX => {
                self.apu.spc700.a = self.apu.spc700.x;
                self.set_nz(self.apu.spc700.a);
            }
            MoveReg::MovXA => {
                self.apu.spc700.x = self.apu.spc700.a;
                self.set_nz(self.apu.spc700.x);
            }
            MoveReg::MovAY => {
                self.apu.spc700.a = self.apu.spc700.y;
                self.set_nz(self.apu.spc700.a);
            }
            MoveReg::MovYA => {
                self.apu.spc700.y = self.apu.spc700.a;
                self.set_nz(self.apu.spc700.y);
            }
            MoveReg::MovSpX => self.apu.spc700.sp = self.apu.spc700.x,
            MoveReg::MovXSp => {
                self.apu.spc700.x = self.apu.spc700.sp;
                self.set_nz(self.apu.spc700.x);
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
                self.set_nz(self.apu.spc700.a);
            }
            RegStep::DecA => {
                self.apu.spc700.a = self.apu.spc700.a.wrapping_sub(1);
                self.set_nz(self.apu.spc700.a);
            }
            RegStep::IncX => {
                self.apu.spc700.x = self.apu.spc700.x.wrapping_add(1);
                self.set_nz(self.apu.spc700.x);
            }
            RegStep::DecX => {
                self.apu.spc700.x = self.apu.spc700.x.wrapping_sub(1);
                self.set_nz(self.apu.spc700.x);
            }
            RegStep::IncY => {
                self.apu.spc700.y = self.apu.spc700.y.wrapping_add(1);
                self.set_nz(self.apu.spc700.y);
            }
            RegStep::DecY => {
                self.apu.spc700.y = self.apu.spc700.y.wrapping_sub(1);
                self.set_nz(self.apu.spc700.y);
            }
        }
    }

    fn alu(&mut self, op: Alu) {
        match op {
            Alu::CmpDpImm { imm, dp } => {
                let value = self.apu.read(self.dp_addr(dp));
                let result = value.wrapping_sub(imm);
                self.set_nz(result);
                self.apu.spc700.psw.set_carry(value >= imm);
            }
            other => todo!("ALU instruction not implemented: {other:?}"),
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
}

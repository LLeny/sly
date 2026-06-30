use super::decoder::{Instruction, Load};
use crate::apu::Apu;

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

    pub(super) fn exec(&mut self, inst: Instruction) {
        match inst {
            Instruction::Load(op) => self.load(op),
            other => todo!("Instruction not implemented: {other:?}"),
        }
    }

    fn load(&mut self, op: Load) {
        match op {
            Load::MovXImm { imm } => {
                self.apu.spc700.x = imm;
                let psw = &mut self.apu.spc700.psw;
                psw.set_negative(imm & 0x80 != 0);
                psw.set_zero(imm == 0);
            }
            other => todo!("Load instruction not implemented: {other:?}"),
        }
    }
}

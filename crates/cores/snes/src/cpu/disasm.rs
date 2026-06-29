use super::decoder;
use crate::Snes;
use crate::cpu::CodeReader;
use decoder::{Branches, Call, Instruction, Jump};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstructionClass {
    ControlFlow,
    Memory,
    Arithmetic,
    Transfer,
    System,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disasm {
    pub text: String,
    pub length: u8,
    pub target: Option<u32>,
    pub class: InstructionClass,
}

impl std::fmt::Display for Disasm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.text)
    }
}

#[inline]
pub fn disassemble_at(snes: &Snes, pc: u16) -> Disasm {
    let reader = CodeReader { snes };
    let addr = pc as u64;
    let modes = decoder::pack_modes(snes.cpu.flags.m() as u64, snes.cpu.flags.x() as u64);

    let (inst, length) = decoder::decode(modes, addr, &reader);
    let text = inst.render(addr, &reader);
    let target = control_target(&inst, pc, length, snes.cpu.program_bank);
    let class = instruction_class(&inst);

    Disasm {
        text,
        length,
        target,
        class,
    }
}

fn instruction_class(inst: &Instruction) -> InstructionClass {
    use InstructionClass::*;
    match inst {
        Instruction::Branches(_)
        | Instruction::BrlRell { .. }
        | Instruction::Jump(_)
        | Instruction::Call(_)
        | Instruction::Ret(_) => ControlFlow,
        Instruction::Load(_)
        | Instruction::Store(_)
        | Instruction::Push(_)
        | Instruction::Pop(_)
        | Instruction::BlockMove(_) => Memory,
        Instruction::Alu(_)
        | Instruction::Compare(_)
        | Instruction::BitTest(_)
        | Instruction::Shift(_)
        | Instruction::MemStep(_)
        | Instruction::BitModify(_)
        | Instruction::RegStep(_) => Arithmetic,
        Instruction::Transfer(_) | Instruction::Xba => Transfer,
        Instruction::FlagOps(_)
        | Instruction::FlagMask(_)
        | Instruction::SwTrap(_)
        | Instruction::Halt(_)
        | Instruction::Nop
        | Instruction::WdmImm { .. }
        | Instruction::Xce => System,
        Instruction::Invalid { .. } => Other,
    }
}

fn control_target(inst: &Instruction, pc: u16, length: u8, bank: u8) -> Option<u32> {
    let next = pc.wrapping_add(length as u16);

    let same_bank = |full: u32| ((full >> 16) as u8 == bank).then_some(full & 0xFFFF);

    match inst {
        Instruction::Branches(b) => {
            let disp = match b {
                Branches::BraRel { disp }
                | Branches::BeqRel { disp }
                | Branches::BneRel { disp }
                | Branches::BcsRel { disp }
                | Branches::BccRel { disp }
                | Branches::BmiRel { disp }
                | Branches::BplRel { disp }
                | Branches::BvsRel { disp }
                | Branches::BvcRel { disp } => *disp,
            };
            Some(next.wrapping_add(disp as i16 as u16) as u32)
        }
        Instruction::BrlRell { disp } => Some(next.wrapping_add(*disp as u16) as u32),
        Instruction::Jump(j) => match j {
            Jump::JmpAbs { addr } => Some(*addr as u32),
            Jump::JmpAbl { addr } => same_bank(*addr),
            Jump::JmpInd { .. } | Jump::JmpIax { .. } | Jump::JmlIal { .. } => None,
        },
        Instruction::Call(c) => match c {
            Call::JsrAbs { addr } => Some(*addr as u32),
            Call::JslAbl { addr } => same_bank(*addr),
            Call::JsrIax { .. } => None,
        },
        _ => None,
    }
}

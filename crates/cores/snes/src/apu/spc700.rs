pub mod regs;

mod decoder;
mod interpreter;

pub use regs::{Flags, Spc700};

use crate::apu::Apu;
use crate::cpu::disasm::FlowKind;
use interpreter::Interpreter;

struct Reader<'a> {
    apu: &'a Apu,
}

impl decoder::DisasmCtx for Reader<'_> {
    fn read_u8(&self, addr: u64) -> u8 {
        self.apu.peek(addr as u16)
    }
}

pub fn step(apu: &mut Apu) {
    let pc = apu.spc700.pc;
    let (inst, len) = {
        let reader = Reader { apu: &*apu };
        decoder::decode(pc as u64, &reader)
    };

    let mut view = Interpreter::new(apu, len);
    view.exec(inst);
    apu.spc700.pc = view.next_pc();
}

pub fn disassemble(apu: &Apu, pc: u16) -> (String, u8, Option<u16>, FlowKind) {
    let reader = Reader { apu };
    let (inst, len) = decoder::decode(pc as u64, &reader);
    let text = inst.render(pc as u64, &reader);
    let target = control_target(&inst, pc, len);
    let flow = flow_kind(&inst);

    (text, len, target, flow)
}

fn flow_kind(inst: &decoder::Instruction) -> FlowKind {
    use decoder::{Branches, Halt, Instruction};

    match inst {
        Instruction::Branches(Branches::BraRel { .. }) => FlowKind::Branch,
        Instruction::Branches(_) => FlowKind::CondBranch,
        Instruction::BitBranch(_) => FlowKind::CondBranch,
        Instruction::Jump(_) => FlowKind::Jump,
        Instruction::Call(_) => FlowKind::Call,
        Instruction::Returns(_) => FlowKind::Return,
        Instruction::Halt(Halt::Stop) => FlowKind::Return,
        _ => FlowKind::None,
    }
}

fn control_target(inst: &decoder::Instruction, pc: u16, len: u8) -> Option<u16> {
    use decoder::{BitBranch, Branches, Call, Instruction, Jump};

    let next = pc.wrapping_add(len as u16);
    let rel = |disp: i8| next.wrapping_add(disp as i16 as u16);

    match inst {
        Instruction::Branches(b) => {
            let disp = match b {
                Branches::BraRel { disp }
                | Branches::BeqRel { disp }
                | Branches::BneRel { disp }
                | Branches::BcsRel { disp }
                | Branches::BccRel { disp }
                | Branches::BvsRel { disp }
                | Branches::BvcRel { disp }
                | Branches::BmiRel { disp }
                | Branches::BplRel { disp } => *disp,
            };
            Some(rel(disp))
        }
        Instruction::BitBranch(b) => {
            let disp = match b {
                BitBranch::BbsB0 { disp, .. }
                | BitBranch::BbsB1 { disp, .. }
                | BitBranch::BbsB2 { disp, .. }
                | BitBranch::BbsB3 { disp, .. }
                | BitBranch::BbsB4 { disp, .. }
                | BitBranch::BbsB5 { disp, .. }
                | BitBranch::BbsB6 { disp, .. }
                | BitBranch::BbsB7 { disp, .. }
                | BitBranch::BbcB0 { disp, .. }
                | BitBranch::BbcB1 { disp, .. }
                | BitBranch::BbcB2 { disp, .. }
                | BitBranch::BbcB3 { disp, .. }
                | BitBranch::BbcB4 { disp, .. }
                | BitBranch::BbcB5 { disp, .. }
                | BitBranch::BbcB6 { disp, .. }
                | BitBranch::BbcB7 { disp, .. }
                | BitBranch::CbneDp { disp, .. }
                | BitBranch::CbneDpx { disp, .. }
                | BitBranch::DbnzDp { disp, .. }
                | BitBranch::DbnzY { disp } => *disp,
            };
            Some(rel(disp))
        }
        Instruction::Jump(Jump::JmpAbs { addr }) => Some(*addr),
        Instruction::Call(Call::CallAbs { addr }) => Some(*addr),
        Instruction::Call(Call::Pcall { u }) => Some(0xFF00 | *u as u16),
        _ => None,
    }
}

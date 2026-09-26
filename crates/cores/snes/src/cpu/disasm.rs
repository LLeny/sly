use super::decoder::{self, DisasmCtx, Form, Instruction, Mnemonic};
use super::operand::{Arg, fetch_arg};
use crate::Snes;
use crate::cpu::{CodeReader, CpuPeek};
use crate::mmio::map::MemoryMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstructionClass {
    ControlFlow,
    Memory,
    Arithmetic,
    Transfer,
    System,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlowKind {
    #[default]
    None,
    CondBranch,
    Branch,
    Jump,
    Call,
    Return,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disasm {
    pub text: String,
    pub length: u8,
    /// Full 24-bit address.
    pub target: Option<u32>,
    pub class: InstructionClass,
    pub flow: FlowKind,
}

impl std::fmt::Display for Disasm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.text)
    }
}

#[inline]
pub fn disassemble_at<M: MemoryMap>(snes: &Snes<M>, pc: u16) -> Disasm {
    self::disassemble(&snes.cpu, &snes.bus, pc)
}

pub(crate) fn disassemble<B: CpuPeek>(cpu: &crate::Cpu, bus: &B, pc: u16) -> Disasm {
    let reader = CodeReader {
        cpu,
        bus,
        bank: cpu.program_bank,
    };
    let addr = pc as u64;
    let combo = super::mode_selectors(cpu);

    let (text, length) = decoder::disasm_ctx(addr, &reader);
    let super::Decoded {
        inst,
        id,
        mnemonic,
        form,
    } = super::decode(reader.read_u8(addr), combo);

    let flow = self::flow_kind(mnemonic);
    let class = self::instruction_class(id, flow, mnemonic);
    let target = self::control_target(inst, mnemonic, form, pc, length, &reader);

    Disasm {
        text,
        length,
        target,
        class,
        flow,
    }
}

fn flow_kind(m: Mnemonic) -> FlowKind {
    use Mnemonic as M;

    match m {
        M::Bra | M::Brl => FlowKind::Branch,
        M::Beq | M::Bne | M::Bcc | M::Bcs | M::Bmi | M::Bpl | M::Bvc | M::Bvs => {
            FlowKind::CondBranch
        }
        M::Jmp | M::Jml => FlowKind::Jump,
        M::Jsr | M::Jsl => FlowKind::Call,
        M::Rts | M::Rtl | M::Rti | M::Stp => FlowKind::Return,
        _ => FlowKind::None,
    }
}

fn instruction_class(id: usize, flow: FlowKind, m: Mnemonic) -> InstructionClass {
    use InstructionClass::*;

    if flow != FlowKind::None && m != Mnemonic::Stp {
        return ControlFlow;
    }

    if m == Mnemonic::Invalid {
        return Other;
    }

    for tag in decoder::OPCODE_TAGS[id] {
        let class = match *tag {
            "load" | "store" | "push" | "pop" | "block_move" => Memory,
            "alu" | "compare" | "shift" | "mem_step" | "bit_modify" | "reg_step" => Arithmetic,
            "transfer" => Transfer,
            "branches" | "jump" | "call" | "ret" => ControlFlow,
            _ => continue,
        };

        return class;
    }

    match m {
        Mnemonic::Bit => Arithmetic,
        Mnemonic::Xba => Transfer,
        _ => System,
    }
}

fn control_target<B: CpuPeek>(
    inst: Instruction,
    m: Mnemonic,
    form: Form,
    pc: u16,
    length: u8,
    reader: &CodeReader<B>,
) -> Option<u32> {
    use Mnemonic as M;

    let next = pc.wrapping_add(length as u16);
    let bank = reader.bank;

    let in_bank = |offset: u16| (u32::from(bank) << 16) | u32::from(offset);

    match (m, form, fetch_arg(inst, m, form, pc as u64, reader)) {
        (
            M::Bra | M::Brl | M::Beq | M::Bne | M::Bcc | M::Bcs | M::Bmi | M::Bpl | M::Bvc | M::Bvs,
            _,
            Arg::Rel(disp),
        ) => Some(in_bank(next.wrapping_add(disp as u16))),
        (M::Jmp | M::Jsr, Form::Abs, Arg::Abs(a)) => Some(in_bank(a)),
        (M::Jmp | M::Jml | M::Jsl, Form::Abl, Arg::Long(a)) => Some(a),
        _ => None,
    }
}

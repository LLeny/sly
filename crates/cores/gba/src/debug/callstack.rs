use std::collections::VecDeque;

use debugger_model::{CallStackView, StackFrame};

use crate::Cpu;
use crate::cpu::decoder::{arm, thumb};

const MAX_FRAMES: usize = 512;
const VISIBLE_FRAMES: usize = 64;

pub(super) struct Before {
    pc: u32,
    instruction: u32,
    psr: u32,
    sp: u32,
    lr: u32,
}

impl Before {
    pub fn new(cpu: &Cpu) -> Option<Self> {
        let (pc, instruction) = cpu.pipeline.pending()[0]?;
        Some(Self {
            pc,
            instruction,
            psr: cpu.cpsr.raw(),
            sp: cpu.sp(),
            lr: cpu.lr(),
        })
    }
}

struct Frame {
    caller: u32,
    target: u32,
    return_address: u32,
    psr: u32,
    sp: u32,
    kind: &'static str,
}

#[derive(Default)]
pub(super) struct Stack {
    frames: VecDeque<Frame>,
}

impl Stack {
    pub fn clear(&mut self) {
        self.frames.clear();
    }

    pub fn observe(&mut self, before: Before, cpu: &Cpu) {
        if !cpu.pipeline.is_empty() {
            return;
        }

        let target = cpu.pc();
        let thumb_mode = before.psr & 0x20 != 0;
        let (call, exception, kind) = if thumb_mode {
            let mnemonic = thumb::Instruction(before.instruction as u16).mnemonic();
            (
                mnemonic == thumb::Mnemonic::Bll,
                matches!(mnemonic, thumb::Mnemonic::Swi | thumb::Mnemonic::Invalid),
                "BL Thumb",
            )
        } else {
            let inst = arm::Instruction(before.instruction);
            let mnemonic = inst.mnemonic();
            let direct = mnemonic == arm::Mnemonic::B && inst.link();
            let indirect = mnemonic == arm::Mnemonic::Bx
                && inst.rm() != 14
                && before.lr == before.pc.wrapping_add(4);
            (
                direct || indirect,
                matches!(mnemonic, arm::Mnemonic::Swi | arm::Mnemonic::Invalid),
                if indirect { "BX call" } else { "BL ARM" },
            )
        };

        if call || exception {
            if self.frames.len() == MAX_FRAMES {
                self.frames.pop_front();
            }
            self.frames.push_back(Frame {
                caller: before.pc,
                target,
                return_address: before.pc.wrapping_add(if thumb_mode { 2 } else { 4 }),
                psr: before.psr,
                sp: before.sp,
                kind: if exception {
                    if target == 8 { "SWI" } else { "UND" }
                } else {
                    kind
                },
            });
        } else if let Some(index) = self.frames.iter().rposition(|frame| {
            frame.return_address == target
                && (frame.psr & 0x3F) == (cpu.cpsr.raw() & 0x3F)
                && cpu.sp() >= frame.sp
        }) {
            self.frames.truncate(index);
        }
    }

    pub fn view(&self, pc: u32) -> CallStackView {
        let mut frames = vec![StackFrame {
            kind: "PC".into(),
            location: format!("{pc:08X}").into(),
            symbol: "".into(),
            caller: "".into(),
        }];
        frames.extend(self.frames.iter().rev().take(VISIBLE_FRAMES).map(|frame| {
            StackFrame {
                kind: frame.kind.into(),
                location: format!("{:08X}", frame.target).into(),
                symbol: "".into(),
                caller: format!(
                    "from {:08X}, return {:08X}",
                    frame.caller, frame.return_address
                )
                .into(),
            }
        }));

        let older = self.frames.len().saturating_sub(VISIBLE_FRAMES);
        if older > 0 {
            frames.push(StackFrame {
                kind: "...".into(),
                location: "".into(),
                symbol: format!("{older} older observed frames").into(),
                caller: "".into(),
            });
        }

        CallStackView { frames }
    }
}

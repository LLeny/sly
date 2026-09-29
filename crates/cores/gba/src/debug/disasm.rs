use debugger_model::{
    DisasmLine, DisassemblyRequest, FlowKind, SyntaxSpec, TokenKind, format_bytes, tokenize,
};

use crate::Gba;
use crate::bus::BusView;
use crate::cpu::decoder::{arm, thumb};

const SYNTAX: SyntaxSpec = SyntaxSpec::new(
    &[
        "r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "r9", "r10", "r11", "r12", "r13",
        "r14", "r15", "sp", "lr", "pc", "cpsr", "spsr",
    ],
    |_| false,
)
.with_identifier_kind(identifier_kind);

struct Decoded {
    raw: u32,
    text: String,
    kind: TokenKind,
    flow: FlowKind,
    conditional: bool,
    target: Option<u32>,
    note: Option<String>,
}

pub(super) fn view(core: &Gba, request: &DisassemblyRequest) -> Vec<DisasmLine> {
    let thumb_mode = match request.mode.as_deref() {
        None => core.cpu.is_thumb(),
        Some("arm") => false,
        Some("thumb") => true,
        _ => return Vec::new(),
    };

    let length: u8 = if thumb_mode { 2 } else { 4 };
    let bus = core.bus_view();
    let mut addr = request.base & !(u32::from(length) - 1);
    let mut lines = Vec::with_capacity(request.count);

    for _ in 0..request.count {
        let decoded = if thumb_mode {
            self::decode_thumb(core, &bus, addr)
        } else {
            self::decode_arm(core, &bus, addr)
        };

        let mnemonic = if decoded.flow == FlowKind::None {
            decoded.kind
        } else {
            TokenKind::MnemonicFlow
        };
        let tokens = tokenize(&decoded.text, mnemonic, &SYNTAX);

        let raw = decoded.raw.to_le_bytes();
        let bytes = format_bytes(length, |i| raw[usize::from(i)]);

        let note = decoded
            .note
            .or_else(|| decoded.target.map(|target| format!("→ {target:08X}")))
            .unwrap_or_default();

        lines.push(
            DisasmLine::new(
                addr,
                length,
                format!("{addr:08X}"),
                bytes,
                decoded.text,
                tokens,
            )
            .flow(decoded.flow)
            .conditional(decoded.conditional)
            .target(decoded.target)
            .note(note),
        );

        let Some(next) = addr.checked_add(u32::from(length)) else {
            break;
        };
        addr = next;
    }

    lines
}

fn decode_thumb(core: &Gba, bus: &BusView, addr: u32) -> Decoded {
    use thumb::Mnemonic::*;

    let inst = thumb::Instruction(bus.peek16(addr));
    let mnemonic = inst.mnemonic();
    let conditional = mnemonic == B && inst.0 & 0xF000 == 0xD000;

    let (flow, target) = match mnemonic {
        B => {
            let offset = if conditional {
                inst.cond_offset()
            } else {
                inst.offset()
            } as i32;
            let flow = if conditional {
                FlowKind::Branch
            } else {
                FlowKind::Jump
            };

            (flow, Some(addr.wrapping_add(4).wrapping_add_signed(offset)))
        }
        Bx => (
            self::jump_or_return(inst.high_src() == 14),
            self::exchange_target(core, addr, true, inst.high_src()),
        ),
        Pop if inst.extra() => (FlowKind::Return, None),
        Bll => {
            let prefix = thumb::Instruction(bus.peek16(addr.wrapping_sub(2)));
            let target = (prefix.mnemonic() == Blh).then(|| {
                addr.wrapping_add(2)
                    .wrapping_add_signed(prefix.prefix() as i32)
                    .wrapping_add(inst.suffix() as u32)
            });

            (FlowKind::Call, target)
        }
        Mov | Add if inst.0 & 0xFC00 == 0x4400 && inst.high_dest() == 15 => (
            self::jump_or_return(mnemonic == Mov && inst.high_src() == 14),
            None,
        ),
        Swi => (FlowKind::Call, Some(8)),
        _ => (FlowKind::None, None),
    };

    Decoded {
        raw: u32::from(inst.0),
        text: inst.to_string(),
        kind: self::thumb_kind(mnemonic),
        flow,
        conditional,
        target,
        note: self::thumb_note(bus, addr, &inst),
    }
}

fn decode_arm(core: &Gba, bus: &BusView, addr: u32) -> Decoded {
    use arm::Mnemonic::*;

    let inst = arm::Instruction(bus.peek32(addr));
    let mnemonic = inst.mnemonic();
    let conditional = inst.cond() != 14;

    let (flow, target) = match mnemonic {
        B => {
            let flow = if inst.link() {
                FlowKind::Call
            } else if conditional {
                FlowKind::Branch
            } else {
                FlowKind::Jump
            };

            (
                flow,
                Some(
                    addr.wrapping_add(8)
                        .wrapping_add_signed(inst.offset() as i32),
                ),
            )
        }
        Bx => (
            self::jump_or_return(inst.rm() == 14),
            self::exchange_target(core, addr, false, inst.rm()),
        ),
        Swi => (FlowKind::Call, Some(8)),
        Ldm if inst.0 & 0x8000 != 0 => (self::jump_or_return(inst.rn() == 13), None),
        Ldr | Ldrh | Ldrsb | Ldrsh if inst.rd() == 15 => (FlowKind::Jump, None),
        And | Eor | Sub | Rsb | Add | Adc | Sbc | Rsc | Orr | Mov | Bic | Mvn
            if inst.rd() == 15 =>
        {
            let returns = mnemonic == Mov && inst.0 & 0x0200_0FF0 == 0 && inst.rm() == 14;
            (self::jump_or_return(returns), None)
        }
        _ => (FlowKind::None, None),
    };

    Decoded {
        raw: inst.0,
        text: inst.to_string(),
        kind: self::arm_kind(mnemonic),
        flow,
        conditional,
        target,
        note: self::arm_note(bus, addr, &inst),
    }
}

fn jump_or_return(returns: bool) -> FlowKind {
    if returns {
        FlowKind::Return
    } else {
        FlowKind::Jump
    }
}

fn arm_kind(mnemonic: arm::Mnemonic) -> TokenKind {
    use arm::Mnemonic::*;

    match mnemonic {
        B | Bx => TokenKind::MnemonicFlow,
        Ldr | Str | Ldrh | Strh | Ldrsb | Ldrsh | Ldm | Stm | Swp => TokenKind::MnemonicMemory,
        Mov => TokenKind::MnemonicTransfer,
        Mrs | Msr | Swi => TokenKind::MnemonicSystem,
        Invalid => TokenKind::Plain,
        _ => TokenKind::MnemonicAlu,
    }
}

fn thumb_kind(mnemonic: thumb::Mnemonic) -> TokenKind {
    use thumb::Mnemonic::*;

    match mnemonic {
        B | Bx | Blh | Bll => TokenKind::MnemonicFlow,
        Ldr | Str | Ldrb | Strb | Ldrh | Strh | Ldrsb | Ldrsh | Ldmia | Stmia | Push | Pop => {
            TokenKind::MnemonicMemory
        }
        Mov => TokenKind::MnemonicTransfer,
        Swi => TokenKind::MnemonicSystem,
        Invalid => TokenKind::Plain,
        _ => TokenKind::MnemonicAlu,
    }
}

fn identifier_kind(text: &str) -> Option<TokenKind> {
    let psr_fields = text.split_once('_').is_some_and(|(name, fields)| {
        (name.eq_ignore_ascii_case("cpsr") || name.eq_ignore_ascii_case("spsr"))
            && !fields.is_empty()
            && fields
                .bytes()
                .all(|field| matches!(field.to_ascii_lowercase(), b'f' | b's' | b'x' | b'c'))
    });
    if psr_fields {
        return Some(TokenKind::Register);
    }

    ["lsl", "lsr", "asr", "ror", "rrx"]
        .iter()
        .any(|shift| shift.eq_ignore_ascii_case(text))
        .then_some(TokenKind::MnemonicAlu)
}

fn exchange_target(core: &Gba, addr: u32, thumb_mode: bool, register: u8) -> Option<u32> {
    if addr != super::cpu::execution_address(&core.cpu) || thumb_mode != core.cpu.is_thumb() {
        return None;
    }

    let value = if register == 15 {
        addr.wrapping_add(if thumb_mode { 4 } else { 8 })
    } else {
        core.cpu.registers[register as usize]
    };

    Some(value & if value & 1 != 0 { !1 } else { !3 })
}

fn thumb_note(bus: &BusView, addr: u32, inst: &thumb::Instruction) -> Option<String> {
    let base = addr.wrapping_add(4) & !3;

    match (inst.mnemonic(), inst.form()) {
        (thumb::Mnemonic::Ldr, thumb::Form::Literal) => {
            let address = base.wrapping_add(inst.scaled_offset() as u32);
            Some(format!("[{address:08X}] = {:08X}", bus.peek32(address)))
        }
        (thumb::Mnemonic::Add, thumb::Form::Pc) => {
            let address = base.wrapping_add(inst.scaled_offset() as u32);
            Some(format!("→ {address:08X}"))
        }
        _ => None,
    }
}

fn arm_note(bus: &BusView, addr: u32, inst: &arm::Instruction) -> Option<String> {
    if inst.rn() != 15 {
        return None;
    }

    let base = addr.wrapping_add(8);

    match (inst.mnemonic(), inst.form()) {
        (arm::Mnemonic::Add | arm::Mnemonic::Sub, arm::Form::Imm) => {
            let offset = u32::from(inst.imm8()).rotate_right(u32::from(inst.rotate()) * 2);
            let address = if inst.mnemonic() == arm::Mnemonic::Add {
                base.wrapping_add(offset)
            } else {
                base.wrapping_sub(offset)
            };

            Some(format!("→ {address:08X}"))
        }
        (arm::Mnemonic::Ldr | arm::Mnemonic::Str, arm::Form::MemImm) => {
            let offset = u32::from(inst.off12());
            let address = if !inst.pre() {
                base
            } else if inst.up() {
                base.wrapping_add(offset)
            } else {
                base.wrapping_sub(offset)
            };

            if inst.mnemonic() != arm::Mnemonic::Ldr {
                return Some(format!("→ {address:08X}"));
            }

            let value = if inst.byte() {
                u32::from(bus.peek8(address))
            } else {
                bus.peek32(address & !3).rotate_right((address & 3) * 8)
            };

            Some(format!("[{address:08X}] = {value:08X}"))
        }
        _ => None,
    }
}

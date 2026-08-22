use super::decoder::{Form, Instruction, Mnemonic};
use super::{CodeReader, CpuPeek};

#[derive(Clone, Copy)]
pub(crate) enum Arg {
    None,
    Imm(u16),
    Dp(u8),
    Sr(u8),
    Abs(u16),
    Long(u32),
    Rel(i16),
    Move { dst: u8, src: u8 },
}

impl Arg {
    pub(crate) fn abs(self) -> u16 {
        match self {
            Arg::Abs(a) => a,
            _ => unreachable!("expected an absolute operand"),
        }
    }
}

pub(crate) fn fetch_arg<B: CpuPeek>(
    inst: Instruction,
    mnemonic: Mnemonic,
    form: Form,
    pc: u64,
    r: &CodeReader<B>,
) -> Arg {
    use Form as F;
    use Mnemonic as M;

    match mnemonic {
        M::RepImm | M::SepImm => return Arg::Imm(inst.mask(pc, r) as u16),
        M::BrkImm | M::CopImm | M::WdmImm => return Arg::Imm(inst.sig(pc, r) as u16),
        _ => {}
    }

    match form {
        F::None | F::A => Arg::None,
        F::Imm => Arg::Imm(inst.imm_u16_m(pc, r)),
        F::Immx => Arg::Imm(inst.imm_u16_x(pc, r)),
        F::Dp | F::Dpx | F::Dpy | F::Idp | F::Idx | F::Idy | F::Idl | F::Idly => {
            Arg::Dp(inst.dp(pc, r))
        }
        F::Sr | F::Isy => Arg::Sr(inst.sr(pc, r)),
        F::Abs | F::Abx | F::Aby | F::Ind | F::Iax | F::Ial => Arg::Abs(inst.addr_word16(pc, r)),
        F::Abl | F::Alx => Arg::Long(inst.addr_long24(pc, r)),
        F::Rel => Arg::Rel(inst.disp_rel8(pc, r) as i16),
        F::Rell => Arg::Rel(inst.disp_rel16(pc, r)),
        F::Bm => Arg::Move {
            dst: inst.dst(pc, r),
            src: inst.src(pc, r),
        },
    }
}

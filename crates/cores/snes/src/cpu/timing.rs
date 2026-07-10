use super::addressing;
use super::decoder::{Form, Mnemonic};
use super::operand::Arg;
use crate::Snes;
use crate::cpu::Cpu;
use crate::mmio::MemoryMap;

pub(super) fn cycles<M: MemoryMap>(
    snes: &Snes<M>,
    mnemonic: Mnemonic,
    form: Form,
    arg: Arg,
) -> u64 {
    use Form as F;
    use Mnemonic as M;

    let cpu = &snes.cpu;
    let m_wide = cpu.acc_wide() as u64;
    let x_wide = cpu.index_wide() as u64;
    let native = !cpu.emulation as u64;
    let dl = (cpu.direct_page & 0x00ff != 0) as u64;

    match mnemonic {
        M::Lda | M::Adc | M::Sbc | M::And | M::Ora | M::Eor | M::Cmp | M::Bit => {
            self::read(snes, form, arg, m_wide, dl)
        }
        M::Ldx | M::Ldy | M::Cpx | M::Cpy => self::read(snes, form, arg, x_wide, dl),

        M::Sta | M::Stz => self::store(form, m_wide, dl),
        M::Stx | M::Sty => self::store(form, x_wide, dl),

        M::Asl | M::Lsr | M::Rol | M::Ror | M::Inc | M::Dec | M::Tsb | M::Trb => match form {
            F::A => 2,
            F::Dp => 5 + 2 * m_wide + dl,
            F::Dpx => 6 + 2 * m_wide + dl,
            F::Abs => 6 + 2 * m_wide,
            F::Abx => 7 + 2 * m_wide,
            _ => unreachable!("rmw form"),
        },

        M::Bra | M::Beq | M::Bne | M::Bcs | M::Bcc | M::Bmi | M::Bpl | M::Bvs | M::Bvc => {
            self::branch(cpu, mnemonic, arg)
        }
        M::Brl => 4,

        M::Jmp => match form {
            F::Abs => 3,
            F::Abl => 4,
            F::Ind => 5,
            F::Iax => 6,
            _ => unreachable!("jmp form"),
        },
        M::Jml => 6,
        M::Jsr => match form {
            F::Iax => 8,
            _ => 6,
        },
        M::Jsl => 8,
        M::Rts | M::Rtl => 6,
        M::Rti => 6 + native,

        M::Pha => 3 + m_wide,
        M::Phx | M::Phy => 3 + x_wide,
        M::Php | M::Phb | M::Phk => 3,
        M::Phd => 4,
        M::Pla => 4 + m_wide,
        M::Plx | M::Ply => 4 + x_wide,
        M::Plp | M::Plb => 4,
        M::Pld => 5,
        M::Pea => 5,
        M::Pei => 6 + dl,
        M::Per => 6,

        M::RepImm | M::SepImm => 3,
        M::Xba => 3,
        M::BrkImm | M::CopImm => self::interrupt_cycles(cpu),
        M::Stp | M::Wai => 3,

        M::Mvn | M::Mvp => 7,

        _ => 2,
    }
}

pub(super) fn interrupt_cycles(cpu: &Cpu) -> u64 {
    7 + !cpu.emulation as u64
}

fn read<M: MemoryMap>(snes: &Snes<M>, form: Form, arg: Arg, wide: u64, dl: u64) -> u64 {
    use Form as F;

    let cpu = &snes.cpu;

    let base = match form {
        F::Imm | F::Immx => 2,
        F::Dp => 3 + dl,
        F::Dpx | F::Dpy => 4 + dl,
        F::Abs => 4,
        F::Abx => 4 + self::indexed(cpu, arg.abs(), cpu.index_x),
        F::Aby => 4 + self::indexed(cpu, arg.abs(), cpu.index_y),
        F::Abl | F::Alx => 5,
        F::Idp => 5 + dl,
        F::Idy => {
            let Arg::Dp(dp) = arg else {
                unreachable!("idy operand")
            };

            let ptr = snes.peek16(addressing::direct(snes, dp, 0));
            5 + dl + self::indexed(cpu, ptr, cpu.index_y)
        }
        F::Idx | F::Idl | F::Idly => 6 + dl,
        F::Sr => 4,
        F::Isy => 7,
        _ => unreachable!("read form"),
    };

    base + wide
}

fn store(form: Form, wide: u64, dl: u64) -> u64 {
    use Form as F;

    let base = match form {
        F::Dp => 3 + dl,
        F::Dpx | F::Dpy => 4 + dl,
        F::Abs => 4,
        F::Abx | F::Aby | F::Abl | F::Alx => 5,
        F::Idp => 5 + dl,
        F::Idx | F::Idy | F::Idl | F::Idly => 6 + dl,
        F::Sr => 4,
        F::Isy => 7,
        _ => unreachable!("store form"),
    };

    base + wide
}

fn indexed(cpu: &Cpu, base: u16, index: u16) -> u64 {
    if cpu.index_wide() {
        return 1;
    }

    let sum = base as u32 + (index & 0x00ff) as u32;
    (sum >> 8 != (base as u32) >> 8) as u64
}

fn branch(cpu: &Cpu, mnemonic: Mnemonic, arg: Arg) -> u64 {
    if !super::branch_taken(&cpu.flags, mnemonic) {
        return 2;
    }

    let Arg::Rel(disp) = arg else {
        unreachable!("branch form")
    };
    let fallthrough = cpu.pc.wrapping_add(2);
    let target = fallthrough.wrapping_add(disp as u16);
    let crossed = cpu.emulation && target & 0xff00 != fallthrough & 0xff00;

    3 + crossed as u64
}

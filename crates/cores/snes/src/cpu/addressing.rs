use super::decoder::Form;
use super::operand::Arg;
use crate::cpu::{Cpu, CpuBus};
use crate::mmio::Address;

pub(super) fn direct(cpu: &Cpu, dp: u8, index: u16) -> Address {
    let d = cpu.direct_page;
    let sum = (dp as u16).wrapping_add(index);
    let offset = if cpu.emulation && d & 0x00ff == 0 {
        (d & 0xff00) | (sum & 0x00ff)
    } else {
        d.wrapping_add(sum)
    };
    Address::new(0, offset)
}

pub(super) fn stack_relative(cpu: &Cpu, sr: u8) -> Address {
    Address::new(0, cpu.stack_pointer.wrapping_add(sr as u16))
}

pub(super) fn pointer_fetch(cpu: &Cpu, form: Form, arg: Arg) -> Option<(Address, u8)> {
    use Form as F;

    match (form, arg) {
        (F::Idp | F::Idy, Arg::Dp(dp)) => Some((self::direct(cpu, dp, 0), 2)),
        (F::Idx, Arg::Dp(dp)) => Some((self::direct(cpu, dp, cpu.index_x), 2)),
        (F::Idl | F::Idly, Arg::Dp(dp)) => Some((self::direct(cpu, dp, 0), 3)),
        (F::Isy, Arg::Sr(sr)) => Some((self::stack_relative(cpu, sr), 2)),
        _ => None,
    }
}

pub(crate) fn resolve<B: CpuBus>(
    cpu: &Cpu,
    bus: &B,
    form: Form,
    arg: Arg,
) -> Option<(Address, Address)> {
    use Form as F;

    let (dbr, x, y) = (cpu.data_bank, cpu.index_x, cpu.index_y);

    let abs_idx = |addr: u16, idx: u16| {
        Address::from_24bit(Address::new(dbr, addr).as_24bit().wrapping_add(idx as u32))
    };
    let direct = |dp: u8, index: u16| self::direct(cpu, dp, index);
    let dptr16 = |dp: u8, index: u16| bus.peek16(direct(dp, index));
    let dptr24 = |dp: u8| bus.peek24(direct(dp, 0));

    let lo = match (form, arg) {
        (F::Dp, Arg::Dp(dp)) => direct(dp, 0),
        (F::Dpx, Arg::Dp(dp)) => direct(dp, x),
        (F::Dpy, Arg::Dp(dp)) => direct(dp, y),
        (F::Abs, Arg::Abs(a)) => Address::new(dbr, a),
        (F::Abx, Arg::Abs(a)) => abs_idx(a, x),
        (F::Aby, Arg::Abs(a)) => abs_idx(a, y),
        (F::Abl, Arg::Long(a)) => Address::from_24bit(a),
        (F::Alx, Arg::Long(a)) => Address::from_24bit(a.wrapping_add(x as u32)),
        (F::Idp, Arg::Dp(dp)) => Address::new(dbr, dptr16(dp, 0)),
        (F::Idx, Arg::Dp(dp)) => Address::new(dbr, dptr16(dp, x)),
        (F::Idy, Arg::Dp(dp)) => abs_idx(dptr16(dp, 0), y),
        (F::Idl, Arg::Dp(dp)) => Address::from_24bit(dptr24(dp)),
        (F::Idly, Arg::Dp(dp)) => Address::from_24bit(dptr24(dp).wrapping_add(y as u32)),
        (F::Sr, Arg::Sr(sr)) => self::stack_relative(cpu, sr),
        (F::Isy, Arg::Sr(sr)) => abs_idx(bus.peek16(self::stack_relative(cpu, sr)), y),
        _ => return None,
    };

    let hi = if matches!(form, F::Dp | F::Dpx | F::Dpy | F::Sr) {
        lo.wrapping_add(1)
    } else {
        Address::from_24bit(lo.as_24bit().wrapping_add(1))
    };

    Some((lo, hi))
}

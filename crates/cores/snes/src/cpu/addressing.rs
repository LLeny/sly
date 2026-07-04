use super::decoder::Form;
use super::operand::Arg;
use crate::Snes;
use crate::mmio::{Address, MemoryMap};

pub(crate) fn resolve<M: MemoryMap>(
    snes: &Snes<M>,
    form: Form,
    arg: Arg,
) -> Option<(Address, Address)> {
    use Form as F;

    let cpu = &snes.cpu;
    let (dbr, x, y, sp) = (cpu.data_bank, cpu.index_x, cpu.index_y, cpu.stack_pointer);

    let abs_idx = |addr: u16, idx: u16| {
        Address::from_24bit(Address::new(dbr, addr).as_24bit().wrapping_add(idx as u32))
    };
    let direct = |dp: u8, index: u16| {
        let d = snes.cpu.direct_page;
        let sum = (dp as u16).wrapping_add(index);
        let offset = if snes.cpu.emulation && d & 0x00ff == 0 {
            (d & 0xff00) | (sum & 0x00ff)
        } else {
            d.wrapping_add(sum)
        };
        Address::new(0, offset)
    };
    let dptr16 = |dp: u8, index: u16| snes.peek16(direct(dp, index));
    let dptr24 = |dp: u8| snes.peek24(direct(dp, 0));

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
        (F::Sr, Arg::Sr(sr)) => Address::new(0, sp.wrapping_add(sr as u16)),
        (F::Isy, Arg::Sr(sr)) => {
            let ptr = Address::new(0, sp.wrapping_add(sr as u16));
            abs_idx(snes.peek16(ptr), y)
        }
        _ => return None,
    };

    let hi = if matches!(form, F::Dp | F::Dpx | F::Dpy | F::Sr) {
        lo.wrapping_add(1)
    } else {
        Address::from_24bit(lo.as_24bit().wrapping_add(1))
    };

    Some((lo, hi))
}

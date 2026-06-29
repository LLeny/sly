use crate::Snes;
use crate::mmio::Address;

#[derive(Clone, Copy, Debug)]
pub enum AddrMode {
    Dp(u8),
    DpX(u8),
    DpY(u8),
    Abs(u16),
    AbsX(u16),
    AbsY(u16),
    Abl(u32),
    AlX(u32),
    Idp(u8),
    Idx(u8),
    Idy(u8),
    Idl(u8),
    Idly(u8),
    Sr(u8),
    Isy(u8),
}

/// Resolve any addressing mode to its 24bit effective [`Address`] using the current CPU register and
/// memory state. Indirect modes read their pointer from memory, so the result reflects live state.
pub fn resolve(snes: &Snes, mode: AddrMode) -> Address {
    let (dbr, d, x, y, sp) = (
        snes.cpu.data_bank,
        snes.cpu.direct_page,
        snes.cpu.index_x,
        snes.cpu.index_y,
        snes.cpu.stack_pointer,
    );

    let dp_ptr = |dp: u8| Address::new(0, d.wrapping_add(dp as u16));
    let dp_idx = |dp: u8, idx: u16| Address::new(0, d.wrapping_add(dp as u16).wrapping_add(idx));
    let abs_idx = |addr: u16, idx: u16| Address::new(dbr, addr.wrapping_add(idx));
    let indirect_y = |ptr: Address| {
        let base = Address::new(dbr, snes.read16(ptr));
        Address::from_24bit(base.as_24bit().wrapping_add(y as u32))
    };

    match mode {
        AddrMode::Dp(dp) => dp_ptr(dp),
        AddrMode::DpX(dp) => dp_idx(dp, x),
        AddrMode::DpY(dp) => dp_idx(dp, y),
        AddrMode::Abs(addr) => Address::new(dbr, addr),
        AddrMode::AbsX(addr) => abs_idx(addr, x),
        AddrMode::AbsY(addr) => abs_idx(addr, y),
        AddrMode::Abl(addr) => Address::from_24bit(addr),
        AddrMode::AlX(addr) => Address::from_24bit(addr.wrapping_add(x as u32)),
        AddrMode::Idp(dp) => Address::new(dbr, snes.read16(dp_ptr(dp))),
        AddrMode::Idx(dp) => {
            let ptr = resolve(snes, AddrMode::DpX(dp));
            Address::new(dbr, snes.read16(ptr))
        }
        AddrMode::Idy(dp) => indirect_y(dp_ptr(dp)),
        AddrMode::Idl(dp) => Address::from_24bit(read24(snes, dp_ptr(dp))),
        AddrMode::Idly(dp) => Address::from_24bit(read24(snes, dp_ptr(dp)).wrapping_add(y as u32)),
        AddrMode::Sr(sr) => Address::new(0, sp.wrapping_add(sr as u16)),
        AddrMode::Isy(sr) => indirect_y(Address::new(0, sp.wrapping_add(sr as u16))),
    }
}

/// Read a 24bit pointer starting at `ptr`.
pub(crate) fn read24(snes: &Snes, ptr: Address) -> u32 {
    let lo = snes.read16(ptr) as u32;
    let hi = snes.read8(ptr.wrapping_add(2)) as u32;
    lo | (hi << 16)
}

use crate::Snes;

pub(crate) trait MmioReg {
    fn read(snes: &Snes) -> u8;
    fn write(snes: &mut Snes, value: u8);
    fn peek(snes: &Snes) -> u8;
}

pub struct MmioSymbol {
    pub addr: u16,
    pub name: &'static str,
}

inventory::collect!(MmioSymbol);

/// Look up a symbol name for a given MMIO address.
pub fn symbol(addr: u16) -> Option<&'static str> {
    inventory::iter::<MmioSymbol>()
        .find(|reg| reg.addr == addr)
        .map(|reg| reg.name)
}

/// Whether `name` is a registered MMIO register name.
pub fn is_symbol(name: &str) -> bool {
    address_of(name).is_some()
}

/// The address a symbol name resolves to.
pub fn address_of(name: &str) -> Option<u16> {
    inventory::iter::<MmioSymbol>()
        .find(|reg| reg.name == name)
        .map(|reg| reg.addr)
}

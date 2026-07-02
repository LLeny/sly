use crate::Snes;
use crate::mmio::MemoryMap;

pub(crate) trait MmioReg {
    fn read<M: MemoryMap>(snes: &Snes<M>) -> u8;
    fn write<M: MemoryMap>(snes: &mut Snes<M>, value: u8);
    fn peek<M: MemoryMap>(snes: &Snes<M>) -> u8;
}

pub struct MmioSymbol {
    pub addr: u16,
    pub name: &'static str,
    pub bits: u8,
    pub fields: &'static [chapa::FieldInfo],
}

inventory::collect!(MmioSymbol);

/// Iterates every registered MMIO symbol.
pub fn symbols() -> impl Iterator<Item = &'static MmioSymbol> {
    inventory::iter::<MmioSymbol>().into_iter()
}

/// Look up a symbol name for a given MMIO address.
pub fn symbol(addr: u16) -> Option<&'static str> {
    inventory::iter::<MmioSymbol>()
        .find(|reg| reg.addr == addr)
        .map(|reg| reg.name)
}

/// Whether `name` is a registered MMIO register name.
pub fn is_symbol(name: &str) -> bool {
    self::address_of(name).is_some()
}

/// The address a symbol name resolves to.
pub fn address_of(name: &str) -> Option<u16> {
    inventory::iter::<MmioSymbol>()
        .find(|reg| reg.name == name)
        .map(|reg| reg.addr)
}

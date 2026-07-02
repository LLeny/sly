use crate::apu::regs::Control;
use crate::mmio::MmioSymbol;

const fn raw(addr: u16, name: &'static str) -> MmioSymbol {
    MmioSymbol {
        addr,
        name,
        bits: 8,
        fields: &[],
    }
}

const SYMBOLS: &[MmioSymbol] = &[
    raw(0x00F0, "TEST"),
    MmioSymbol {
        addr: 0x00F1,
        name: "CONTROL",
        bits: 8,
        fields: Control::FIELDS,
    },
    raw(0x00F2, "DSPADDR"),
    raw(0x00F3, "DSPDATA"),
    raw(0x00F4, "CPUIO0"),
    raw(0x00F5, "CPUIO1"),
    raw(0x00F6, "CPUIO2"),
    raw(0x00F7, "CPUIO3"),
    raw(0x00FA, "T0TARGET"),
    raw(0x00FB, "T1TARGET"),
    raw(0x00FC, "T2TARGET"),
    raw(0x00FD, "T0OUT"),
    raw(0x00FE, "T1OUT"),
    raw(0x00FF, "T2OUT"),
];

/// Iterates every SPC-side I/O register symbol.
pub fn symbols() -> impl Iterator<Item = &'static MmioSymbol> {
    SYMBOLS.iter()
}

pub fn symbol(addr: u16) -> Option<&'static str> {
    SYMBOLS
        .iter()
        .find(|sym| sym.addr == addr)
        .map(|sym| sym.name)
}

pub fn is_symbol(name: &str) -> bool {
    SYMBOLS.iter().any(|sym| sym.name == name)
}

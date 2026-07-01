const SYMBOLS: &[(u16, &str)] = &[
    (0x00F0, "TEST"),
    (0x00F1, "CONTROL"),
    (0x00F2, "DSPADDR"),
    (0x00F3, "DSPDATA"),
    (0x00F4, "CPUIO0"),
    (0x00F5, "CPUIO1"),
    (0x00F6, "CPUIO2"),
    (0x00F7, "CPUIO3"),
    (0x00FA, "T0TARGET"),
    (0x00FB, "T1TARGET"),
    (0x00FC, "T2TARGET"),
    (0x00FD, "T0OUT"),
    (0x00FE, "T1OUT"),
    (0x00FF, "T2OUT"),
];

pub fn symbol(addr: u16) -> Option<&'static str> {
    SYMBOLS
        .iter()
        .find(|(a, _)| *a == addr)
        .map(|(_, name)| *name)
}

pub fn is_symbol(name: &str) -> bool {
    SYMBOLS.iter().any(|(_, n)| *n == name)
}

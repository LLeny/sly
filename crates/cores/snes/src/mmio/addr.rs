/// A 24bit address: an 8bit bank and a 16bit address within it.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Address {
    bank: u8,
    addr: u16,
}

impl Address {
    pub const fn new(bank: u8, addr: u16) -> Self {
        Self { bank, addr }
    }

    pub const fn wrapping_add(self, n: u16) -> Self {
        Self {
            bank: self.bank,
            addr: self.addr.wrapping_add(n),
        }
    }

    pub const fn as_24bit(self) -> u32 {
        ((self.bank as u32) << 16) | self.addr as u32
    }

    pub const fn from_24bit(value: u32) -> Self {
        Self {
            bank: (value >> 16) as u8,
            addr: value as u16,
        }
    }

    pub const fn bank(self) -> u8 {
        self.bank
    }

    pub const fn address(self) -> u16 {
        self.addr
    }
}

impl From<u32> for Address {
    fn from(value: u32) -> Self {
        Self::from_24bit(value)
    }
}

impl From<Address> for u32 {
    fn from(addr: Address) -> Self {
        addr.as_24bit()
    }
}

impl std::fmt::Debug for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02X}:{:04X}", self.bank, self.addr)
    }
}

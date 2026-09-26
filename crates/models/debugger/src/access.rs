use std::sync::Arc;

use crate::CpuId;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MemoryAddress {
    pub region: Arc<str>,
    pub addr: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessWidth {
    Byte,
    Halfword,
    Word,
}

impl AccessWidth {
    pub const fn bytes(self) -> u32 {
        match self {
            Self::Byte => 1,
            Self::Halfword => 2,
            Self::Word => 4,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccessSource {
    Cpu(CpuId),
    Dma,
    Device(Arc<str>),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AccessTracking {
    #[default]
    Off,
    Latest,
    Highlights,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DebugMemoryError {
    Unsupported,
    InvalidAddress,
    Misaligned { address: u32, width: AccessWidth },
}

impl std::fmt::Display for DebugMemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported => f.write_str("this region does not support bus transactions"),
            Self::InvalidAddress => f.write_str("address lies outside the selected region"),
            Self::Misaligned { address, width } => write!(
                f,
                "address {address:#X} is not aligned for a {}-byte transaction",
                width.bytes()
            ),
        }
    }
}

impl std::error::Error for DebugMemoryError {}

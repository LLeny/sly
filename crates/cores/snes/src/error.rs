use thiserror::Error;

#[derive(Debug, Error)]
pub enum SnesError {
    #[error("Invalid ROM header")]
    InvalidRomHeader,
    #[error("Invalid memory map")]
    InvalidMemoryMap,
}

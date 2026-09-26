#[derive(Debug, thiserror::Error)]
pub enum GbaError {
    #[error("GBA emulation is not implemented yet")]
    NotImplemented,
}

use crate::CartridgeInfo;

pub trait Cartridge {
    fn data(&self) -> &[u8];
    fn filename(&self) -> &str;
    fn info(&self) -> Option<CartridgeInfo>;
}

#[derive(Debug, Clone, Default)]
pub struct RomFile {
    pub data: Vec<u8>,
    pub filename: String,
}

impl RomFile {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            filename: String::new(),
        }
    }

    pub fn data(mut self, data: Vec<u8>) -> Self {
        self.data = data;
        self
    }

    pub fn filename(mut self, filename: impl Into<String>) -> Self {
        self.filename = filename.into();
        self
    }
}

impl Cartridge for RomFile {
    fn data(&self) -> &[u8] {
        &self.data
    }

    fn filename(&self) -> &str {
        &self.filename
    }

    fn info(&self) -> Option<CartridgeInfo> {
        None
    }
}

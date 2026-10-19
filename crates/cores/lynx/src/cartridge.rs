use core_model::RomFile;

use crate::cartridge_header::CartridgeHeader;

pub struct Cartridge {
    rom: RomFile,
    header: CartridgeHeader,
}

impl Cartridge {
    pub fn load(rom: RomFile) -> Self {
        let header = CartridgeHeader::parse(&rom.data).unwrap_or_default();
        Self { rom, header }
    }

    pub fn title(&self) -> String {
        self.header.title()
    }

    pub fn data(&self) -> &[u8] {
        &self.rom.data
    }

    pub fn rom(&self) -> &RomFile {
        &self.rom
    }

    pub fn header(&self) -> &CartridgeHeader {
        &self.header
    }
}

impl Default for Cartridge {
    fn default() -> Self {
        Self {
            rom: RomFile::new(),
            header: CartridgeHeader::default(),
        }
    }
}

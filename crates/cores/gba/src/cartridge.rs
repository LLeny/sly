use core_model::{CartridgeInfo, Region, RomFile};

use crate::cartridge_header::CartridgeHeader;

pub struct Cartridge {
    rom: RomFile,
    header: CartridgeHeader,
}

impl Cartridge {
    pub fn load(rom: RomFile) -> Self {
        let header = CartridgeHeader::parse(&rom.data);
        Self { rom, header }
    }

    pub fn title(&self) -> String {
        self.header.title()
    }

    pub fn region(&self) -> Option<Region> {
        self.header.region()
    }

    pub fn header(&self) -> &CartridgeHeader {
        &self.header
    }

    pub fn data(&self) -> &[u8] {
        &self.rom.data
    }

    pub fn info(&self) -> Option<CartridgeInfo> {
        Some(core_model::CartridgeInfo {
            system: core_model::System::Gba,
            title: self.header.title(),
            region: self.header.region(),
            tv_system: None,
            rom: crate::cartridge_header::identity(&self.rom.data),
            backup: None,
        })
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

impl core_model::Cartridge for Cartridge {
    fn data(&self) -> &[u8] {
        self.data()
    }

    fn filename(&self) -> &str {
        &self.rom.filename
    }

    fn info(&self) -> Option<CartridgeInfo> {
        self.info()
    }
}

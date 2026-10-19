use core_model::{CartridgeInfo, Region, RomFile, TvSystem};
use serde::{Deserialize, Serialize};

use crate::cartridge_header::{CartridgeHeader, Coprocessor};
use crate::error::SnesError;

#[derive(Serialize, Deserialize)]
pub struct Cartridge {
    #[serde(skip)]
    rom: RomFile,
    header: CartridgeHeader,
    sram: Vec<u8>,
}

impl Cartridge {
    pub fn load(mut rom: RomFile) -> Result<Self, SnesError> {
        let header = CartridgeHeader::parse(&mut rom.data)?;
        let sram = vec![0; header.sram_size];
        Ok(Self { rom, header, sram })
    }

    pub fn title(&self) -> String {
        self.header.title()
    }

    pub fn region(&self) -> Region {
        self.header.region()
    }

    pub fn tv_system(&self) -> TvSystem {
        self.header.tv_system()
    }

    pub fn coprocessor(&self) -> Coprocessor {
        self.header.coprocessor()
    }

    pub fn has_battery(&self) -> bool {
        self.header.has_battery()
    }

    pub fn sram_read(&self, index: usize) -> u8 {
        match self.header.sram_index(index) {
            Some(index) => self.sram[index],
            None => 0,
        }
    }

    pub fn sram_write(&mut self, index: usize, value: u8) {
        if let Some(index) = self.header.sram_index(index) {
            self.sram[index] = value;
        }
    }

    pub fn sram(&self) -> &[u8] {
        &self.sram
    }

    pub fn sram_mut(&mut self) -> &mut [u8] {
        &mut self.sram
    }

    pub fn header(&self) -> &CartridgeHeader {
        &self.header
    }

    pub fn data(&self) -> &[u8] {
        &self.rom.data
    }

    pub fn data_mut(&mut self) -> &mut Vec<u8> {
        &mut self.rom.data
    }

    pub fn info(&self) -> Option<CartridgeInfo> {
        crate::cartridge_header::info(&self.rom, &self.sram).ok()
    }

    pub(crate) fn take_rom(&mut self) -> RomFile {
        std::mem::take(&mut self.rom)
    }

    pub(crate) fn set_rom(&mut self, rom: RomFile) {
        self.rom = rom;
    }
}

impl Default for Cartridge {
    fn default() -> Self {
        Self {
            rom: RomFile::new(),
            header: CartridgeHeader::default(),
            sram: Vec::new(),
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

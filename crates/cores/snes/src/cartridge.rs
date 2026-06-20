use chapa::{BitEnum, BitField};

use crate::error::SnesError;

pub const TITLE_LENGTH: usize = 21;

pub struct Cartridge {
    pub rom: Vec<u8>,
    header_offset: usize,
}

impl Cartridge {
    pub fn new(rom: Vec<u8>) -> Result<Self, SnesError> {
        let header_offset = Self::search_header(&rom).ok_or(SnesError::InvalidRomHeader)?;
        Ok(Self { rom, header_offset })
    }

    pub fn title(&self) -> String {
        let title_bytes = &self.rom[self.header_offset..self.header_offset + TITLE_LENGTH];
        String::from_utf8_lossy(title_bytes)
            .trim_end_matches('\0')
            .to_string()
    }

    pub fn memory_map(&self) -> Result<MemoryMap, SnesError> {
        const MEMORY_MAP_OFFSET: usize = 0x15;

        let memory_map = self.rom[self.header_offset + MEMORY_MAP_OFFSET];
        MemoryMap::try_from_raw(memory_map).map_err(|_| SnesError::InvalidMemoryMap)
    }

    pub fn raw_header(&self) -> &[u8] {
        &self.rom[self.header_offset..self.header_offset + 0x40]
    }

    fn search_header(rom: &[u8]) -> Option<usize> {
        const OFFSETS: [usize; 3] = [0x007FC0, 0x00FFC0, 0x40FFC0];

        for offset in OFFSETS {
            if rom.len() < offset + TITLE_LENGTH {
                continue;
            }

            if rom[offset..offset + TITLE_LENGTH]
                .iter()
                .all(|&b| b.is_ascii() || b == 0)
            {
                return Some(offset);
            }
        }

        None
    }
}

#[derive(BitEnum, Clone, Copy, Debug)]
pub enum Speed {
    #[fallback]
    Slow = 0x00,
    Fast = 0x01,
}

#[derive(BitEnum, Clone, Copy, Debug)]
pub enum MapMode {
    #[fallback]
    LoRom = 0x00,
    HiRom = 0x01,
    Sdd1 = 0x02,
    Sa1 = 0x03,
    ExHiRom = 0x05,
    Spc7110 = 0x0A,
}

#[chapa::bitfield(u8, order = msb0)]
#[derive(Clone, Copy, Debug)]
pub struct MemoryMap {
    #[bits(3)]
    pub speed: Speed,

    #[bits(4..=7)]
    pub map_mode: MapMode,
}

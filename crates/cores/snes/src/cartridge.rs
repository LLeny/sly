use chapa::{BitField, bitenum};
pub use core_model::{Region, TvSystem};
use serde::{Deserialize, Serialize};

use crate::error::SnesError;

pub const TITLE_LENGTH: usize = 21;
const HEADER_LENGTH: usize = 0x20;

#[derive(Serialize, Deserialize)]
pub struct Cartridge {
    #[serde(skip)]
    pub rom: Vec<u8>,

    pub sram: Vec<u8>,
    header_offset: usize,
    map_mode: Option<MapMode>,
    rom_mask: Option<usize>,
}

impl Cartridge {
    pub fn new(mut rom: Vec<u8>) -> Result<Self, SnesError> {
        Self::strip_copier_header(&mut rom);

        let header_offset = Self::search_header(&rom).ok_or(SnesError::InvalidRomHeader)?;

        let mut cartridge = Self {
            rom,
            header_offset,
            map_mode: None,
            rom_mask: None,
            sram: Vec::new(),
        };
        cartridge.map_mode = cartridge.memory_map().ok().map(|map| map.map_mode());
        cartridge.rom_mask = cartridge
            .rom
            .len()
            .is_power_of_two()
            .then(|| cartridge.rom.len() - 1);
        if cartridge.has_sram() {
            cartridge.sram.resize(cartridge.sram_size(), 0);
        }

        Ok(cartridge)
    }

    pub fn map_mode(&self) -> Option<MapMode> {
        self.map_mode
    }

    pub fn rom_mask(&self) -> Option<usize> {
        self.rom_mask
    }

    pub fn rom_crc32(&self) -> u32 {
        crc32fast::hash(&self.rom)
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

    pub fn has_sram(&self) -> bool {
        const EXTRA_HARDWARE_OFFSET: usize = 0x16;

        let chipset = self.rom[self.header_offset + EXTRA_HARDWARE_OFFSET];
        matches!(chipset & 0x0F, 0x01 | 0x02 | 0x04 | 0x05 | 0x09 | 0x0A)
    }

    pub fn sram_size(&self) -> usize {
        const SRAM_SIZE_OFFSET: usize = 0x18;

        let size_code = self.rom[self.header_offset + SRAM_SIZE_OFFSET];
        if size_code == 0 {
            0
        } else {
            1024usize.checked_shl(size_code.into()).unwrap_or(0)
        }
    }

    pub fn header_offset(&self) -> usize {
        self.header_offset
    }

    pub fn region(&self) -> Region {
        const REGION_OFFSET: usize = 0x19;

        let code = self
            .rom
            .get(self.header_offset + REGION_OFFSET)
            .copied()
            .unwrap_or(0xFF);
        region_from_code(code)
    }

    fn strip_copier_header(rom: &mut Vec<u8>) {
        const COPIER_HEADER_LENGTH: usize = 512;

        if rom.len() % 1024 == COPIER_HEADER_LENGTH {
            rom.drain(..COPIER_HEADER_LENGTH);
        }
    }

    fn search_header(rom: &[u8]) -> Option<usize> {
        const OFFSETS: [usize; 3] = [0x007FC0, 0x00FFC0, 0x40FFC0];

        let mut best: Option<(usize, i32)> = None;
        for offset in OFFSETS {
            if rom.len() < offset + HEADER_LENGTH {
                continue;
            }

            let score = Self::score_header(rom, offset);
            if best.is_none_or(|(_, best_score)| score > best_score) {
                best = Some((offset, score));
            }
        }

        best.filter(|&(_, score)| score > 0)
            .map(|(offset, _)| offset)
    }

    fn score_header(rom: &[u8], offset: usize) -> i32 {
        let header = &rom[offset..offset + HEADER_LENGTH];
        let mut score = 0;

        let complement = u16::from_le_bytes([header[0x1C], header[0x1D]]);
        let checksum = u16::from_le_bytes([header[0x1E], header[0x1F]]);
        if complement ^ checksum == 0xFFFF && checksum != 0 && complement != 0 {
            score += 8;
        }

        let map_mode = header[0x15];
        if map_mode & 0xE0 == 0x20 {
            score += 2;
        }
        let mode_matches_offset = match offset {
            0x007FC0 => matches!(map_mode & 0x0F, 0x00 | 0x02 | 0x03),
            0x00FFC0 => matches!(map_mode & 0x0F, 0x01 | 0x0A),
            0x40FFC0 => map_mode & 0x0F == 0x05,
            _ => false,
        };
        if mode_matches_offset {
            score += 2;
        }

        if header[..TITLE_LENGTH]
            .iter()
            .all(|&b| b == 0 || (0x20..=0x7E).contains(&b))
        {
            score += 1;
        }
        if header[0x19] <= 0x11 {
            score += 1;
        }

        score
    }
}

#[bitenum]
#[derive(Debug)]
pub enum Speed {
    #[fallback]
    Slow = 0x00,
    Fast = 0x01,
}

#[bitenum]
#[derive(Debug)]
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
#[derive(Debug)]
pub struct MemoryMap {
    #[bits(3)]
    pub speed: Speed,

    #[bits(4..=7)]
    pub map_mode: MapMode,
}

pub fn frame_rate(system: TvSystem) -> f64 {
    match system {
        TvSystem::Ntsc => 60.0988,
        TvSystem::Pal => 50.007,
    }
}

// SNES header destination codes ($FFD9): $00 Japan .. $11 Australia,
// $03 Scandinavia was originally specific to Sweden, $06 France is Europe French-only.
fn region_from_code(code: u8) -> Region {
    match code {
        0x00 => Region::Japan,
        0x01 => Region::NorthAmerica,
        0x02 => Region::Europe,
        0x03 => Region::Scandinavia,
        0x04 => Region::Finland,
        0x05 => Region::Denmark,
        0x06 => Region::France,
        0x07 => Region::Netherlands,
        0x08 => Region::Spain,
        0x09 => Region::Germany,
        0x0A => Region::Italy,
        0x0B => Region::China,
        0x0C => Region::Indonesia,
        0x0D => Region::SouthKorea,
        0x0E => Region::Common,
        0x0F => Region::Canada,
        0x10 => Region::Brazil,
        0x11 => Region::Australia,
        other => Region::Other(other),
    }
}

crate::macros::impl_bitfield_serde!(Speed, MapMode, MemoryMap,);

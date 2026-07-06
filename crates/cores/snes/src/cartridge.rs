use chapa::{BitEnum, BitField};

use crate::error::SnesError;

pub const TITLE_LENGTH: usize = 21;

pub struct Cartridge {
    pub rom: Vec<u8>,
    header_offset: usize,
    map_mode: Option<MapMode>,
    rom_mask: Option<usize>,
}

impl Cartridge {
    pub fn new(rom: Vec<u8>) -> Result<Self, SnesError> {
        let header_offset = Self::search_header(&rom).ok_or(SnesError::InvalidRomHeader)?;
        let mut cartridge = Self {
            rom,
            header_offset,
            map_mode: None,
            rom_mask: None,
        };
        cartridge.map_mode = cartridge.memory_map().ok().map(|map| map.map_mode());
        cartridge.rom_mask = cartridge
            .rom
            .len()
            .is_power_of_two()
            .then(|| cartridge.rom.len() - 1);
        Ok(cartridge)
    }

    pub fn map_mode(&self) -> Option<MapMode> {
        self.map_mode
    }

    pub fn rom_mask(&self) -> Option<usize> {
        self.rom_mask
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
        Region::from_code(code)
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TvSystem {
    Ntsc,
    Pal,
}

impl std::fmt::Display for TvSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TvSystem::Ntsc => "NTSC",
            TvSystem::Pal => "PAL",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    Japan,        // $00
    NorthAmerica, // $01
    Europe,       // $02
    Scandinavia,  // $03 (originally specific to Sweden)
    Finland,      // $04
    Denmark,      // $05
    France,       // $06 (Europe, French only)
    Netherlands,  // $07
    Spain,        // $08
    Germany,      // $09
    Italy,        // $0A
    China,        // $0B
    Indonesia,    // $0C
    SouthKorea,   // $0D
    Common,       // $0E
    Canada,       // $0F
    Brazil,       // $10
    Australia,    // $11
    Other(u8),
}

impl Region {
    pub fn from_code(code: u8) -> Self {
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

    pub fn tv_system(self) -> TvSystem {
        match self {
            Region::Japan
            | Region::NorthAmerica
            | Region::SouthKorea
            | Region::Canada
            | Region::Common
            | Region::Other(_) => TvSystem::Ntsc,
            _ => TvSystem::Pal,
        }
    }
}

impl std::fmt::Display for Region {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Region::Japan => "Japan",
            Region::NorthAmerica => "North America",
            Region::Europe => "Europe",
            Region::Scandinavia => "Scandinavia",
            Region::Finland => "Finland",
            Region::Denmark => "Denmark",
            Region::France => "France",
            Region::Netherlands => "Netherlands",
            Region::Spain => "Spain",
            Region::Germany => "Germany",
            Region::Italy => "Italy",
            Region::China => "China",
            Region::Indonesia => "Indonesia",
            Region::SouthKorea => "South Korea",
            Region::Common => "Common",
            Region::Canada => "Canada",
            Region::Brazil => "Brazil",
            Region::Australia => "Australia",
            Region::Other(code) => return write!(f, "Region ${code:02X}"),
        };
        write!(f, "{name} ({})", self.tv_system())
    }
}

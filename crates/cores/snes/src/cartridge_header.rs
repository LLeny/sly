use chapa::{BitField, bitenum};
use core_model::System;
pub use core_model::{CartridgeInfo, Region, TvSystem};
use serde::{Deserialize, Serialize};

use crate::error::SnesError;

pub const TITLE_LENGTH: usize = 21;
const HEADER_LENGTH: usize = 0x20;
const COPIER_HEADER_LENGTH: usize = 512;
const REGION_OFFSET: usize = 0x19;

#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct CartridgeHeader {
    header_offset: usize,
    title: String,
    region_code: u8,
    map_mode: Option<MapMode>,
    rom_mask: Option<usize>,
    sram_mask: Option<usize>,
    coprocessor: Coprocessor,
    has_battery: bool,
    has_sram: bool,
    pub(crate) sram_size: usize,
}

impl CartridgeHeader {
    pub fn parse(rom: &mut Vec<u8>) -> Result<Self, SnesError> {
        Self::strip_copier_header(rom);
        Self::parse_normalized(rom)
    }

    fn parse_normalized(rom: &[u8]) -> Result<Self, SnesError> {
        let header_offset = Self::search_header(rom).ok_or(SnesError::InvalidRomHeader)?;

        let mut header = Self {
            header_offset,
            title: Self::read_title(rom, header_offset),
            region_code: rom
                .get(header_offset + REGION_OFFSET)
                .copied()
                .unwrap_or(0xFF),
            ..Self::default()
        };
        header.coprocessor = self::coprocessor_from_chipset(header.chipset(rom));
        header.map_mode = header.memory_map(rom).ok().map(|map| map.map_mode());
        header.rom_mask = rom.len().is_power_of_two().then(|| rom.len() - 1);
        header.has_battery = header.chipset(rom).has_battery();
        header.has_sram = header.chipset(rom).has_sram();
        header.sram_size = match header.coprocessor() {
            Coprocessor::SuperFx => header.expansion_ram_size(rom),
            _ if header.has_sram => header.sram_size(rom),
            _ => 0,
        };
        header.sram_mask = header
            .sram_size
            .is_power_of_two()
            .then(|| header.sram_size - 1);

        Ok(header)
    }

    fn read_title(rom: &[u8], header_offset: usize) -> String {
        let end = header_offset + TITLE_LENGTH;
        String::from_utf8_lossy(&rom[header_offset..end])
            .trim_end_matches('\0')
            .to_string()
    }

    pub fn map_mode(&self) -> Option<MapMode> {
        self.map_mode
    }

    pub fn rom_mask(&self) -> Option<usize> {
        self.rom_mask
    }

    pub(crate) fn sram_index(&self, index: usize) -> Option<usize> {
        if let Some(mask) = self.sram_mask {
            return Some(index & mask);
        }

        (self.sram_size != 0).then(|| index % self.sram_size)
    }

    pub fn title(&self) -> String {
        self.title.clone()
    }

    pub fn region(&self) -> Region {
        region_from_code(self.region_code)
    }

    pub fn tv_system(&self) -> TvSystem {
        self::tv_system(self.region())
    }

    pub fn memory_map(&self, rom: &[u8]) -> Result<MemoryMap, SnesError> {
        const MEMORY_MAP_OFFSET: usize = 0x15;

        let raw = rom
            .get(self.header_offset + MEMORY_MAP_OFFSET)
            .copied()
            .unwrap_or(0);
        MemoryMap::try_from_raw(raw).map_err(|_| SnesError::InvalidMemoryMap)
    }

    pub fn has_battery(&self) -> bool {
        self.has_battery
    }

    pub fn has_sram(&self) -> bool {
        self.has_sram
    }

    fn chipset(&self, rom: &[u8]) -> Chipset {
        const EXTRA_HARDWARE_OFFSET: usize = 0x16;
        Chipset::from_raw(
            rom.get(self.header_offset + EXTRA_HARDWARE_OFFSET)
                .copied()
                .unwrap_or(0),
        )
    }

    pub fn coprocessor(&self) -> Coprocessor {
        self.coprocessor
    }

    fn has_later_extended_header(&self, rom: &[u8]) -> bool {
        const DEV_ID_OFFSET: usize = 0x1A;
        const MOVED_TO_EXTENDED: u8 = 0x33;

        rom.get(self.header_offset + DEV_ID_OFFSET)
            .is_some_and(|id| *id == MOVED_TO_EXTENDED)
    }

    pub fn expansion_ram_size(&self, rom: &[u8]) -> usize {
        const FALLBACK: usize = 32 * 1024;

        if !self.has_later_extended_header(rom) {
            return FALLBACK;
        }

        let code = rom
            .get(self.header_offset.wrapping_sub(3))
            .copied()
            .unwrap_or(0);

        if code == 0 {
            FALLBACK
        } else {
            1024usize.checked_shl(code.into()).unwrap_or(FALLBACK)
        }
    }

    pub fn sram_size(&self, rom: &[u8]) -> usize {
        const SRAM_SIZE_OFFSET: usize = 0x18;

        let size_code = rom
            .get(self.header_offset + SRAM_SIZE_OFFSET)
            .copied()
            .unwrap_or(0);
        if size_code == 0 {
            0
        } else {
            1024usize.checked_shl(size_code.into()).unwrap_or(0)
        }
    }

    pub fn header_offset(&self) -> usize {
        self.header_offset
    }

    fn region_at(rom: &[u8], header_offset: usize) -> Region {
        let code = rom
            .get(header_offset + REGION_OFFSET)
            .copied()
            .unwrap_or(0xFF);
        region_from_code(code)
    }

    fn strip_copier_header(rom: &mut Vec<u8>) {
        let prefix = rom.len() - normalized_rom(rom).len();
        if prefix != 0 {
            rom.drain(..prefix);
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

pub fn info(rom: &core_model::RomFile, sram: &[u8]) -> Result<CartridgeInfo, SnesError> {
    let rom = normalized_rom(&rom.data);
    let header = CartridgeHeader::parse_normalized(rom)?;

    Ok(CartridgeInfo {
        system: System::Snes,
        title: header.title(),
        region: Some(header.region()),
        tv_system: Some(header.tv_system()),
        rom: identity(rom),
        backup: (!sram.is_empty() && header.has_battery()).then_some(core_model::BackupMemory {
            kind: core_model::BackupKind::Sram,
            capacity: sram.len(),
            battery_backed: true,
        }),
    })
}

/// Reads the region straight from a borrowed ROM, without building a header.
pub fn region_of(rom: &[u8]) -> Result<Region, SnesError> {
    let rom = normalized_rom(rom);
    let header_offset = CartridgeHeader::search_header(rom).ok_or(SnesError::InvalidRomHeader)?;
    Ok(CartridgeHeader::region_at(rom, header_offset))
}

pub fn normalized_rom(rom: &[u8]) -> &[u8] {
    if rom.len() % 1024 == COPIER_HEADER_LENGTH {
        &rom[COPIER_HEADER_LENGTH..]
    } else {
        rom
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Coprocessor {
    #[default]
    None,
    Dsp,
    SuperFx,
    Obc1,
    Sa1,
    Sdd1,
    Srtc,
    Other(u8),
}

#[chapa::bitfield(u8, order = lsb0)]
#[derive(Debug)]
pub struct Chipset {
    #[bits(0..=3)]
    pub kind: u8,

    #[bits(4..=7)]
    pub coprocessor: u8,
}

impl Chipset {
    pub fn has_battery(&self) -> bool {
        matches!(self.kind(), 0x02 | 0x05 | 0x06 | 0x09 | 0x0A)
    }

    pub fn has_sram(&self) -> bool {
        matches!(self.kind(), 0x01 | 0x02 | 0x04 | 0x05 | 0x09 | 0x0A)
    }
}

fn coprocessor_from_chipset(chipset: Chipset) -> Coprocessor {
    if chipset.kind() < 0x03 {
        return Coprocessor::None;
    }

    match chipset.coprocessor() {
        0x0 => Coprocessor::Dsp,
        0x1 => Coprocessor::SuperFx,
        0x2 => Coprocessor::Obc1,
        0x3 => Coprocessor::Sa1,
        0x4 => Coprocessor::Sdd1,
        0x5 => Coprocessor::Srtc,
        other => Coprocessor::Other(other),
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
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

pub fn tv_system(region: Region) -> TvSystem {
    match region {
        Region::Japan
        | Region::NorthAmerica
        | Region::SouthKorea
        | Region::Canada
        | Region::Common
        | Region::Other(_) => TvSystem::Ntsc,
        _ => TvSystem::Pal,
    }
}

pub fn identity(rom: &[u8]) -> core_model::RomIdentity {
    use sha1::{Digest, Sha1};
    use std::fmt::Write;
    let rom = normalized_rom(rom);
    let sha1 = Sha1::digest(rom)
        .iter()
        .fold(String::with_capacity(40), |mut hex, byte| {
            write!(hex, "{byte:02x}").expect("writing to a String cannot fail");
            hex
        });

    core_model::RomIdentity {
        sha1,
        crc32: crc32fast::hash(rom),
        len: rom.len(),
    }
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

use chapa::{BitField, bitenum};
use core_model::System;
pub use core_model::{CartridgeInfo, Region, TvSystem};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use crate::error::SnesError;

pub const TITLE_LENGTH: usize = 21;
const HEADER_LENGTH: usize = 0x20;
const COPIER_HEADER_LENGTH: usize = 512;

#[derive(Serialize, Deserialize)]
pub struct Cartridge {
    #[serde(skip)]
    pub rom: Vec<u8>,
    #[serde(skip)]
    identity: OnceLock<core_model::RomIdentity>,
    pub sram: Vec<u8>, // Either save RAM or Game Pak RAM (Super FX, maybe more?)
    header_offset: usize,
    map_mode: Option<MapMode>,
    rom_mask: Option<usize>,
    sram_mask: Option<usize>,
    coprocessor: Coprocessor,
}

impl Cartridge {
    pub fn new(mut rom: Vec<u8>) -> Result<Self, SnesError> {
        Self::strip_copier_header(&mut rom);

        let header_offset = Self::search_header(&rom).ok_or(SnesError::InvalidRomHeader)?;
        // Remember the ROM as it was loaded, so debugger patches or a restored state don't change
        // its identity.
        let identity = OnceLock::from(identity(&rom));

        let mut cartridge = Self {
            rom,
            identity,
            header_offset,
            map_mode: None,
            rom_mask: None,
            sram_mask: None,
            coprocessor: Coprocessor::None,
            sram: Vec::new(),
        };
        cartridge.coprocessor = self::coprocessor_from_chipset(cartridge.chipset());
        cartridge.map_mode = cartridge.memory_map().ok().map(|map| map.map_mode());
        cartridge.rom_mask = cartridge
            .rom
            .len()
            .is_power_of_two()
            .then(|| cartridge.rom.len() - 1);
        cartridge.sram.resize(
            match cartridge.coprocessor() {
                Coprocessor::SuperFx => cartridge.expansion_ram_size(),
                _ if cartridge.has_sram() => cartridge.sram_size(),
                _ => 0,
            },
            0,
        );
        cartridge.sram_mask = cartridge
            .sram
            .len()
            .is_power_of_two()
            .then(|| cartridge.sram.len() - 1);

        Ok(cartridge)
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

        let len = self.sram.len();
        (len != 0).then(|| index % len)
    }

    pub(crate) fn sram_read(&self, index: usize) -> u8 {
        match self.sram_index(index) {
            Some(index) => self.sram[index],
            None => 0,
        }
    }

    pub(crate) fn sram_write(&mut self, index: usize, value: u8) {
        if let Some(index) = self.sram_index(index) {
            self.sram[index] = value;
        }
    }

    pub fn rom_crc32(&self) -> u32 {
        self.rom_identity().crc32
    }

    pub fn rom_identity(&self) -> &core_model::RomIdentity {
        self.identity.get_or_init(|| identity(&self.rom))
    }

    pub(crate) fn restore_rom_from(&mut self, current: &mut Self) {
        self.rom = std::mem::take(&mut current.rom);
        self.identity = std::mem::take(&mut current.identity);
    }

    pub fn info(&self) -> CartridgeInfo {
        CartridgeInfo {
            system: System::Snes,
            title: self.title(),
            region: Some(self.region()),
            tv_system: Some(tv_system(self.region())),
            rom: self.rom_identity().clone(),
            backup: (!self.sram.is_empty() && self.has_battery()).then_some(
                core_model::BackupMemory {
                    kind: core_model::BackupKind::Sram,
                    capacity: self.sram.len(),
                    battery_backed: true,
                },
            ),
        }
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

    pub fn has_battery(&self) -> bool {
        self.chipset().has_battery()
    }

    pub fn has_sram(&self) -> bool {
        self.chipset().has_sram()
    }

    fn chipset(&self) -> Chipset {
        const EXTRA_HARDWARE_OFFSET: usize = 0x16;
        Chipset::from_raw(self.rom[self.header_offset + EXTRA_HARDWARE_OFFSET])
    }

    pub fn coprocessor(&self) -> Coprocessor {
        self.coprocessor
    }

    fn has_later_extended_header(&self) -> bool {
        const DEV_ID_OFFSET: usize = 0x1A;
        const MOVED_TO_EXTENDED: u8 = 0x33;

        self.rom
            .get(self.header_offset + DEV_ID_OFFSET)
            .is_some_and(|id| *id == MOVED_TO_EXTENDED)
    }

    pub fn expansion_ram_size(&self) -> usize {
        const FALLBACK: usize = 32 * 1024;

        if !self.has_later_extended_header() {
            return FALLBACK;
        }

        let code = self
            .rom
            .get(self.header_offset.wrapping_sub(3))
            .copied()
            .unwrap_or(0);

        if code == 0 {
            FALLBACK
        } else {
            1024usize.checked_shl(code.into()).unwrap_or(FALLBACK)
        }
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
        Self::region_at(&self.rom, self.header_offset)
    }

    /// Reads the region straight from a borrowed ROM, without building a cartridge.
    pub fn region_of(rom: &[u8]) -> Result<Region, SnesError> {
        let rom = Self::normalized_rom(rom);
        let header_offset = Self::search_header(rom).ok_or(SnesError::InvalidRomHeader)?;
        Ok(Self::region_at(rom, header_offset))
    }

    fn region_at(rom: &[u8], header_offset: usize) -> Region {
        const REGION_OFFSET: usize = 0x19;

        let code = rom
            .get(header_offset + REGION_OFFSET)
            .copied()
            .unwrap_or(0xFF);
        region_from_code(code)
    }

    pub fn normalized_rom(rom: &[u8]) -> &[u8] {
        if rom.len() % 1024 == COPIER_HEADER_LENGTH {
            &rom[COPIER_HEADER_LENGTH..]
        } else {
            rom
        }
    }

    fn strip_copier_header(rom: &mut Vec<u8>) {
        let prefix = rom.len() - Self::normalized_rom(rom).len();
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

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Coprocessor {
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
    let rom = Cartridge::normalized_rom(rom);
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

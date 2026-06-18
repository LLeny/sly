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

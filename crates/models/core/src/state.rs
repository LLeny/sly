use thiserror::Error;

use crate::System;

const MAGIC: [u8; 4] = *b"SLYS";

pub const ENVELOPE_VERSION: u16 = 7;
pub const HEADER_LENGTH: usize = 28;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StateError {
    #[error("This core does not support savestates")]
    Unsupported,
    #[error("Not a Sly savestate")]
    BadMagic,
    #[error("Savestate version {found}, expected {expected}")]
    Version { found: u16, expected: u16 },
    #[error("Core payload version {found}, expected {expected}")]
    PayloadVersion { found: u16, expected: u16 },
    #[error("Savestate is for a different system")]
    SystemMismatch,
    #[error("Savestate belongs to a different ROM")]
    RomMismatch,
    #[error("Savestate is truncated")]
    Truncated,
    #[error("Corrupt savestate: {0}")]
    Corrupt(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thumbnail<'a> {
    pub width: u32,
    pub height: u32,
    pub pixels: &'a [u8],
}

impl Thumbnail<'_> {
    pub fn byte_length(width: u32, height: u32) -> Option<usize> {
        (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(4)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Envelope<'a> {
    pub system: System,
    pub rom_crc32: u32, // normalized (e.g. no copier header)
    pub rom_len: u32,
    /// Zero is reserved.
    pub payload_version: u16,
    pub thumbnail: Option<Thumbnail<'a>>,
    pub payload: &'a [u8],
}

impl Envelope<'_> {
    pub fn encode(&self) -> Result<Vec<u8>, StateError> {
        let thumb = self.thumbnail.map(|t| t.pixels).unwrap_or_default();
        let (width, height) = self
            .thumbnail
            .map(|t| (t.width, t.height))
            .unwrap_or((0, 0));

        if self.payload_version == 0 {
            return Err(StateError::Corrupt(
                "payload version zero is reserved".into(),
            ));
        }

        let width = u16::try_from(width)
            .map_err(|_| StateError::Corrupt("thumbnail width exceeds envelope limits".into()))?;
        let height = u16::try_from(height)
            .map_err(|_| StateError::Corrupt("thumbnail height exceeds envelope limits".into()))?;
        let expected_thumb = Thumbnail::byte_length(width.into(), height.into())
            .ok_or_else(|| StateError::Corrupt("thumbnail size exceeds platform limits".into()))?;
        if (width == 0) != (height == 0) || thumb.len() != expected_thumb {
            return Err(StateError::Corrupt(
                "thumbnail dimensions do not match its pixels".into(),
            ));
        }

        let payload_len = u32::try_from(self.payload.len())
            .map_err(|_| StateError::Corrupt("payload exceeds envelope limits".into()))?;
        let capacity = HEADER_LENGTH
            .checked_add(thumb.len())
            .and_then(|size| size.checked_add(self.payload.len()))
            .ok_or_else(|| StateError::Corrupt("state exceeds platform limits".into()))?;

        let mut out = Vec::with_capacity(capacity);
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&ENVELOPE_VERSION.to_le_bytes());
        out.extend_from_slice(&system_tag(self.system).to_le_bytes());
        out.extend_from_slice(&self.rom_crc32.to_le_bytes());
        out.extend_from_slice(&self.rom_len.to_le_bytes());
        out.extend_from_slice(&width.to_le_bytes());
        out.extend_from_slice(&height.to_le_bytes());
        out.extend_from_slice(&payload_len.to_le_bytes());
        out.extend_from_slice(&self.payload_version.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        debug_assert_eq!(out.len(), HEADER_LENGTH);

        out.extend_from_slice(thumb);
        out.extend_from_slice(self.payload);
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Envelope<'_>, StateError> {
        if bytes.len() < HEADER_LENGTH {
            return Err(StateError::Truncated);
        }
        if bytes[..4] != MAGIC {
            return Err(StateError::BadMagic);
        }

        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != ENVELOPE_VERSION {
            return Err(StateError::Version {
                found: version,
                expected: ENVELOPE_VERSION,
            });
        }

        let payload_version = u16::from_le_bytes([bytes[24], bytes[25]]);
        if payload_version == 0 || bytes[26..28] != [0, 0] {
            return Err(StateError::Corrupt(
                "invalid payload version or reserved header bits".into(),
            ));
        }

        let system = system_from_tag(u16::from_le_bytes([bytes[6], bytes[7]]))
            .ok_or(StateError::SystemMismatch)?;
        let rom_crc32 = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let rom_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        let width = u16::from_le_bytes([bytes[16], bytes[17]]) as u32;
        let height = u16::from_le_bytes([bytes[18], bytes[19]]) as u32;
        let payload_len = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]) as usize;

        if (width == 0) != (height == 0) {
            return Err(StateError::Corrupt(
                "incomplete thumbnail dimensions".into(),
            ));
        }

        let thumb_len = Thumbnail::byte_length(width, height).ok_or(StateError::Truncated)?;
        let thumb_end = HEADER_LENGTH
            .checked_add(thumb_len)
            .ok_or(StateError::Truncated)?;
        let payload_end = thumb_end
            .checked_add(payload_len)
            .ok_or(StateError::Truncated)?;
        if bytes.len() < payload_end {
            return Err(StateError::Truncated);
        }

        if bytes.len() != payload_end {
            return Err(StateError::Corrupt(
                "trailing bytes after state payload".into(),
            ));
        }

        let thumbnail = (thumb_len > 0).then(|| Thumbnail {
            width,
            height,
            pixels: &bytes[HEADER_LENGTH..thumb_end],
        });

        Ok(Envelope {
            system,
            rom_crc32,
            rom_len,
            payload_version,
            thumbnail,
            payload: &bytes[thumb_end..payload_end],
        })
    }

    pub fn check_payload_version(&self, expected: u16) -> Result<(), StateError> {
        if self.payload_version == expected {
            Ok(())
        } else {
            Err(StateError::PayloadVersion {
                found: self.payload_version,
                expected,
            })
        }
    }

    pub fn check_rom(&self, crc32: u32, len: usize) -> Result<(), StateError> {
        if self.rom_crc32 == crc32 && self.rom_len as usize == len {
            Ok(())
        } else {
            Err(StateError::RomMismatch)
        }
    }
}

pub fn is_savestate(bytes: &[u8]) -> bool {
    bytes.len() >= MAGIC.len() && bytes[..MAGIC.len()] == MAGIC
}

pub fn peek_thumbnail(bytes: &[u8]) -> Option<Thumbnail<'_>> {
    Envelope::decode(bytes).ok()?.thumbnail
}

const fn system_tag(system: System) -> u16 {
    match system {
        System::Snes => 2,
    }
}

const fn system_from_tag(tag: u16) -> Option<System> {
    match tag {
        2 => Some(System::Snes),
        _ => None,
    }
}

use core_model::System;
use core_model::state::{Envelope, StateError, Thumbnail};

use crate::cartridge_header;
use crate::mmio::MemoryMap;
use crate::{Snes, ppu};

const PAYLOAD_VERSION: u16 = 3;

impl<M: MemoryMap> Snes<M> {
    pub fn save_state(&self) -> Result<Vec<u8>, StateError> {
        let payload = postcard::to_allocvec(self)
            .map_err(|err| StateError::Corrupt(format!("encoding state: {err}")))?;

        let frame = self.ppu.frame_buffer_rgba();
        let cartridge = &self.mmu.cartridge;

        Envelope {
            system: System::Snes,
            rom_crc32: cartridge_header::identity(cartridge.data()).crc32,
            rom_len: cartridge.data().len() as u32,
            payload_version: PAYLOAD_VERSION,
            thumbnail: Some(Thumbnail {
                width: ppu::SCREEN_WIDTH,
                height: ppu::SCREEN_HEIGHT,
                pixels: frame,
            }),
            payload: &payload,
        }
        .encode()
    }

    pub fn load_state(&mut self, bytes: &[u8]) -> Result<(), StateError> {
        let envelope = Envelope::decode(bytes)?;
        if envelope.system != System::Snes {
            return Err(StateError::SystemMismatch);
        }

        envelope.check_payload_version(PAYLOAD_VERSION)?;

        let cartridge = &self.mmu.cartridge;
        envelope.check_rom(
            cartridge_header::identity(cartridge.data()).crc32,
            cartridge.data().len(),
        )?;

        let mut restored: Snes<M> = postcard::from_bytes(envelope.payload)
            .map_err(|err| StateError::Corrupt(format!("decoding state: {err}")))?;

        let rom = self.mmu.cartridge.take_rom();
        restored.mmu.cartridge.set_rom(rom);

        restored.rebuild_devices();
        restored
            .ppu
            .restore_frame_buffer(envelope.thumbnail.map(|thumb| thumb.pixels));

        *self = restored;

        Ok(())
    }

    pub fn save_ram(&self) -> Option<&[u8]> {
        if !self.mmu.cartridge.has_battery() {
            return None;
        }

        let sram = self.mmu.cartridge.sram();
        (!sram.is_empty()).then_some(sram)
    }

    pub fn load_save_ram(&mut self, bytes: &[u8]) -> bool {
        if !self.mmu.cartridge.has_battery() {
            return false;
        }

        let sram = self.mmu.cartridge.sram_mut();
        if sram.is_empty() || sram.len() != bytes.len() {
            return false;
        }

        sram.copy_from_slice(bytes);

        true
    }
}

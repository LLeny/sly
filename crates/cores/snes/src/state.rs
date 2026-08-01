use core_model::System;
use core_model::state::{Envelope, StateError, Thumbnail};

use crate::mmio::MemoryMap;
use crate::{Snes, ppu};

impl<M: MemoryMap> Snes<M> {
    pub fn save_state(&self) -> Result<Vec<u8>, StateError> {
        let payload = postcard::to_allocvec(self)
            .map_err(|err| StateError::Corrupt(format!("encoding state: {err}")))?;

        let frame = self.ppu.frame_buffer_rgb();
        let cartridge = &self.mmu.cartridge;

        Ok(Envelope {
            system: System::Snes,
            rom_crc32: cartridge.rom_crc32(),
            rom_len: cartridge.rom.len() as u32,
            thumbnail: Some(Thumbnail {
                width: ppu::SCREEN_WIDTH,
                height: ppu::SCREEN_HEIGHT,
                pixels: &frame,
            }),
            payload: &payload,
        }
        .encode())
    }

    pub fn load_state(&mut self, bytes: &[u8]) -> Result<(), StateError> {
        let envelope = Envelope::decode(bytes)?;
        if envelope.system != System::Snes {
            return Err(StateError::SystemMismatch);
        }

        let cartridge = &self.mmu.cartridge;
        envelope.check_rom(cartridge.rom_crc32(), cartridge.rom.len())?;

        let mut restored: Snes<M> = postcard::from_bytes(envelope.payload)
            .map_err(|err| StateError::Corrupt(format!("decoding state: {err}")))?;

        restored.mmu.cartridge.rom = std::mem::take(&mut self.mmu.cartridge.rom);
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

        let sram = &self.mmu.cartridge.sram;
        (!sram.is_empty()).then_some(sram.as_slice())
    }

    pub fn load_save_ram(&mut self, bytes: &[u8]) -> bool {
        if !self.mmu.cartridge.has_battery() {
            return false;
        }

        let sram = &mut self.mmu.cartridge.sram;
        if sram.is_empty() || sram.len() != bytes.len() {
            return false;
        }

        sram.copy_from_slice(bytes);

        true
    }
}

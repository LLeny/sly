use std::borrow::Cow;
use std::cmp::Reverse;

use core_model::state::{Envelope, Thumbnail};
use core_model::{StateError, System};
use serde::{Deserialize, Serialize};

use crate::cpu::{Mode, Psr};
use crate::dma::{LatchedTransfer, control_from_io};
use crate::gamepak::SRAM_SIZE;
use crate::input::KeyInput;
use crate::mmio::{EWRAM_SIZE, IO_SIZE, IWRAM_SIZE};
use crate::ppu::Rgba;
use crate::ppu::regs::{DisplayStatus, VerticalCount};
use crate::scheduler::Event;
use crate::{Gba, SCREEN_HEIGHT, SCREEN_WIDTH, VIDEO_EVENTS};

const VERSION: u16 = 2;

#[derive(Serialize, Deserialize)]
struct State<'a> {
    registers: [u32; 16],
    cpsr: u32,
    spsr: [u32; 5],
    banks: [[u32; 16]; 5],
    user_bank: [u32; 16],
    pipeline: [Option<(u32, u32)>; 3],
    ewram: Cow<'a, [u8]>,
    iwram: Cow<'a, [u8]>,
    io: Cow<'a, [u8]>,
    vram: Cow<'a, [u8]>,
    palette: Cow<'a, [u8]>,
    oam: Cow<'a, [u8]>,
    frame: Cow<'a, [Rgba]>,
    sram: Cow<'a, [u8]>,
    dispstat: u16,
    vcount: u16,
    keyinput: u16,
    cycles: u64,
    deadlines: [u64; 4],
    dma_latches: [LatchedTransfer; 4],
}

impl Gba {
    pub(crate) fn encode_state(&self) -> Result<Vec<u8>, StateError> {
        let mut io = self.mmio.io.clone();
        self.dma.save_io(&mut io);
        self.ppu.save_io(&mut io);
        let state = State {
            registers: self.cpu.registers,
            cpsr: self.cpu.cpsr.raw(),
            spsr: self.cpu.spsr.map(|psr| psr.raw()),
            banks: self.cpu.banks,
            user_bank: self.cpu.user_bank,
            pipeline: self.cpu.pipeline.stages,
            ewram: Cow::Borrowed(&self.mmio.ewram),
            iwram: Cow::Borrowed(&self.mmio.iwram),
            io: Cow::Owned(io),
            vram: Cow::Borrowed(&self.ppu.vram),
            palette: Cow::Borrowed(&self.ppu.palette),
            oam: Cow::Borrowed(&self.ppu.oam),
            frame: Cow::Borrowed(&self.ppu.frame),
            sram: Cow::Borrowed(&self.gamepak.sram),
            dispstat: self.ppu.dispstat.raw(),
            vcount: self.ppu.vcount.raw(),
            keyinput: self.input.keyinput.raw(),
            cycles: self.scheduler.cycles,
            deadlines: VIDEO_EVENTS.map(|(kind, ..)| {
                self.scheduler
                    .events
                    .iter()
                    .find(|e| e.0.kind == kind)
                    .expect("periodic video event")
                    .0
                    .deadline
            }),
            dma_latches: self.dma.latches,
        };

        let payload = postcard::to_allocvec(&state)
            .map_err(|e| StateError::Corrupt(format!("encoding GBA state: {e}")))?;
        let rom = self.gamepak.cartridge().data();

        Envelope {
            system: System::Gba,
            rom_crc32: crc32fast::hash(rom),
            rom_len: rom.len() as u32,
            payload_version: VERSION,
            thumbnail: Some(Thumbnail {
                width: SCREEN_WIDTH,
                height: SCREEN_HEIGHT,
                pixels: self.ppu.frame.as_flattened(),
            }),
            payload: &payload,
        }
        .encode()
    }

    pub(crate) fn restore_state(&mut self, bytes: &[u8]) -> Result<(), StateError> {
        let envelope = Envelope::decode(bytes)?;
        if envelope.system != System::Gba {
            return Err(StateError::SystemMismatch);
        }
        envelope.check_payload_version(VERSION)?;

        let rom = self.gamepak.cartridge().data();
        envelope.check_rom(crc32fast::hash(rom), rom.len())?;
        if envelope.payload.len() > 1024 * 1024 {
            return Err(corrupt("oversized GBA payload"));
        }

        let (state, trailing): (State, _) = postcard::take_from_bytes(envelope.payload)
            .map_err(|e| StateError::Corrupt(format!("decoding GBA state: {e}")))?;
        if !trailing.is_empty() {
            return Err(corrupt("trailing GBA payload bytes"));
        }
        state.validate()?;

        self.cpu.registers = state.registers;
        self.cpu.cpsr = Psr::from_raw(state.cpsr);
        self.cpu.spsr = state.spsr.map(Psr::from_raw);
        self.cpu.banks = state.banks;
        self.cpu.user_bank = state.user_bank;
        self.cpu.pipeline.stages = state.pipeline;
        self.mmio.ewram = state.ewram.into_owned();
        self.mmio.iwram = state.iwram.into_owned();
        self.mmio.io = state.io.into_owned();
        self.dma.load_io(&self.mmio.io);
        self.ppu.load_io(&self.mmio.io);
        self.dma.latches = state.dma_latches;
        self.ppu.vram.copy_from_slice(&state.vram);
        self.ppu.palette.copy_from_slice(&state.palette);
        self.ppu.oam.copy_from_slice(&state.oam);
        self.ppu.frame.copy_from_slice(&state.frame);
        self.gamepak.sram = state.sram.into_owned();
        self.ppu.dispstat = DisplayStatus::from_raw(state.dispstat);
        self.ppu.vcount = VerticalCount::from_raw(state.vcount);
        self.input.keyinput = KeyInput::from_raw(state.keyinput);
        self.scheduler.cycles = state.cycles;
        self.scheduler.events = VIDEO_EVENTS
            .into_iter()
            .zip(state.deadlines)
            .map(|((kind, _, period), deadline)| {
                Reverse(Event {
                    kind,
                    deadline,
                    period,
                })
            })
            .collect();

        #[cfg(feature = "debug")]
        {
            self.ppu.revisions.bump();
            self.ppu.window_trace.clear();
        }

        Ok(())
    }
}

fn corrupt(message: &str) -> StateError {
    StateError::Corrupt(message.into())
}

impl State<'_> {
    fn validate(&self) -> Result<(), StateError> {
        let sizes = [
            (&self.ewram, EWRAM_SIZE),
            (&self.iwram, IWRAM_SIZE),
            (&self.io, IO_SIZE),
            (&self.vram, 0x18000),
            (&self.palette, 0x400),
            (&self.oam, 0x400),
            (&self.sram, SRAM_SIZE),
        ];
        if sizes
            .into_iter()
            .any(|(memory, size)| memory.len() != size as usize)
            || self.frame.len() != (SCREEN_WIDTH * SCREEN_HEIGHT) as usize
        {
            return Err(corrupt("invalid memory size"));
        }
        if !Mode::is_valid(self.cpsr) {
            return Err(corrupt("invalid CPU mode"));
        }

        let alignment = if self.cpsr & 0x20 != 0 { 2 } else { 4 };
        if !self.registers[15].is_multiple_of(alignment)
            || self.pipeline.iter().flatten().any(|&(addr, instruction)| {
                !addr.is_multiple_of(alignment) || (alignment == 2 && instruction > 0xFFFF)
            })
        {
            return Err(corrupt("invalid pipeline alignment or instruction"));
        }
        if self.pipeline[1].is_some() && self.pipeline[2].is_none() {
            return Err(corrupt("invalid pipeline occupancy"));
        }
        if self.vcount >= 228
            || self.dispstat & !DisplayStatus::READ_MASK != 0
            || self.keyinput & !0x3FF != 0
        {
            return Err(corrupt("invalid video or keypad state"));
        }

        for ((_, offset, period), deadline) in VIDEO_EVENTS.into_iter().zip(self.deadlines) {
            if deadline < self.cycles
                || deadline - self.cycles > period
                || deadline < offset
                || !(deadline - offset).is_multiple_of(period)
                || deadline.checked_add(period).is_none()
            {
                return Err(corrupt("invalid scheduler deadline"));
            }
        }

        if self
            .dma_latches
            .iter()
            .enumerate()
            .any(|(channel, latch)| !latch.is_valid(channel, control_from_io(&self.io, channel)))
        {
            return Err(corrupt("invalid DMA latch"));
        }

        Ok(())
    }
}

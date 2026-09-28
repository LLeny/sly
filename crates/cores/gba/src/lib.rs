pub mod cartridge;
pub mod cpu;
pub mod error;
pub mod mmio;

#[cfg(feature = "debug")]
pub mod debug;

pub use cartridge::Cartridge;
pub use cpu::Cpu;
use mmio::Mmio;

use crate::cpu::{Bus, Mode, Psr};

pub struct Gba {
    cpu: Cpu,
    mmio: Mmio,
    cycles: usize,
}

impl Gba {
    pub fn new(cartridge: Cartridge) -> Self {
        let mut gba = Self {
            cpu: Cpu::new(),
            mmio: Mmio::new(cartridge),
            cycles: 0,
        };
        gba.skip_bios();
        gba
    }

    pub fn skip_bios(&mut self) {
        self.cpu.cpsr = Psr::from_raw(0x1F);

        self.cpu.banks[Mode::Irq.index().unwrap()][13] = 0x0300_7FA0;
        self.cpu.banks[Mode::Supervisor.index().unwrap()][13] = 0x0300_7FE0;
        
        self.cpu.set_sp(0x0300_7F00);
        self.cpu.set_lr(0x0800_0000);
        self.cpu.set_pc(0x0800_0000);

        self.mmio.write8(0x0400_0300, 1); // POSTFLG
        self.mmio.write16(0x0400_0204, 0); // WAITCNT
    }
}

impl core_model::Core for Gba {
    fn reset(&mut self) {
        self.cpu = Cpu::new();
    }

    fn step(&mut self) {
        self.cpu.step(&mut self.mmio);
        self.cycles += 1;
    }

    fn run_frame(&mut self) {
        const FRAME_CYCLES: usize = 280_896;

        while self.cycles < FRAME_CYCLES {
            self.step();
        }

        self.cycles -= FRAME_CYCLES;
    }

    fn software_frame(&self) -> Option<core_model::SoftwareFrame<'_>> {
        static PIXELS: [[u8; 4]; 240 * 160] = [[255; 4]; 240 * 160];

        Some(core_model::SoftwareFrame {
            width: 240,
            height: 160,
            pixels: &PIXELS,
        })
    }

    fn frame_interval(&self) -> Option<std::time::Duration> {
        Some(std::time::Duration::from_secs_f64(280_896.0 / 16_777_216.0))
    }

    fn cartridge_info(&self) -> Option<core_model::CartridgeInfo> {
        Some(self.mmio.cartridge().info())
    }
}

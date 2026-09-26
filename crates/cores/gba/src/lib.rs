pub mod cartridge;
pub mod cpu;
pub mod error;

#[cfg(feature = "debug")]
pub mod debug;

pub use cartridge::Cartridge;
pub use cpu::Cpu;

pub struct Gba {
    cartridge: Cartridge,
    cpu: Cpu,
    cycles: usize,
}

impl Gba {
    pub fn new(cartridge: Cartridge) -> Self {
        Self {
            cartridge,
            cpu: Cpu::new(),
            cycles: 0,
        }
    }
}

impl core_model::Core for Gba {
    fn reset(&mut self) {
        self.cpu = Cpu::new();
    }

    fn step(&mut self) {
        self.cpu.step();
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
        Some(self.cartridge.info())
    }
}

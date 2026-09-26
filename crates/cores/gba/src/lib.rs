pub mod cartridge;
pub mod error;

#[cfg(feature = "debug")]
pub mod debug;

pub use cartridge::Cartridge;

pub struct Gba {
    cartridge: Cartridge,
}

impl Gba {
    pub fn new(cartridge: Cartridge) -> Self {
        Self { cartridge }
    }
}

impl core_model::Core for Gba {
    fn reset(&mut self) {
        todo!()
    }

    fn step(&mut self) {
        todo!()
    }

    fn run_frame(&mut self) {
        println!("lol");
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

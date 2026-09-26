pub mod layle;
pub mod libretro;

pub use core_model::System;

#[derive(Clone, Copy)]
pub struct SystemInfo {
    pub libretro_name: &'static str,
}

pub const fn info(system: System) -> SystemInfo {
    match system {
        System::Snes => SystemInfo {
            libretro_name: "Nintendo - Super Nintendo Entertainment System",
        },
    }
}

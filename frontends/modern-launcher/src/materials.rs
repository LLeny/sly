use core_model::System;

#[derive(Clone, Copy, Debug)]
pub struct ShellFinish {
    /// Linear RGB for the plastic only.
    pub color: [f32; 3],
    pub roughness: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct CartridgeFinish {
    pub id: &'static str,
    pub name: &'static str,
    pub shell: Option<ShellFinish>,
}

const DEFAULT_FINISH: CartridgeFinish = CartridgeFinish {
    id: "default",
    name: "Default",
    shell: None,
};

const SNES_FINISHES: &[CartridgeFinish] = &[DEFAULT_FINISH];

const GBA_FINISHES: &[CartridgeFinish] = &[
    DEFAULT_FINISH,
    CartridgeFinish {
        id: "red",
        name: "Red plastic",
        shell: Some(ShellFinish {
            color: [0.31, 0.018, 0.026],
            roughness: 0.24,
        }),
    },
    CartridgeFinish {
        id: "blue",
        name: "Blue plastic",
        shell: Some(ShellFinish {
            color: [0.015, 0.055, 0.31],
            roughness: 0.26,
        }),
    },
    CartridgeFinish {
        id: "green",
        name: "Green plastic",
        shell: Some(ShellFinish {
            color: [0.018, 0.24, 0.075],
            roughness: 0.23,
        }),
    },
];

const LYNX_FINISHES: &[CartridgeFinish] = &[
    DEFAULT_FINISH,
    CartridgeFinish {
        id: "black",
        name: "Black plastic",
        shell: Some(ShellFinish {
            color: [0.012, 0.012, 0.014],
            roughness: 0.3,
        }),
    },
];

pub fn cartridge_finishes(system: System) -> &'static [CartridgeFinish] {
    match system {
        System::Snes => SNES_FINISHES,
        System::Gba => GBA_FINISHES,
        System::Lynx => LYNX_FINISHES,
    }
}

pub fn find_cartridge_finish(system: System, id: &str) -> Option<&'static CartridgeFinish> {
    cartridge_finishes(system)
        .iter()
        .find(|finish| finish.id == id)
}

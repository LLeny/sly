mod arm7tdmi;
mod snes;

use std::path::Path;

use crate::harness::{self, OpReport, TestCore};

pub enum Runner {
    Snes(Box<dyn TestCore>),
    Arm7tdmi,
}

impl Runner {
    pub fn run_file(&mut self, path: &Path, limit: Option<usize>) -> anyhow::Result<OpReport> {
        match self {
            Self::Snes(core) => harness::run_file(core.as_mut(), path, limit),
            Self::Arm7tdmi => arm7tdmi::run_file(path, limit),
        }
    }
}

#[derive(Clone, Copy)]
pub enum FileFormat {
    HexOpcodes { suffix: &'static str },
    ArmGroups,
}

pub struct Target {
    pub name: &'static str,
    pub system: &'static str,
    pub tests: &'static str,
    pub format: FileFormat,
    build: fn() -> Runner,
}

impl Target {
    pub fn build(&self) -> Runner {
        (self.build)()
    }
}

static TARGETS: &[Target] = &[
    Target {
        name: "65816e",
        system: "snes",
        tests: "submodules/65816/v1",
        format: FileFormat::HexOpcodes { suffix: ".e" },
        build: || Runner::Snes(Box::new(snes::SnesCpu::new())),
    },
    Target {
        name: "65816n",
        system: "snes",
        tests: "submodules/65816/v1",
        format: FileFormat::HexOpcodes { suffix: ".n" },
        build: || Runner::Snes(Box::new(snes::SnesCpu::new())),
    },
    Target {
        name: "spc700",
        system: "snes",
        tests: "submodules/spc700/v1",
        format: FileFormat::HexOpcodes { suffix: "" },
        build: || Runner::Snes(Box::new(snes::Spc700::new())),
    },
    Target {
        name: "arm7tdmi",
        system: "gba",
        tests: "submodules/ARM7TDMI/v1",
        format: FileFormat::ArmGroups,
        build: || Runner::Arm7tdmi,
    },
];

pub fn select(selector: &str) -> Vec<&'static Target> {
    if let Some(target) = TARGETS.iter().find(|t| t.name == selector) {
        return vec![target];
    }

    TARGETS.iter().filter(|t| t.system == selector).collect()
}

pub fn available() -> Vec<&'static str> {
    let mut names = Vec::new();

    for target in TARGETS {
        if !names.contains(&target.system) {
            names.push(target.system);
        }

        names.push(target.name);
    }

    names
}

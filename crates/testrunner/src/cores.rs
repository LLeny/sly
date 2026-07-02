mod snes;

use crate::harness::TestCore;

pub struct Target {
    pub name: &'static str,
    pub system: &'static str,
    pub tests: &'static str,
    pub suffix: &'static str,
    build: fn() -> Box<dyn TestCore>,
}

impl Target {
    pub fn build(&self) -> Box<dyn TestCore> {
        (self.build)()
    }
}

static TARGETS: &[Target] = &[
    Target {
        name: "65816e",
        system: "snes",
        tests: "submodules/65816/v1",
        suffix: ".e",
        build: || Box::new(snes::SnesCpu::new()),
    },
    Target {
        name: "65816n",
        system: "snes",
        tests: "submodules/65816/v1",
        suffix: ".n",
        build: || Box::new(snes::SnesCpu::new()),
    },
    Target {
        name: "spc700",
        system: "snes",
        tests: "submodules/spc700/v1",
        suffix: "",
        build: || Box::new(snes::Spc700::new()),
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

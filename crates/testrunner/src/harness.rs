use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

use anyhow::Context;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Regs {
    pub pc: u16,
    pub s: u16,
    pub a: u16,
    pub x: u16,
    pub y: u16,
    pub p: u8,
    pub dbr: u8,
    pub d: u16,
    pub pbr: u8,
    pub e: u8,
}

#[derive(Debug, Clone, Deserialize)]
pub struct State {
    pub pc: u16,
    #[serde(alias = "sp")]
    pub s: u16,
    pub a: u16,
    pub x: u16,
    pub y: u16,
    #[serde(alias = "psw")]
    pub p: u8,
    #[serde(default)]
    pub dbr: u8,
    #[serde(default)]
    pub d: u16,
    #[serde(default)]
    pub pbr: u8,
    #[serde(default)]
    pub e: u8,
    pub ram: Vec<(u32, u8)>,
}

impl State {
    pub fn regs(&self) -> Regs {
        Regs {
            pc: self.pc,
            s: self.s,
            a: self.a,
            x: self.x,
            y: self.y,
            p: self.p,
            dbr: self.dbr,
            d: self.d,
            pbr: self.pbr,
            e: self.e,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Test {
    pub name: String,
    pub initial: State,
    #[serde(rename = "final")]
    pub expected: State,
}

pub trait TestCore {
    fn load(&mut self, initial: &State);
    fn step(&mut self);
    fn regs(&self) -> Regs;
    fn peek(&self, addr: u32) -> u8;
}

#[derive(Debug)]
pub struct Field {
    pub name: String,
    pub got: u64,
    pub want: u64,
    pub bytes: u8,
}

impl Field {
    pub fn matched(&self) -> bool {
        self.got == self.want
    }
}

pub enum Outcome {
    Clean,
    Empty,
    Panicked { test: String },
    Failed { test: String, fields: Vec<Field> },
}

pub struct OpReport {
    pub passed: usize,
    pub outcome: Outcome,
}

pub fn run_file(
    core: &mut dyn TestCore,
    path: &Path,
    limit: Option<usize>,
) -> anyhow::Result<OpReport> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(OpReport {
            passed: 0,
            outcome: Outcome::Empty,
        });
    }

    let tests: Vec<Test> =
        serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))?;

    let mut passed = 0;

    for test in tests.iter().take(limit.unwrap_or(usize::MAX)) {
        core.load(&test.initial);

        let ran = panic::catch_unwind(AssertUnwindSafe(|| core.step())).is_ok();
        if !ran {
            let outcome = Outcome::Panicked {
                test: test.name.clone(),
            };
            return Ok(OpReport { passed, outcome });
        }

        if self::state_matches(core, &test.expected) {
            passed += 1;
        } else {
            let fields = self::compare(core, &test.expected);
            let outcome = Outcome::Failed {
                test: test.name.clone(),
                fields,
            };
            return Ok(OpReport { passed, outcome });
        }
    }

    Ok(OpReport {
        passed,
        outcome: Outcome::Clean,
    })
}

fn state_matches(core: &dyn TestCore, want: &State) -> bool {
    core.regs() == want.regs() && want.ram.iter().all(|&(addr, val)| core.peek(addr) == val)
}

fn compare(core: &dyn TestCore, want: &State) -> Vec<Field> {
    let got = core.regs();
    let expect = want.regs();

    let reg = |name: &str, got: u64, want: u64, bytes| Field {
        name: name.into(),
        got,
        want,
        bytes,
    };

    let mut fields = vec![
        reg("pc", got.pc as u64, expect.pc as u64, 2),
        reg("s", got.s as u64, expect.s as u64, 2),
        reg("a", got.a as u64, expect.a as u64, 2),
        reg("x", got.x as u64, expect.x as u64, 2),
        reg("y", got.y as u64, expect.y as u64, 2),
        reg("p", got.p as u64, expect.p as u64, 1),
        reg("dbr", got.dbr as u64, expect.dbr as u64, 1),
        reg("d", got.d as u64, expect.d as u64, 2),
        reg("pbr", got.pbr as u64, expect.pbr as u64, 1),
        reg("e", got.e as u64, expect.e as u64, 1),
    ];

    for &(addr, want) in &want.ram {
        fields.push(Field {
            name: format!("mem[{addr:#08X}]"),
            got: core.peek(addr) as u64,
            want: want as u64,
            bytes: 1,
        });
    }

    fields
}

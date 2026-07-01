use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

use anyhow::Context;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Regs {
    pub pc: u16,
    pub sp: u8,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub psw: u8,
}

#[derive(Debug, Clone, Deserialize)]
pub struct State {
    pub pc: u16,
    #[serde(alias = "s")]
    pub sp: u8,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    #[serde(alias = "p")]
    pub psw: u8,
    pub ram: Vec<(u16, u8)>,
}

impl State {
    pub fn regs(&self) -> Regs {
        Regs {
            pc: self.pc,
            sp: self.sp,
            a: self.a,
            x: self.x,
            y: self.y,
            psw: self.psw,
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
    fn peek(&self, addr: u16) -> u8;

    fn ignore_flags(&self) -> u8 {
        0
    }
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

    let ignore = core.ignore_flags();
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

        if state_matches(core, &test.expected, ignore) {
            passed += 1;
        } else {
            let fields = compare(core, &test.expected, ignore);
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

fn mask_regs(mut r: Regs, ignore: u8) -> Regs {
    r.psw &= !ignore;
    r
}

fn state_matches(core: &dyn TestCore, want: &State, ignore: u8) -> bool {
    mask_regs(core.regs(), ignore) == mask_regs(want.regs(), ignore)
        && want.ram.iter().all(|&(addr, val)| core.peek(addr) == val)
}

fn compare(core: &dyn TestCore, want: &State, ignore: u8) -> Vec<Field> {
    let got = mask_regs(core.regs(), ignore);
    let expect = mask_regs(want.regs(), ignore);

    let reg = |name: &str, got: u64, want: u64, bytes| Field {
        name: name.into(),
        got,
        want,
        bytes,
    };

    let mut fields = vec![
        reg("pc", got.pc as u64, expect.pc as u64, 2),
        reg("sp", got.sp as u64, expect.sp as u64, 1),
        reg("a", got.a as u64, expect.a as u64, 1),
        reg("x", got.x as u64, expect.x as u64, 1),
        reg("y", got.y as u64, expect.y as u64, 1),
        reg("psw", got.psw as u64, expect.psw as u64, 1),
    ];

    for &(addr, want) in &want.ram {
        fields.push(Field {
            name: format!("mem[{addr:#06X}]"),
            got: core.peek(addr) as u64,
            want: want as u64,
            bytes: 1,
        });
    }

    fields
}

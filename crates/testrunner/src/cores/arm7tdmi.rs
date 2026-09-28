mod binary;
mod cpu;

use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

use anyhow::{Context, Result, ensure};
use gba::Cpu;
use gba::cpu::decoder::{arm, thumb};
use gba::cpu::{Bus, Mode};
use serde::Deserialize;

use crate::harness::{Field, OpReport, Outcome};
use cpu::{State as CpuState, TestCpu};

const FETCH: u32 = 0;
const READ: u32 = 1;
const WRITE: u32 = 2;

const MODES: [Mode; 5] = [
    Mode::Fiq,
    Mode::Supervisor,
    Mode::Abort,
    Mode::Irq,
    Mode::Undefined,
];
const BANK_NAMES: [&str; 5] = ["fiq", "svc", "abt", "irq", "und"];

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct State {
    #[serde(rename = "R")]
    registers: [u32; 16],
    #[serde(rename = "R_fiq")]
    fiq: [u32; 7],
    #[serde(rename = "R_svc")]
    svc: [u32; 2],
    #[serde(rename = "R_abt")]
    abt: [u32; 2],
    #[serde(rename = "R_irq")]
    irq: [u32; 2],
    #[serde(rename = "R_und")]
    und: [u32; 2],
    #[serde(rename = "CPSR")]
    cpsr: u32,
    #[serde(rename = "SPSR")]
    spsr: [u32; 5],
    pipeline: [u32; 2],
}

impl State {
    fn cpu_state(&self) -> CpuState {
        let mut state = CpuState {
            registers: self.registers,
            cpsr: self.cpsr,
            pipeline: self.pipeline,
            ..CpuState::default()
        };

        let banks: [&[u32]; 5] = [&self.fiq, &self.svc, &self.abt, &self.irq, &self.und];
        for ((mode, bank), &spsr) in MODES.iter().zip(banks).zip(&self.spsr) {
            let index = mode.index().unwrap();
            let range = mode.bank_register_range().unwrap();

            state.banks[index][range].copy_from_slice(bank);
            state.spsr[index] = spsr;
        }

        state
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct Transaction {
    kind: u32,
    size: u32,
    addr: u32,
    data: u32,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct Test {
    name: Option<String>,
    initial: State,
    #[serde(rename = "final")]
    expected: State,
    transactions: Vec<Transaction>,
    opcode: u32,
    base_addr: u32,
}

impl Test {
    fn signed_halfword_load(&self) -> bool {
        let opcode = self.initial.pipeline[0];
        if self.initial.cpsr & (1 << 5) != 0 {
            thumb::Instruction(opcode as u16).mnemonic() == thumb::Mnemonic::Ldrsh
        } else {
            arm::Instruction(opcode).mnemonic() == arm::Mnemonic::Ldrsh
        }
    }

    fn name(&self, index: usize) -> String {
        self.name.clone().unwrap_or_else(|| {
            format!(
                "#{index} opcode={:#010X} at {:#010X}",
                self.opcode, self.base_addr
            )
        })
    }

    fn validate(&self) -> Result<()> {
        for transaction in &self.transactions {
            ensure!(
                matches!(transaction.kind, FETCH | READ | WRITE),
                "invalid transaction kind {}",
                transaction.kind
            );
            ensure!(
                matches!(transaction.size, 1 | 2 | 4),
                "invalid transaction size {}",
                transaction.size
            );
        }

        Ok(())
    }
}

pub fn run_file(path: &Path, limit: Option<usize>) -> Result<OpReport> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;

    let run = || {
        if path.extension().is_some_and(|ext| ext == "bin") {
            run_tests(binary::Tests::new(&bytes)?, limit)
        } else if bytes.iter().all(u8::is_ascii_whitespace) {
            run_tests(std::iter::empty(), limit)
        } else {
            let tests: Vec<Test> = serde_json::from_slice(&bytes)?;
            run_tests(tests.into_iter().map(Ok), limit)
        }
    };

    run().with_context(|| format!("running {}", path.display()))
}

fn run_tests(tests: impl Iterator<Item = Result<Test>>, limit: Option<usize>) -> Result<OpReport> {
    let mut passed = 0;
    let mut cpu = Cpu::new();

    for (index, test) in tests.take(limit.unwrap_or(usize::MAX)).enumerate() {
        let test = test.with_context(|| format!("parsing test #{index}"))?;
        test.validate().with_context(|| test.name(index))?;

        let outcome = match panic::catch_unwind(AssertUnwindSafe(|| run_test(&mut cpu, &test))) {
            Ok(fields) if fields.is_empty() => {
                passed += 1;
                continue;
            }
            Ok(fields) => Outcome::Failed {
                test: test.name(index),
                fields,
            },
            Err(_) => Outcome::Panicked {
                test: test.name(index),
            },
        };

        return Ok(OpReport { passed, outcome });
    }

    let outcome = if passed == 0 {
        Outcome::Empty
    } else {
        Outcome::Clean
    };

    Ok(OpReport { passed, outcome })
}

fn run_test(cpu: &mut Cpu, test: &Test) -> Vec<Field> {
    cpu.load_test_state(&test.initial.cpu_state());

    let mut bus = TransactionBus::new(&test.transactions, test.signed_halfword_load());
    cpu.step_test_instruction(&mut bus);

    let got = cpu.test_state();
    let mut want = test.expected.cpu_state();
    want.registers[15] &= if want.cpsr & (1 << 5) != 0 { !1 } else { !3 };
    let mut fields = bus.fields;
    if got == want && bus.consumed == test.transactions.len() {
        return fields;
    }

    compare(
        &mut fields,
        bus.consumed as u32,
        test.transactions.len() as u32,
        || "transactions".into(),
    );

    for (reg, (&got, &want)) in got.registers.iter().zip(&want.registers).enumerate() {
        compare(&mut fields, got, want, || format!("R{reg}"));
    }

    compare(&mut fields, got.cpsr, want.cpsr, || "CPSR".into());

    for (mode, name) in MODES.iter().zip(BANK_NAMES) {
        let index = mode.index().unwrap();

        for reg in mode.bank_register_range().unwrap() {
            compare(
                &mut fields,
                got.banks[index][reg],
                want.banks[index][reg],
                || format!("R{reg}_{name}"),
            );
        }

        compare(&mut fields, got.spsr[index], want.spsr[index], || {
            format!("SPSR_{name}")
        });
    }

    for (stage, (&got, &want)) in got.pipeline.iter().zip(&want.pipeline).enumerate() {
        compare(&mut fields, got, want, || format!("pipeline[{stage}]"));
    }

    fields
}

fn compare(fields: &mut Vec<Field>, got: u32, want: u32, name: impl FnOnce() -> String) {
    if got != want {
        fields.push(Field::hex(name(), got, want));
    }
}

struct TransactionBus<'a> {
    expected: &'a [Transaction],
    consumed: usize,
    fields: Vec<Field>,
    signed_halfword_load: bool,
}

impl<'a> TransactionBus<'a> {
    fn new(expected: &'a [Transaction], signed_halfword_load: bool) -> Self {
        Self {
            expected,
            consumed: 0,
            fields: Vec::new(),
            signed_halfword_load,
        }
    }

    fn transact(&mut self, kind: u32, size: u32, addr: u32, data: u32) -> u32 {
        let index = self.consumed;
        self.consumed += 1;

        let fields = &mut self.fields;
        let Some(want) = self.expected.get(index) else {
            compare(fields, 1, 0, || format!("bus[{index}].unexpected"));
            return 0;
        };

        compare(fields, kind, want.kind, || format!("bus[{index}].kind"));

        // Fuck you
        if self.signed_halfword_load
            && kind == READ
            && want.kind == READ
            && size == 1
            && want.size == 2
            && want.addr & 1 != 0
            && addr == want.addr
        {
            return (want.data >> 8) & 0xFF;
        }

        compare(fields, size, want.size, || format!("bus[{index}].size"));

        // NBA records the requested address before bus alignment. We align
        // halfword/word transactions in the interpreter before issuing them.
        compare(fields, addr, want.addr & !(want.size - 1), || {
            format!("bus[{index}].addr")
        });

        let value = want.data & (u32::MAX >> (32 - want.size * 8));
        if kind == WRITE {
            compare(fields, data, value, || format!("bus[{index}].data"));
        }

        value
    }
}

impl Bus for TransactionBus<'_> {
    fn read8(&mut self, address: u32) -> u8 {
        self.transact(READ, 1, address, 0) as u8
    }

    fn read16(&mut self, address: u32) -> u16 {
        self.transact(READ, 2, address, 0) as u16
    }

    fn read32(&mut self, address: u32) -> u32 {
        self.transact(READ, 4, address, 0)
    }

    fn write8(&mut self, address: u32, value: u8) {
        self.transact(WRITE, 1, address, value.into());
    }

    fn write16(&mut self, address: u32, value: u16) {
        self.transact(WRITE, 2, address, value.into());
    }

    fn write32(&mut self, address: u32, value: u32) {
        self.transact(WRITE, 4, address, value);
    }

    fn fetch16(&mut self, address: u32) -> u16 {
        self.transact(FETCH, 2, address, 0) as u16
    }

    fn fetch32(&mut self, address: u32) -> u32 {
        self.transact(FETCH, 4, address, 0)
    }
}

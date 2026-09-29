use debugger_model::{CpuView, FlagView, Highlight, RegisterView};

use crate::Cpu;
use crate::cpu::{Mode, Psr};

pub(super) fn execution_address(cpu: &Cpu) -> u32 {
    cpu.pipeline
        .pending()
        .into_iter()
        .flatten()
        .next()
        .map_or(cpu.pc(), |(addr, _)| addr)
}

pub(super) fn view(cpu: &Cpu) -> CpuView {
    let mut registers: Vec<_> = cpu
        .registers
        .iter()
        .enumerate()
        .map(|(i, &value)| match i {
            13 => RegisterView::new("SP", value, 32),
            14 => RegisterView::new("LR", value, 32),
            15 => RegisterView {
                highlight: Highlight::Pipeline,
                ..RegisterView::new("PC", value, 32)
            },
            _ => RegisterView::new(format!("R{i}"), value, 32),
        })
        .collect();

    registers.push(RegisterView::new("CPSR", cpu.cpsr.raw(), 32));
    if let Some(bank) = cpu.cpsr.mode().index() {
        registers.push(RegisterView::new("SPSR", cpu.spsr[bank].raw(), 32));
    }

    let isa = if cpu.is_thumb() { "Thumb" } else { "ARM" };

    CpuView {
        isa: format!("{isa} / {}", cpu.cpsr.mode().name()).into(),
        registers,
        flags: [
            ("N", cpu.cpsr.n()),
            ("Z", cpu.cpsr.z()),
            ("C", cpu.cpsr.c()),
            ("V", cpu.cpsr.v()),
            ("I", cpu.cpsr.i()),
            ("F", cpu.cpsr.f()),
            ("T", cpu.cpsr.t()),
        ]
        .into_iter()
        .map(|(name, set)| FlagView::new(name, set))
        .collect(),
    }
}

pub(super) fn write(cpu: &mut Cpu, name: &str, value: u32) -> bool {
    let name = name.to_ascii_uppercase();
    let index = match name.as_str() {
        "CPSR" | "SPSR" if !Mode::is_valid(value) => return false,
        "CPSR" => {
            let pc = execution_address(cpu);
            let psr = Psr::from_raw(value);
            cpu.switch_mode(psr.mode());
            cpu.cpsr = psr;
            cpu.set_pc(pc);
            return true;
        }
        "SPSR" => {
            let Some(bank) = cpu.cpsr.mode().index() else {
                return false;
            };
            cpu.spsr[bank] = Psr::from_raw(value);
            return true;
        }
        "SP" => 13,
        "LR" => 14,
        "PC" => 15,
        _ => match name
            .strip_prefix('R')
            .and_then(|n| n.parse::<usize>().ok())
            .filter(|&n| n < 16)
        {
            Some(index) => index,
            None => return false,
        },
    };

    cpu.set_register(index, value);
    true
}

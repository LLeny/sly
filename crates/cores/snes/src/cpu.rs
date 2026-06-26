mod decoder;
pub mod disasm;
mod interpreter;

pub use disasm::{Disasm, disassemble_at};

use crate::Snes;
use interpreter::InterpreterView;

pub struct Cpu {
    pub acc: u16,
    pub index_x: u16,
    pub index_y: u16,
    pub data_bank: u8,
    pub direct_page: u16,
    pub program_bank: u8,
    pub flags: Flags,
    pub pc: u16,
    pub stack_pointer: u16,
}

impl Cpu {
    pub fn new(addr: u16) -> Self {
        Self {
            pc: addr,
            acc: 0,
            index_x: 0,
            index_y: 0,
            data_bank: 0,
            direct_page: 0,
            program_bank: 0,
            flags: Flags::default(),
            stack_pointer: 0x01ff,
        }
    }
}

pub fn step(snes: &mut Snes) {
    let (bank, pc, modes) = {
        let cpu = &snes.cpu;
        (
            cpu.program_bank,
            cpu.pc,
            decoder::pack_modes(cpu.flags.m() as u64, cpu.flags.x() as u64),
        )
    };

    let opcode = snes.read8(bank, pc);
    let len = decoder::stream_len(modes, opcode);

    let mut view = InterpreterView::new(&mut *snes, len);
    decoder::run_ops(modes, &mut view, opcode);
    let (next_bank, next_pc) = view.next();

    snes.cpu.program_bank = next_bank;
    snes.cpu.pc = next_pc;
}

#[chapa::bitfield(u8, order = msb0)]
#[derive(Default, Debug, Clone, Copy)]
pub struct Flags {
    #[bits(0, alias = "c")]
    pub carry: bool,

    #[bits(1, alias = "z")]
    pub zero_: bool,

    #[bits(2, alias = "i")]
    pub irq_disable: bool,

    #[bits(3, alias = "d")]
    pub decimal: bool,

    #[bits(4, alias = "x", default = true)]
    pub index_register_size: bool,

    #[bits(5, alias = "m", default = true)]
    pub accumulator_register_size: bool,

    #[bits(6, alias = "v")]
    pub overflow: bool,

    #[bits(7, alias = "n")]
    pub negative: bool,
}

impl Flags {
    pub fn is_acc_16bits(&self) -> bool {
        !self.m()
    }

    pub fn is_index_16bits(&self) -> bool {
        !self.x()
    }
}

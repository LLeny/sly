use crate::interpreter::InterpreterView;
use crate::{Bus, decoder};

pub struct Cpu {
    pub acc: u16,
    pub index_x: u16,
    pub index_y: u16,
    pub data_bank: u8,
    pub direct_page: u16,
    pub program_bank: u8,
    pub flags: Flags,
    pub pc: u16,
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
            flags: Flags::new().with_m(true).with_x(true),
        }
    }

    pub fn step(&mut self, bus: &mut (impl Bus + ?Sized)) {
        let cia = ((self.program_bank as u32) << 16) | (self.pc as u32);
        let opcode = bus.read(cia);

        let modes = crate::combo(self.flags.m(), self.flags.x());
        let len = decoder::stream_len(modes, opcode);

        let mut view = InterpreterView::new(&mut *self, bus, len);
        decoder::run_ops(modes, &mut view, opcode);
        let (next_bank, next_pc) = view.next();

        self.program_bank = next_bank;
        self.pc = next_pc;
    }
}

#[chapa::bitfield(u8, order = msb0)]
#[derive(Debug, Clone, Copy)]
pub struct Flags {
    #[bits(0, alias = "c")]
    pub carry: bool,

    #[bits(1, alias = "z")]
    pub zero: bool,

    #[bits(2, alias = "i")]
    pub irq_disable: bool,

    #[bits(3, alias = "d")]
    pub decimal: bool,

    #[bits(4, alias = "x")]
    pub index_register_size: bool,

    #[bits(5, alias = "m")]
    pub accumulator_register_size: bool,

    #[bits(6, alias = "v")]
    pub overflow: bool,

    #[bits(7, alias = "n")]
    pub negative: bool,
}

use crate::interpreter::InterpreterView;
use crate::{Bus, decoder};

pub struct Cpu {
    pub pc: u16,
    pub flags: Flags,
}

impl Cpu {
    pub fn new(addr: u16) -> Self {
        Self {
            pc: addr,
            flags: Flags::new().with_m(true).with_x(true),
        }
    }

    pub fn step(&mut self, bus: &(impl Bus + ?Sized)) {
        let opcode = bus.read(self.pc as u32);
        let modes = crate::combo(self.flags.m(), self.flags.x());

        let len = decoder::stream_len(modes, opcode);

        let mut view = InterpreterView::new(&mut *self, bus);
        decoder::run_ops(modes, &mut view, opcode);

        self.pc = self.pc.wrapping_add(len as u16);
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

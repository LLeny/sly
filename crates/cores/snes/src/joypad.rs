pub const JOYSER0: u16 = 0x4016;
pub const JOYSER1: u16 = 0x4017;

pub const JOYSER1_ALWAYS_SET: u8 = 0b0001_1100;

pub const JOYPAD_IO_START: u16 = 0x4218;
pub const JOYPAD_IO_END: u16 = 0x421F;

#[chapa::bitfield(u16, order = lsb0)]
#[derive(Default, Clone, Copy)]
pub struct Buttons {
    #[bits(4)]
    pub r: bool,

    #[bits(5)]
    pub l: bool,

    #[bits(6)]
    pub x: bool,

    #[bits(7)]
    pub a: bool,

    #[bits(8)]
    pub right: bool,

    #[bits(9)]
    pub left: bool,

    #[bits(10)]
    pub down: bool,

    #[bits(11)]
    pub up: bool,

    #[bits(12)]
    pub start: bool,

    #[bits(13)]
    pub select: bool,

    #[bits(14)]
    pub y: bool,

    #[bits(15)]
    pub b: bool,
}

#[derive(Default)]
pub struct Joypads {
    input: [Buttons; 2],
    latched: [u16; 4],
}

impl Joypads {
    pub fn set_input(&mut self, port: usize, buttons: Buttons) {
        self.input[port] = buttons;
    }

    pub fn input(&self, port: usize) -> Buttons {
        self.input[port]
    }

    pub(crate) fn reset(&mut self) {
        self.latched = [0; 4];
    }

    pub(crate) fn autoread(&mut self) {
        self.latched = [self.input[0].raw(), self.input[1].raw(), 0, 0];
    }

    pub(crate) fn read(&self, reg: u16) -> u8 {
        let index = (reg - JOYPAD_IO_START) as usize;
        let word = self.latched[index / 2];
        (word >> ((index % 2) * 8)) as u8
    }
}

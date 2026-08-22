use crate::SnesBus;
use crate::mmio::MemoryMap;
use crate::mmio::device::{IoRegister, RegEntry};
use serde::{Deserialize, Serialize};

pub const JOYSER0: u16 = 0x4016;
pub const JOYSER1: u16 = 0x4017;

pub const JOYPAD_IO_START: u16 = 0x4218;
pub const JOYPAD_IO_END: u16 = 0x421F;

pub fn registers<M: MemoryMap>() -> Vec<RegEntry<M>> {
    vec![Serial0::entry(), Serial1::entry(), AutoRead::entry()]
}

struct Serial0;

impl IoRegister for Serial0 {
    const START: u16 = JOYSER0;

    fn read<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16) -> u8 {
        snes.joypads.read_serial(0)
    }

    fn write<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16, value: u8) {
        snes.joypads
            .set_strobe(JoypadOutput::from_raw(value).strobe());
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        snes.joypads.peek_serial(0)
    }
}

struct Serial1;

impl IoRegister for Serial1 {
    const START: u16 = JOYSER1;

    fn read<M: MemoryMap>(snes: &mut SnesBus<M>, _reg: u16) -> u8 {
        snes.joypads.read_serial(1)
    }

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, _reg: u16) -> u8 {
        snes.joypads.peek_serial(1)
    }
}

struct AutoRead;

impl IoRegister for AutoRead {
    const START: u16 = JOYPAD_IO_START;
    const END: u16 = JOYPAD_IO_END;

    fn peek<M: MemoryMap>(snes: &SnesBus<M>, reg: u16) -> u8 {
        snes.joypads.read(reg)
    }
}

#[chapa::bitfield(u8, order = lsb0)]
pub struct JoypadOutput {
    #[bits(0)]
    pub strobe: bool,

    #[bits(1)]
    pub out1: bool,

    #[bits(2)]
    pub out2: bool,
}

#[chapa::bitfield(u16, order = lsb0)]
#[derive(Default)]
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

#[chapa::bitfield(u8, order = lsb0)]
#[derive(Default)]
pub struct SerialPort {
    #[bits(0)]
    pub data1: bool,

    #[bits(1)]
    pub data2: bool,

    #[bits(2..=4)]
    pub expansion: u8,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Joypads {
    input: [Buttons; 2],
    latched: [u16; 4],
    strobe: bool,
    shift: [u16; 2],
}

impl Joypads {
    pub fn set_input(&mut self, port: usize, buttons: Buttons) {
        self.input[port] = buttons;
    }

    pub fn set_button(&mut self, port: usize, button: core_model::Button, pressed: bool) {
        use core_model::Button as B;

        let Some(input) = self.input.get_mut(port) else {
            return;
        };

        match button {
            B::Up => input.set_up(pressed),
            B::Down => input.set_down(pressed),
            B::Left => input.set_left(pressed),
            B::Right => input.set_right(pressed),
            B::A => input.set_a(pressed),
            B::B => input.set_b(pressed),
            B::X => input.set_x(pressed),
            B::Y => input.set_y(pressed),
            B::L => input.set_l(pressed),
            B::R => input.set_r(pressed),
            B::Start => input.set_start(pressed),
            B::Select => input.set_select(pressed),
        }
    }

    pub fn input(&self, port: usize) -> Buttons {
        self.input[port]
    }

    pub(crate) fn reset(&mut self) {
        *self = Self {
            input: self.input,
            ..Self::default()
        };
    }

    pub(crate) fn autoread(&mut self) {
        self.set_strobe(true);
        self.set_strobe(false);

        self.latched = [self.shift[0], self.shift[1], 0, 0];
        self.shift = [u16::MAX; 2];
    }

    pub(crate) fn set_strobe(&mut self, high: bool) {
        if self.strobe && !high {
            self.shift = [self.input[0].raw(), self.input[1].raw()];
        }

        self.strobe = high;
    }

    pub(crate) fn peek_serial(&self, port: usize) -> u8 {
        let shift = if self.strobe {
            self.input[port].raw()
        } else {
            self.shift[port]
        };

        let mut value = SerialPort::default();
        value.set_data1(shift & 0x8000 != 0);

        if port == 1 {
            value.set_expansion(0b111);
        }

        value.raw()
    }

    pub(crate) fn read_serial(&mut self, port: usize) -> u8 {
        let value = self.peek_serial(port);

        if !self.strobe {
            self.shift[port] = (self.shift[port] << 1) | 1;
        }

        value
    }

    pub(crate) fn read(&self, reg: u16) -> u8 {
        let index = (reg - JOYPAD_IO_START) as usize;
        let word = self.latched[index / 2];
        (word >> ((index % 2) * 8)) as u8
    }
}

crate::macros::impl_bitfield_serde!(Buttons, SerialPort,);

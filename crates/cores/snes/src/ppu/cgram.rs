use crate::ppu::latch::WordLatch;

pub struct Cgram {
    words: [u16; 256],
    addr: u8,
    latch: WordLatch,
}

impl Cgram {
    pub fn new() -> Self {
        Self {
            words: [0; 256],
            addr: 0,
            latch: WordLatch::default(),
        }
    }

    pub fn word(&self, index: u8) -> u16 {
        self.words[index as usize]
    }

    pub fn word_addr(&self) -> u8 {
        self.addr
    }

    pub(crate) fn set_addr(&mut self, value: u8) {
        self.addr = value;
        self.latch.reset();
    }

    pub(crate) fn write_data(&mut self, value: u8) {
        if let Some(word) = self.latch.write(value) {
            self.words[self.addr as usize] = word & 0x7FFF;
            self.addr = self.addr.wrapping_add(1);
        }
    }

    pub(crate) fn read_data(&mut self) -> u8 {
        let value = self.peek_data();

        if self.latch.toggle() {
            self.addr = self.addr.wrapping_add(1);
        }

        value
    }

    pub(crate) fn peek_data(&self) -> u8 {
        let word = self.words[self.addr as usize];
        if self.latch.is_high() {
            (word >> 8) as u8
        } else {
            word as u8
        }
    }
}

const RED_SHIFT: u16 = 0;
const GREEN_SHIFT: u16 = 5;
const BLUE_SHIFT: u16 = 10;
pub const CHANNEL_SHIFTS: [u16; 3] = [RED_SHIFT, GREEN_SHIFT, BLUE_SHIFT];
pub const CHANNEL_MASK: u16 = 0b1_1111;

pub fn direct_color(palette: u8, color: u8) -> u16 {
    let (palette, color) = (palette as u16, color as u16);

    let red = (color & 0b111) << 2 | (palette & 0b001) << 1;
    let green = (color >> 3 & 0b111) << 2 | (palette >> 1 & 0b1) << 1;
    let blue = (color >> 6 & 0b11) << 3 | (palette >> 2 & 0b1) << 2;

    red << RED_SHIFT | green << GREEN_SHIFT | blue << BLUE_SHIFT
}

pub fn rgb(word: u16) -> (u8, u8, u8) {
    (
        self::expand5(word >> RED_SHIFT & CHANNEL_MASK),
        self::expand5(word >> GREEN_SHIFT & CHANNEL_MASK),
        self::expand5(word >> BLUE_SHIFT & CHANNEL_MASK),
    )
}

fn expand5(channel: u16) -> u8 {
    let channel = channel as u8;
    (channel << 3) | (channel >> 2)
}

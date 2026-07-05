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

pub fn rgb(word: u16) -> (u8, u8, u8) {
    (
        self::expand5(word & 0x1F),
        self::expand5((word >> 5) & 0x1F),
        self::expand5((word >> 10) & 0x1F),
    )
}

fn expand5(channel: u16) -> u8 {
    let channel = channel as u8;
    (channel << 3) | (channel >> 2)
}

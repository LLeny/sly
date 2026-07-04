pub struct Cgram {
    words: [u16; 256],
    addr: u8,
    high: bool,
    latch: u8,
}

impl Cgram {
    pub fn new() -> Self {
        Self {
            words: [0; 256],
            addr: 0,
            high: false,
            latch: 0,
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
        self.high = false;
    }

    pub(crate) fn write_data(&mut self, value: u8) {
        if self.high {
            self.words[self.addr as usize] = u16::from_le_bytes([self.latch, value & 0x7F]);
            self.addr = self.addr.wrapping_add(1);
        } else {
            self.latch = value;
        }

        self.high = !self.high;
    }

    pub(crate) fn read_data(&mut self) -> u8 {
        let value = self.peek_data();

        if self.high {
            self.addr = self.addr.wrapping_add(1);
        }
        self.high = !self.high;

        value
    }

    pub(crate) fn peek_data(&self) -> u8 {
        let word = self.words[self.addr as usize];
        if self.high {
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

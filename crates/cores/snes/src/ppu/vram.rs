const WORDS: usize = 0x8000;

pub struct Vram {
    words: Vec<u16>,
    addr: u16,
    main: u8,
    prefetch: u16,
}

impl Vram {
    pub fn new() -> Self {
        Self {
            words: vec![0; WORDS],
            addr: 0,
            main: 0,
            prefetch: 0,
        }
    }

    pub fn word(&self, index: u16) -> u16 {
        self.words[index as usize % WORDS]
    }

    pub fn byte(&self, addr: u32) -> u8 {
        let word = self.word((addr >> 1) as u16);
        if addr & 1 == 1 {
            (word >> 8) as u8
        } else {
            word as u8
        }
    }

    pub fn main(&self) -> u8 {
        self.main
    }

    pub fn word_addr(&self) -> u16 {
        self.addr
    }

    pub(crate) fn set_main(&mut self, value: u8) {
        self.main = value;
    }

    pub(crate) fn set_addr_low(&mut self, value: u8) {
        self.addr = (self.addr & 0xFF00) | value as u16;
        self.prefetch = self.word(self.translated());
    }

    pub(crate) fn set_addr_high(&mut self, value: u8) {
        self.addr = (self.addr & 0x00FF) | ((value as u16) << 8);
        self.prefetch = self.word(self.translated());
    }

    pub(crate) fn write_low(&mut self, value: u8) {
        let index = self.translated() as usize % WORDS;
        self.words[index] = (self.words[index] & 0xFF00) | value as u16;

        if !self.increment_on_high() {
            self.increment();
        }
    }

    pub(crate) fn write_high(&mut self, value: u8) {
        let index = self.translated() as usize % WORDS;
        self.words[index] = (self.words[index] & 0x00FF) | ((value as u16) << 8);

        if self.increment_on_high() {
            self.increment();
        }
    }

    pub(crate) fn read_low(&mut self) -> u8 {
        let value = self.peek_low();

        if !self.increment_on_high() {
            self.prefetch = self.word(self.translated());
            self.increment();
        }

        value
    }

    pub(crate) fn read_high(&mut self) -> u8 {
        let value = self.peek_high();

        if self.increment_on_high() {
            self.prefetch = self.word(self.translated());
            self.increment();
        }

        value
    }

    pub(crate) fn peek_low(&self) -> u8 {
        self.prefetch as u8
    }

    pub(crate) fn peek_high(&self) -> u8 {
        (self.prefetch >> 8) as u8
    }

    fn increment_on_high(&self) -> bool {
        self.main & 0x80 != 0
    }

    fn increment(&mut self) {
        let step = match self.main & 0x3 {
            0 => 1,
            1 => 32,
            _ => 128,
        };
        self.addr = self.addr.wrapping_add(step);
    }

    fn translated(&self) -> u16 {
        let addr = self.addr;
        match (self.main >> 2) & 0x3 {
            0 => addr,
            1 => (addr & 0xFF00) | ((addr << 3) & 0x00F8) | ((addr >> 5) & 0x0007),
            2 => (addr & 0xFE00) | ((addr << 3) & 0x01F8) | ((addr >> 6) & 0x0007),
            _ => (addr & 0xFC00) | ((addr << 3) & 0x03F8) | ((addr >> 7) & 0x0007),
        }
    }
}

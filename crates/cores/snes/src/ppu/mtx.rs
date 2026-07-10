use crate::ppu::latch::ByteLatch;

#[derive(Default)]
pub struct Matrix {
    pub a: i16,
    pub b: i16,
    pub c: i16,
    pub d: i16,
    pub center_x: i16,
    pub center_y: i16,
    pub hofs: i16,
    pub vofs: i16,
    pub result: i32, // 24bit
    latch: ByteLatch,
}

impl Matrix {
    pub fn latch(&mut self, value: u8) -> u16 {
        let prev = self.latch.exchange(value);
        (value as u16) << 8 | prev as u16
    }

    pub fn update_result(&mut self) {
        self.result = self.a as i32 * (self.b >> 8) as i32;
    }
}

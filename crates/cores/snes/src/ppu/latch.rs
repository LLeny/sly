use serde::{Deserialize, Serialize};

#[derive(Default, Clone, Copy, Serialize, Deserialize)]
pub struct ByteLatch(u8);

impl ByteLatch {
    pub fn exchange(&mut self, value: u8) -> u8 {
        core::mem::replace(&mut self.0, value)
    }
}

#[derive(Default, Clone, Copy, Serialize, Deserialize)]
pub struct WordLatch {
    low: u8,
    high: bool,
}

impl WordLatch {
    pub fn write(&mut self, value: u8) -> Option<u16> {
        if self.toggle() {
            Some(u16::from_le_bytes([self.low, value]))
        } else {
            self.low = value;
            None
        }
    }

    pub fn is_high(&self) -> bool {
        self.high
    }

    pub fn toggle(&mut self) -> bool {
        let high = self.high;
        self.high = !self.high;
        high
    }

    pub fn reset(&mut self) {
        self.high = false;
    }
}

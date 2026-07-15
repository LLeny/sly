use super::tables::{COUNTER_RANGE, RATE, RATE_OFFSET};

pub(super) struct RateCounter {
    value: i32,
}

impl RateCounter {
    pub(super) const fn new() -> Self {
        Self { value: 0 }
    }

    pub(super) fn advance(&mut self) {
        self.value -= 1;
        if self.value < 0 {
            self.value = COUNTER_RANGE - 1;
        }
    }

    pub(super) fn fires(&self, rate: usize) -> bool {
        ((self.value + RATE_OFFSET[rate]) as u32).is_multiple_of(RATE[rate])
    }
}

pub(super) struct NoiseGenerator {
    level: i32,
}

impl NoiseGenerator {
    pub(super) const fn new() -> Self {
        Self { level: 0x4000 }
    }

    pub(super) fn clock(&mut self, fires: bool) {
        if fires {
            let feedback = (self.level << 13) ^ (self.level << 14);
            self.level = (feedback & 0x4000) ^ (self.level >> 1);
        }
    }

    pub(super) const fn sample(&self) -> i32 {
        self.level
    }
}

pub(super) struct KeyOnLatch {
    poll_this_sample: bool,
    current: u8,
    pending: u8,
}

impl KeyOnLatch {
    pub(super) const fn new() -> Self {
        Self {
            poll_this_sample: true,
            current: 0,
            pending: 0,
        }
    }

    pub(super) fn write(&mut self, voices: u8) {
        self.pending = voices;
    }

    #[inline]
    pub(super) fn begin_sample(&mut self) -> (bool, u8) {
        self.poll_this_sample = !self.poll_this_sample;

        if self.poll_this_sample {
            self.pending &= !self.current;
            self.current = self.pending;
        }

        (self.poll_this_sample, self.current)
    }
}

pub struct Apu {
    pub ports: [u8; 4],
}

impl Apu {
    pub fn new() -> Self {
        Self { ports: [0; 4] }
    }
}

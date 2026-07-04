#[derive(Clone, Copy)]
pub struct Tile {
    pub data: [u8; 64],
}

impl Default for Tile {
    fn default() -> Self {
        Self { data: [0; 64] }
    }
}

impl Tile {
    pub fn decode(&self, bpp: u8) -> [[u8; 8]; 8] {
        let mut pixels = [[0u8; 8]; 8];

        for (y, row) in pixels.iter_mut().enumerate() {
            for pair in 0..(bpp as usize / 2) {
                let low = self.data[pair * 16 + y * 2];
                let high = self.data[pair * 16 + y * 2 + 1];

                for (x, pixel) in row.iter_mut().enumerate() {
                    let bit0 = (low >> (7 - x)) & 1;
                    let bit1 = (high >> (7 - x)) & 1;
                    *pixel |= (bit1 << 1 | bit0) << (pair * 2);
                }
            }
        }

        pixels
    }
}

use serde::{Deserialize, Serialize};

pub const WRMPYA: u16 = 0x4202;
pub const WRMPYB: u16 = 0x4203;
pub const WRDIVL: u16 = 0x4204;
pub const WRDIVH: u16 = 0x4205;
pub const WRDIVB: u16 = 0x4206;

pub const RDDIVL: u16 = 0x4214;
pub const RDDIVH: u16 = 0x4215;
pub const RDMPYL: u16 = 0x4216;
pub const RDMPYH: u16 = 0x4217;

#[derive(Serialize, Deserialize)]
pub struct Math {
    pub mul_factor_a: u8,
    pub mul_factor_b: u8,
    pub div_dividend: u16,
    pub div_divisor: u8,
    pub div_quotient: u16,
    pub product_or_remainder: u16,
}

impl Math {
    pub fn new() -> Self {
        Self {
            mul_factor_a: 0xFF,
            mul_factor_b: 0,
            div_dividend: 0xFFFF,
            div_divisor: 0,
            product_or_remainder: 0,
            div_quotient: 0,
        }
    }

    pub fn process_mul(&mut self) {
        self.product_or_remainder = (self.mul_factor_a as u16) * (self.mul_factor_b as u16);
    }

    pub fn process_div(&mut self) {
        if self.div_divisor == 0 {
            self.div_quotient = 0xFFFF;
            self.product_or_remainder = self.div_dividend;
        } else {
            self.div_quotient = self.div_dividend / (self.div_divisor as u16);
            self.product_or_remainder = self.div_dividend % (self.div_divisor as u16);
        }
    }
}

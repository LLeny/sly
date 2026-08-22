use crate::Bus;
use crate::mmio::MemoryMap;
use crate::mmio::device::{Access, IoRegister, RegEntry};
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

pub fn registers<M: MemoryMap>() -> Vec<RegEntry<M>> {
    vec![
        MulFactorA::entry(),
        MulFactorB::entry(),
        DividendLow::entry(),
        DividendHigh::entry(),
        Divisor::entry(),
        QuotientLow::entry(),
        QuotientHigh::entry(),
        ProductLow::entry(),
        ProductHigh::entry(),
    ]
}

struct MulFactorA;

impl IoRegister for MulFactorA {
    const START: u16 = WRMPYA;
    const READ: Access = Access::Unmapped;

    fn write<M: MemoryMap>(snes: &mut Bus<M>, _reg: u16, value: u8) {
        snes.math.mul_factor_a = value;
    }
}

struct MulFactorB;

impl IoRegister for MulFactorB {
    const START: u16 = WRMPYB;
    const READ: Access = Access::Unmapped;

    fn write<M: MemoryMap>(snes: &mut Bus<M>, _reg: u16, value: u8) {
        snes.math.mul_factor_b = value;
        snes.math.process_mul();
    }
}

struct DividendLow;

impl IoRegister for DividendLow {
    const START: u16 = WRDIVL;
    const READ: Access = Access::Unmapped;

    fn write<M: MemoryMap>(snes: &mut Bus<M>, _reg: u16, value: u8) {
        snes.math.div_dividend = (snes.math.div_dividend & 0xFF00) | value as u16;
    }
}

struct DividendHigh;

impl IoRegister for DividendHigh {
    const START: u16 = WRDIVH;
    const READ: Access = Access::Unmapped;

    fn write<M: MemoryMap>(snes: &mut Bus<M>, _reg: u16, value: u8) {
        snes.math.div_dividend = (snes.math.div_dividend & 0x00FF) | ((value as u16) << 8);
    }
}

struct Divisor;

impl IoRegister for Divisor {
    const START: u16 = WRDIVB;
    const READ: Access = Access::Unmapped;

    fn write<M: MemoryMap>(snes: &mut Bus<M>, _reg: u16, value: u8) {
        snes.math.div_divisor = value;
        snes.math.process_div();
    }
}

struct QuotientLow;

impl IoRegister for QuotientLow {
    const START: u16 = RDDIVL;

    fn peek<M: MemoryMap>(snes: &Bus<M>, _reg: u16) -> u8 {
        snes.math.div_quotient as u8
    }
}

struct QuotientHigh;

impl IoRegister for QuotientHigh {
    const START: u16 = RDDIVH;

    fn peek<M: MemoryMap>(snes: &Bus<M>, _reg: u16) -> u8 {
        (snes.math.div_quotient >> 8) as u8
    }
}

struct ProductLow;

impl IoRegister for ProductLow {
    const START: u16 = RDMPYL;

    fn peek<M: MemoryMap>(snes: &Bus<M>, _reg: u16) -> u8 {
        snes.math.product_or_remainder as u8
    }
}

struct ProductHigh;

impl IoRegister for ProductHigh {
    const START: u16 = RDMPYH;

    fn peek<M: MemoryMap>(snes: &Bus<M>, _reg: u16) -> u8 {
        (snes.math.product_or_remainder >> 8) as u8
    }
}

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

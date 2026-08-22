use serde::{Deserialize, Serialize};

use super::regs::{
    ARITHMETIC_OVERFLOW, ARITHMETIC_PARAMETER_A_END, ARITHMETIC_PARAMETER_A_START,
    ARITHMETIC_PARAMETER_B_END, ARITHMETIC_PARAMETER_B_START, ARITHMETIC_RESULT_END,
    ARITHMETIC_RESULT_START, ArithmeticControl,
};

const RESULT_BITS: u32 = 40;
const RESULT_MASK: u64 = (1_u64 << RESULT_BITS) - 1;

#[derive(Default, Serialize, Deserialize)]
pub(super) struct Math {
    control: ArithmeticControl,
    parameter_a: u16,
    parameter_b: u16,
    result: u64,
    overflow: bool,
}

impl Math {
    pub(super) fn read(&self, reg: u16) -> Option<u8> {
        if (ARITHMETIC_RESULT_START..=ARITHMETIC_RESULT_END).contains(&reg) {
            let index = usize::from(reg - ARITHMETIC_RESULT_START);
            return Some(self.result.to_le_bytes()[index]);
        }
        if reg == ARITHMETIC_OVERFLOW {
            return Some(u8::from(self.overflow) << 7);
        }

        None
    }

    pub(super) fn write(&mut self, reg: u16, value: u8) -> bool {
        match reg {
            reg if reg == ArithmeticControl::ADDRESS => self.set_control(value),
            ARITHMETIC_PARAMETER_A_START => {
                self.parameter_a = (self.parameter_a & 0xFF00) | u16::from(value);
            }
            ARITHMETIC_PARAMETER_A_END => {
                self.parameter_a = (self.parameter_a & 0x00FF) | (u16::from(value) << 8);
            }
            ARITHMETIC_PARAMETER_B_START => {
                self.parameter_b = (self.parameter_b & 0xFF00) | u16::from(value);
            }
            ARITHMETIC_PARAMETER_B_END => {
                self.parameter_b = (self.parameter_b & 0x00FF) | (u16::from(value) << 8);
                self.execute();
            }
            _ => return false,
        }

        true
    }

    pub(super) fn peek(&self, reg: u16) -> Option<u8> {
        match reg {
            reg if reg == ArithmeticControl::ADDRESS => Some(self.control.raw()),
            ARITHMETIC_PARAMETER_A_START => Some(self.parameter_a.to_le_bytes()[0]),
            ARITHMETIC_PARAMETER_A_END => Some(self.parameter_a.to_le_bytes()[1]),
            ARITHMETIC_PARAMETER_B_START => Some(self.parameter_b.to_le_bytes()[0]),
            ARITHMETIC_PARAMETER_B_END => Some(self.parameter_b.to_le_bytes()[1]),
            _ => self.read(reg),
        }
    }

    pub(super) fn is_write_only(reg: u16) -> bool {
        reg == ArithmeticControl::ADDRESS
            || (ARITHMETIC_PARAMETER_A_START..=ARITHMETIC_PARAMETER_B_END).contains(&reg)
    }

    fn set_control(&mut self, value: u8) {
        self.control = ArithmeticControl::from_raw(value & 0x03);
        if self.control.mode() & 0x02 != 0 {
            self.result = 0;
        }
    }

    fn execute(&mut self) {
        match self.control.mode() {
            0 => self.multiply(),
            1 => self.divide(),
            2 | 3 => self.multiply_sum(),
            _ => unreachable!(),
        }
        self.parameter_b = 0;
    }

    fn multiply(&mut self) {
        let parameter_a = i16::from_le_bytes(self.parameter_a.to_le_bytes());
        let parameter_b = i16::from_le_bytes(self.parameter_b.to_le_bytes());
        let product = i32::from(parameter_a) * i32::from(parameter_b);
        self.result = u64::from(u32::from_le_bytes(product.to_le_bytes()));
    }

    fn divide(&mut self) {
        let dividend = i32::from(i16::from_le_bytes(self.parameter_a.to_le_bytes()));
        let divisor = i32::from(self.parameter_b);

        self.result = if divisor == 0 {
            0
        } else {
            let remainder = dividend.rem_euclid(divisor);
            let quotient = (dividend - remainder) / divisor;
            let quotient_bytes = quotient.to_le_bytes();
            let remainder_bytes = remainder.to_le_bytes();
            let quotient = u16::from_le_bytes([quotient_bytes[0], quotient_bytes[1]]);
            let remainder = u16::from_le_bytes([remainder_bytes[0], remainder_bytes[1]]);
            u64::from(quotient) | (u64::from(remainder) << 16)
        };

        self.parameter_a = 0;
    }

    fn multiply_sum(&mut self) {
        let parameter_a = i16::from_le_bytes(self.parameter_a.to_le_bytes());
        let parameter_b = i16::from_le_bytes(self.parameter_b.to_le_bytes());
        let product = i64::from(parameter_a) * i64::from(parameter_b);
        let result = i64::from_le_bytes(self.result.to_le_bytes());
        let sum = result + product;

        self.overflow = (sum >> RESULT_BITS) & 1 != 0;
        self.result = u64::from_le_bytes(sum.to_le_bytes()) & RESULT_MASK;
    }
}

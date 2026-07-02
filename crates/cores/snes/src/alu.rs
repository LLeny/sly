pub(crate) struct AluResult {
    pub(crate) result: u16,
    pub(crate) carry: bool,
    pub(crate) overflow: bool,
    pub(crate) negative: bool,
    pub(crate) zero: bool,
}

pub(crate) trait Width {
    fn mask(self) -> u16;
    fn sign_bit(self) -> u16;
}

// lmao what the fuck am i doing
impl Width for bool {
    fn mask(self) -> u16 {
        if self { 0xffff } else { 0x00ff }
    }

    fn sign_bit(self) -> u16 {
        if self { 0x8000 } else { 0x0080 }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum ShiftOp {
    Asl,
    Lsr,
    Rol,
    Ror,
}

pub(crate) fn shift(op: ShiftOp, value: u16, carry_in: bool, wide: bool) -> (u16, bool) {
    let mask = wide.mask();
    let msb = wide.sign_bit();
    let value = value & mask;
    let carry = carry_in as u16;

    match op {
        ShiftOp::Asl => ((value << 1) & mask, value & msb != 0),
        ShiftOp::Lsr => (value >> 1, value & 1 != 0),
        ShiftOp::Rol => (((value << 1) | carry) & mask, value & msb != 0),
        ShiftOp::Ror => {
            let hi = if carry != 0 { msb } else { 0 };
            ((value >> 1) | hi, value & 1 != 0)
        }
    }
}

pub(crate) fn adc_bcd8(a: u16, b: u16, carry_in: bool) -> AluResult {
    let a = a & 0xff;
    let b = b & 0xff;
    let c = carry_in as u16;

    let mut lo = (a & 0x0f) + (b & 0x0f) + c;
    if lo >= 0x0a {
        lo = ((lo + 0x06) & 0x0f) + 0x10;
    }

    let mut sum = (a & 0xf0) + (b & 0xf0) + lo;
    let overflow = (!(a ^ b) & (a ^ sum) & 0x80) != 0;
    if sum >= 0xa0 {
        sum += 0x60;
    }

    let result = sum & 0xff;

    AluResult {
        result,
        carry: sum >= 0x100,
        overflow,
        negative: result & 0x80 != 0,
        zero: result == 0,
    }
}

pub(crate) fn sbc_bcd8(a: u16, b: u16, carry_in: bool) -> AluResult {
    let a = (a & 0xff) as i32;
    let b = (b & 0xff) as i32;
    let c = carry_in as i32;

    let bin = a - b - (1 - c);
    let carry = bin >= 0;
    let overflow = ((a ^ b) & (a ^ (bin & 0xff)) & 0x80) != 0;

    let mut al = (a & 0x0f) - (b & 0x0f) + c - 1;
    if al < 0 {
        al = ((al - 0x06) & 0x0f) - 0x10;
    }

    let mut res = (a & 0xf0) - (b & 0xf0) + al;
    if res < 0 {
        res -= 0x60;
    }
    let result = (res & 0xff) as u16;

    AluResult {
        result,
        carry,
        overflow,
        negative: result & 0x80 != 0,
        zero: result == 0,
    }
}

pub(crate) fn bcd16(
    fold: fn(u16, u16, bool) -> AluResult,
    a: u16,
    b: u16,
    carry_in: bool,
) -> AluResult {
    let low = fold(a, b, carry_in);
    let high = fold(a >> 8, b >> 8, low.carry);
    let result = (low.result & 0xff) | (high.result << 8);

    AluResult {
        result,
        carry: high.carry,
        overflow: high.overflow,
        negative: result & 0x8000 != 0,
        zero: result == 0,
    }
}

pub(crate) fn add_binary(a: u16, b: u16, carry_in: bool, wide: bool) -> AluResult {
    let mask = wide.mask() as u32;
    let sign = wide.sign_bit() as u32;
    let a = (a as u32) & mask;
    let b = (b as u32) & mask;
    let sum = a + b + carry_in as u32;
    let result = sum & mask;

    AluResult {
        result: result as u16,
        carry: sum & (mask + 1) != 0,
        overflow: (!(a ^ b) & (a ^ result) & sign) != 0,
        negative: result & sign != 0,
        zero: result == 0,
    }
}

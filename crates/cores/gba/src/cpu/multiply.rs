// https://bmchtech.github.io/post/multiply/
// Credits to zaydlang :)

pub(super) fn carry(multiplicand: u32, multiplier: u32) -> bool {
    let upper = multiplier >> 24;
    if !matches!(upper, 0 | 0xFF) {
        return multiplier >> 30 == 2;
    }

    let multiplicand = multiplicand | 1;
    let mut booth = (multiplier & 1).wrapping_neg();
    let mut carry = multiplicand.wrapping_mul(booth);
    let mut sum = carry;
    let mut partial_sum = 0;

    for group in 0..3 {
        for digit in 0..4 {
            let shift = 29 - group * 8 - digit * 2;
            let next = ((multiplier << shift) as i32 >> shift) as u32;
            let addend = multiplicand.wrapping_mul(next.wrapping_sub(booth));
            booth = next;
            partial_sum ^= carry ^ addend;
            sum = sum.wrapping_add(addend);
            carry = sum.wrapping_sub(partial_sum);
        }

        if booth == multiplier {
            break;
        }
    }

    carry >> 31 != 0
}

pub(super) fn accumulate_carry(rm: u32, rs: u32, accumulator: u32) -> bool {
    if !matches!(rs >> 24, 0 | 0xFF) {
        return rs >> 30 == 2;
    }

    self::long_carry(rm, rs, u64::from(accumulator), true)
}

// Adaptation of https://github.com/zaydlang/multiplication-algorithm
pub(super) fn long_carry(rm: u32, rs: u32, accumulator: u64, signed: bool) -> bool {
    const MASK_33: u64 = (1 << 33) - 1;

    let multiplicand = if signed {
        i64::from(rm as i32) as u64
    } else {
        u64::from(rm)
    };
    let mut multiplier = if signed {
        i64::from(rs as i32)
    } else {
        i64::from(rs)
    };
    let mut sum = accumulator >> 1;
    let mut carry = if rs & 1 != 0 { !multiplicand >> 1 } else { 0 };
    let mut accumulator_high = accumulator >> 34;
    let mut output_bit = 30;

    for carry_bit in [22, 14, 6, 30] {
        for shift in (0..8).step_by(2) {
            let (addend, negate) = self::booth_addend(multiplicand, (multiplier >> shift) as u8);
            sum &= MASK_33;
            carry &= MASK_33;

            let bits = addend & MASK_33;
            let next_sum = sum ^ carry ^ bits;
            let next_carry =
                (((sum & carry) | (sum & bits) | (carry & bits)) << 1) | u64::from(negate);
            let extension = (accumulator_high & 1) + ((!carry >> 32) & 1) + ((!addend >> 33) & 1);

            sum = (next_sum >> 2) | (extension << 31);
            carry = (next_carry >> 2) | (((!accumulator_high >> 1) & 1) << 32);
            accumulator_high >>= 2;
        }

        multiplier >>= 8;
        if multiplier == 0 || (signed && multiplier == -1) {
            output_bit = carry_bit;
            break;
        }
    }

    carry & (1 << output_bit) != 0
}

fn booth_addend(multiplicand: u64, chunk: u8) -> (u64, bool) {
    match chunk & 0b111 {
        0 | 7 => (0, false),
        1 | 2 => (multiplicand, false),
        3 => (multiplicand << 1, false),
        4 => (!(multiplicand << 1), true),
        5 | 6 => (!multiplicand, true),
        _ => unreachable!(),
    }
}

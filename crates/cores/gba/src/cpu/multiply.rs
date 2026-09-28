// https://bmchtech.github.io/post/multiply/
// Credits to zaydlang :)

pub(super) fn carry(multiplicand: u32, multiplier: u32) -> bool {
    let upper = multiplier >> 24;
    if upper != 0 && upper != 0xFF {
        return multiplier >> 30 == 2;
    }

    let multiplicand = multiplicand | 1;
    let mut booth = 0u32.wrapping_sub(multiplier & 1);
    let mut carry = multiplicand.wrapping_mul(booth);
    let mut sum = carry;
    let mut accum = 0;

    for group in 0..3 {
        for digit in 0..4 {
            let shift = 29 - group * 8 - digit * 2;
            let next = ((multiplier << shift) as i32 >> shift) as u32;
            let addend = multiplicand.wrapping_mul(next.wrapping_sub(booth));
            booth = next;
            accum ^= carry ^ addend;
            sum = sum.wrapping_add(addend);
            carry = sum.wrapping_sub(accum);
        }

        if booth == multiplier {
            break;
        }
    }

    carry >> 31 != 0
}

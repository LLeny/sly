pub(super) enum Shift {
    Lsl(u32),
    Lsr(u32),
    Asr(u32),
    Ror(u32),
    Rrx,
}

impl Shift {
    pub(super) fn decode(kind: u8, amount: u32) -> Self {
        match kind {
            0 => Self::Lsl(amount),
            1 => Self::Lsr(amount),
            2 => Self::Asr(amount),
            3 => Self::Ror(amount),
            _ => unreachable!(),
        }
    }

    pub(super) fn apply(self, value: u32, carry: bool) -> (u32, bool) {
        match self {
            Self::Lsl(0) | Self::Lsr(0) | Self::Asr(0) | Self::Ror(0) => (value, carry),
            Self::Lsl(amount @ 1..=32) => (
                value.checked_shl(amount).unwrap_or(0),
                (value >> (32 - amount)) & 1 != 0,
            ),
            Self::Lsr(amount @ 1..=32) => (
                value.checked_shr(amount).unwrap_or(0),
                (value >> (amount - 1)) & 1 != 0,
            ),
            Self::Lsl(_) | Self::Lsr(_) => (0, false),
            Self::Asr(amount) => (
                ((value as i32) >> amount.min(31)) as u32,
                (value >> (amount - 1).min(31)) & 1 != 0,
            ),
            Self::Ror(amount) => {
                let result = value.rotate_right(amount);
                (result, result >> 31 != 0)
            }
            Self::Rrx => ((u32::from(carry) << 31) | (value >> 1), value & 1 != 0),
        }
    }
}

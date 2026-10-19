//! Private IEEE bit ordering for native indexes, never SQL float equality.
const SINGLE_SIGN: u32 = 1 << (u32::BITS - 1);
const DOUBLE_SIGN: u64 = 1 << (u64::BITS - 1);

pub(super) fn single_key(bits: u32) -> i64 {
    let ordered = if bits & SINGLE_SIGN != 0 {
        !bits
    } else {
        bits ^ SINGLE_SIGN
    };
    single_encoded(ordered)
}
pub(super) fn double_key(bits: u64) -> i64 {
    let ordered = if bits & DOUBLE_SIGN != 0 {
        !bits
    } else {
        bits ^ DOUBLE_SIGN
    };
    double_encoded(ordered)
}

// The page codec already undoes descending complementation. Bias unsigned key
// bytes into signed atoms without converting a float, rounding or losing NaNs.
pub(super) fn single_encoded(ordered: u32) -> i64 {
    i64::from((ordered ^ SINGLE_SIGN) as i32)
}
pub(super) fn double_encoded(ordered: u64) -> i64 {
    (ordered ^ DOUBLE_SIGN) as i64
}

pub(super) fn double_bits(ordered: u64) -> u64 {
    if ordered & DOUBLE_SIGN != 0 {
        ordered ^ DOUBLE_SIGN
    } else {
        !ordered
    }
}

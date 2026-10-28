//! Audit-owned bit ordering; no floating comparison or civil DATE conversion.
use crate::EditError;
use tablecraft_core::values::LegacyDateTime;

const SINGLE_SIGN: u32 = 1 << (u32::BITS - 1);
const DOUBLE_SIGN: u64 = 1 << (u64::BITS - 1);

pub(super) fn single(bits: u32) -> i64 {
    let atom = if bits & SINGLE_SIGN == 0 {
        bits
    } else {
        !bits ^ SINGLE_SIGN
    };

    i64::from(atom as i32)
}

pub(super) fn double(bits: u64) -> i64 {
    let atom = if bits & DOUBLE_SIGN == 0 {
        bits
    } else {
        !bits ^ DOUBLE_SIGN
    };

    atom as i64
}

pub(super) fn validate_datetime(ordered: u64) -> Result<(), EditError> {
    let bits = if ordered & DOUBLE_SIGN == 0 {
        !ordered
    } else {
        ordered ^ DOUBLE_SIGN
    };

    LegacyDateTime::from_bits(bits).map_err(|_| EditError::Preservation)?;

    Ok(())
}

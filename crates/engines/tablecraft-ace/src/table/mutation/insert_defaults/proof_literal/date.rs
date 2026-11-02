//! Independent fixed ISO component assignment; never calls producer Date parsing.
use crate::{
    EditError, NativeColumn, WriteBlocker,
    format::{
        data_type::DATE_TIME,
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
    },
    header::check,
};
use jiff::civil::DateTime;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{LegacyDateTime, Value},
};
use tablecraft_schema::ScalarType;

const WHOLE_BYTES: usize = 19;
const FRACTION_START: usize = WHOLE_BYTES + size_of::<u8>();
const MAX_FRACTION_DIGITS: usize = 9;
const MIN_NATIVE_YEAR: u32 = 100;
const MAX_NATIVE_YEAR: u32 = 9999;
const COMPONENT_RANGES: [(usize, usize); 6] =
    [(0, 4), (5, 7), (8, 10), (11, 13), (14, 16), (17, 19)];

fn refused() -> EditError {
    EditError::Unsupported(WriteBlocker::Properties)
}

fn component(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<u32, EditError> {
    let mut value = 0_u32;
    for byte in bytes {
        check(budget, cancelled)?;
        if !byte.is_ascii_digit() {
            return Err(refused());
        }
        value = value
            .checked_mul(10)
            .and_then(|value| value.checked_add(u32::from(byte - b'0')))
            .ok_or_else(refused)?;
    }
    Ok(value)
}

pub(super) fn derive(
    column: &NativeColumn,
    text: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value, EditError> {
    check(budget, cancelled)?;
    if column.scalar_type() != Some(ScalarType::DateTime)
        || column.native_type() != DATE_TIME
        || !column.fixed()
        || column.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
        || column.storage_length() != size_of::<f64>()
        || column.extended_flags() != 0
    {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    let bytes = text.as_bytes();
    if bytes.len() != WHOLE_BYTES
        && !(FRACTION_START + 1..=FRACTION_START + MAX_FRACTION_DIGITS).contains(&bytes.len())
    {
        return Err(refused());
    }
    for (at, expected) in [(4, b'-'), (7, b'-'), (10, b' '), (13, b':'), (16, b':')] {
        check(budget, cancelled)?;
        if bytes[at] != expected {
            return Err(refused());
        }
    }
    let mut components = [0_u32; 6];
    for ((start, end), value) in COMPONENT_RANGES.into_iter().zip(&mut components) {
        *value = component(&bytes[start..end], budget, cancelled)?;
    }
    let [year, month, day, hour, minute, second] = components;
    if !(MIN_NATIVE_YEAR..=MAX_NATIVE_YEAR).contains(&year) || second > 59 {
        return Err(refused());
    }
    let mut nanos = 0;
    if bytes.len() > WHOLE_BYTES {
        if bytes[WHOLE_BYTES] != b'.' {
            return Err(refused());
        }
        nanos = component(&bytes[FRACTION_START..], budget, cancelled)?;
        for _ in bytes.len() - FRACTION_START..MAX_FRACTION_DIGITS {
            check(budget, cancelled)?;
            nanos *= 10;
        }
    }
    check(budget, cancelled)?;
    let calendar = DateTime::new(
        year as i16,
        month as i8,
        day as i8,
        hour as i8,
        minute as i8,
        second as i8,
        nanos as i32,
    )
    .map_err(|_| refused())?;
    let value = LegacyDateTime::from_calendar(calendar).map_err(|_| refused())?;
    check(budget, cancelled)?;
    Ok(Value::DateTime(value))
}

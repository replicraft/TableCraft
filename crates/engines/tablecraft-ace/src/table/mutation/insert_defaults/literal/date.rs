//! Qualified ISO civil literals; no host locale, year, clock or numeric coercion.
use crate::{
    EditError, InspectError, NativeColumn, WriteBlocker,
    format::{
        data_type::DATE_TIME,
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
    },
    header::check,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

const DATE_TIME_BYTES: usize = 19;
const MIN_FRACTION_BYTES: usize = 21;
const MAX_DATE_BYTES: usize = 29;
const SEPARATORS: [(usize, u8); 6] = [
    (4, b'-'),
    (7, b'-'),
    (10, b' '),
    (13, b':'),
    (16, b':'),
    (19, b'.'),
];

pub(super) fn resolve(
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
    if bytes.len() != DATE_TIME_BYTES
        && !(MIN_FRACTION_BYTES..=MAX_DATE_BYTES).contains(&bytes.len())
    {
        return Err(EditError::Unsupported(WriteBlocker::Properties));
    }
    budget
        .charge(Resource::Steps, (2 * bytes.len()) as u64)
        .map_err(InspectError::from)?;
    for (position, byte) in bytes.iter().enumerate() {
        check(budget, cancelled)?;
        let valid = match SEPARATORS.iter().find(|(at, _)| *at == position) {
            Some((_, expected)) => byte == expected,
            None => byte.is_ascii_digit(),
        };
        if !valid {
            return Err(EditError::Unsupported(WriteBlocker::Properties));
        }
    }
    // Complete four-digit ISO years must not fall through to the separate
    // two-digit U.S. token-year mapping of the broader query literal service.
    if bytes.starts_with(b"00") {
        return Err(EditError::Unsupported(WriteBlocker::Properties));
    }
    let parsed = tablecraft_expressions::datetime::parse_query_literal(text)
        .map_err(|_| EditError::Unsupported(WriteBlocker::Properties))?;
    check(budget, cancelled)?;
    Ok(Value::DateTime(parsed))
}

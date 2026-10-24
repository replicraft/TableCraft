//! Reversal of complete String units without a Unicode publication boundary.
use super::ByteString;
use crate::{EvalError, functions::text::output::ByteOutput};

const UNIT_BYTES: usize = size_of::<u16>();

/// Write complete two-byte units in reverse order, retaining each unit's bytes.
/// Unaligned ranges and isolated surrogates are data; odd byte counts refuse
/// before writing. No grapheme or surrogate-pair repair is performed. The caller
/// meters the output, checks cancellation and discards partial output on failure.
pub fn write_reversed_units(
    value: &ByteString,
    output: &mut impl ByteOutput,
) -> Result<(), EvalError> {
    output.step()?;
    let (units, remainder) = value.as_bytes().as_chunks::<UNIT_BYTES>();
    if !remainder.is_empty() {
        return Err(EvalError::UnrepresentableText);
    }

    for unit in units.iter().rev() {
        output.step()?;
        output.append(unit)?;
    }
    Ok(())
}

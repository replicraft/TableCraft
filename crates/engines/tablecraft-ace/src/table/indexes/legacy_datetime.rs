//! Validate every DATE key, including internal bounds with no directly read row.
use super::floating;
use crate::InspectError;
use tablecraft_core::values::LegacyDateTime;

pub(super) fn decode(ordered: u64) -> Result<i64, InspectError> {
    LegacyDateTime::from_bits(floating::double_bits(ordered))
        .map_err(|_| InspectError::Malformed)?;
    // Native storage sorts the exact serial, not its civil interpretation.
    // Keep negative aliases and sub-millisecond bits without a calendar round trip.
    Ok(floating::double_encoded(ordered))
}

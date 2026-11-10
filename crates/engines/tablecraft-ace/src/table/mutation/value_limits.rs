//! Native descriptor and decoded-text representation bounds, not writer quotas.
use crate::{InspectError, LongTextReadLimits, LongValueReadLimits};

/// Two descriptor tag bits leave thirty bits for encoded Memo/OLE length.
pub(in crate::table::mutation) const NATIVE_LONG_VALUE_BYTES: usize =
    LongValueReadLimits::MAX_BYTES as usize;
/// Compressed single-byte units expand at most twofold when decoded to UTF-8.
pub(in crate::table::mutation) const NATIVE_LONG_TEXT_BYTES: usize =
    LongTextReadLimits::MAX_BYTES as usize;

pub(in crate::table::mutation) fn require_encoded_length(
    length: usize,
) -> Result<(), InspectError> {
    if length > NATIVE_LONG_VALUE_BYTES {
        return Err(InspectError::StructureLimit);
    }
    Ok(())
}

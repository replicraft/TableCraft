use super::super::{
    key::{ASC_NULL, ASC_VALUE, DESC_NULL, DESC_VALUE, INT32_SIGN_BIT, MARKER_BYTES},
    page::{Entry, ROW_BYTES},
};
use crate::{EditError, InspectError, WriteBlocker, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const KEY_BYTES: usize = size_of::<i32>();
const CHILD_BYTES: usize = size_of::<u32>();
const ENTRY_BYTES: usize = MARKER_BYTES + KEY_BYTES + ROW_BYTES + CHILD_BYTES;

// Retain the original page prefix and entry lengths. Recompression, bitmap and
// free-space changes require a separately qualified writer.
pub(super) fn encode(
    entries: &[Entry],
    prefix: usize,
    length: usize,
    descending: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    budget
        .charge_allocation(length as u64, 1)
        .map_err(InspectError::from)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    let mut first = [0; ENTRY_BYTES];
    for (position, entry) in entries.iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, ENTRY_BYTES as u64)
            .map_err(InspectError::from)?;
        let mut encoded = [0; ENTRY_BYTES];
        encoded[0] = match (entry.value.key, descending) {
            (None, false) => ASC_NULL,
            (None, true) => DESC_NULL,
            (Some(_), false) => ASC_VALUE,
            (Some(_), true) => DESC_VALUE,
        };
        let key_end = if let Some(key) = entry.value.key {
            let key = i32::try_from(key).map_err(|_| InspectError::Malformed)?;
            let mut bytes = ((key as u32) ^ INT32_SIGN_BIT).to_be_bytes();
            if descending {
                for byte in &mut bytes {
                    *byte = !*byte;
                }
            }
            encoded[MARKER_BYTES..MARKER_BYTES + KEY_BYTES].copy_from_slice(&bytes);
            MARKER_BYTES + KEY_BYTES
        } else {
            MARKER_BYTES
        };
        encoded[key_end..key_end + ROW_BYTES].copy_from_slice(&entry.value.row.to_be_bytes());
        let mut end = key_end + ROW_BYTES;
        if entry.child != 0 {
            encoded[end..end + CHILD_BYTES].copy_from_slice(&entry.child.to_be_bytes());
            end += CHILD_BYTES;
        }
        let shared = if position == 0 { 0 } else { prefix };
        if prefix > end || shared != 0 && encoded[..shared] != first[..shared] {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        if position == 0 {
            first = encoded;
        }
        if output.len() + end - shared > length {
            return Err(EditError::Preservation);
        }
        output.extend_from_slice(&encoded[shared..end]);
    }
    if output.len() != length {
        return Err(EditError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(output)
}

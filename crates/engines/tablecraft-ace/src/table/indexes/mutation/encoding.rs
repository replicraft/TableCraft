use super::super::{
    key::{ASC_NULL, ASC_VALUE, DESC_NULL, DESC_VALUE, MARKER_BYTES},
    page::{Entry, ROW_BYTES},
};
use super::IntegralKey;
use crate::{EditError, InspectError, WriteBlocker, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAX_KEY_BYTES: usize = size_of::<i64>();
const CHILD_BYTES: usize = size_of::<u32>();
pub(super) const MAX_ENTRY_BYTES: usize = MARKER_BYTES + MAX_KEY_BYTES + ROW_BYTES + CHILD_BYTES;

#[derive(Clone, Copy)]
pub(super) enum Mode {
    Fixed { descending: bool },
    Nullable { descending: bool },
}
impl Mode {
    pub fn descending(self) -> bool {
        match self {
            Self::Fixed { descending } | Self::Nullable { descending } => descending,
        }
    }
}
pub(super) struct Encoded {
    pub bytes: [u8; MAX_ENTRY_BYTES],
    pub length: usize,
    pub key_length: usize,
}
pub(super) fn entry(
    entry: &Entry,
    kind: IntegralKey,
    descending: bool,
) -> Result<Encoded, InspectError> {
    let mut bytes = [0; MAX_ENTRY_BYTES];
    bytes[0] = match (entry.value.key, descending) {
        (None, false) => ASC_NULL,
        (None, true) => DESC_NULL,
        (Some(_), false) => ASC_VALUE,
        (Some(_), true) => DESC_VALUE,
    };
    let key_end = if let Some(key) = entry.value.key {
        let mut encoded = kind.ordered(key)?;
        let encoded = &mut encoded[..kind.bytes()];
        if descending {
            for byte in &mut *encoded {
                *byte = !*byte;
            }
        }
        bytes[MARKER_BYTES..MARKER_BYTES + kind.bytes()].copy_from_slice(encoded);
        MARKER_BYTES + kind.bytes()
    } else {
        MARKER_BYTES
    };
    bytes[key_end..key_end + ROW_BYTES].copy_from_slice(&entry.value.row.to_be_bytes());
    let key_length = key_end + ROW_BYTES;
    let mut length = key_length;
    if entry.child != 0 {
        bytes[length..length + CHILD_BYTES].copy_from_slice(&entry.child.to_be_bytes());
        length += CHILD_BYTES;
    }
    Ok(Encoded {
        bytes,
        length,
        key_length,
    })
}

// Retain the original page prefix and entry lengths. Recompression, bitmap and
// free-space changes require a separately qualified writer.
pub(super) fn encode(
    entries: &[Entry],
    kind: IntegralKey,
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
    let mut first = [0; MAX_ENTRY_BYTES];
    for (position, entry) in entries.iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, MAX_ENTRY_BYTES as u64)
            .map_err(InspectError::from)?;
        let wire = self::entry(entry, kind, descending)?;
        let encoded = wire.bytes;
        let end = wire.length;
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

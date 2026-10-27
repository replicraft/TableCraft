pub(super) use super::super::numeric::encode_component as component;
use super::super::{
    key::MARKER_BYTES,
    page::{Entry, ROW_BYTES},
};
use super::profile::WriteProfile;
#[cfg(feature = "ace16-write-preview")]
use super::retained::RetainedProfile;
use crate::{EditError, InspectError, WriteBlocker, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAX_KEY_BYTES: usize = size_of::<i64>();
const CHILD_BYTES: usize = size_of::<u32>();
pub(super) const MAX_ENTRY_BYTES: usize = MARKER_BYTES + MAX_KEY_BYTES + ROW_BYTES + CHILD_BYTES;

#[derive(Clone, Copy)]
pub(super) enum Mode {
    Fixed,
    Repack,
}
pub(super) struct Encoded<const N: usize> {
    pub bytes: [u8; N],
    pub length: usize,
    pub key_length: usize,
}

pub(super) fn entry<P: WriteProfile, const N: usize>(
    entry: &Entry<P::Key>,
    profile: &P,
) -> Result<Encoded<N>, InspectError> {
    entry_with::<_, N>(entry, profile.max_key_bytes(), |key, bytes| {
        profile.encode(*key, bytes)
    })
}

#[cfg(feature = "ace16-write-preview")]
pub(super) fn retained_entry<P: RetainedProfile, const N: usize>(
    entry: &Entry<P::Key>,
    profile: &P,
) -> Result<Encoded<N>, InspectError> {
    entry_with::<_, N>(entry, profile.max_key_bytes(), |key, bytes| {
        profile.encode_retained(key, bytes)
    })
}

/// Serialize a borrowed key without cloning an owned physical key payload.
#[cfg(feature = "ace16-write-preview")]
pub(super) fn borrowed_entry<P: RetainedProfile, const N: usize>(
    entry: &Entry<&P::Key>,
    profile: &P,
) -> Result<Encoded<N>, InspectError> {
    entry_with::<_, N>(entry, profile.max_key_bytes(), |key, bytes| {
        profile.encode_retained(*key, bytes)
    })
}

fn entry_with<K, const N: usize>(
    entry: &Entry<K>,
    maximum: usize,
    encode: impl FnOnce(&K, &mut [u8]) -> Result<usize, InspectError>,
) -> Result<Encoded<N>, InspectError> {
    if maximum + ROW_BYTES + CHILD_BYTES > N {
        return Err(InspectError::Malformed);
    }
    let mut bytes = [0; N];
    let key_end = encode(&entry.value.key, &mut bytes)?;
    let key_length = key_end + ROW_BYTES;
    bytes
        .get_mut(key_end..key_length)
        .ok_or(InspectError::Malformed)?
        .copy_from_slice(&entry.value.row.to_be_bytes());
    let mut length = key_length;
    if entry.child != 0 {
        bytes
            .get_mut(length..length + CHILD_BYTES)
            .ok_or(InspectError::Malformed)?
            .copy_from_slice(&entry.child.to_be_bytes());
        length += CHILD_BYTES;
    }
    Ok(Encoded {
        bytes,
        length,
        key_length,
    })
}

// Retain the original page prefix and entry lengths. Recompression, bitmap and
// free-space changes use the separate existing-page repacking path.
pub(super) fn encode<P: WriteProfile, const N: usize>(
    entries: &[Entry<P::Key>],
    profile: &P,
    prefix: usize,
    length: usize,
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
    let mut first = [0; N];
    for (position, entry) in entries.iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, N as u64)
            .map_err(InspectError::from)?;
        let wire = self::entry::<_, N>(entry, profile)?;
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

use super::model::NativeIntentError;
use crate::{InspectError, header::check, limits::MAX_IMAGE_BYTES};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::{Resource, ResourceBudget};

const DOMAIN: &[u8; 16] = b"TCNI0001:BEFORE\0";
const BLOCK_BYTES: usize = 64 * 1024;

pub(super) fn length(length: u64) -> Result<(), NativeIntentError> {
    if length == 0 || length > MAX_IMAGE_BYTES {
        return Err(NativeIntentError::InvalidRecord);
    }
    Ok(())
}

pub(super) fn digest(
    before: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<[u8; 32], NativeIntentError> {
    check(budget, cancelled)?;
    length(before.len() as u64)?;
    budget
        .charge(Resource::Steps, (DOMAIN.len() + size_of::<u64>()) as u64)
        .map_err(InspectError::from)?;
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update((before.len() as u64).to_le_bytes());
    for chunk in before.chunks(BLOCK_BYTES) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
        hash.update(chunk);
    }
    check(budget, cancelled)?;
    let result = hash.finalize().into();
    check(budget, cancelled)?;
    Ok(result)
}

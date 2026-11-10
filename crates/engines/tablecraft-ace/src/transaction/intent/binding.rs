use super::model::NativeIntentError;
use crate::{
    InspectError,
    header::check,
    limits::{MAX_IMAGE_BYTES, MAX_READ_FILE_BYTES},
};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) const DIGEST_BYTES: usize = size_of::<sha2::digest::Output<Sha256>>();

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
) -> Result<[u8; DIGEST_BYTES], NativeIntentError> {
    check(budget, cancelled)?;
    length(before.len() as u64)?;
    digest_checked(before, budget, cancelled)
}

pub(super) fn table_append_length(length: u64) -> Result<(), NativeIntentError> {
    if length == 0
        || length > MAX_READ_FILE_BYTES
        || !length.is_multiple_of(crate::format::page::PAGE_BYTES as u64)
    {
        return Err(NativeIntentError::InvalidRecord);
    }
    Ok(())
}

pub(super) fn digest_for_table_append(
    before: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<[u8; DIGEST_BYTES], NativeIntentError> {
    check(budget, cancelled)?;
    table_append_length(before.len() as u64)?;
    digest_checked(before, budget, cancelled)
}

fn digest_checked(
    before: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<[u8; DIGEST_BYTES], NativeIntentError> {
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

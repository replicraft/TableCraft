//! Reuse the declaration owner, then copy only the selected table definition.
use super::super::{create_ace16_int32_database, profile};
use super::{AddColumnError, preservation};
use crate::{InspectError, NativeImage, format::page::PAGE_BYTES, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const COPY_CHUNK_BYTES: usize = 16 * 1024;

pub(super) fn candidate(
    before: &NativeImage,
    table: &str,
    columns: &[&str],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, AddColumnError> {
    let rebuilt = create_ace16_int32_database(table, columns, &[], budget, cancelled)?;
    budget
        .charge_allocation(before.as_bytes().len() as u64, 1)
        .map_err(InspectError::from)?;
    let mut after = Vec::new();
    after
        .try_reserve_exact(before.as_bytes().len())
        .map_err(|_| InspectError::Allocation)?;
    for chunk in before.as_bytes().chunks(COPY_CHUNK_BYTES) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(InspectError::from)?;
        after.extend_from_slice(chunk);
    }
    let definition = profile::USER_DEFINITION * PAGE_BYTES;
    let end = definition + PAGE_BYTES;
    after[definition..end].copy_from_slice(&rebuilt.as_bytes()[definition..end]);
    preservation::verify(before, &after, table, columns, budget, cancelled)?;
    Ok(after)
}

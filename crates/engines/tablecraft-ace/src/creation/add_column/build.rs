//! Reuse the declaration owner, then copy only the selected table definition.
use super::super::{create_ace16_int32_database, profile};
use super::{AddColumnError, common, preservation};
use crate::{NativeImage, format::page::PAGE_BYTES};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn candidate(
    before: &NativeImage,
    table: &str,
    columns: &[&str],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, AddColumnError> {
    let rebuilt = create_ace16_int32_database(table, columns, &[], budget, cancelled)?;
    let after = common::copy_definition(
        before,
        rebuilt.as_bytes(),
        profile::USER_DEFINITION * PAGE_BYTES,
        budget,
        cancelled,
    )?;
    preservation::verify(before, &after, table, columns, budget, cancelled)?;
    Ok(after)
}

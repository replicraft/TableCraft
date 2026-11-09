//! Exact descriptor-only catalog changes and retained database/origin facts.
use super::{FieldAdditionAfterOffError as Error, source::Source};
use crate::{
    format::{page::PAGE_BYTES, reader::row_count, row::reference},
    header::check,
    inspect_catalog, inspect_database_properties,
    table::mutation::catalog_property::{RepeatedChange, ledger},
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

const CATALOG_PAGE: usize = 4;
const PROPERTY_PAGE: usize = 10;
const MAP_PAGE: usize = 3;

pub(super) fn candidate(
    source: &Source,
    after: &[u8],
    change: &RepeatedChange,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    check(budget, cancelled)?;
    let before = source.proof.image();
    if after.len() != before.as_bytes().len()
        || change.length() != after.len()
        || change.catalog_page != CATALOG_PAGE as u32
        || change.lval_page() != PROPERTY_PAGE as u32
    {
        return Err(Error::Preservation);
    }
    let map = MAP_PAGE * PAGE_BYTES..(MAP_PAGE + 1) * PAGE_BYTES;
    budget.charge(Resource::Steps, (2 * PAGE_BYTES) as u64)?;
    if before.as_bytes()[map.clone()] != after[map] {
        return Err(Error::Preservation);
    }
    let base = CATALOG_PAGE * PAGE_BYTES;
    budget.charge(Resource::Steps, (2 * PAGE_BYTES) as u64)?;
    for offset in base..base + PAGE_BYTES {
        if before.as_bytes()[offset] != after[offset] && !source.descriptor.contains(&offset) {
            return Err(Error::Preservation);
        }
    }
    let rows = row_count(
        &before.as_bytes()[PROPERTY_PAGE * PAGE_BYTES..(PROPERTY_PAGE + 1) * PAGE_BYTES],
    )?;
    let expected = reference(
        PROPERTY_PAGE as u32,
        u8::try_from(rows).map_err(|_| Error::Preservation)?,
    );
    if change.property_reference() != expected
        || expected == source.user_reference
        || expected == source.database_reference
    {
        return Err(Error::Preservation);
    }
    let native = inspect_catalog(
        &mut Cursor::new(after),
        before.catalog().catalog().scope(),
        budget,
        cancelled,
    )?;
    if native.header() != before.catalog().header()
        || native.origins().len() != before.catalog().origins().len()
    {
        return Err(Error::Preservation);
    }
    for (original, current) in before.catalog().origins().iter().zip(native.origins()) {
        check(budget, cancelled)?;
        if original.object_id() != current.object_id()
            || original.native_id() != current.native_id()
            || original.parent_id() != current.parent_id()
            || original.type_code() != current.type_code()
            || original.flags() != current.flags()
            || original.page() != current.page()
            || original.slot() != current.slot()
        {
            return Err(Error::Preservation);
        }
    }
    let database = inspect_database_properties(
        &mut Cursor::new(after),
        native.catalog().scope(),
        budget,
        cancelled,
    )?
    .ok_or(Error::Preservation)?;
    budget.charge(
        Resource::Steps,
        (database.raw_bytes().len() + source.database.raw_bytes().len()) as u64,
    )?;
    if database.object_id() != source.database.object_id()
        || database.raw_bytes() != source.database.raw_bytes()
    {
        return Err(Error::Preservation);
    }
    let ledger = ledger::capture(after, &native, budget, cancelled)?;
    if !ledger.two_singles(
        [
            (
                source.checked.schema().object_id(),
                expected,
                change.body.len(),
            ),
            (
                source.database.object_id(),
                source.database_reference,
                source.database.raw_bytes().len(),
            ),
        ],
        budget,
        cancelled,
    )? {
        return Err(Error::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}

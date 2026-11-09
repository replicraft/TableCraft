//! Reuse declaration, property, catalog-root and owned-LVAL placement owners.
use super::{Error, verify};
use crate::{
    CreateColumn, NativeImage, TableCheck, create_ace16_database, format::page::PAGE_BYTES,
    header::check, table::mutation::catalog_property,
};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn append(
    before: &NativeImage,
    checked: &TableCheck,
    columns: &[CreateColumn<'_>],
    default_first: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    check(budget, cancelled)?;
    let column = columns.last().ok_or(Error::Source)?.name();
    let rebuilt = create_ace16_database(checked.schema().name(), columns, &[], budget, cancelled)?;
    let mut after = super::super::super::super::common::copy_definition(
        before,
        rebuilt.as_bytes(),
        checked.schema().definition_page() as usize * PAGE_BYTES,
        budget,
        cancelled,
    )?;
    super::super::super::preservation::verify(before, &after, checked, column, budget, cancelled)?;
    let body = super::super::build::properties(columns, default_first, budget, cancelled)?;
    let change =
        catalog_property::compose_repeated(before, checked.schema(), body, budget, cancelled)?;
    change.apply(&mut after, budget, cancelled)?;
    verify::candidate(before, &after, checked, column, &change, budget, cancelled)?;
    Ok(after)
}

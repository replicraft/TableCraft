//! Final admission after independently rebuilding original record and map intent.
use super::super::super::source;
use super::proof;
use crate::{
    Ace16WritePreview, EditError, InspectError, NativeImage, SlotScalarUpdate,
    format::{page::PAGE_BYTES, row::reference_page},
    header::check,
    table::mutation::{
        variable_split::metadata::Metadata,
        variable_update::{finish, map_audit, placement::Plan},
    },
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;

pub(super) struct Observed<'a> {
    pub placement: &'a Plan,
    pub changed_rows: usize,
}

pub(super) fn candidate(
    image: NativeImage,
    table: &str,
    column: &str,
    updates: &[SlotScalarUpdate],
    observed: Observed<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    let Observed {
        placement,
        changed_rows: changed,
    } = observed;
    check(budget, cancelled)?;
    let verified = proof::records(&image, table, column, updates, budget, cancelled)?;
    verified.require_source(&image)?;
    if verified.records.len() != changed {
        return Err(EditError::Preservation);
    }

    let schema = verified.original.table.schema();
    let metadata = Metadata::capture_for_preserved_boolean(
        &image,
        schema,
        verified.original.indexes.indexes(),
        budget,
        cancelled,
    )?;
    metadata.require_source(&image, schema.definition_page())?;
    for record in &verified.original.records {
        check(budget, cancelled)?;
        metadata.require_data_page(reference_page(record.reference), budget, cancelled)?;
        let terminal =
            u32::try_from(record.offset / PAGE_BYTES).map_err(|_| InspectError::Malformed)?;
        metadata.require_data_page(terminal, budget, cancelled)?;
    }
    map_audit::prove_protected(
        &image,
        schema,
        placement,
        &verified.records,
        &metadata,
        budget,
        cancelled,
    )?;

    // This consumer has no INDEX patch capability. The common assembly keeps
    // complete byte preservation and represented append checks independent of
    // the caller's final table-admission profile.
    let after = finish::assemble(&image, placement, &[], budget, cancelled)?;
    let scope = image.catalog().catalog().scope();
    let candidate = NativeImage::capture(&mut Cursor::new(&after), scope, budget, cancelled)?;
    let qualified = source::capture(&candidate, table, budget, cancelled)?;
    placement
        .maps
        .audit_membership(&after, qualified.table.schema(), budget, cancelled)?;
    check(budget, cancelled)?;

    Ok(Ace16WritePreview::from_candidate(
        image.bytes,
        after,
        changed,
    ))
}

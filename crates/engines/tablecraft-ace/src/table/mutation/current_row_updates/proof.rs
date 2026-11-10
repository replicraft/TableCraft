//! Original bytes and separate row encoding prove the complete placed image.
use super::bind;
use crate::{
    EditError, FittedColumnUpdates, InspectError, NativeImage, WriteBlocker,
    format::row::{OFFSET_MASK, RELOCATED_MASK},
    header::check,
    preservation::COPY_CHUNK_BYTES,
    table::mutation::{
        historical_fixed::source::Source,
        preserved_source::validation,
        record_placement::{proof as placement, space::Space},
        record_roots, record_serialization,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn prove(
    image: &NativeImage,
    table: &str,
    groups: &[FittedColumnUpdates<'_>],
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let source = Source::capture_current_rows(image, table, budget, cancelled)?;
    source
        .facts()
        .require_preserved_definition_tail(image, after, budget, cancelled)?;
    let requests = [bind(image, &source, groups, budget, cancelled)?];
    let schema = source.facts().table.schema();
    let captured = record_roots::capture(image, schema, &requests, budget, cancelled)?;
    let root = captured.selected.first().ok_or(EditError::Conflict)?;
    if captured.selected.len() != 1 || !matches!(root.word & !OFFSET_MASK, 0 | RELOCATED_MASK) {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let value = groups[0].cell(0).ok_or(EditError::Preservation)?.value;
    if !root.update.changed {
        if !super::binding::equal_bytes(image.as_bytes(), after, budget, cancelled)? {
            return Err(EditError::Preservation);
        }
        return Ok(());
    }
    let record = record_serialization::proof::current_rows::reconstruct(
        schema,
        root.update.column,
        &root.final_record.data,
        value,
        budget,
        cancelled,
    )?;
    let space = Space::capture_normal(image, schema, &captured.owned, budget, cancelled)?;
    let mut physical =
        placement::Placement::capture(image, &space, &captured.owned, budget, cancelled)?;
    let choice = physical.reserve(root, &record, budget, cancelled)?;
    let mut expected = record_serialization::copy(image.as_bytes(), budget, cancelled)?;
    placement::apply(
        image,
        &space,
        root,
        &record,
        &choice,
        &mut expected,
        budget,
        cancelled,
    )?;
    space.audit(image.as_bytes(), &expected, budget, cancelled)?;
    if expected.len() != after.len() {
        return Err(EditError::Preservation);
    }
    for (original, actual) in expected
        .chunks(COPY_CHUNK_BYTES)
        .zip(after.chunks(COPY_CHUNK_BYTES))
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, original.len() as u64)
            .map_err(InspectError::from)?;
        if original != actual {
            return Err(EditError::Preservation);
        }
    }
    validation::expected(
        image,
        table,
        &source.facts().table,
        &expected,
        budget,
        cancelled,
    )?;
    Ok(())
}

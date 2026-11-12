//! Independent original record reconstruction and cumulative placement proof.
use super::{Policy, bind};
use crate::format::row::{OFFSET_MASK, RELOCATED_MASK};
use crate::preservation::COPY_CHUNK_BYTES;
use crate::table::mutation::{preserved_source::validation, record_placement::proof as placement};
use crate::{
    EditError, FittedColumnUpdates, InspectError, NativeImage, WriteBlocker, header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn prove(
    image: &NativeImage,
    table: &str,
    groups: &[FittedColumnUpdates<'_>],
    after: &[u8],
    policy: Policy,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let family = policy.family();
    let source = family.capture(image, table, budget, cancelled)?;
    source
        .facts()
        .require_preserved_definition_tail(image, after, budget, cancelled)?;
    let requests = bind(image, &source, groups, policy, budget, cancelled)?;
    let schema = source.facts().table.schema();
    let captured = family.roots(image, schema, &requests, budget, cancelled)?;
    let space = family.space(image, schema, &captured.owned, budget, cancelled)?;
    let mut physical =
        placement::Placement::capture(image, &space, &captured.owned, budget, cancelled)?;
    let mut expected = family.copy_image(image, budget, cancelled)?;
    for root in &captured.selected {
        check(budget, cancelled)?;
        if !matches!(root.word & !OFFSET_MASK, 0 | RELOCATED_MASK) {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let original = source
            .facts()
            .records
            .get(root.update.selected.ordinal)
            .ok_or(EditError::Conflict)?;
        if original.offset != root.final_record.offset || original.data != root.final_record.data {
            return Err(EditError::Preservation);
        }
        if !root.update.selected.changed {
            continue;
        }
        let record = family.reconstruct_record(
            schema,
            root.update.selected.column,
            &root.final_record.data,
            root.update.value,
            budget,
            cancelled,
        )?;
        let choice = physical.reserve(root, &record, budget, cancelled)?;
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
    }
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

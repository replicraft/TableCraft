//! Reacquire original-request authority before granting DATA or INDEX spans.
use super::super::{
    record_proof,
    request::{self, FittedColumnUpdates},
    serialization::RecordPolicy,
};
use super::stage::Prepared;
use crate::{
    Ace16WritePreview, EditError, NativeImage,
    format::{page::PAGE_BYTES, row::reference_page},
    table::{
        indexes::IndexDataPatch,
        mutation::{
            page_update::roots,
            variable_split::metadata::Metadata,
            variable_update::{finish::compose, map_audit},
        },
    },
};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn finish(
    image: NativeImage,
    table: &str,
    groups: &[FittedColumnUpdates<'_>],
    prepared: &Prepared,
    indexes: &[IndexDataPatch],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    let verified = record_proof::records(
        &image,
        table,
        groups,
        RecordPolicy::GrowingPreferred,
        budget,
        cancelled,
    )?;
    verified.require_source(&image)?;
    let schema = verified.checked.schema();
    let metadata = Metadata::capture_for_mixed_updates(
        &image,
        schema,
        verified.checked.indexes(),
        &verified.columns,
        budget,
        cancelled,
    )?;

    let bound = request::bind(schema, groups, budget, cancelled)?;
    let captured = roots::capture(&image, schema, &bound.rows, budget, cancelled)?;
    for root in captured.selected {
        request::check(budget, cancelled)?;
        metadata.require_data_page(reference_page(root.reference), budget, cancelled)?;
        let terminal = u32::try_from(root.final_record.offset / PAGE_BYTES)
            .map_err(|_| EditError::Preservation)?;
        metadata.require_data_page(terminal, budget, cancelled)?;
    }
    map_audit::prove_protected(
        &image,
        schema,
        &prepared.placement,
        &verified.records,
        &metadata,
        budget,
        cancelled,
    )?;
    verified.prove_indexes(&image, indexes, budget, cancelled)?;

    compose(
        image,
        table,
        &prepared.placement,
        indexes,
        verified.records.len(),
        budget,
        cancelled,
    )
}

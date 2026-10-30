//! Fresh original authority precedes the shared physical placement consumer.
use super::super::{
    record_proof::{self, verified::Verified},
    request::{self, FittedColumnUpdates},
    serialization::RecordPolicy,
    stage,
};
use crate::{
    EditError, InspectError, NativeImage,
    format::{page::PAGE_BYTES, row::reference_page},
    table::mutation::{
        page_update::{roots, stage::Placement},
        variable_split::metadata::Metadata,
        variable_update::placement::Plan,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) struct Prepared {
    pub verified: Verified,
    pub placement: Plan,
}

pub(super) enum Continuation {
    DataOnly,
    JointIndexes,
}

pub(super) fn prepare(
    image: &NativeImage,
    table: &str,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Prepared, EditError> {
    prepare_with(
        image,
        table,
        groups,
        Continuation::DataOnly,
        budget,
        cancelled,
    )
}

pub(super) fn prepare_with(
    image: &NativeImage,
    table: &str,
    groups: &[FittedColumnUpdates<'_>],
    continuation: Continuation,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Prepared, EditError> {
    let verified = record_proof::records(
        image,
        table,
        groups,
        RecordPolicy::GrowingPreferred,
        budget,
        cancelled,
    )?;
    let staged = stage::build_with(
        image,
        &verified.checked,
        groups,
        RecordPolicy::GrowingPreferred,
        budget,
        cancelled,
    )?;
    verified.compare_staged(&staged, budget, cancelled)?;

    let schema = verified.checked.schema();
    let metadata = Metadata::capture_for_mixed_updates(
        image,
        schema,
        verified.checked.indexes(),
        &verified.columns,
        budget,
        cancelled,
    )?;
    let bound = request::bind(schema, groups, budget, cancelled)?;
    let captured = roots::capture(image, schema, &bound.rows, budget, cancelled)?;
    for root in &captured.selected {
        request::check(budget, cancelled)?;
        metadata.require_data_page(reference_page(root.reference), budget, cancelled)?;
        let terminal = u32::try_from(root.final_record.offset / PAGE_BYTES)
            .map_err(|_| EditError::Preservation)?;
        metadata.require_data_page(terminal, budget, cancelled)?;
    }

    let mut placement = Plan::capture(image, schema, budget, cancelled)?;
    let space = placement
        .capture_space(image, schema, &captured.owned, budget, cancelled)?
        .with_metadata(metadata);
    for root in &captured.selected {
        request::check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, staged.records.len() as u64)
            .map_err(InspectError::from)?;
        if let Some(record) = staged
            .records
            .iter()
            .find(|record| record.reference == root.reference)
        {
            placement.apply(image, root, &record.after, &space, budget, cancelled)?;
        }
    }
    placement.audit(budget, cancelled)?;
    crate::table::mutation::variable_update::map_audit::record::compare(
        &verified.records,
        placement.view(),
        budget,
        cancelled,
    )?;

    match continuation {
        Continuation::DataOnly => {}
        Continuation::JointIndexes => {
            let metadata = space.into_metadata().ok_or(EditError::Preservation)?;
            placement.append.retain_source_metadata(image, metadata)?;
        }
    }

    Ok(Prepared {
        verified,
        placement,
    })
}

//! Variable DATA placement and encoded index growth share one append operation.
mod finish;
pub(super) mod metadata;
#[cfg(test)]
mod tests;

use super::{
    EditError, SlotScalarUpdate, gates,
    page_update::stage,
    preview,
    slot::{self, storage},
    variable_update::{
        authority::Operation,
        placement::{Data, Plan},
    },
};
use crate::{
    Ace16WritePreview, InspectError, NativeImage, TableCheck,
    header::check,
    limits::MAX_LIVE_ROWS,
    table::indexes::{
        SlotKeyChanges, SplitIndexPlan, continue_index_split_slot_patches,
        qualify_split_slot_indexes,
    },
};
use metadata::Metadata;
use tablecraft_core::limits::{Resource, ResourceBudget};

struct Staged {
    data: Data,
    indexes: SplitIndexPlan,
    changed: usize,
}

impl NativeImage {
    /// Preview ordinary variable Short Text/Binary replacements together with
    /// qualified single-key General/Binary index splitting.
    ///
    /// DATA forwarding and index growth share one bounded append cursor and map
    /// ledger. Immutable original requests independently certify record bytes,
    /// placement and index changes. Affected tuples, unknown/French collations,
    /// Memo/OLE and unqualified map/constraint profiles remain refused.
    ///
    /// This separate non-journalable engineering candidate preserves logical
    /// ROW references and existing API contracts. It does not enable normal ACE
    /// saves or establish native Access qualification.
    pub fn preview_ace16_variable_split_updates(
        self,
        table: &str,
        column: &str,
        requests: &[SlotScalarUpdate],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16WritePreview, EditError> {
        let _span = tracing::debug_span!(
            "native.edit.preview.variable_split",
            assignments = requests.len()
        )
        .entered();
        let operation = Operation::new(table, column, requests);
        let (structural, position) = preflight(&self, &operation, budget, cancelled)?;
        let staged = prepare(&self, &operation, &structural, position, budget, cancelled)?;

        finish::finish(self, &operation, &staged, budget, cancelled)
    }
}

fn preflight(
    image: &NativeImage,
    operation: &Operation<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(TableCheck, usize), EditError> {
    check(budget, cancelled)?;
    preview::qualify(image)?;
    if operation.requests().len() > MAX_LIVE_ROWS {
        return Err(InspectError::StructureLimit.into());
    }

    let structural = gates::before_slot(image, operation.table, budget, cancelled)?;
    let column = slot::selected(structural.schema(), operation.column_name())?;
    qualify_split_slot_indexes(
        structural.schema(),
        structural.indexes(),
        column,
        budget,
        cancelled,
    )?;

    let field = &structural.schema().columns[column];
    for request in operation.requests() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, (2 * field.storage_length()) as u64)
            .map_err(InspectError::from)?;
        storage::validate(field, request.expected())?;
        storage::validate(field, request.value())?;
    }

    Ok((structural, column))
}

fn prepare(
    image: &NativeImage,
    operation: &Operation<'_>,
    structural: &TableCheck,
    column: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Staged, EditError> {
    let metadata = Metadata::capture(
        image,
        structural.schema(),
        structural.indexes(),
        column,
        budget,
        cancelled,
    )?;
    let placement =
        Plan::capture_protected(image, structural.schema(), metadata, budget, cancelled)?;
    let staged = stage::build_with(
        image,
        structural,
        column,
        operation.requests(),
        placement,
        budget,
        cancelled,
    )?;
    staged.audit(image, structural, column, budget, cancelled)?;
    let changed = staged.changed_rows();

    let (placement, facts, _space) = staged.into_parts();
    let (data, append) = placement.into_parts();
    let keys = facts.keys(column, budget, cancelled)?;
    let indexes = continue_index_split_slot_patches(
        image,
        structural.schema(),
        structural.indexes(),
        &SlotKeyChanges {
            column,
            updates: &keys,
        },
        append,
        budget,
        cancelled,
    )?;

    Ok(Staged {
        data,
        indexes,
        changed,
    })
}

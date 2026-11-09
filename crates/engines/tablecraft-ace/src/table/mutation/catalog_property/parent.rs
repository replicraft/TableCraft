//! Shared exact catalog root selection, record encoding and stable-root placement.
use super::super::{
    EditError,
    record_placement::{audit, forward::Plan, space::Space},
    record_roots,
    rows::RowAssignment,
};
use crate::{InspectError, NativeImage, TableSchema, format::row::reference};
use tablecraft_core::limits::ResourceBudget;

pub(super) struct Selected(usize);

impl RowAssignment for Selected {
    fn row(&self) -> usize {
        self.0
    }
}

/// Keep original root-admission order before the caller allocates descriptors.
pub(super) fn with_selected<T>(
    image: &NativeImage,
    table: &TableSchema,
    catalog: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
    compose: impl FnOnce(
        &record_roots::Root<'_, Selected>,
        &[u32],
        &mut ResourceBudget,
    ) -> Result<T, EditError>,
) -> Result<T, EditError> {
    let origin = image
        .catalog()
        .origins()
        .iter()
        .find(|origin| origin.object_id() == table.object_id())
        .ok_or(InspectError::Malformed)?;
    let ordinal = usize::try_from(catalog.declared_rows())
        .ok()
        .and_then(|count| count.checked_sub(1))
        .ok_or(InspectError::Malformed)?;
    let updates = [Selected(ordinal)];
    let roots = record_roots::capture(image, &catalog, &updates, budget, cancelled)?;
    let root = roots.selected.first().ok_or(EditError::Preservation)?;
    if roots.selected.len() != 1 || root.reference != reference(origin.page(), origin.slot()) {
        return Err(EditError::Preservation);
    }
    compose(root, &roots.owned, budget)
}

#[expect(
    clippy::too_many_arguments,
    reason = "Keep selected root, ownership and descriptor proof separate."
)]
pub(super) fn place(
    image: &NativeImage,
    catalog: &TableSchema,
    column: usize,
    descriptor: &[u8],
    root: &record_roots::Root<'_, Selected>,
    owned: &[u32],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(u32, Vec<u8>), EditError> {
    let record = super::record::compose(
        catalog,
        column,
        &root.final_record.data,
        descriptor,
        budget,
        cancelled,
    )?;
    // Complete canonical source equality is this caller's exclusion proof. This
    // is deliberately not a retry after normal user-row admission has failed.
    let space = Space::capture(image, catalog, owned, budget, cancelled)?;
    let mut placement = Plan::new();
    placement.apply(image, root, &record, &space, budget, cancelled)?;
    audit::pages(&placement, budget, cancelled)?;
    if placement.pages.len() != 1 {
        return Err(EditError::Preservation);
    }
    let patch = placement.pages.pop().ok_or(EditError::Preservation)?;
    Ok((patch.number, patch.after))
}

//! Snapshot handles become normal raw witnesses on the same immutable image.
use super::NativeEditor;
use crate::editing::{NativeEditError, journal::Error};
use tablecraft_ace::{EditError, InspectError, RecoverableNativeDeletePlan};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_db::RowHandle;

impl NativeEditor {
    pub(in crate::editing) fn plan_recoverable_deletes(
        self,
        rows: &[RowHandle],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeDeletePlan, Error> {
        let _span = tracing::debug_span!("access.delete.resolve_original").entered();
        tick(budget, cancelled)?;
        budget
            .charge_allocation(rows.len() as u64, size_of::<usize>() as u64)
            .map_err(InspectError::from)
            .map_err(NativeEditError::from)?;
        let mut ordinals = Vec::new();
        ordinals
            .try_reserve_exact(rows.len())
            .map_err(|_| NativeEditError::from(InspectError::Allocation))?;
        for row in rows {
            tick(budget, cancelled)?;
            ordinals.push(self.snapshot.ordinal(*row).map_err(NativeEditError::from)?);
        }
        let mut original = self
            .image
            .prepare_ordinary_row_deletes(self.snapshot.name(), &ordinals, budget, cancelled)
            .map_err(NativeEditError::from)?;
        // Native traversal returns source order. Retain the caller's written
        // request order in TCND without cloning any raw witness payload.
        for (position, ordinal) in ordinals.iter().enumerate() {
            let mut found = None;
            for (index, request) in original.iter().enumerate().skip(position) {
                tick(budget, cancelled)?;
                if request.row() == *ordinal {
                    found = Some(index);
                    break;
                }
            }
            let index = found.ok_or_else(|| NativeEditError::from(EditError::Preservation))?;
            original.swap(position, index);
        }
        Ok(self.image.plan_recoverable_ordinary_row_deletes(
            self.snapshot.name(),
            original,
            budget,
            cancelled,
        )?)
    }
}

fn tick(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), NativeEditError> {
    if cancelled() {
        return Err(InspectError::Cancelled.into());
    }
    budget
        .charge(Resource::Steps, 1)
        .map_err(InspectError::from)?;
    Ok(())
}

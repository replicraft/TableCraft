//! Snapshot handles become normal raw witnesses on the same immutable image.
use super::NativeEditor;
use crate::editing::{NativeEditError, journal::Error};
use tablecraft_ace::{EditError, InspectError, RecoverableNativeDeletePlan};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_db::RowHandle;

enum Family {
    Ordinary,
    #[cfg(feature = "ace16-create")]
    Current,
    #[cfg(feature = "ace16-create")]
    Retained,
}

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

        let family = Family::Ordinary;
        #[cfg(feature = "ace16-create")]
        let family = if self
            .image
            .has_retained_table_source(self.snapshot.name(), budget, cancelled)
            .map_err(NativeEditError::from)?
        {
            Family::Retained
        } else if self
            .image
            .select_native_current_delete_source(self.snapshot.name(), budget, cancelled)
            .map_err(NativeEditError::from)?
        {
            Family::Current
        } else {
            family
        };

        // The original metadata selects one source language. A failed selected
        // witness proof must not be retried through ordinary DELETE admission.
        let mut original = match family {
            Family::Ordinary => self.image.prepare_ordinary_row_deletes(
                self.snapshot.name(),
                &ordinals,
                budget,
                cancelled,
            ),
            #[cfg(feature = "ace16-create")]
            Family::Current => self.image.prepare_current_row_deletes(
                self.snapshot.name(),
                &ordinals,
                budget,
                cancelled,
            ),
            #[cfg(feature = "ace16-create")]
            Family::Retained => self.image.prepare_retained_row_deletes(
                self.snapshot.name(),
                &ordinals,
                budget,
                cancelled,
            ),
        }
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

        Ok(match family {
            Family::Ordinary => self.image.plan_recoverable_ordinary_row_deletes(
                self.snapshot.name(),
                original,
                budget,
                cancelled,
            )?,
            #[cfg(feature = "ace16-create")]
            Family::Current => self.image.plan_recoverable_current_row_deletes(
                self.snapshot.name(),
                original,
                budget,
                cancelled,
            )?,
            #[cfg(feature = "ace16-create")]
            Family::Retained => self.image.plan_recoverable_retained_row_deletes(
                self.snapshot.name(),
                original,
                budget,
                cancelled,
            )?,
        })
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
